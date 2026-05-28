use path_editor_core::system;

#[tauri::command]
pub fn check_admin() -> bool { system::check_admin() }
#[tauri::command]
pub fn validate_path(path: &str) -> bool { system::validate_path(path) }
#[tauri::command]
pub fn expand_env_vars(path: &str) -> String { system::expand_env_vars(path) }
#[tauri::command]
pub fn broadcast_env_change() { system::broadcast_env_change() }
