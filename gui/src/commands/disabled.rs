use path_editor_core::disabled;
use path_editor_core::{PathEntry, PathSnapshot};

#[tauri::command]
pub fn save_disabled_state(system: Vec<String>, user: Vec<String>) -> Result<(), String> {
    disabled::save_disabled_state(system, user)
}

#[tauri::command]
pub fn load_disabled_state() -> Result<(Vec<String>, Vec<String>), String> {
    disabled::load_disabled_state()
}

#[tauri::command]
pub fn save_path_snapshot(
    system: Option<Vec<PathEntry>>,
    user: Option<Vec<PathEntry>>,
) -> Result<(), String> {
    disabled::save_path_snapshot(system, user)
}

#[tauri::command]
pub fn load_path_snapshot() -> Result<PathSnapshot, String> {
    disabled::load_path_snapshot()
}
