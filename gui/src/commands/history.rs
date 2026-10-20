//! 历史 / 配置 / 备份的 Tauri 命令。
//!
//! **只做参数转换与错误呈现**，业务规则全在 core（spec §3.2 所有权边界）。
//!
//! 降级语义（spec B7 / D16 fail-open）：DB 不可用时 `DbState` 为 `None`，
//! 所有 `history_*` / `profile_*` / `backup_*` 命令返回 `CoreError { code: database }`；
//! **注册表编辑命令不受影响**（它们在 `commands::service` / `registry` 里）。

use path_editor_core::db::backups::{self, BackupView};
use path_editor_core::db::history::{self, EventFilter, EventPage, SnapshotView};
use path_editor_core::db::profiles::{self, ProfileView};
use path_editor_core::db::Db;
use path_editor_core::{CoreError, ErrorCode};
use serde::Deserialize;
use std::sync::Mutex;
use tauri::State;

/// 全局 DB 句柄。`None` = 降级模式（打开失败）。
pub struct DbState(pub Mutex<Option<Db>>);

/// 事件筛选的 IPC 入参；所有字段可选（spec §3.2.1：单个结构体参数）。
#[derive(Debug, Deserialize)]
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
}

/// 见 spec §3.2.1：默认 50，上限 500。
pub fn from_request(req: Option<EventFilterRequest>) -> EventFilter {
    let req = req.unwrap_or(EventFilterRequest {
        limit: None,
        offset: None,
        action: None,
        source: None,
    });
    EventFilter {
        limit: req.limit.unwrap_or(50).min(500),
        offset: req.offset.unwrap_or(0),
        action: req.action,
        source: req.source,
    }
}

/// 从共享状态取 DB；降级模式返回结构化错误。
fn with_db<T>(
    state: &State<'_, DbState>,
    op: &'static str,
    f: impl FnOnce(&Db) -> Result<T, CoreError>,
) -> Result<T, CoreError> {
    let guard = state
        .0
        .lock()
        .map_err(|_| CoreError::new(ErrorCode::Internal, op, "数据库状态锁中毒"))?;
    let db = guard
        .as_ref()
        .ok_or_else(|| CoreError::new(ErrorCode::Database, op, "历史库不可用，当前处于降级模式"))?;
    f(db)
}

/// 从共享状态取可变 DB（写入路径）。
fn with_db_mut<T>(
    state: &State<'_, DbState>,
    op: &'static str,
    f: impl FnOnce(&mut Db) -> Result<T, CoreError>,
) -> Result<T, CoreError> {
    let mut guard = state
        .0
        .lock()
        .map_err(|_| CoreError::new(ErrorCode::Internal, op, "数据库状态锁中毒"))?;
    let db = guard
        .as_mut()
        .ok_or_else(|| CoreError::new(ErrorCode::Database, op, "历史库不可用，当前处于降级模式"))?;
    f(db)
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

/// 列出配置方案。
#[tauri::command]
pub fn profile_list(state: State<'_, DbState>) -> Result<Vec<ProfileView>, CoreError> {
    with_db(&state, "profile_list", profiles::list_profiles)
}

/// 软删除配置方案。
#[tauri::command]
pub fn profile_delete(uid: String, state: State<'_, DbState>) -> Result<bool, CoreError> {
    with_db_mut(&state, "profile_delete", |db| {
        profiles::delete_profile(db, &uid)
    })
}

/// 记录配置被应用（应用次数 + 最后应用时间）。
///
/// **不在此处写注册表**：注册表写入走 `commands::service::apply_profile`，
/// 成功后才调用本命令记账（避免把「记账」和「真写」混在一个命令里）。
#[tauri::command]
pub fn profile_mark_applied(uid: String, state: State<'_, DbState>) -> Result<bool, CoreError> {
    with_db_mut(&state, "profile_mark_applied", |db| {
        profiles::mark_profile_applied(db, &uid)
    })
}

/// 注意：`profile_save` 需要把 PATH 条目 + 元数据一起传，形状较重，
/// M1 暂由 v5 文件通路 + 迁移承接（见回执「未覆盖项」），此处不注册。
///
/// 列出备份索引。
#[tauri::command]
pub fn backup_list(state: State<'_, DbState>) -> Result<Vec<BackupView>, CoreError> {
    with_db(&state, "backup_list", backups::list_backups)
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
        }));
        assert_eq!(f.limit, 500, "上限必须钳制到 500");

        // limit=0 交给 core 的 clamp(1,500) 处理，这里不越权改写
        let f = from_request(Some(EventFilterRequest {
            limit: Some(0),
            offset: Some(7),
            action: Some("save".into()),
            source: None,
        }));
        assert_eq!(f.offset, 7);
        assert_eq!(f.action.as_deref(), Some("save"));
    }
}
