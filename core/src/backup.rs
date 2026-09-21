use crate::env_var::{is_reserved, revision_of, EnvHive, EnvValueKind};
use crate::error::{CoreError, ErrorCode};
use crate::persist::{Versioned, PERSIST_SCHEMA_VERSION};
use crate::reg_store::{EnvHiveStore, WinregHive};
// 经 registry 根 re-export —— 不能写 crate::registry::env_var::X（E0603）
use crate::registry::{self, hive_location, SYS_REG_PATH, USER_REG_PATH};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::types::FromRegValue;

/// env 备份默认保留份数（设计文档 D6）。
///
/// 全量快照单文件约几十 KB；20 份在正常使用下是几百 KB 量级。
/// 写操作密集的用户可调大——注意 README 已提示备份含明文敏感值，
/// 保留份数越大暴露面越大。
pub const ENV_BACKUP_KEEP: usize = 20;

/// 配置文件路径：`~/.patheditor/config.ini`。
///
/// 放用户目录而非 exe 同目录（设计文档 §配置文件）：CLI/GUI 是两份独立 exe，
/// scoop 升级换目录、NSIS 装 Program Files 需提权——只有用户目录能让双 exe
/// 共享、升级不丢、免提权。
fn config_file_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("config.ini")
}

/// 读取 env 备份保留份数；任何异常一律回落 [`ENV_BACKUP_KEEP`]。
///
/// 回落情形（设计文档 §解析与回落行为）：文件不存在 / 键不存在 / 值非整数 /
/// 值为负数 / 值空 / 文件不可读。**不报错、不中止备份**，非默认值时记一次 warn。
/// 读取**无副作用**：文件不存在时不创建。
///
/// 手写极简 INI 解析（`key = value`，`;` 或 `#` 起始为注释），不引入新依赖——
/// 单键配置不值得拉一个 crate，与项目「手写 FNV-1a 而不引 sha2」的先例一致。
fn read_env_backup_keep(path: &Path) -> usize {
    let fallback = ENV_BACKUP_KEEP;
    let Ok(content) = std::fs::read_to_string(path) else {
        return fallback; // 不存在或不可读，无副作用地回落
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "env_backup_keep" {
            continue;
        }
        match value.trim().parse::<usize>() {
            Ok(n) => return n,
            Err(_) => {
                log::warn!(
                    "config.ini 的 env_backup_keep 值非法（{}），回落默认 {}",
                    value.trim(),
                    fallback
                );
                return fallback;
            }
        }
    }
    fallback
}

/// 从真实配置文件读取保留份数。
fn env_backup_keep() -> usize {
    read_env_backup_keep(&config_file_path())
}

/// 备份根目录。`PATHEDITOR_BACKUP_DIR` 存在时用它（测试隔离用），
/// 否则回落 `~/.patheditor/backups/`。
fn backup_base_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("PATHEDITOR_BACKUP_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("backups")
}

/// env 备份目录的公开访问器。
pub fn env_backup_dir() -> PathBuf {
    backup_base_dir()
}

/// 获取备份目录路径
pub fn get_appdata_dir() -> String {
    backup_base_dir().to_string_lossy().to_string()
}

/// 备份中的单个环境变量条目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvBackupVar {
    /// 注册表返回的原始大小写变量名
    pub name: String,
    /// 注册表类型；备份中只出现 String / ExpandString
    pub kind: EnvValueKind,
    /// 完整明文值（设计文档 D2：备份的价值在于能还原，故不打码）
    pub value: String,
    /// 采集时的并发校验摘要（FNV-1a 64，16 位十六进制）
    pub revision: String,
}

/// 按 hive 分组的备份内容。
///
/// 加 `rename_all = "camelCase"` 与 `EnvVarSnapshot`（`core/src/env_var.rs:127`）
/// 及本文件的 `EnvBackupPayload` 保持风格一致；`system` / `user` 两个单字字段
/// 在此宏下不变形，无功能影响（核对轮一致性观察）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvBackupHives {
    #[serde(default)]
    pub system: Vec<EnvBackupVar>,
    #[serde(default)]
    pub user: Vec<EnvBackupVar>,
}

