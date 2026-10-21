//! `maintenance` 子命令：保留策略清理、v5 原件归档、遗留明文备份清理。
//!
//! **打开数据库的方式是刻意的**：这里用 `open_default() + migrate_to_current()`，
//! 而不是 `open_and_migrate()`。后者会顺带执行 `migrate_v5::migrate_legacy`——
//! 维护命令（尤其是 `--dry-run`）不该以「顺便把 v5 文件导进库」作为副作用。

use crate::runtime::exit_core_error;
use path_editor_core as core;
use path_editor_core::db::archive_v5::archive_v5_sources;
use path_editor_core::db::maintenance::{
    cleanup_legacy_env_backups, prune, prune_plan, scan_legacy_env_backups,
};

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

/// `maintenance archive-v5 [--dry-run]`
///
/// 只处理 `disabled.json` 与 `profiles/*.json`；目标已存在则跳过，**绝不覆盖**。
/// dry-run 是纯读：不改名、不写任何文件。
pub(crate) fn maintenance_archive_v5(dry_run: bool) {
    let home = core::db::crypto::config_dir();
    let report = archive_v5_sources(&home, dry_run).unwrap_or_else(|e| exit_core_error(&e));
    for path in &report.archived {
        // `archived` 存的是**目标路径**（`<原名>.v5.bak`）。写「归档到」而不是
        // 「归档 <目标>」，否则读起来像在归档一个 .v5.bak 文件。
        println!(
            "{} {}",
            if dry_run {
                "将归档到"
            } else {
                "已归档到"
            },
            path.display()
        );
    }
    for (path, why) in &report.skipped {
        eprintln!("跳过 {}：{why}", path.display());
    }
    println!(
        "{}共 {} 个，跳过 {} 个",
        if dry_run { "（dry-run）" } else { "" },
        report.archived.len(),
        report.skipped.len()
    );
}

/// `maintenance cleanup-legacy-backups [--dry-run] [--yes]`
///
/// 把遗留 `env_backup_*.json` 转成 `.pebak` 后删除明文。**必须先 `--yes`**：
/// 这是唯一会删除用户文件的维护命令。
///
/// dry-run 走 [`scan_legacy_env_backups`] 而**不加载密钥**——`load_or_create_key`
/// 在密钥不存在时会新建 `~/.patheditor/keys/db.key.dpapi`，预览不该有磁盘副作用。
pub(crate) fn maintenance_cleanup_legacy(dry_run: bool, yes: bool) {
    let dir = core::backup::env_backup_dir();

    if !dry_run && !yes {
        eprintln!("错误: 这会删除明文备份。先运行 --dry-run 查看计划，确认后加 --yes。");
        std::process::exit(1);
    }

    let report = if dry_run {
        scan_legacy_env_backups(&dir).unwrap_or_else(|e| exit_core_error(&e))
    } else {
        let home = core::db::crypto::config_dir();
        let material =
            core::db::crypto::load_or_create_key(&home).unwrap_or_else(|e| exit_core_error(&e));
        cleanup_legacy_env_backups(&dir, &material, false).unwrap_or_else(|e| exit_core_error(&e))
    };

    for (json, pebak) in &report.converted {
        if dry_run {
            println!("将转换 {}", json.display());
        } else {
            println!("已转换 {} -> {}", json.display(), pebak.display());
        }
    }
    for (path, why) in &report.skipped {
        eprintln!("跳过 {}：{why}", path.display());
    }
    println!(
        "{}共 {} 个，跳过 {} 个",
        if dry_run { "（dry-run）" } else { "" },
        report.converted.len(),
        report.skipped.len()
    );
}
