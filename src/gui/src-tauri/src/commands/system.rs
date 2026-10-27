use path_editor_core::platform::system;

#[tauri::command]
pub fn check_admin() -> bool {
    system::check_admin()
}
#[tauri::command]
pub fn get_path_capabilities() -> path_editor_core::platform::capabilities::PathCapabilities {
    path_editor_core::platform::capabilities::detect()
}
#[tauri::command]
pub fn validate_path(path: &str) -> bool {
    system::validate_path(path)
}
#[tauri::command]
pub fn expand_env_vars(path: &str) -> String {
    system::expand_env_vars(path)
}
