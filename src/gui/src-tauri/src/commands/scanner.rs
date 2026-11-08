use super::db_state::{with_db, with_db_mut, DbState};
use path_editor_core::db::scan_history;
use path_editor_core::platform::scanner;
use path_editor_core::CoreError;
use tauri::State;

/// 扫描 PATH 目录，并在 DB 可用时记录本次扫描（best-effort）。
#[tauri::command(async)]
pub fn scan_paths(
    paths: Vec<String>,
    query: String,
    state: State<'_, DbState>,
) -> Result<scanner::ScanResult, CoreError> {
    let result = scanner::scan_paths(paths, query)?;
    let names: Vec<String> = result.conflicts.iter().map(|c| c.name.clone()).collect();
    let dirs: Vec<String> = result.tools.iter().map(|g| g.dir.clone()).collect();
    let _ = with_db_mut(&state, "scan_record", |db| {
        scan_history::record_run(db, &names, &dirs)
    });
    Ok(result)
}

/// 最近的扫描历史（按时间倒序）。
#[tauri::command(async)]
pub fn scan_history_list(
    limit: u32,
    state: State<'_, DbState>,
) -> Result<Vec<scan_history::ScanRunView>, CoreError> {
    with_db(&state, "scan_history_list", |db| {
        scan_history::list_runs(db, limit)
    })
}

/// 比较两次扫描（`a` 上次、`b` 本次）。
#[tauri::command(async)]
pub fn scan_history_diff(
    a: String,
    b: String,
    state: State<'_, DbState>,
) -> Result<scan_history::ScanRunDiff, CoreError> {
    with_db(&state, "scan_history_diff", |db| {
        scan_history::diff_runs(db, &a, &b)
    })
}
