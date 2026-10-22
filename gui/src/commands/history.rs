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
    /// 按 hive 筛选（`system` / `user`）——P1-1
    pub hive: Option<String>,
    /// 按操作目标筛选——P1-1
    pub target: Option<String>,
    /// 按结果筛选（`success` / `partial` / `failure`）——P1-1
    pub result: Option<String>,
    /// 时间范围起点（含，Unix 毫秒）——P1-1
    pub from_ms: Option<i64>,
    /// 时间范围终点（含，Unix 毫秒）——P1-1
    pub to_ms: Option<i64>,
}

/// 见 spec §3.2.1：默认 50，上限 500。
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

/// 从共享状态取 DB；降级模式返回结构化错误。
///
/// `pub(crate)` 供 `commands::analytics` 复用——**不要复制第二份**：
/// 锁中毒与降级（`DbState::None`）的判定必须在一处，否则两个模块会对
/// 「什么算降级」产生分歧。
pub(crate) fn with_db<T>(
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

/// 从共享状态取可变 DB（写入路径）。`pub(crate)` 供 `commands::analytics` 复用。
pub(crate) fn with_db_mut<T>(
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

/// 显式 reveal 一个历史快照里的环境变量值（M1 欠账 P2-1）。
///
/// **独立命令**：与 `history_get_snapshot` 分离，让「掩码」成为默认，
/// 「看明文」成为一次可审计的显式动作（spec §3.2.1）。
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
/// **不在此处写注册表**：前端先读取 `profile_paths`，再经 `save_path_with_sidecar`
/// 写入注册表；成功后才调用本命令记账，避免把「记账」和「真写」混在一个命令里。
#[tauri::command]
pub fn profile_mark_applied(uid: String, state: State<'_, DbState>) -> Result<bool, CoreError> {
    with_db_mut(&state, "profile_mark_applied", |db| {
        profiles::mark_profile_applied(db, &uid)
    })
}

/// 配置方案的保存请求（PATH 条目 + 元数据一次写入）。
///
/// **P0-1 方案 A**：UI 从 v5 文件通路切到 DB 后，这是唯一的写入口。
/// 快照与 profile 行在**同一事务**内写入（`save_profile_tx`），
/// 不会出现「配置建了但快照没建」的中间态。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSaveRequest {
    /// 配置名称（未删除的同名配置已存在时返回 `NameExists`）
    pub name: String,
    /// 描述
    pub description: Option<String>,
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 系统 PATH 条目
    #[serde(default)]
    pub system: Vec<path_editor_core::PathEntry>,
    /// 用户 PATH 条目
    #[serde(default)]
    pub user: Vec<path_editor_core::PathEntry>,
}

/// 保存配置方案（PATH 条目 + 元数据，单事务）。
#[tauri::command]
pub fn profile_save(
    request: ProfileSaveRequest,
    state: State<'_, DbState>,
) -> Result<String, CoreError> {
    with_db_mut(&state, "profile_save", |db| {
        let snap = history::NewSnapshot {
            kind: history::SnapshotKind::Profile,
            source: path_editor_core::history::EventSource::Gui,
            system: request.system,
            user: request.user,
            env: Vec::new(),
            parent_uid: None,
        };
        profiles::save_profile(
            db,
            &request.name,
            request.description.as_deref(),
            &request.tags,
            snap,
        )
    })
}

/// 把配置导出成 v5 `ProfileData` 的 JSON 字符串（M1 欠账 P2-2）。
///
/// 这是**降级到 v5 的退路**：DB 不可用或需要回到旧版本时，用它可以导出
/// 与 v5 自身写出的格式完全同构的文件（含 `schemaVersion` 信封）。
///
/// 返回的字符串由前端负责落盘（`save` 对话框），Rust 侧不写用户文件。
#[tauri::command]
pub fn profile_export_v5(uid: String, state: State<'_, DbState>) -> Result<String, CoreError> {
    with_db(&state, "profile_export_v5", |db| {
        path_editor_core::db::v5_export::export_profile_v5(db, &uid)
    })
}

/// 读取配置的 PATH 条目（UI 预览与应用用）。
///
/// 返回 `null` 表示配置不存在或没有 revision。
#[tauri::command]
pub fn profile_paths(
    name: String,
    state: State<'_, DbState>,
) -> Result<Option<ProfilePathsView>, CoreError> {
    with_db(&state, "profile_paths", |db| {
        // 名称查一次即可拿到 updated_at 与 PATH 条目
        let view = profiles::get_profile_by_name(db, &name)?;
        let Some(view) = view else {
            return Ok(None);
        };
        let Some((system, user)) = profiles::profile_paths(db, &name)? else {
            return Ok(None);
        };
        Ok(Some(ProfilePathsView {
            name: view.name,
            updated_at: view.updated_at,
            system,
            user,
        }))
    })
}

/// 配置的 PATH 条目视图（形状对齐前端既有的 `ProfileData`）。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePathsView {
    /// 配置名称
    pub name: String,
    /// 最后更新时间（Unix 毫秒）；UI 展示用
    pub updated_at: i64,
    /// 系统 PATH
    pub system: Vec<path_editor_core::PathEntry>,
    /// 用户 PATH
    pub user: Vec<path_editor_core::PathEntry>,
}

/// 重命名配置（UI 用名称标识）。
#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    state: State<'_, DbState>,
) -> Result<bool, CoreError> {
    with_db_mut(&state, "profile_rename", |db| {
        profiles::rename_profile(db, &old_name, &new_name)
    })
}

/// 按**名称**删除配置（软删除）。
///
/// 与 `profile_delete`（按 uid）并存：UI 用名称，测试与未来同步用 uid。
#[tauri::command]
pub fn profile_delete_by_name(name: String, state: State<'_, DbState>) -> Result<bool, CoreError> {
    with_db_mut(&state, "profile_delete_by_name", |db| {
        profiles::delete_profile_by_name(db, &name)
    })
}

/// 按**名称**记录配置被应用。
#[tauri::command]
pub fn profile_mark_applied_by_name(
    name: String,
    state: State<'_, DbState>,
) -> Result<bool, CoreError> {
    with_db_mut(&state, "profile_mark_applied_by_name", |db| {
        profiles::mark_profile_applied_by_name(db, &name)
    })
}

/// 把历史快照恢复回注册表（P0-3）。
///
/// **走既有注册表编排**（`service::save_path_with_sidecar`），不新增写注册表的代码路径。
/// 双 hive 一次提交：快照里 system / user 都恢复。
///
/// **append-only**：恢复成功后追加**新快照 + 新事件**，旧快照原样保留——
/// 这样「恢复」本身也可被撤销（spec §6.1）。
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

///
/// 列出备份索引。
#[tauri::command]
pub fn backup_list(state: State<'_, DbState>) -> Result<Vec<BackupView>, CoreError> {
    // P1-2：每次调用前先增量索引。只在启动时索引会漏掉「启动后新做的备份」——
    // 用户启动 → `env backup` → 打开备份列表，就会看到空列表。
    // `index_backups` 按 `file_path` 去重，重复调用幂等，成本只是目录 stat。
    with_db_mut(&state, "backup_list", |db| {
        let dir = path_editor_core::backup::env_backup_dir();
        // 索引失败不阻断列表：已有索引行仍然有价值
        if let Err(e) = backups::index_backups(db, &dir) {
            log::warn!("备份索引刷新失败（展示已有索引）: {}", e.message);
        }
        backups::list_backups(db)
    })
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
