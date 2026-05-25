use winreg::enums::*;
use winreg::RegKey;

const SYS_REG_PATH: &str = "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment";
const USER_REG_PATH: &str = "Environment";
const PATH_VALUE: &str = "Path";

fn load_paths(root: winreg::HKEY, sub_path: &str, label: &str) -> Result<Vec<String>, String> {
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
    let key = RegKey::predef(root);
    let env_key = key
        .open_subkey_with_flags(sub_path, KEY_WRITE)
        .map_err(|e| format!("无法写入{}注册表（需要管理员权限）: {}", label, e))?;

    let value = join_path(paths);
    env_key
        .set_value(PATH_VALUE, &value)
        .map_err(|e| format!("无法写入{} PATH: {}", label, e))?;

    log::info!("已保存{} PATH，{} 个条目", label, paths.len());
    Ok(())
}

#[tauri::command]
pub fn load_system_paths() -> Result<Vec<String>, String> {
    load_paths(HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统")
}

#[tauri::command]
pub fn load_user_paths() -> Result<Vec<String>, String> {
    load_paths(HKEY_CURRENT_USER, USER_REG_PATH, "用户")
}

#[tauri::command]
pub fn save_system_paths(paths: Vec<String>) -> Result<(), String> {
    save_paths(HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统", &paths)
}

#[tauri::command]
pub fn save_user_paths(paths: Vec<String>) -> Result<(), String> {
    save_paths(HKEY_CURRENT_USER, USER_REG_PATH, "用户", &paths)
}

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
