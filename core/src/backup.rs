use crate::env_var::{is_reserved, revision_of, EnvHive, EnvValueKind};
use crate::error::{CoreError, ErrorCode};
use crate::reg_store::{EnvHiveStore, WinregHive};
// 经 registry 根 re-export —— 不能写 crate::registry::env_var::X（E0603）
use crate::registry::{self, hive_location, SYS_REG_PATH, USER_REG_PATH};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use winreg::enums::*;
use winreg::types::FromRegValue;

fn backup_base_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("backups")
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
}
