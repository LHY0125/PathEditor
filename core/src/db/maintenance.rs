//! 维护任务：保留策略清理（spec §4.2 第 10 条、§6.5.2）。
//!
//! 这里只有**保留策略**，没有用户主动触发的删除：
//!
//! - `changes` 中「已导出且超过 90 天」的变更会被删除；
//! - `sync_conflicts` 中「已解决且超过 30 天」的记录会被删除。
//!
//! **待导出队列（`exported_at IS NULL`）永远不删**——那里面是会永久丢失的变更。

use super::{db_err, Db};
use crate::error::CoreError;

/// 已导出 `changes` 的保留期：90 天。
pub const CHANGES_RETENTION_MS: i64 = 90 * 24 * 60 * 60 * 1000;
/// 已解决 `sync_conflicts` 的保留期：30 天。
pub const CONFLICTS_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// 清理结果。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PruneReport {
    /// 删除的 changes 行数
    pub changes_deleted: u32,
    /// 删除的 sync_conflicts 行数
    pub conflicts_deleted: u32,
}

/// `changes` 的删除谓词。
///
/// `applied_at > 0` 排除「从未落地」的行；`exported_at IS NOT NULL` 排除待导出队列。
/// 任务本身产生的本地变更在 `record_change_tx` 里就把 `applied_at` 写成当时时间，
/// 因此本地导出过的变更同样会到期清理。
const CHANGES_PREDICATE: &str = "exported_at IS NOT NULL AND applied_at > 0 AND applied_at < ?1";

/// `sync_conflicts` 的删除谓词：只删**已解决**且到期的。
const CONFLICTS_PREDICATE: &str = "resolved_at IS NOT NULL AND resolved_at < ?1";

/// 执行清理，返回删除行数。
///
/// **绝不删除 `exported_at IS NULL`**：那是待导出队列，删了就永久丢变更。
pub fn prune(db: &mut Db, now_ms: i64) -> Result<PruneReport, CoreError> {
    db.with_transaction(|tx| {
        let changes_deleted = tx
            .execute(
                &format!("DELETE FROM changes WHERE {CHANGES_PREDICATE}"),
                rusqlite::params![now_ms - CHANGES_RETENTION_MS],
            )
            .map_err(|e| db_err("prune_changes", e))?;
        let conflicts_deleted = tx
            .execute(
                &format!("DELETE FROM sync_conflicts WHERE {CONFLICTS_PREDICATE}"),
                rusqlite::params![now_ms - CONFLICTS_RETENTION_MS],
            )
            .map_err(|e| db_err("prune_conflicts", e))?;
        Ok(PruneReport {
            changes_deleted: changes_deleted as u32,
            conflicts_deleted: conflicts_deleted as u32,
        })
    })
}

/// dry-run：只统计将被删除的行数，**不写库**。
pub fn prune_plan(db: &Db, now_ms: i64) -> Result<PruneReport, CoreError> {
    let changes_deleted: i64 = db.query_row_with(
        &format!("SELECT COUNT(*) FROM changes WHERE {CHANGES_PREDICATE}"),
        rusqlite::params![now_ms - CHANGES_RETENTION_MS],
        |r| r.get(0),
    )?;
    let conflicts_deleted: i64 = db.query_row_with(
        &format!("SELECT COUNT(*) FROM sync_conflicts WHERE {CONFLICTS_PREDICATE}"),
        rusqlite::params![now_ms - CONFLICTS_RETENTION_MS],
        |r| r.get(0),
    )?;
    Ok(PruneReport {
        changes_deleted: changes_deleted as u32,
        conflicts_deleted: conflicts_deleted as u32,
    })
}

/// 以当前时间执行清理（GUI 启动序列用）。
///
/// 时间源收在 core：GUI crate 没有直接依赖 `chrono`，也不该为一次清理新增依赖。
pub fn prune_now(db: &mut Db) -> Result<PruneReport, CoreError> {
    prune(db, chrono::Utc::now().timestamp_millis())
}