/// env 备份文件的载荷（由 `persist::Versioned` 信封包裹后落盘）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvBackupPayload {
    /// 采集时刻（Unix 毫秒），口径同 `EnvVarSnapshot.capturedAt`
    pub captured_at: i64,
    pub hives: EnvBackupHives,
}

/// 采集单个 hive 中全部可恢复的环境变量。
///
/// 排除两类变量：保留名（`Path` 由专用通路管理）与 `Unsupported` 类型
/// （本就不可写，备份了也无法通过通用通路恢复）。
///
/// **B1：`Unsupported` 必须「跳过该变量」，不得让整次采集失败。** 因此本函数
/// 用 `store.get_raw` + `from_reg_type` 自己判类型，而**不能**用 `read_env_var`
/// ——后者（`core/src/registry/env_var.rs:30-36`）对 `Unsupported` 直接返回
/// `Err`，配上调用方的 `?` 会让「机器上存在一个 `REG_DWORD` 变量」变成
/// 「每次写前备份都失败」。写法与 `list_env_vars_in_store`
/// （`core/src/registry/env_var.rs:116-199`）保持一致。
///
/// # Returns
/// - `Ok(Vec<EnvBackupVar>)` — 该 hive 的可恢复变量，含明文与 revision
/// - `Err(CoreError)` — 枚举失败（`Io`）或读取/解码失败（`Io`/`Parse`）；
///   **不含** `UnsupportedType`
pub(crate) fn collect_hive_vars_in_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
) -> Result<Vec<EnvBackupVar>, CoreError> {
    let (_, _, label) = hive_location(hive);
    let with_hive = |code: ErrorCode, msg: String| {
        let mut e = CoreError::new(code, "collect_env_backup", msg);
        e.hive = Some(hive);
        e
    };

    let names = store.enum_names().map_err(|e| {
        with_hive(
            ErrorCode::Io,
            format!("读取{}环境变量列表失败: {}", label, e),
        )
    })?;

    let mut vars = Vec::new();
    for name in names {
        if is_reserved(&name) {
            continue;
        }
        let raw = store.get_raw(&name).map_err(|e| {
            with_hive(
                ErrorCode::Io,
                format!("读取{}环境变量 {} 失败: {}", label, name, e),
            )
            .with_target(hive, &name)
        })?;
        let kind = EnvValueKind::from_reg_type(raw.vtype.clone());
        if !kind.is_writable() {
            continue; // Unsupported：跳过该变量，**不**使整次采集失败（B1）
        }
        let value = String::from_reg_value(&raw).map_err(|e| {
            with_hive(
                ErrorCode::Parse,
                format!("无法解码{}环境变量 {}: {}", label, name, e),
            )
            .with_target(hive, &name)
        })?;
        // 先算 revision 再移动 raw.vtype；`RegType` 非 Copy（winreg 0.52）。
        vars.push(EnvBackupVar {
            revision: revision_of(&name, raw.vtype, &value),
            name,
            kind,
            value,
        });
    }
    Ok(vars)
}

/// 采集两个 hive 的完整备份载荷。
///
/// # Returns
/// - `Ok(EnvBackupPayload)` — system 与 user 两个 hive 的可恢复变量
/// - `Err(CoreError)` — 任一 hive 采集失败即整体失败（沿用 F-04 的 fail-fast：
///   不产出「成功但不完整」的备份）
pub fn collect_env_backup() -> Result<EnvBackupPayload, CoreError> {
    let sys_store = WinregHive::open(EnvHive::System, false)?;
    let usr_store = WinregHive::open(EnvHive::User, false)?;
    let system = collect_hive_vars_in_store(&sys_store, EnvHive::System)?;
    let user = collect_hive_vars_in_store(&usr_store, EnvHive::User)?;
    Ok(EnvBackupPayload {
        captured_at: Local::now().timestamp_millis(),
        hives: EnvBackupHives { system, user },
    })
}

