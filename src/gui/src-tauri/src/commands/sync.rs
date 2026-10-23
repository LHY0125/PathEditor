//! 同步中心的 Tauri 命令。
//!
//! **同步不写注册表**：导入只改 DB，用户要生效必须走既有的
//! `apply_path_snapshot` / `apply_profile`。
//!
//! **口令不进前端 store**：这里的 `passphrase` 参数用完即弃，不返回、不记录。
//! 组件侧用 `type="password"` + 局部 state（与历史快照 reveal 同口径）。

use path_editor_core::db::sync::conflict::{list_conflicts, Resolution};
use path_editor_core::db::sync::export::export_bundle;
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::sync::{self, ExportReport, ImportReport, SyncStatus};
use path_editor_core::CoreError;
use tauri::State;

use super::db_state::{with_db, with_db_mut, DbState};

/// 待导出数与未解决冲突数。
#[tauri::command]
pub fn sync_status(state: State<'_, DbState>) -> Result<SyncStatus, CoreError> {
    with_db(&state, "sync_status", sync::status)
}

/// 导出同步包。
///
/// `path` 是用户选择的保存路径；`passphrase` 只用于本次加密，不落盘。
#[tauri::command]
pub fn sync_export(
    passphrase: String,
    path: String,
    state: State<'_, DbState>,
) -> Result<ExportReport, CoreError> {
    with_db_mut(&state, "sync_export", |db| {
        export_bundle(db, &passphrase, std::path::Path::new(&path))
    })
}

/// 导入同步包。
#[tauri::command]
pub fn sync_import(
    passphrase: String,
    path: String,
    state: State<'_, DbState>,
) -> Result<ImportReport, CoreError> {
    with_db_mut(&state, "sync_import", |db| {
        import_bundle(db, &passphrase, std::path::Path::new(&path))
    })
}

/// 冲突列表。`include_resolved = true` 时含已解决的。
#[tauri::command]
pub fn sync_list_conflicts(
    include_resolved: bool,
    state: State<'_, DbState>,
) -> Result<Vec<path_editor_core::db::sync::conflict::SyncConflictView>, CoreError> {
    with_db(&state, "sync_list_conflicts", |db| {
        list_conflicts(db, include_resolved)
    })
}

/// 解决一条冲突。`resolution` 取 `local` / `remote`。
#[tauri::command]
pub fn sync_resolve_conflict(
    id: i64,
    resolution: String,
    state: State<'_, DbState>,
) -> Result<(), CoreError> {
    let Some(r) = Resolution::parse(&resolution) else {
        return Err(CoreError::new(
            path_editor_core::ErrorCode::InvalidValue,
            "sync_resolve_conflict",
            "解决方式只能是 local 或 remote",
        ));
    };
    with_db_mut(&state, "sync_resolve_conflict", |db| {
        path_editor_core::db::sync::conflict::resolve_conflict(db, id, r)
    })
}
