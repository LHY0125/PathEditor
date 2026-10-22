//! 配置方案的 Tauri 命令。
//!
//! 配置方案统一存 DB；这里只做参数转换与错误呈现。

use super::db_state::{with_db, with_db_mut, DbState};
use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
use path_editor_core::db::profiles::{self, ProfileView};
use path_editor_core::history::EventSource;
use path_editor_core::{CoreError, PathEntry};
use serde::Deserialize;
use tauri::State;

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
/// 配置方案统一走 DB；快照与 profile 行在**同一事务**内写入
/// （`save_profile_tx`），不会出现「配置建了但快照没建」的中间态。
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
    pub system: Vec<PathEntry>,
    /// 用户 PATH 条目
    #[serde(default)]
    pub user: Vec<PathEntry>,
}

/// 保存配置方案（PATH 条目 + 元数据，单事务）。
#[tauri::command]
pub fn profile_save(
    request: ProfileSaveRequest,
    state: State<'_, DbState>,
) -> Result<String, CoreError> {
    with_db_mut(&state, "profile_save", |db| {
        let snap = NewSnapshot {
            kind: SnapshotKind::Profile,
            source: EventSource::Gui,
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
    pub system: Vec<PathEntry>,
    /// 用户 PATH
    pub user: Vec<PathEntry>,
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