/// 把备份载荷写入目录，随后执行保留策略轮换。
///
/// 文件名为 `env_backup_<YYYYMMDD>_<HHMMSS>_<毫秒3位>.json`，时间戳格式与
/// PATH 备份一致，便于用户在同一目录中找到全部备份。
///
/// **顺序：先写新文件、再轮换。** 目录稳态恰好等于保留份数 `keep`：
/// `keep` 份 → 写入 → `keep+1` 份 → 轮换删 1 份 → `keep` 份。
/// 若反过来「先轮换、后写入」，`keep` 份时轮换无可删、写入后变成 `keep+1` 份，
/// 目录会长期稳定在 `keep+1` 份——与 spec 验收标准 11 的端到端口径不符。
///
/// **新文件必被保留**：`rotate_env_backups` 按文件名降序排列后从 `keep` 处开始
/// 删，丢弃的是字典序最小（最旧）的一端；刚写入的文件时间戳最大，永不落入删除集。
/// （例外：`keep = 0` 时用户明确要求零保留，新文件也会被删，见实现说明。）
///
/// # Returns
/// - `Ok(PathBuf)` — 写入的备份文件绝对路径
/// - `Err(CoreError)` — 目录创建、写文件或轮换失败（code=`Io`）。
///   **注意轮换失败这一路**：此时新备份文件**已经在磁盘上**，返回值只表示
///   「保留策略未能执行」，**不等于**备份未落盘。调用方措辞须按此理解。
pub fn write_env_backup_to(dir: &Path, payload: &EnvBackupPayload) -> Result<PathBuf, CoreError> {
    write_env_backup_to_with_keep(dir, payload, env_backup_keep())
}

/// [`write_env_backup_to`] 的实现体，保留份数由调用方显式注入。
///
/// 拆出这一层是为了让测试能显式指定 `keep` 断言端到端稳态，而**不必**去写
/// 用户真实的 `~/.patheditor/config.ini`（`config_file_path()` 不受
/// `PATHEDITOR_BACKUP_DIR` 重定向）。生产路径只有 `write_env_backup_to` 一个入口。
fn write_env_backup_to_with_keep(
    dir: &Path,
    payload: &EnvBackupPayload,
    keep: usize,
) -> Result<PathBuf, CoreError> {
    std::fs::create_dir_all(dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "backup_env_vars",
            format!("无法创建备份目录 {}: {}", dir.display(), e),
        )
    })?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    let filepath = dir.join(format!("env_backup_{}.json", timestamp));

    let versioned = Versioned {
        schema_version: PERSIST_SCHEMA_VERSION,
        inner: payload.clone(),
    };
    let json = serde_json::to_string_pretty(&versioned).map_err(|e| {
        CoreError::new(
            ErrorCode::Internal,
            "backup_env_vars",
            format!("序列化备份失败: {e}"),
        )
    })?;

    std::fs::write(&filepath, json).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "backup_env_vars",
            format!("无法写入备份文件 {}: {}", filepath.display(), e),
        )
    })?;

    // 写入后才轮换，目录稳态恰为 keep 份（见 write_env_backup_to 的顺序说明）。
    rotate_env_backups(dir, keep)?;

    log::info!("env 备份已保存到: {}", filepath.display());
    Ok(filepath)
}

