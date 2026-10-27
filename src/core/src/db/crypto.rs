//! DPAPI 保护的本地密钥文件。
//!
//! **密钥丢失即历史不可读，不得静默重建空库**。
//!
//! 文件内容：`version(1) + db_key(32) + hash_salt(16)` = 49 字节明文，
//! 整体经 `CryptProtectData`（用户作用域）后落盘。
//!
//! **并发安全**：用命名互斥量 `Local\PathEditor.KeyInit` 串行化首次创建，
//! 密钥文件以 `create_new(true)` 原子创建 —— **禁止「先 exists() 再写」的 TOCTOU**。

use crate::domain::error::{CoreError, ErrorCode};
use std::path::{Path, PathBuf};

/// 密钥材料的明文长度：`version(1) + db_key(32) + hash_salt(16)`。
const KEY_PLAIN_LEN: usize = 49;

/// 当前密钥文件格式版本。
const KEY_FORMAT_VERSION: u8 = 1;

/// 密钥材料：数据库密钥 + 备份去重盐。
#[derive(Clone)]
pub struct KeyMaterial {
    /// 32 字节 SQLCipher 数据库密钥
    pub db_key: [u8; 32],
    /// 16 字节盐，用于 `.pebak` 的 `payload_hash`（**不写进备份文件**）
    pub hash_salt: [u8; 16],
}

/// 十六进制表示（用于 `PRAGMA key = "x'...'"`）。
pub fn key_hex(m: &KeyMaterial) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(64);
    for b in m.db_key {
        // 写入 String 不会失败；忽略 Result 是安全的。
        let _ = write!(s, "{b:02x}");
    }
    s
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

/// 密钥文件路径：`<home>/keys/db.key.dpapi`。
fn key_path(home: &Path) -> PathBuf {
    home.join("keys").join("db.key.dpapi")
}

/// 读取已有密钥；不存在时原子创建。
///
/// `home` 显式传入（见 [`config_dir`] 的说明），测试用 `tempfile::TempDir`。
pub fn load_or_create_key(home: &Path) -> Result<KeyMaterial, CoreError> {
    let path = key_path(home);
    if let Some(k) = read_key(&path)? {
        return Ok(k);
    }
    create_key(&path)
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

/// 原子创建密钥文件；竞争失败时读取胜出者的密钥。
fn create_key(path: &Path) -> Result<KeyMaterial, CoreError> {
    use std::fs;
    use std::io::Write;

    // 命名互斥量串行化首次创建，把「读-判-写」压成临界区。
    let _guard = InitGuard::acquire()?;

    // 临界区内复查：另一进程可能已在我们拿锁之前写好了。
    if let Some(k) = read_key(path)? {
        return Ok(k);
    }

    let mut material = KeyMaterial {
        db_key: [0u8; 32],
        hash_salt: [0u8; 16],
    };
    rand_fill(&mut material.db_key)?;
    rand_fill(&mut material.hash_salt)?;

    let mut plain = Vec::with_capacity(KEY_PLAIN_LEN);
    plain.push(KEY_FORMAT_VERSION);
    plain.extend_from_slice(&material.db_key);
    plain.extend_from_slice(&material.hash_salt);
    let blob = super::dpapi::protect(&plain)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err("create_key_dir", e))?;
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

/// 解析密钥明文（`version + db_key + hash_salt`）。
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
    let mut db_key = [0u8; 32];
    let mut hash_salt = [0u8; 16];
    db_key.copy_from_slice(&plain[1..33]);
    hash_salt.copy_from_slice(&plain[33..49]);
    Ok(KeyMaterial { db_key, hash_salt })
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

    /// 纯函数测试：密钥材料的 hex 编解码与格式校验不触碰文件/DPAPI。
    #[test]
    fn key_hex_is_lowercase_64_chars_and_parse_validates() {
        let mut material = KeyMaterial {
            db_key: [0u8; 32],
            hash_salt: [0u8; 16],
        };
        material.db_key[0] = 0xab;
        material.db_key[31] = 0x01;
        let hex = key_hex(&material);
        assert_eq!(hex.len(), 64);
        assert!(hex.starts_with("ab"));
        assert!(hex.ends_with("01"));
        assert!(hex
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));

        // 长度不符 / 版本不符都必须拒绝
        assert!(parse_material(&[1u8; 10]).is_err());
        let mut wrong_version = vec![KEY_FORMAT_VERSION];
        wrong_version.extend_from_slice(&[0u8; 48]);
        assert!(parse_material(&wrong_version).is_ok());
        wrong_version[0] = 9;
        assert!(parse_material(&wrong_version).is_err());
    }
}
