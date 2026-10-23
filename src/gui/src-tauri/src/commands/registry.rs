use path_editor_core::registry;
use path_editor_core::PathEntry;

/// 清理 PATH 条目（移除空路径、不存在的目录与重复项）。
///
/// 读取 PATH 本身走 `load_path_snapshot`（含 DB 快照合并与降级语义）。
#[tauri::command]
pub fn clean_path_entries(entries: Vec<PathEntry>) -> (Vec<PathEntry>, Vec<PathEntry>) {
    registry::clean_path_entries(entries)
}
