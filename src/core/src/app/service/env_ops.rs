//! 通用环境变量写入的历史编排。
//!
//! 注册表写入本身在 `platform::registry::env_edit`；本模块只负责：
//! 1. 写前采集完整状态（PATH + env）并生成 `History` 快照；
//! 2. 写后生成「操作后」快照，供前端做会话内撤销/重做；
//! 3. 写一条带 `EnvSet` / `EnvAdd` / `EnvRemove` 的审计事件。
//!
//! 所有历史副作用都是 **best-effort**：快照或事件失败不得让已经成功的
//! 注册表写入变成失败。

use super::*;
use crate::db::history::{EnvValueRow, NewSnapshot, SnapshotKind};
use crate::domain::env_var::EnvValueKind;
use crate::domain::history::{EventAction, EventSource, HistorySink};
use crate::platform::registry::{self, WriteOutcome};

/// 一次环境变量写入的结果：注册表结果 + 前后快照 uid。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvMutationOutcome {
    /// 注册表写入结果（含写前备份）
    pub write: WriteOutcome,
    /// 操作前快照（撤销目标）
    pub before_snapshot_uid: Option<String>,
    /// 操作后快照（重做目标）
    pub after_snapshot_uid: Option<String>,
}

fn capture_state(db: &Db) -> Result<(PathSnapshot, Vec<EnvValueRow>), CoreError> {
    Ok((load_path_snapshot_db(db)?, collect_env_snapshot_rows()?))
}

fn insert_state_snapshot(
    db: &mut Db,
    source: EventSource,
    path: &PathSnapshot,
    env: Vec<EnvValueRow>,
) -> Option<String> {
    match history::insert_snapshot(
        db,
        NewSnapshot {
            kind: SnapshotKind::History,
            source,
            system: path.system.clone(),
            user: path.user.clone(),
            env,
            parent_uid: None,
        },
    ) {
        Ok(uid) => Some(uid),
        Err(e) => {
            log::warn!("环境变量操作快照写入失败（写入已成功）: {}", e.message);
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn record_event(
    db: &mut Db,
    source: EventSource,
    action: EventAction,
    hive: EnvHive,
    name: &str,
    result: EventResult,
    before_revision: Option<String>,
    snapshot_uid: Option<String>,
) {
    let event = crate::domain::history::HistoryEvent {
        source,
        action,
        hive: Some(hive),
        target: Some(name.to_string()),
        changed_fields: vec!["value".to_string()],
        before_revision,
        after_revision: None,
        result,
        snapshot_uid,
    };
    if let Err(e) = DbHistory::new(db).record(event) {
        log::warn!("环境变量历史事件写入失败（写入已成功）: {}", e.message);
    }
}

/// 写入已有环境变量，并留下「操作前/后」快照与审计事件。
pub fn update_env_with_db(
    db: &mut Db,
    source: EventSource,
    hive: EnvHive,
    name: &str,
    value: &str,
    expected_revision: &str,
) -> Result<EnvMutationOutcome, CoreError> {
    let before = capture_state(db);
    let result = registry::update_env_var(hive, name, value, expected_revision);
    finish_mutation(
        db,
        source,
        EventAction::EnvSet,
        hive,
        name,
        Some(expected_revision.to_string()),
        before,
        result,
    )
}

/// 新建环境变量，并留下「操作前/后」快照与审计事件。
pub fn create_env_with_db(
    db: &mut Db,
    source: EventSource,
    hive: EnvHive,
    name: &str,
    value: &str,
    kind: EnvValueKind,
) -> Result<EnvMutationOutcome, CoreError> {
    let before = capture_state(db);
    let result = registry::create_env_var(hive, name, value, kind);
    finish_mutation(
        db,
        source,
        EventAction::EnvAdd,
        hive,
        name,
        None,
        before,
        result,
    )
}

/// 删除环境变量，并留下「操作前/后」快照与审计事件。
pub fn delete_env_with_db(
    db: &mut Db,
    source: EventSource,
    hive: EnvHive,
    name: &str,
    expected_revision: &str,
) -> Result<EnvMutationOutcome, CoreError> {
    let before = capture_state(db);
    let result = registry::delete_env_var(hive, name, expected_revision);
    finish_mutation(
        db,
        source,
        EventAction::EnvRemove,
        hive,
        name,
        Some(expected_revision.to_string()),
        before,
        result,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_mutation(
    db: &mut Db,
    source: EventSource,
    action: EventAction,
    hive: EnvHive,
    name: &str,
    before_revision: Option<String>,
    before_state: Result<(PathSnapshot, Vec<EnvValueRow>), CoreError>,
    result: Result<WriteOutcome, CoreError>,
) -> Result<EnvMutationOutcome, CoreError> {
    match result {
        Ok(write) => {
            let (before_uid, after_uid) = match before_state {
                Ok((path, env)) => {
                    let before_uid = insert_state_snapshot(db, source, &path, env);
                    // 写后快照：PATH 未变，只需重新采集 env。
                    let after_uid = match collect_env_snapshot_rows() {
                        Ok(env) => insert_state_snapshot(db, source, &path, env),
                        Err(e) => {
                            log::warn!("采集操作后环境变量失败（重做不可用）: {}", e.message);
                            None
                        }
                    };
                    (before_uid, after_uid)
                }
                Err(e) => {
                    log::warn!("采集操作前状态失败（本次无历史快照）: {}", e.message);
                    (None, None)
                }
            };
            record_event(
                db,
                source,
                action,
                hive,
                name,
                EventResult::Success,
                before_revision,
                before_uid.clone(),
            );
            Ok(EnvMutationOutcome {
                write,
                before_snapshot_uid: before_uid,
                after_snapshot_uid: after_uid,
            })
        }
        Err(e) => {
            // 失败也留审计（无快照可关联）
            record_event(
                db,
                source,
                action,
                hive,
                name,
                EventResult::Failure,
                before_revision,
                None,
            );
            Err(e)
        }
    }
}

/// 强制覆盖已有环境变量（不校验 revision），保留历史与审计。
pub fn update_env_force_with_db(
    db: &mut Db,
    source: EventSource,
    hive: EnvHive,
    name: &str,
    value: &str,
) -> Result<EnvMutationOutcome, CoreError> {
    let before = capture_state(db);
    let result = registry::update_env_var_force(hive, name, value);
    finish_mutation(
        db,
        source,
        EventAction::EnvSet,
        hive,
        name,
        None,
        before,
        result,
    )
}

/// 强制删除环境变量（不校验 revision），保留历史与审计。
pub fn delete_env_force_with_db(
    db: &mut Db,
    source: EventSource,
    hive: EnvHive,
    name: &str,
) -> Result<EnvMutationOutcome, CoreError> {
    let before = capture_state(db);
    let result = registry::delete_env_var_force(hive, name);
    finish_mutation(
        db,
        source,
        EventAction::EnvRemove,
        hive,
        name,
        None,
        before,
        result,
    )
}