/// 保留策略轮换：只删除**本功能生成**的旧备份，保留最近 `keep` 份。
///
/// 授权范围（设计文档 §S1，用户 2026-09-21 明确授权的例外）：
/// 1. 只匹配文件名 `env_backup_*.json`；
/// 2. 只在给定目录内操作（自定义备份目录不参与轮换）；
/// 3. 只删最旧的、超出 `keep` 之外的文件；
/// 4. 绝不删除 `.txt` PATH 备份、`.bak`、`.corrupt-*` 及目录中任何其他文件。
///
/// `keep = 0` 是合法输入，表示用户明确要求零保留，此时本函数会删空所有候选
/// 文件（含刚写入的那份）。`config.ini` 的 `env_backup_keep = 0` 即可触发。
///
/// # Returns
/// - `Ok(Vec<PathBuf>)` — 被删除的文件路径（供测试与日志核对）
/// - `Err(CoreError)` — 目录枚举失败（code=`Io`）；单个文件删除失败只记 warn，不影响其他
fn rotate_env_backups(dir: &Path, keep: usize) -> Result<Vec<PathBuf>, CoreError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    // 只收集本功能生成的备份：前缀 env_backup_ + 后缀 .json，
    // `.bak` / `.corrupt-*` 因后缀不同天然被排除。
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "rotate_env_backups",
            format!("枚举备份目录 {} 失败: {}", dir.display(), e),
        )
    })?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("env_backup_") || !name.ends_with(".json") {
            continue;
        }
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        candidates.push((name, path));
    }

    if candidates.len() <= keep {
        return Ok(Vec::new());
    }

    // 文件名内嵌时间戳，字典序即时间序；降序排列后丢弃最旧的。
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    let mut removed = Vec::new();
    for (_, path) in candidates.into_iter().skip(keep) {
        match std::fs::remove_file(&path) {
            Ok(()) => {
                log::info!("已轮换删除旧备份: {}", path.display());
                removed.push(path);
            }
            // 删除失败不中止轮换：留着旧文件无害，报错反而阻断后续写入。
            Err(e) => log::warn!("删除旧备份 {} 失败: {}", path.display(), e),
        }
    }
    Ok(removed)
}

/// 采集两个 hive 并落盘一份 env 备份（公开入口，供 CLI `env backup` 与写前自动备份使用）。
///
/// # Returns
/// - `Ok(PathBuf)` — 备份文件绝对路径
/// - `Err(CoreError)` — 采集或落盘失败
pub fn backup_env_vars() -> Result<PathBuf, CoreError> {
    let payload = collect_env_backup()?;
    let dir = env_backup_dir();
    write_env_backup_to(&dir, &payload)
}

