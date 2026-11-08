//! DPAPI 保护的本地密钥材料文件。
//!
//! **数据库不加密**，本文件只承载 `.pebak` 备份去重盐
//! （`payload_hash = BLAKE3(hash_salt ‖ canonical_json(payload))`）。
//!
//! 文件内容：`version(1) + reserved(32) + hash_salt(16)` = 49 字节明文，
//! 整体经 `CryptProtectData`（用户作用域）后落盘。
//! 中间的 32 字节是旧版 SQLCipher 的 `db_key` 位置，现已**不再使用**；保留长度与随机填充
//! 是为了让磁盘上已有的备份索引（依赖同一 `hash_salt`）继续有效，避免任何迁移。
//!
//! **并发安全**：用命名互斥量 `Local\PathEditor.KeyInit` 串行化首次创建，
//! 文件以 `create_new(true)` 原子创建 —— **禁止「先 exists() 再写」的 TOCTOU**。

use crate::domain::error::{CoreError, ErrorCode};
use std::path::{Path, PathBuf};

/// 密钥材料的明文长度：`version(1) + reserved(32) + hash_salt(16)`。
const KEY_PLAIN_LEN: usize = 49;

/// `hash_salt` 在明文中的起始偏移。
const SALT_OFFSET: usize = 33;

/// 当前密钥文件格式版本。
const KEY_FORMAT_VERSION: u8 = 1;

/// 本机密钥材料，只承载备份去重盐。
#[derive(Clone)]
pub struct KeyMaterial {
    /// 16 字节盐，用于 `.pebak` 的 `payload_hash`（**不写进备份文件**）
    pub hash_salt: [u8; 16],
}

/// 配置目录 `~/.patheditor`。
///
/// 生产入口 `open_default()` 用它；**测试一律显式传 `home` 参数**，
/// 不通过 `PATHEDITOR_HOME` 重定向 —— 那是进程级全局状态，会污染同进程其它测试
/// 。
pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
}

