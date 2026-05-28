use winreg::enums::*;
use winreg::RegKey;

pub(crate) const SYS_REG_PATH: &str = "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment";
pub(crate) const USER_REG_PATH: &str = "Environment";
const PATH_VALUE: &str = "Path";

pub(crate) fn load_paths(root: winreg::HKEY, sub_path: &str, label: &str) -> Result<Vec<String>, String> {
    let key = RegKey::predef(root);
    let env_key = key
        .open_subkey_with_flags(sub_path, KEY_READ)
        .map_err(|e| format!("无法打开{}注册表项: {}", label, e))?;

    let value: String = env_key
        .get_value(PATH_VALUE)
        .map_err(|e| format!("无法读取{} PATH: {}", label, e))?;

    Ok(split_path(&value))
}

fn save_paths(root: winreg::HKEY, sub_path: &str, label: &str, paths: &[String]) -> Result<(), String> {
    let value = join_path(paths);

    // Windows 注册表 REG_EXPAND_SZ 上限 32767 字符
    const MAX_PATH_LEN: usize = 32767;
    if value.len() > MAX_PATH_LEN {
        return Err(format!(
            "{} PATH 总长度 {} 超出 Windows 限制 {} 字符，请移除部分路径后再保存",
            label, value.len(), MAX_PATH_LEN
        ));
    }

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

/// 将分号分隔的 PATH 字符串拆分为数组。
/// 注意：TS 端 src/core/validation.ts 有相同逻辑的 split_path，修改时需同步两端。
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

/// 清理路径列表：移除不存在的目录 + 重复路径（保留首次出现）
/// 返回 (保留的路径, 被移除的路径)
pub fn clean_paths(paths: Vec<String>) -> (Vec<String>, Vec<String>) {
    use std::collections::HashSet;
    let mut seen: HashSet<String> = HashSet::new();
    let mut kept = Vec::new();
    let mut removed = Vec::new();
    for p in paths {
        let key = p.trim().to_lowercase();
        if seen.contains(&key) {
            removed.push(p);
            continue;
        }
        seen.insert(key);
        if !p.contains('%') && !std::path::Path::new(&p).is_dir() {
            removed.push(p);
            continue;
        }
        kept.push(p);
    }
    (kept, removed)
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
        assert_eq!(
            split_path(" C:\\ ; ; D:\\ "),
            vec!["C:\\", "D:\\"]
        );
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
}
