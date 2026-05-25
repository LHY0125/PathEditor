use winreg::enums::*;
use winreg::RegKey;

const SYS_REG_PATH: &str = "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment";
const USER_REG_PATH: &str = "Environment";
const PATH_VALUE: &str = "Path";

/// 从注册表加载系统 PATH
#[tauri::command]
pub fn load_system_paths() -> Result<Vec<String>, String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let env_key = hklm
        .open_subkey_with_flags(SYS_REG_PATH, KEY_READ)
        .map_err(|e| format!("无法打开系统注册表项: {}", e))?;

    let value: String = env_key
        .get_value(PATH_VALUE)
        .map_err(|e| format!("无法读取系统 PATH: {}", e))?;

    Ok(split_path(&value))
}

/// 从注册表加载用户 PATH
#[tauri::command]
pub fn load_user_paths() -> Result<Vec<String>, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu
        .open_subkey_with_flags(USER_REG_PATH, KEY_READ)
        .map_err(|e| format!("无法打开用户注册表项: {}", e))?;

    let value: String = env_key
        .get_value(PATH_VALUE)
        .map_err(|e| format!("无法读取用户 PATH: {}", e))?;

    Ok(split_path(&value))
}

/// 保存系统 PATH 到注册表
#[tauri::command]
pub fn save_system_paths(paths: Vec<String>) -> Result<(), String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let env_key = hklm
        .open_subkey_with_flags(SYS_REG_PATH, KEY_WRITE)
        .map_err(|e| format!("无法写入系统注册表（需要管理员权限）: {}", e))?;

    let value = join_path(&paths);
    env_key
        .set_value(PATH_VALUE, &value)
        .map_err(|e| format!("无法写入系统 PATH: {}", e))?;

    log::info!("已保存系统 PATH，{} 个条目", paths.len());
    Ok(())
}

/// 保存用户 PATH 到注册表
#[tauri::command]
pub fn save_user_paths(paths: Vec<String>) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu
        .open_subkey_with_flags(USER_REG_PATH, KEY_WRITE)
        .map_err(|e| format!("无法写入用户注册表: {}", e))?;

    let value = join_path(&paths);
    env_key
        .set_value(PATH_VALUE, &value)
        .map_err(|e| format!("无法写入用户 PATH: {}", e))?;

    log::info!("已保存用户 PATH，{} 个条目", paths.len());
    Ok(())
}

/// 用分号分割 PATH 字符串
fn split_path(raw: &str) -> Vec<String> {
    raw.split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 用分号连接路径列表
fn join_path(paths: &[String]) -> String {
    paths.join(";")
}
