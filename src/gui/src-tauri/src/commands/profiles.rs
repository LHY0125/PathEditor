//! 配置方案的 Tauri 命令。
//!
//! 配置方案统一存 DB；这里只做参数转换与错误呈现。

use super::db_state::{with_db, with_db_mut, DbState};
use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
use path_editor_core::db::profiles::{self, ProfileView};
use path_editor_core::domain::history::EventSource;
use path_editor_core::app::service::{self, ApplyOutcome};
use path_editor_core::{CoreError, PathEntry};
use serde::Deserialize;
use tauri::State;

/// 列出配置方案。
#[tauri::command]
pub fn profile_list(state: State<'_, DbState>) -> Result<Vec<ProfileView>, CoreError> {
    with_db(&state, "profile_list", profiles::list_profiles)
}

/// 应用一个配置方案：读配置内容、写注册表与当前快照、记录应用次数。
///
/// 与 CLI 的 `profile apply` 共用 `core::app::service::apply_profile`：
/// 「读配置 → 保存 → 记账」只有一份实现。记账失败只告警，不影响保存结果。
#[tauri::command]
pub fn profile_apply(name: String, state: State<'_, DbState>) -> Result<ApplyOutcome, CoreError> {
    with_db_mut(&state, "profile_apply", |db| {
        service::apply_profile(db, &name, EventSource::Gui)
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
) -> Result<Option<profiles::ProfilePathsView>, CoreError> {
    with_db(&state, "profile_paths", |db| {
        profiles::get_profile_paths_view(db, &name)
    })
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
