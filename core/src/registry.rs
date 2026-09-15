use crate::path_entry::PathEntry;
use std::path::Path;
use winreg::enums::*;
use winreg::types::ToRegValue;
use winreg::{RegKey, RegValue};

pub(crate) const SYS_REG_PATH: &str =
    "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment";
pub(crate) const USER_REG_PATH: &str = "Environment";
const PATH_VALUE: &str = "Path";

pub(crate) fn load_paths(
    root: winreg::HKEY,
    sub_path: &str,
    label: &str,
) -> Result<Vec<String>, String> {
    let key = RegKey::predef(root);
    let env_key = key
        .open_subkey_with_flags(sub_path, KEY_READ)
        .map_err(|e| format!("无法打开{}注册表项: {}", label, e))?;

    let value: String = env_key
        .get_value(PATH_VALUE)
        .map_err(|e| format!("无法读取{} PATH: {}", label, e))?;

    Ok(split_path(&value))
}

fn select_path_value_type(existing: Option<RegType>) -> RegType {
    match existing {
        Some(REG_SZ) => REG_SZ,
        _ => REG_EXPAND_SZ,
    }
}

fn path_value_type(env_key: &RegKey) -> RegType {
    let existing = env_key.get_raw_value(PATH_VALUE).ok().map(|raw| raw.vtype);
    select_path_value_type(existing)
}

fn make_path_value(value: &str, vtype: RegType) -> RegValue {
    // 复用 winreg 的 UTF-16LE 编码与结尾 NUL 逻辑，只覆盖值类型。
    let mut raw = value.to_reg_value();
    raw.vtype = vtype;
    raw
}

