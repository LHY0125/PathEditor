//! 历史快照的 Tauri 命令。
//!
//! **只做参数转换与错误呈现**，业务规则全在 core。
//!
//! 降级语义：DB 不可用时 `DbState` 为 `None`，所有 `history_*` 命令返回
//! `CoreError { code: database }`；注册表编辑命令不受影响。

use super::db_state::{with_db, with_db_mut, DbState};
use path_editor_core::db::history::{self, EventFilter, EventPage, SnapshotView};
use path_editor_core::CoreError;
use serde::Deserialize;
use tauri::State;

/// 事件筛选的 IPC 入参；所有字段可选。
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventFilterRequest {
    /// 本页条数
    pub limit: Option<u32>,
    /// 偏移
    pub offset: Option<u32>,
    /// 按动作筛选
    pub action: Option<String>,
    /// 按来源筛选
    pub source: Option<String>,
    /// 按 hive 筛选（`system` / `user`）——
    pub hive: Option<String>,
    /// 按操作目标筛选——
    pub target: Option<String>,
    /// 按结果筛选（`success` / `partial` / `failure`）——
    pub result: Option<String>,
    /// 时间范围起点（含，Unix 毫秒）——
    pub from_ms: Option<i64>,
    /// 时间范围终点（含，Unix 毫秒）——
    pub to_ms: Option<i64>,
}

/// 把 IPC 入参归一化为 core 的事件筛选器；默认 50 条，上限 500。
pub fn from_request(req: Option<EventFilterRequest>) -> EventFilter {
    let req = req.unwrap_or(EventFilterRequest {
        limit: None,
        offset: None,
        action: None,
        source: None,
        hive: None,
        target: None,
        result: None,
        from_ms: None,
        to_ms: None,
    });
    EventFilter {
        limit: req.limit.unwrap_or(50).min(500),
        offset: req.offset.unwrap_or(0),
        action: req.action,
        source: req.source,
        hive: req.hive,
        target: req.target,
        result: req.result,
        from_ms: req.from_ms,
        to_ms: req.to_ms,
    }
}

/// 列出事件（分页、时间倒序）。
#[tauri::command]
pub fn history_list_events(
    filter: Option<EventFilterRequest>,
    state: State<'_, DbState>,
) -> Result<EventPage, CoreError> {
    with_db(&state, "history_list_events", |db| {
        history::list_events(db, &from_request(filter))
    })
}

/// 读取快照预览（敏感值按 `reveal` 掩码）。
#[tauri::command]
pub fn history_get_snapshot(
    uid: String,
    reveal: Option<bool>,
    state: State<'_, DbState>,
) -> Result<SnapshotView, CoreError> {
    with_db(&state, "history_get_snapshot", |db| {
        history::get_snapshot(db, &uid, reveal.unwrap_or(false))
    })
}

/// 读取当前快照；无当前快照时返回 `null`。
#[tauri::command]
pub fn history_get_current(state: State<'_, DbState>) -> Result<Option<SnapshotView>, CoreError> {
    with_db(&state, "history_get_current", history::current_snapshot)
}

/// 显式 reveal 一个历史快照里的环境变量值。
///
/// **独立命令**：与 `history_get_snapshot` 分离，让「掩码」成为默认，
/// 「看明文」成为一次可审计的显式动作。
///
/// 与 `history_get_snapshot(reveal=true)` 的区别：后者是**批量**揭示，
/// 前者是**单个变量**的显式请求。GUI 上对应的是每个被掩码变量旁的「显示」按钮。
///
/// 返回 `NotFound` 表示：快照/变量不存在，或该变量是 `Unsupported` 类型
/// （`restorable = 0`，库里没有值可读）。
#[tauri::command]
pub fn history_reveal_env_value(
    uid: String,
    hive: String,
    name: String,
    state: State<'_, DbState>,
) -> Result<String, CoreError> {
    with_db(&state, "history_reveal_env_value", |db| {
        history::reveal_env_value(db, &uid, &hive, &name)
    })
}

/// 把历史快照恢复回注册表。
///
/// **走既有注册表编排**（`service::save_path_with_db`），不新增写注册表的代码路径。
/// 双 hive 一次提交：快照里 system / user 都恢复。
///
/// **append-only**：恢复成功后追加**新快照 + 新事件**，旧快照原样保留——
/// 这样「恢复」本身也可被撤销。
///
/// 返回恢复后的结果；失败时注册表可能部分写入（best-effort 语义由 `ApplyOutcome` 表达）。
#[tauri::command]
pub fn history_restore_snapshot(
    uid: String,
    state: State<'_, DbState>,
) -> Result<path_editor_core::service::ApplyOutcome, CoreError> {
    // ① 读快照（掩码无关：恢复的是 PATH，不含 env 值）
    let view = with_db(&state, "history_restore_snapshot", |db| {
        history::get_snapshot(db, &uid, false)
    })?;

    // ② 交给 core 编排写注册表、更新当前快照并记录事件
    let outcome = with_db_mut(&state, "history_restore_snapshot", |db| {
        path_editor_core::service::save_path_with_db(
            db,
            path_editor_core::history::EventSource::Restore,
            Some(path_editor_core::service::PathHiveWrite {
                entries: view.system,
                expected: None,
            }),
            Some(path_editor_core::service::PathHiveWrite {
                entries: view.user,
                expected: None,
            }),
        )
    })?;

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_filter_defaults_are_bounded() {
        let f = from_request(None);
        assert_eq!(f.limit, 50, "默认 50 条");
        assert_eq!(f.offset, 0);

        let f = from_request(Some(EventFilterRequest {
            limit: Some(9999),
            offset: Some(0),
            action: None,
            source: None,
            ..Default::default()
        }));
        assert_eq!(f.limit, 500, "上限必须钳制到 500");

        // limit=0 交给 core 的 clamp(1,500) 处理，这里不越权改写
        let f = from_request(Some(EventFilterRequest {
            limit: Some(0),
            offset: Some(7),
            action: Some("save".into()),
            source: None,
            ..Default::default()
        }));
        assert_eq!(f.offset, 7);
        assert_eq!(f.action.as_deref(), Some("save"));
    }
}
