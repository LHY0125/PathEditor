use path_editor_core::registry;
use path_editor_core::PathEntry;

#[tauri::command]
pub fn load_system_paths() -> Result<Vec<String>, String> {
    registry::load_system_paths()
}
#[tauri::command]
pub fn load_user_paths() -> Result<Vec<String>, String> {
    registry::load_user_paths()
}

#[tauri::command]
pub fn clean_path_entries(entries: Vec<PathEntry>) -> (Vec<PathEntry>, Vec<PathEntry>) {
    registry::clean_path_entries(entries)
}
