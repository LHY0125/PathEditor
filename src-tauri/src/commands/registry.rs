use path_editor_core::registry;

#[tauri::command]
pub fn load_system_paths() -> Result<Vec<String>, String> { registry::load_system_paths() }
#[tauri::command]
pub fn load_user_paths() -> Result<Vec<String>, String> { registry::load_user_paths() }
#[tauri::command]
pub fn save_system_paths(paths: Vec<String>) -> Result<(), String> { registry::save_system_paths(paths) }
#[tauri::command]
pub fn save_user_paths(paths: Vec<String>) -> Result<(), String> { registry::save_user_paths(paths) }
