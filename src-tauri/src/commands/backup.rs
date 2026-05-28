use path_editor_core::backup;

#[tauri::command]
pub fn backup_registry(custom_dir: Option<String>) -> Result<String, String> { backup::backup_registry(custom_dir) }
#[tauri::command]
pub fn get_appdata_dir() -> String { backup::get_appdata_dir() }
