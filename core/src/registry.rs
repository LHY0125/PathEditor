use crate::path_entry::PathEntry;
use std::path::Path;
use winreg::enums::*;
use winreg::RegKey;

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

fn save_paths(
    root: winreg::HKEY,
    sub_path: &str,
    label: &str,
    paths: &[String],
) -> Result<(), String> {
    let value = validate_and_join_paths(paths, label)?;

    let key = RegKey::predef(root);
    let env_key = key
        .open_subkey_with_flags(sub_path, KEY_WRITE)
        .map_err(|e| format!("无法写入{}注册表（需要管理员权限）: {}", label, e))?;

    env_key
        .set_value(PATH_VALUE, &value)
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
