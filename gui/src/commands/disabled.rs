use path_editor_core::disabled;
use path_editor_core::{PathEntry, PathSnapshot};

#[tauri::command]
pub fn save_disabled_state(system: Vec<String>, user: Vec<String>) -> Result<(), String> {
    // F-11：core 侧已迁移为 CoreError；IPC 契约仍是自由文本，透传 message。
    disabled::save_disabled_state(system, user).map_err(|e| e.message)
}

#[tauri::command]
pub fn load_disabled_state() -> Result<(Vec<String>, Vec<String>), String> {
    disabled::load_disabled_state().map_err(|e| e.message)
}

#[tauri::command]
pub fn save_path_snapshot(
    system: Option<Vec<PathEntry>>,
    user: Option<Vec<PathEntry>>,
) -> Result<(), String> {
    disabled::save_path_snapshot(system, user).map_err(|e| e.message)
}

#[tauri::command]
pub fn load_path_snapshot() -> Result<PathSnapshot, String> {
    disabled::load_path_snapshot().map_err(|e| e.message)
}
