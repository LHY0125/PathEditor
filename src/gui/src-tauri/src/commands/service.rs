//! 共享应用服务层（`core::service`）的 Tauri 命令封装。
//!
//! 只做参数转换与错误呈现。DB 可用时把 PATH 保存交给
//! `service::save_path_with_db`（写注册表 + 当前快照 + 事件）；DB 不可用时
//! 降级为只写注册表，编辑照常成功，但禁用状态与历史无法持久化。

use path_editor_core::history::EventSource;
use path_editor_core::history::NoopHistory;
use path_editor_core::service::{self, ApplyOutcome, PathHiveWrite};
use path_editor_core::CoreError;
use tauri::State;

use super::db_state::DbState;

/// 同时写注册表与 sidecar（best-effort 多 hive，partial 语义按 hive 分字段）。
///
/// 每个 hive 传 [`PathHiveWrite`]（含 `expected` 用于乐观并发校验）。
#[tauri::command]
pub fn save_path_with_sidecar(
    system: Option<PathHiveWrite>,
    user: Option<PathHiveWrite>,
    state: State<'_, DbState>,
) -> Result<ApplyOutcome, CoreError> {
    let Ok(mut guard) = state.0.lock() else {
        log::warn!("历史库状态锁中毒，本次保存不记录历史");
        return service::save_path_registry_only(system, user, EventSource::Gui, &NoopHistory);
    };

    match guard.as_mut() {
        Some(db) => service::save_path_with_db(db, EventSource::Gui, system, user),
        None => service::save_path_registry_only(system, user, EventSource::Gui, &NoopHistory),
    }
}
