//! 共享应用服务层（`core::service`）的 Tauri 命令封装。
//!
//! F-08（Wave 2 Task 5）：GUI 编排的注册表写入、广播、sidecar/pending
//! 事务语义下沉到 core；本文件只做参数转换与错误透传（自由文本过渡期）。
//!
//! v6 M1：服务层新增 `history: &dyn HistorySink` 参数。命令层先统一传
//! [`NoopHistory`]，历史落库由 Task 9 的 `DbState` 接入后替换。

use path_editor_core::history::NoopHistory;
use path_editor_core::service::{self, ApplyOutcome, PathHiveWrite};
use path_editor_core::PathEntry;

/// 同时写注册表与 sidecar（best-effort 多 hive，partial 语义按 hive 分字段）。
///
/// 每个 hive 传 [`PathHiveWrite`]（含 `expected` 用于乐观并发校验）。
#[tauri::command]
pub fn save_path_with_sidecar(
    system: Option<PathHiveWrite>,
    user: Option<PathHiveWrite>,
) -> Result<ApplyOutcome, String> {
    service::save_path_with_sidecar(system, user, &NoopHistory).map_err(|e| e.message)
}

/// 把单 hive 的 PATH 写入注册表并落盘快照；未触及的另一 hive 为 `Skipped`。
#[tauri::command]
pub fn apply_path_snapshot(
    hive: path_editor_core::EnvHive,
    entries: Vec<PathEntry>,
) -> Result<ApplyOutcome, String> {
    service::apply_path_snapshot(hive, entries, &NoopHistory).map_err(|e| e.message)
}

/// 补写上次未落盘的快照（sidecar-only，不触碰注册表）。
#[tauri::command]
pub fn retry_pending_path_state() -> Result<ApplyOutcome, String> {
    service::retry_pending_path_state(&NoopHistory).map_err(|e| e.message)
}

/// 应用一个配置文件（多 hive best-effort）。
#[tauri::command]
pub fn apply_profile(name: String) -> Result<ApplyOutcome, String> {
    service::apply_profile(&name, &NoopHistory).map_err(|e| e.message)
}
