use path_editor_core::disabled;
use path_editor_core::PathSnapshot;
use tauri::State;

use super::db_state::DbState;

/// 读取完整 PATH 快照（注册表 + DB 当前快照；DB 不可用时只读注册表）。
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
