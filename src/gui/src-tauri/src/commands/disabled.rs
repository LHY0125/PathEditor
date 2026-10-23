use path_editor_core::service;
use path_editor_core::CoreError;
use path_editor_core::PathSnapshot;
use tauri::State;

use super::db_state::{with_optional_db, DbState};

/// 读取完整 PATH 快照（注册表 + DB 当前快照；DB 不可用时只读注册表）。
///
/// 降级分支由 `service::load_path_snapshot_any` 决定；本命令只负责取锁与转发。
#[tauri::command]
pub fn load_path_snapshot(state: State<'_, DbState>) -> Result<PathSnapshot, CoreError> {
    with_optional_db(
        &state,
        "load_path_snapshot",
        service::load_path_snapshot_any,
    )
}