/// 密钥目录：默认 [`config_dir`]，可被 `PATHEDITOR_KEY_DIR` 重定向。
///
/// **只为「子进程」测试隔离而存在**：CLI 的进程级用例要跑真实的 `env backup`
/// / `env restore`，而子进程拿到密钥的唯一通道是环境变量 —— 它无法像同进程
/// 测试那样把 `home` 当参数传给 [`load_or_create_key`]。把
/// `PATHEDITOR_KEY_DIR` 指向临时目录后，密钥文件落在临时目录里，不再读写用户
/// 真实的 `~/.patheditor/keys/db.key.dpapi`。
///
/// **不要在同进程测试里 `set_var` 用它**：那是进程级全局状态，会污染同进程
/// 其它测试（同 [`config_dir`] 的说明）。
pub fn key_home() -> PathBuf {
    std::env::var_os("PATHEDITOR_KEY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(config_dir)
}

/// 数据目录（数据库所在目录）：默认 [`config_dir`]，可被 `PATHEDITOR_DATA_DIR` 重定向。
///
/// 数据库不再需要密钥，不能再借 `PATHEDITOR_KEY_DIR` 顺带把库指到临时目录，
/// 因此单独给它一个开关。**同样只给子进程测试用**；生产环境不设置它。
pub fn data_home() -> PathBuf {
    std::env::var_os("PATHEDITOR_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(config_dir)
}

/// 生产入口：按 [`key_home`] 加载（或首次创建）密钥。
pub fn load_default_key() -> Result<KeyMaterial, CoreError> {
    load_or_create_key(&key_home())
}

/// 密钥文件路径：`<home>/keys/db.key.dpapi`。
fn key_path(home: &Path) -> PathBuf {
    home.join("keys").join("db.key.dpapi")
}

/// 读取已有密钥；不存在时原子创建。
///
/// `home` 显式传入（见 [`config_dir`] 的说明），测试用 `tempfile::TempDir`。
pub fn load_or_create_key(home: &Path) -> Result<KeyMaterial, CoreError> {
    let path = key_path(home);
    // **读也必须在互斥量内**：`create_new` 成功后还要 `write_all` + `sync_all`，
    // 这个窗口里文件已存在但内容是 0/半截。锁外的读者会读到它，DPAPI 解密失败，
    // 整个进程 fail-closed（GUI 降级、CLI exit 5），直到下次重启才自愈。
    let _guard = InitGuard::acquire()?;
    let material = match read_key(&path)? {
        Some(k) => k,
        None => create_key_locked(&path)?,
    };

    // DACL 校验从 `db::open_default` 移到这里：数据库不再读密钥，
    // 但盐被其它用户替换会让磁盘上既有 `.pebak` 全部校验失败，守护仍然需要。
    // 过宽只告警、**不自动修改**——避免误伤用户自建的目录权限。
    #[cfg(windows)]
    {
        match super::dacl::key_dacl_is_restricted(&path) {
            Ok(true) => {}
            Ok(false) => log::warn!(
                "密钥文件 {} 的权限过宽（存在非当前用户/SYSTEM 的授权项）。\
                 其它用户可能替换它，导致既有 .pebak 备份无法校验。",
                path.display()
            ),
            // 读取 ACL 失败不影响使用：这是告警而非门禁
            Err(e) => log::warn!("无法校验密钥文件权限: {}", e.message),
        }
    }

    Ok(material)
}

/// 读取并解密密钥文件；文件不存在返回 `Ok(None)`。
fn read_key(path: &Path) -> Result<Option<KeyMaterial>, CoreError> {
    use std::fs;
    match fs::read(path) {
        Ok(blob) => {
            let plain = super::dpapi::unprotect(&blob)?;
            parse_material(&plain).map(Some)
        }
        // 竞争窗口：另一进程可能在本进程判断「不存在」之后创建了文件，
        // 也可能刚好把文件删掉。两者都按「不存在」处理，由 create_key 的
        // create_new 语义兜底。
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_err("read_key", e)),
    }
}

/// 原子创建密钥文件；**调用方必须持有 [`InitGuard`]**。
///
/// 竞争失败（`AlreadyExists`）时读取胜出者的密钥：这只会发生在旧版本进程
/// 同时存在时（旧版本在锁外创建），保留该分支作为兼容兜底。
fn create_key_locked(path: &Path) -> Result<KeyMaterial, CoreError> {
    use std::fs;
    use std::io::Write;

    let mut hash_salt = [0u8; 16];
    rand_fill(&mut hash_salt)?;
    let material = KeyMaterial { hash_salt };

    // 前 32 字节是旧版 `db_key` 的布局，现只作保留位；填随机值保持长度不变。
    let mut reserved = [0u8; 32];
    rand_fill(&mut reserved)?;

    let mut plain = Vec::with_capacity(KEY_PLAIN_LEN);
    plain.push(KEY_FORMAT_VERSION);
    plain.extend_from_slice(&reserved);
    plain.extend_from_slice(&material.hash_salt);
    let blob = super::dpapi::protect(&plain)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err("create_key_dir", e))?;
        // 目录加固是纵深防御：文件侧已经 `D:P` 独立加固，这里失败只告警，
        // 不阻断密钥创建（否则用户自建的 `.patheditor` 权限会卡死应用）。
        #[cfg(windows)]
        if let Err(e) = super::dacl::set_dir_dacl(parent) {
            log::warn!("密钥目录 DACL 加固失败（不影响密钥创建）: {}", e.message);
        }
    }
    // create_new(true)：多进程竞争时只有一个成功，其余读取已写文件。
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut f) => {
            f.write_all(&blob).map_err(|e| io_err("write_key", e))?;
            f.sync_all().map_err(|e| io_err("sync_key", e))?;
            // 写完立即加固 DACL：失败则删除半成品——宁可不留密钥文件，
            // 也不能留下权限过宽的它（另一个用户可替换 → 历史永久不可读）。
            drop(f);
            if let Err(e) = super::dacl::set_key_dacl(path) {
                let _ = fs::remove_file(path);
                return Err(CoreError::new(
                    ErrorCode::Crypto,
                    "create_key",
                    format!("设置密钥文件权限失败，已删除半成品: {}", e.message),
                ));
            }
            Ok(material)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            read_key(path)?.ok_or_else(|| {
                CoreError::new(
                    ErrorCode::Crypto,
                    "create_key",
                    "密钥文件已存在但无法读取，请检查文件权限",
                )
            })
        }
        Err(e) => Err(io_err("open_key", e)),
    }
}