fn save_paths(
    root: winreg::HKEY,
    sub_path: &str,
    label: &str,
    paths: &[String],
) -> Result<(), String> {
    let value = validate_and_join_paths(paths, label)?;

    let key = RegKey::predef(root);
    // 需要同时读取原值类型并写回，因此请求 READ | WRITE。
    let env_key = key
        .open_subkey_with_flags(sub_path, KEY_READ | KEY_WRITE)
        .map_err(|e| format!("无法写入{}注册表（需要管理员权限）: {}", label, e))?;

    let vtype = path_value_type(&env_key);
    let raw = make_path_value(&value, vtype);

    env_key
        .set_raw_value(PATH_VALUE, &raw)
        .map_err(|e| format!("无法写入{} PATH: {}", label, e))?;

    log::info!("已保存{} PATH，{} 个条目", label, paths.len());
    Ok(())
}
/// 从 HKLM 注册表读取系统 PATH
///
/// # Returns
/// - `Ok(Vec<String>)` — 系统 PATH 路径列表
/// - `Err(String)` — 注册表读取失败
pub fn load_system_paths() -> Result<Vec<String>, String> {
    load_paths(HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统")
}

/// 从 HKCU 注册表读取用户 PATH
///
/// # Returns
/// - `Ok(Vec<String>)` — 用户 PATH 路径列表
/// - `Err(String)` — 注册表读取失败
pub fn load_user_paths() -> Result<Vec<String>, String> {
    load_paths(HKEY_CURRENT_USER, USER_REG_PATH, "用户")
}

/// 保存系统 PATH 到注册表，含 32767 字符上限检查
///
/// # Returns
/// - `Ok(())` — 保存成功
/// - `Err(String)` — 写入失败或超过字符上限
pub fn save_system_paths(paths: Vec<String>) -> Result<(), String> {
    save_paths(HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统", &paths)
}

/// 保存用户 PATH 到注册表
///
/// # Returns
/// - `Ok(())` — 保存成功
/// - `Err(String)` — 写入失败
pub fn save_user_paths(paths: Vec<String>) -> Result<(), String> {
    save_paths(HKEY_CURRENT_USER, USER_REG_PATH, "用户", &paths)
}

/// 探测当前用户是否有权写入 HKCU 的 PATH 注册表项。
pub fn can_write_user() -> bool {
    let key = RegKey::predef(HKEY_CURRENT_USER);
    key.open_subkey_with_flags(USER_REG_PATH, KEY_WRITE).is_ok()
}

/// 将分号分隔的 PATH 字符串拆分为数组。
/// TS 端 split_path 仅保留为测试夹具；正式 PATH 解析以此处为准。
fn split_path(raw: &str) -> Vec<String> {
    raw.split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn join_path(paths: &[String]) -> String {
    paths
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(";")
}

/// 验证路径列表并拼接为分号分隔字符串
/// - 检查 null 字节
/// - 检查 UTF-16 总长度不超过 32767
fn validate_and_join_paths(paths: &[String], label: &str) -> Result<String, String> {
    if let Some(bad) = paths.iter().find(|p| p.contains('\0')) {
        return Err(format!("{} PATH 包含非法字符（null 字节）: {}", label, bad));
    }
    let value = join_path(paths);
    const MAX_PATH_LEN: usize = 32767;
    let utf16_len = value.encode_utf16().count();
    if utf16_len > MAX_PATH_LEN {
        return Err(format!(
            "{} PATH 总长度 {} 超出 Windows 限制 {} 字符，请移除部分路径后再保存",
            label, utf16_len, MAX_PATH_LEN
        ));
    }
    Ok(value)
}

/// 清理 PathEntry 列表：移除空路径、不存在的目录和重复路径（保留首次出现）。
///
/// 环境变量路径会先展开；无法展开的路径视为 unknown 并保留，避免误删。
pub fn clean_path_entries(entries: Vec<PathEntry>) -> (Vec<PathEntry>, Vec<PathEntry>) {
    use std::collections::HashSet;

    let mut seen: HashSet<String> = HashSet::new();
    let mut kept = Vec::new();
    let mut removed = Vec::new();

    for entry in entries {
        let trimmed = entry.path.trim();
        let key = trimmed.to_lowercase();
        if key.is_empty() || seen.contains(&key) {
            removed.push(entry);
            continue;
        }
        seen.insert(key);

        let expanded = crate::system::expand_env_vars(trimmed);
        let exists = expanded.contains('%') || Path::new(&expanded).is_dir();
        if exists {
            kept.push(PathEntry {
                path: trimmed.to_string(),
                enabled: entry.enabled,
            });
        } else {
            removed.push(entry);
        }
    }

    (kept, removed)
}

/// 清理路径字符串列表；保留旧 CLI 接口，语义与 `clean_path_entries` 一致。
pub fn clean_paths(paths: Vec<String>) -> (Vec<String>, Vec<String>) {
    let entries = paths
        .into_iter()
        .map(|path| PathEntry {
            path,
            enabled: true,
        })
        .collect();
    let (kept, removed) = clean_path_entries(entries);
    (
        kept.into_iter().map(|entry| entry.path).collect(),
        removed.into_iter().map(|entry| entry.path).collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_empty() {
        assert_eq!(split_path(""), Vec::<String>::new());
    }

    #[test]
    fn split_single() {
        assert_eq!(split_path("C:\\Windows"), vec!["C:\\Windows"]);
    }

    #[test]
    fn split_multiple() {
        assert_eq!(
            split_path("C:\\Windows;D:\\Projects"),
            vec!["C:\\Windows", "D:\\Projects"]
        );
    }

    #[test]
    fn split_trims_and_filters_empty() {
        assert_eq!(split_path(" C:\\ ; ; D:\\ "), vec!["C:\\", "D:\\"]);
    }

    #[test]
    fn join_and_split_roundtrip() {
        let paths = vec!["C:\\Windows".to_string(), "D:\\Projects".to_string()];
        let joined = join_path(&paths);
        let split = split_path(&joined);
        assert_eq!(split, paths);
    }

    #[test]
    fn join_trims_entries() {
        let paths = vec![" C:\\Windows ".to_string(), " D:\\ ".to_string()];
        assert_eq!(join_path(&paths), "C:\\Windows;D:\\");
    }

    #[test]
    fn validate_rejects_null_bytes() {
        let paths = vec!["C:\\safe".into(), "C:\0invalid".into()];
        let result = validate_and_join_paths(&paths, "测试");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("null 字节"));
    }

    #[test]
    fn validate_accepts_cjk_paths() {
        let paths = vec!["C:\\用户\\工具".into()];
        let result = validate_and_join_paths(&paths, "测试");
        assert!(result.is_ok());
    }

    #[test]
    fn validate_rejects_oversized_paths() {
        // 构造总长超过 32767 UTF-16 字符的路径
        let long_path = "C:\\".to_string() + &"a".repeat(32767);
        let paths = vec![long_path];
        let result = validate_and_join_paths(&paths, "测试");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("超出 Windows 限制"));
    }
}

#[cfg(test)]
mod issue26_tests {
    use super::{join_path, make_path_value, save_paths, select_path_value_type, PATH_VALUE};
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, REG_DWORD, REG_EXPAND_SZ, REG_SZ};
    use winreg::types::FromRegValue;
    use winreg::RegKey;

    struct TempRegistryKey {
        root: winreg::HKEY,
        path: String,
    }

    impl Drop for TempRegistryKey {
        fn drop(&mut self) {
            let root = RegKey::predef(self.root);
            let _ = root.delete_subkey_all(&self.path);
        }
    }

    #[test]
    fn select_path_value_type_keeps_expand_sz() {
        assert_eq!(select_path_value_type(Some(REG_EXPAND_SZ)), REG_EXPAND_SZ);
    }

    #[test]
    fn select_path_value_type_keeps_sz() {
        assert_eq!(select_path_value_type(Some(REG_SZ)), REG_SZ);
    }

    #[test]
    fn select_path_value_type_defaults_for_missing_value() {
        assert_eq!(select_path_value_type(None), REG_EXPAND_SZ);
    }

    #[test]
    fn select_path_value_type_defaults_for_non_string_value() {
        assert_eq!(select_path_value_type(Some(REG_DWORD)), REG_EXPAND_SZ);
    }

    #[test]
    fn make_path_value_preserves_type_and_text() {
        let raw = make_path_value("%SystemRoot%\\system32", REG_EXPAND_SZ);
        assert_eq!(raw.vtype, REG_EXPAND_SZ);
        assert_eq!(
            String::from_reg_value(&raw).expect("解码注册表值失败"),
            "%SystemRoot%\\system32"
        );
    }

    #[test]
    fn make_path_value_uses_utf16_nul_terminator() {
        let raw = make_path_value("C:\\Windows", REG_SZ);
        assert_eq!(raw.bytes.len() % 2, 0);
        assert_eq!(&raw.bytes[raw.bytes.len() - 2..], &[0, 0]);
    }

    #[test]
    #[ignore = "需要真实注册表写权限；只使用隔离测试键，不接触 PATH"]
    fn save_paths_keeps_expand_sz_in_isolated_key() {
        let parent = "Software\\PathEditor\\Tests";
        let unique = format!(
            "{}\\issue26-{}-{}",
            parent,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("系统时间早于 UNIX_EPOCH")
                .as_nanos()
        );

        let root = RegKey::predef(HKEY_CURRENT_USER);
        root.create_subkey(parent).expect("创建隔离测试父键失败");
        let (key, _) = root.create_subkey(&unique).expect("创建隔离测试键失败");
        key.set_raw_value(
            PATH_VALUE,
            &make_path_value("C:\\Windows;%SystemRoot%\\system32", REG_EXPAND_SZ),
        )
        .expect("写入初始 REG_EXPAND_SZ 失败");
        drop(key);

        let _guard = TempRegistryKey {
            root: HKEY_CURRENT_USER,
            path: unique.clone(),
        };

        let paths = vec![
            "C:\\Windows".to_string(),
            "%SystemRoot%\\system32".to_string(),
        ];
        save_paths(HKEY_CURRENT_USER, &unique, "测试", &paths).expect("保存测试 PATH 失败");

        let key = root
            .open_subkey_with_flags(&unique, KEY_READ)
            .expect("重新打开隔离测试键失败");
        let raw = key.get_raw_value(PATH_VALUE).expect("读取测试值失败");
        assert_eq!(raw.vtype, REG_EXPAND_SZ);
        assert_eq!(
            String::from_reg_value(&raw).expect("解码测试值失败"),
            join_path(&paths)
        );
    }
}
