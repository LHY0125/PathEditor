use path_editor_core::registry;

#[tauri::command]
pub fn load_system_paths() -> Result<Vec<String>, String> {
    registry::load_system_paths()
}
#[tauri::command]
pub fn load_user_paths() -> Result<Vec<String>, String> {
    registry::load_user_paths()
}
#[tauri::command]
pub fn save_system_paths(paths: Vec<String>, original: Option<Vec<String>>) -> Result<(), String> {
    if let Some(orig) = original {
        let current = registry::load_system_paths()?;
        if current != orig {
            return Err("注册表已被其他进程修改，请重新加载后重试".to_string());
        }
    }
    registry::save_system_paths(paths)
}
#[tauri::command]
pub fn save_user_paths(paths: Vec<String>, original: Option<Vec<String>>) -> Result<(), String> {
    if let Some(orig) = original {
        let current = registry::load_user_paths()?;
        if current != orig {
            return Err("注册表已被其他进程修改，请重新加载后重试".to_string());
        }
    }
    registry::save_user_paths(paths)
}