/// 备份当前注册表中的系统 PATH 和用户 PATH
/// 在保存前调用，备份的是注册表中的当前值（保存前的状态）
pub fn backup_registry(custom_dir: Option<String>) -> Result<String, String> {
    let backup_dir = match custom_dir {
        Some(ref dir) if !dir.is_empty() => {
            let p = std::path::PathBuf::from(dir);
            let normalized = dir.replace('/', "\\").to_lowercase();
            if normalized.starts_with("c:\\windows\\")
                || normalized.starts_with("c:\\program files\\")
            {
                return Err("不允许备份到系统目录".into());
            }
            p
        }
        _ => backup_base_dir(),
    };

    std::fs::create_dir_all(&backup_dir).map_err(|e| format!("无法创建备份目录: {}", e))?;

    // 读取当前注册表中的值（保存前的旧值）
    let sys_paths = registry::load_paths(HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统")?;
    let user_paths = registry::load_paths(HKEY_CURRENT_USER, USER_REG_PATH, "用户")?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    let filename = format!("path_backup_{}.txt", timestamp);
    let filepath = backup_dir.join(&filename);

    let mut content = String::new();
    content.push_str(&format!(
        "PathEditor Backup - {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    content.push_str("\n[System PATH]\n");
    for path in &sys_paths {
        content.push_str(&format!("{}\n", path));
    }
    content.push_str("\n[User PATH]\n");
    for path in &user_paths {
        content.push_str(&format!("{}\n", path));
    }

    std::fs::write(&filepath, &content).map_err(|e| format!("无法写入备份文件: {}", e))?;

    let result = filepath.to_string_lossy().to_string();
    log::info!("备份已保存到: {}", result);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_var::{revision_of, EnvHive, EnvValueKind};
    // `MemoryHive` 定义在 `reg_store::memory`（`#[cfg(test)] pub(crate) mod`），
    // 未经根 re-export —— brief 写的 `crate::reg_store::MemoryHive` 不成立。
    use crate::reg_store::memory::MemoryHive;
    use winreg::enums::{REG_DWORD, REG_EXPAND_SZ, REG_SZ};

    #[test]
    fn get_appdata_dir_returns_non_empty() {
        assert!(!get_appdata_dir().is_empty());
    }

    #[test]
    fn backup_registry_with_custom_dir() {
        let dir = std::env::temp_dir().join("patheditor_test_backup_custom");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let result = backup_registry(Some(dir.to_string_lossy().to_string()));
        // 可能因无权限读取注册表而失败，但不应 panic
        if let Ok(path) = result {
            assert!(path.contains("patheditor_test_backup_custom"));
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn backup_registry_default_dir_no_panic() {
        // 验证不传参时不会 panic
        let _ = backup_registry(None);
    }

    /// 采集必须排除保留名（Path）与 Unsupported 类型，并携带明文与 revision。
    ///
    /// B1：**先断言 Ok** —— 遇到 Unsupported 必须跳过该变量而非让整次采集失败。
    #[test]
    fn collect_hive_vars_excludes_reserved_and_unsupported() {
        let hive = MemoryHive::new(true);
        hive.seed("JAVA_HOME", "C:\\jdk17", REG_SZ);
        hive.seed("API_TOKEN", "secret-value", REG_SZ);
        hive.seed("Path", "C:\\Windows", REG_EXPAND_SZ); // 保留名，须排除
        hive.seed("SomeDword", "1", REG_DWORD); // Unsupported，须跳过（不是报错）

        // 关键：必须 Ok。若实现走 read_env_var，这里会 Err 而失败。
        let vars = collect_hive_vars_in_store(&hive, EnvHive::User)
            .expect("遇到 Unsupported 类型必须跳过该变量，而不是整次采集失败");

        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"JAVA_HOME"), "普通变量必须被采集");
        assert!(
            names.contains(&"API_TOKEN"),
            "敏感变量也必须采集（备份要能还原）"
        );
        assert!(!names.contains(&"Path"), "保留名必须排除");
        assert!(!names.contains(&"SomeDword"), "Unsupported 类型必须被跳过");

        let java = vars.iter().find(|v| v.name == "JAVA_HOME").unwrap();
        assert_eq!(java.value, "C:\\jdk17", "备份必须携带明文值");
        assert_eq!(java.kind, EnvValueKind::String);
        assert_eq!(java.revision.len(), 16, "revision 是 16 位十六进制摘要");
    }

    /// B1 回归：Unsupported 只在被跳过时出现，**不得**使同一 hive 的其他变量丢失。
    #[test]
    fn collect_hive_vars_keeps_others_when_unsupported_present() {
        let hive = MemoryHive::new(true);
        hive.seed("A_DWORD", "1", REG_DWORD);
        hive.seed("B_NORMAL", "b", REG_SZ);
        hive.seed("C_DWORD", "2", REG_DWORD);
        hive.seed("D_NORMAL", "d", REG_EXPAND_SZ);

        let vars = collect_hive_vars_in_store(&hive, EnvHive::User).expect("必须跳过而非失败");
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();

        assert_eq!(vars.len(), 2, "两个 Unsupported 被跳过，两个正常变量保留");
        assert!(names.contains(&"B_NORMAL"));
        assert!(names.contains(&"D_NORMAL"));
    }

    /// 采集的 revision 必须与 core 的 revision_of 同口径（恢复时据此判冲突）。
    #[test]
    fn collect_hive_vars_revision_matches_revision_of() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "value-1", REG_SZ);

        let vars = collect_hive_vars_in_store(&hive, EnvHive::User).expect("采集失败");
        let expected = revision_of("MY_VAR", REG_SZ, "value-1");

        assert_eq!(vars[0].revision, expected);
    }

    /// 备份目录可用环境变量重定向，供测试隔离；未设置时回落 ~/.patheditor/backups。
    fn with_temp_backup_dir<F: FnOnce(&std::path::Path)>(f: F) {
        // 环境变量是进程级的，与其他触碰它的测试互斥
        let _guard = crate::persist::test_persist_lock();
        let dir =
            std::env::temp_dir().join(format!("patheditor_test_env_backup_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("PATHEDITOR_BACKUP_DIR", &dir);
        f(&dir);
        std::env::remove_var("PATHEDITOR_BACKUP_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `PATHEDITOR_BACKUP_DIR` 必须真正把备份目录整体重定向（否则测试隔离失效，
    /// 所有备份测试会写到用户的真实 `~/.patheditor/backups/`）。
    #[test]
    fn backup_dir_is_redirected_by_env_var() {
        with_temp_backup_dir(|dir| {
            assert_eq!(env_backup_dir(), dir, "备份目录必须被重定向到测试临时目录");
            assert_eq!(
                get_appdata_dir(),
                dir.to_string_lossy(),
                "PATH 备份目录访问器也必须继承同一重定向"
            );
        });
    }

    /// 重定向变量为空时必须回落到默认目录，而不是把备份写到进程当前工作目录。
    #[test]
    fn backup_dir_empty_env_var_falls_back_to_default() {
        let _guard = crate::persist::test_persist_lock();
        let default_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".patheditor")
            .join("backups");

        std::env::set_var("PATHEDITOR_BACKUP_DIR", "");
        let redirected = env_backup_dir();
        std::env::remove_var("PATHEDITOR_BACKUP_DIR");

        assert_eq!(redirected, default_dir, "空值必须回落默认目录");
        assert!(!redirected.as_os_str().is_empty());
    }

    fn sample_payload() -> EnvBackupPayload {
        EnvBackupPayload {
            captured_at: 1_758_400_000_000,
            hives: EnvBackupHives {
                system: vec![],
                user: vec![EnvBackupVar {
                    name: "JAVA_HOME".into(),
                    kind: EnvValueKind::String,
                    value: "C:\\jdk17".into(),
                    revision: "a1b2c3d4e5f60718".into(),
                }],
            },
        }
    }

    /// 落盘文件必须带 schemaVersion 信封，且能被读回（往返一致）。
    #[test]
    fn write_env_backup_round_trips_through_versioned_envelope() {
        with_temp_backup_dir(|dir| {
            let path = write_env_backup_to(dir, &sample_payload()).expect("写备份失败");
            assert!(path.exists());
            assert!(
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("env_backup_"),
                "文件名必须以 env_backup_ 开头"
            );
            assert_eq!(path.extension().unwrap(), "json");

            let content = std::fs::read_to_string(&path).unwrap();
            assert!(
                content.contains("\"schemaVersion\""),
                "必须带 schemaVersion 信封（复用 persist::Versioned）"
            );
            assert!(content.contains("JAVA_HOME"));

            let back: Versioned<EnvBackupPayload> = serde_json::from_str(&content).unwrap();
            assert_eq!(back.inner, sample_payload());
        });
    }

    /// S1 对抗性测试：轮换只删自己生成的 env_backup_*.json，其他文件一律不动。
    ///
    /// **本测试必须真正发生删除**：25 份备份全部唯一，`keep=20` 时轮换必然删 5 份。
    /// 若文件名重复导致候选数 ≤ keep，轮换会提前返回、一份不删——测试就退化成
    /// 空断言，无法发现「误删外部文件」的 bug。故此处额外断言删除确实发生。
    #[test]
    fn rotate_env_backups_never_deletes_foreign_files() {
        with_temp_backup_dir(|dir| {
            // 25 份互不相同的正常备份（超过保留数 20），确保轮换真的删除
            for i in 0..25 {
                let name = format!("env_backup_202601{:02}_120000_000.json", i);
                std::fs::write(dir.join(&name), format!("{{\"n\":{i}}}")).unwrap();
            }
            // 干扰文件：PATH 备份、.bak、损坏隔离、用户自放文件
            std::fs::write(dir.join("path_backup_20260920_152446_043.txt"), "PATH").unwrap();
            // 名字刻意排在待删区间最前（字典序最小）：若过滤条件漏掉 `.ends_with(".json")`
            // 这一半，本文件会被当成候选删掉，断言才会真正失败。
            std::fs::write(dir.join("env_backup_20250101_000000_000.json.bak"), "BAK").unwrap();
            // 另一个 .bak 落在保留区间内（新于 20 份中最旧的一份），
            // 验证「保留区间内的外部文件同样不会被删」。
            std::fs::write(dir.join("env_backup_old.json.bak"), "BAK").unwrap();
            std::fs::write(dir.join("disabled.json.corrupt-20260920-120000000"), "C").unwrap();
            std::fs::write(dir.join("我的笔记.txt"), "note").unwrap();
            std::fs::write(dir.join("env_backup_note.md"), "md").unwrap();

            let removed = rotate_env_backups(dir, ENV_BACKUP_KEEP).expect("轮换失败");

            // 前置条件：轮换确实删了东西，下面的「未删除」断言才有意义
            assert_eq!(
                removed.len(),
                5,
                "25 份候选 - keep 20 份 = 应删 5 份；未删除说明测试空转"
            );
            // 删除目标必须全部是本功能生成的备份，一个外部文件都不许碰
            for path in &removed {
                let name = path.file_name().unwrap().to_string_lossy();
                assert!(
                    name.starts_with("env_backup_") && name.ends_with(".json"),
                    "删除了非本功能生成的文件: {name}"
                );
            }

            assert!(
                dir.join("path_backup_20260920_152446_043.txt").exists(),
                "PATH 备份不得删除"
            );
            assert!(
                dir.join("env_backup_20250101_000000_000.json.bak").exists(),
                ".bak 不得删除"
            );
            assert!(
                dir.join("env_backup_old.json.bak").exists(),
                ".bak 不得删除（保留区间内）"
            );
            assert!(
                dir.join("disabled.json.corrupt-20260920-120000000")
                    .exists(),
                "损坏隔离文件不得删除"
            );
            assert!(dir.join("我的笔记.txt").exists(), "无关文件不得删除");
            assert!(
                dir.join("env_backup_note.md").exists(),
                "非 .json 的同前缀文件不得删除"
            );
        });
    }

    /// 轮换后 env_backup_*.json 恰好保留 keep 份。
    #[test]
    fn rotate_env_backups_keeps_exactly_keep_files() {
        with_temp_backup_dir(|dir| {
            for i in 0..25 {
                std::fs::write(
                    dir.join(format!("env_backup_202601{:02}_120000_000.json", i)),
                    "{}",
                )
                .unwrap();
            }
            rotate_env_backups(dir, ENV_BACKUP_KEEP).expect("轮换失败");

            let remaining = std::fs::read_dir(dir)
                .unwrap()
                .flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.starts_with("env_backup_") && n.ends_with(".json")
                })
                .count();
            assert_eq!(remaining, ENV_BACKUP_KEEP);
        });
    }

    /// 端到端回归：经写入路径之后，目录稳态**恰好**等于 keep 份。
    ///
    /// 这才是 spec 验收标准 11（「轮换保留 config.ini 的 env_backup_keep 份」）的
    /// 口径——用户看到的是目录里有多少份，不是 `rotate_env_backups` 单次调用的
    /// 结果。函数级测试看不到「先轮换、后写入」把稳态推高到 `keep + 1` 的偏差，
    /// 必须走完整写入路径整体断言。
    #[test]
    fn write_env_backup_steady_state_is_exactly_keep_files() {
        with_temp_backup_dir(|dir| {
            let count = |dir: &std::path::Path| {
                std::fs::read_dir(dir)
                    .unwrap()
                    .flatten()
                    .filter(|e| {
                        let n = e.file_name().to_string_lossy().into_owned();
                        n.starts_with("env_backup_") && n.ends_with(".json")
                    })
                    .count()
            };
            const KEEP: usize = 5;

            // 预置远超 keep 份的旧备份
            for i in 0..25 {
                std::fs::write(
                    dir.join(format!("env_backup_202601{:02}_120000_000.json", i)),
                    "{}",
                )
                .unwrap();
            }
            assert_eq!(count(dir), 25, "预置基线");

            let mut written_paths = Vec::new();
            for round in 0..4 {
                let written = write_env_backup_to_with_keep(dir, &sample_payload(), KEEP)
                    .expect("写备份失败");
                assert!(
                    written.exists(),
                    "第 {round} 轮：刚写入的备份必须保留，不得被自己轮换掉"
                );
                assert_eq!(
                    count(dir),
                    KEEP,
                    "第 {round} 轮写入后应为恰好 {KEEP} 份；若为 {} 说明轮换与写入顺序颠倒",
                    KEEP + 1
                );
                written_paths.push(written);
                // 文件名时间戳精度到毫秒；错开 20ms 确保每轮都产生新文件
                // （Windows 计时器粒度约 15.6ms），否则同名覆盖会让本测试失去意义。
                std::thread::sleep(std::time::Duration::from_millis(20));
            }

            // 前置条件：四轮确实各写了不同文件，稳态断言才覆盖「keep+1 → keep」回缩
            let unique: std::collections::HashSet<_> = written_paths.iter().collect();
            assert_eq!(unique.len(), 4, "四轮写入应产生 4 个不同文件，否则测试空转");

            // 最新的一份必须仍在（时间戳最大，永不落入删除集）
            let newest = written_paths.last().unwrap();
            assert!(newest.exists(), "最新备份必须保留");
        });
    }

    /// 配置文件缺失 → 回落默认值，且**不创建文件**。
    #[test]
    fn config_missing_returns_default_and_creates_nothing() {
        let _guard = crate::persist::test_persist_lock();
        let dir =
            std::env::temp_dir().join(format!("patheditor_cfg_missing_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("config.ini");

        assert_eq!(read_env_backup_keep(&cfg), ENV_BACKUP_KEEP);
        assert!(!cfg.exists(), "读配置不得有副作用");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 四种回落情形（键缺失 / 非整数 / 负数 / 空值）都返回默认值而不是报错。
    #[test]
    fn config_invalid_values_fall_back_to_default() {
        let dir =
            std::env::temp_dir().join(format!("patheditor_cfg_invalid_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("config.ini");

        for body in [
            "; 只有注释\n",            // 键缺失
            "env_backup_keep = abc\n", // 非整数
            "env_backup_keep = -5\n",  // 负数
            "env_backup_keep =\n",     // 空值
            "other_key = 1\n",         // 未知键
        ] {
            std::fs::write(&cfg, body).unwrap();
            assert_eq!(
                read_env_backup_keep(&cfg),
                ENV_BACKUP_KEEP,
                "回落失败: {body:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 合法值被采纳；`;` 与 `#` 注释、行内两侧空格都能正确解析。
    #[test]
    fn config_reads_valid_value_with_comments_and_spaces() {
        let dir = std::env::temp_dir().join(format!("patheditor_cfg_valid_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("config.ini");

        std::fs::write(&cfg, "; 注释行\n# 另一种注释\nenv_backup_keep   =   7   \n").unwrap();
        assert_eq!(read_env_backup_keep(&cfg), 7);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
