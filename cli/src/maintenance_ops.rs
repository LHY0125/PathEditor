//! `maintenance` 子命令：保留策略清理、v5 原件归档、遗留明文备份清理。
//!
//! **打开数据库的方式是刻意的**：这里用 `open_default() + migrate_to_current()`，
//! 而不是 `open_and_migrate()`。后者会顺带执行 `migrate_v5::migrate_legacy`——
//! 维护命令（尤其是 `--dry-run`）不该以「顺便把 v5 文件导进库」作为副作用。

use crate::runtime::exit_core_error;
use path_editor_core as core;
use path_editor_core::db::maintenance::{prune, prune_plan};

/// 打开默认库并迁移 schema（不做 v5 文件导入）。
fn open_db() -> core::db::Db {
    let mut db = core::db::open_default().unwrap_or_else(|e| exit_core_error(&e));
    db.migrate_to_current()
        .unwrap_or_else(|e| exit_core_error(&e));
    db
}

/// `maintenance prune [--dry-run]`
///
/// 删除「已导出且超过 90 天」的变更与「已解决超过 30 天」的冲突。
/// 待导出队列（`exported_at IS NULL`）永远不动。
pub(crate) fn maintenance_prune(dry_run: bool) {
    let mut db = open_db();
    let now = chrono::Utc::now().timestamp_millis();
    let report = if dry_run {
        prune_plan(&db, now).unwrap_or_else(|e| exit_core_error(&e))
    } else {
        prune(&mut db, now).unwrap_or_else(|e| exit_core_error(&e))
    };
    println!(
        "{}changes {} 条，冲突 {} 条",
        if dry_run {
            "（dry-run）将删除 "
        } else {
            "已删除 "
        },
        report.changes_deleted,
        report.conflicts_deleted
    );
}