/// 解析密钥明文（`version + reserved + hash_salt`）。
fn parse_material(plain: &[u8]) -> Result<KeyMaterial, CoreError> {
    if plain.len() != KEY_PLAIN_LEN {
        return Err(CoreError::new(
            ErrorCode::Crypto,
            "parse_key",
            format!(
                "密钥文件长度无效：期望 {KEY_PLAIN_LEN} 字节，实际 {}",
                plain.len()
            ),
        ));
    }
    if plain[0] != KEY_FORMAT_VERSION {
        return Err(CoreError::new(
            ErrorCode::Crypto,
            "parse_key",
            format!(
                "密钥文件版本 {} 不受支持（当前支持 {KEY_FORMAT_VERSION}）",
                plain[0]
            ),
        ));
    }
    let mut hash_salt = [0u8; 16];
    hash_salt.copy_from_slice(&plain[SALT_OFFSET..KEY_PLAIN_LEN]);
    Ok(KeyMaterial { hash_salt })
}

/// 用 OS 熵源填充缓冲区。
///
/// `getrandom` 是显式依赖（不是「依赖传递依赖的 API」）；失败代表 OS 熵源
/// 不可用，属于不可恢复的环境错误。
fn rand_fill(buf: &mut [u8]) -> Result<(), CoreError> {
    getrandom::getrandom(buf).map_err(|e| {
        CoreError::new(
            ErrorCode::Crypto,
            "rand_fill",
            format!("OS 熵源不可用: {e}"),
        )
    })
}

/// 密钥文件 IO 错误包装。
fn io_err(op: &'static str, e: std::io::Error) -> CoreError {
    CoreError::new(ErrorCode::Io, op, format!("密钥文件操作失败: {e}"))
}

// ── 命名互斥量 ────────────────────────────────────────────────

/// `Local\PathEditor.KeyInit` 的 RAII 守卫。
///
/// `Local\` 前缀 = 当前会话作用域（同一用户的多进程共享）；不用 `Global\`
/// 是因为那需要额外权限，而密钥文件本身就绑定单个用户。
struct InitGuard(windows_sys::Win32::Foundation::HANDLE);

impl InitGuard {
    /// 获取互斥量并等待；已存在时等待前一个持有者释放。
    fn acquire() -> Result<Self, CoreError> {
        use windows_sys::Win32::Foundation::{
            CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT,
        };
        use windows_sys::Win32::System::Threading::{CreateMutexW, WaitForSingleObject, INFINITE};

        // 互斥量名必须是 UTF-16 且以 NUL 结尾
        let name: Vec<u16> = "Local\\PathEditor.KeyInit\0".encode_utf16().collect();
        // SAFETY: name 是有效的 NUL 结尾宽字符串；lpMutexAttributes 允许为 null
        // （默认安全描述符），bInitialOwner = 0 表示不立即持有。
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(CoreError::new(
                ErrorCode::Crypto,
                "key_init_mutex",
                format!(
                    "创建密钥初始化互斥量失败: {}",
                    std::io::Error::last_os_error()
                ),
            ));
        }
        // SAFETY: handle 刚由 CreateMutexW 成功创建。
        let wait = unsafe { WaitForSingleObject(handle, INFINITE) };
        // WAIT_ABANDONED：前一个持有者崩溃退出；临界区内的操作是幂等的
        // （create_new），因此可以安全继续。
        if wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED {
            Ok(Self(handle))
        } else {
            let code = if wait == WAIT_TIMEOUT { 258 } else { wait };
            // SAFETY: handle 由本函数创建且未被转移。
            unsafe {
                CloseHandle(handle);
            }
            Err(CoreError::new(
                ErrorCode::Crypto,
                "key_init_mutex",
                format!("等待密钥初始化互斥量失败（code {code}）"),
            ))
        }
    }
}

impl Drop for InitGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::ReleaseMutex;
        // SAFETY: handle 由 acquire 成功创建，且本类型独占所有权。
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 纯函数测试：密钥材料的格式校验与盐位置，不触碰文件/DPAPI。
    #[test]
    fn parse_material_rejects_bad_length_and_version_and_reads_salt_at_offset() {
        // 长度不符必须拒绝
        assert!(parse_material(&[1u8; 10]).is_err());

        // 合法长度 + 合法版本：盐必须取自 SALT_OFFSET，前 32 字节不参与
        let mut plain = vec![KEY_FORMAT_VERSION];
        plain.extend_from_slice(&[0xAAu8; 32]);
        plain.extend_from_slice(&[0x11u8; 16]);
        let material = parse_material(&plain).expect("合法长度与版本应通过");
        assert_eq!(material.hash_salt, [0x11u8; 16], "盐必须取自 SALT_OFFSET");

        // 版本不符必须拒绝
        let mut wrong_version = plain.clone();
        wrong_version[0] = 9;
        assert!(parse_material(&wrong_version).is_err());
    }
}
