use path_editor_core::registry;
use path_editor_core::{CoreError, PathEntry};

#[tauri::command]
pub fn load_system_paths() -> Result<Vec<String>, CoreError> {
    registry::load_system_paths()
}
#[tauri::command]
pub fn load_user_paths() -> Result<Vec<String>, CoreError> {
    registry::load_user_paths()
}

#[tauri::command]
pub fn clean_path_entries(entries: Vec<PathEntry>) -> (Vec<PathEntry>, Vec<PathEntry>) {
    registry::clean_path_entries(entries)
}
