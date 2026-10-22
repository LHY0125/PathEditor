use path_editor_core::disabled;
use path_editor_core::{PathEntry, PathSnapshot};
use tauri::State;

use super::history::DbState;

#[tauri::command]
pub fn save_disabled_state(system: Vec<String>, user: Vec<String>) -> Result<(), String> {
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

/// 读取完整 PATH 快照（注册表 + DB 当前快照；DB 不可用时回落 v5 文件）。
#[tauri::command]
pub fn load_path_snapshot(state: State<'_, DbState>) -> Result<PathSnapshot, String> {
    let Ok(guard) = state.0.lock() else {
        return disabled::load_path_snapshot().map_err(|e| e.message);
    };
    match guard.as_ref() {
        Some(db) => path_editor_core::service::load_path_snapshot_db(db).map_err(|e| e.message),
        None => disabled::load_path_snapshot().map_err(|e| e.message),
    }
}
