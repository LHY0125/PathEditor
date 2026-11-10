//! `maintenance` 子命令：保留策略清理与遗留明文备份清理。
//!
//! 维护命令只打开库并校验 schema，不触发额外的文件导入。

use crate::runtime::{exit_core_error, open_db_or_exit};
use path_editor_core as core;
use path_editor_core::db::health;
use path_editor_core::db::maintenance::{
    cleanup_legacy_env_backups, prune, prune_plan, scan_legacy_env_backups, vacuum,
};

/// `maintenance prune [--dry-run]`
///
/// 删除「已导出且超过 90 天」的变更、「已解决超过 30 天」的冲突、
/// 到期/超限的未保护快照与到期事件。待导出队列（`exported_at IS NULL`）永远不动。
pub(crate) fn maintenance_prune(dry_run: bool) {
    let mut db = open_db_or_exit();
    let now = chrono::Utc::now().timestamp_millis();
    let report = if dry_run {
        prune_plan(&db, now).unwrap_or_else(|e| exit_core_error(&e))
    } else {
        prune(&mut db, now).unwrap_or_else(|e| exit_core_error(&e))
    };
    println!(
        "{}{} 条，冲突 {} 条，事件 {} 条，快照 {} 条",
        if dry_run {
            "（dry-run）将删除 "
        } else {
            "已删除 "
        },
        report.changes_deleted,
        report.conflicts_deleted,
        report.events_deleted,
        report.snapshots_deleted
    );
}

/// `maintenance vacuum`：WAL 截断 + `VACUUM`。
///
/// 先按当前保留策略清理一轮再压缩，避免刚删掉的行还占着空闲页。
pub(crate) fn maintenance_vacuum() {
    let mut db = open_db_or_exit();
    let report = prune(&mut db, chrono::Utc::now().timestamp_millis())
        .unwrap_or_else(|e| exit_core_error(&e));
    vacuum(&db).unwrap_or_else(|e| exit_core_error(&e));
    let _ = path_editor_core::db::maintenance::record_prune_time_default(
        chrono::Utc::now().timestamp_millis(),
    );
    println!(
        "已清理并压缩：变更 {}，冲突 {}，事件 {}，快照 {}",
        report.changes_deleted,
        report.conflicts_deleted,
        report.events_deleted,
        report.snapshots_deleted
    );
}

/// `maintenance health [--json]`：只读自检。
pub(crate) fn maintenance_health(json_out: bool) {
    let db = open_db_or_exit();
    let report = health::health(&db).unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        crate::output::print_json(&report);
        return;
    }
    let mib = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
    println!("数据库：{}", report.path);
    println!(
        "体积：主文件 {:.2} MiB，WAL {:.2} MiB，SHM {:.2} MiB",
        mib(report.file_bytes),
        mib(report.wal_bytes),
        mib(report.shm_bytes)
    );
    println!(
        "schema：v{}（程序支持 v{}）",
        report.schema_version, report.expected_schema_version
    );
    println!(
        "完整性：{}",
        if report.integrity_ok {
            "ok".to_string()
        } else {
            report.integrity.clone()
        }
    );
    for row in &report.table_rows {
        println!("{:>16}: {}", row.name, row.rows);
    }
    match report.last_prune_at {
        Some(at) => println!("最近清理：{}", at),
        None => println!("最近清理：无记录"),
    }
}

/// `maintenance cleanup-legacy-backups [--dry-run] [--yes]`
///
/// 把遗留 `env_backup_*.json` 转成 `.pebak` 后删除明文。**必须先 `--yes`**：
/// 这是唯一会删除用户文件的维护命令。
///
/// dry-run 走 [`scan_legacy_env_backups`] 而**不加载密钥**——`load_or_create_key`
/// 在密钥不存在时会新建 `~/.patheditor/keys/db.key.dpapi`（本机盐），预览不该有磁盘副作用。
pub(crate) fn maintenance_cleanup_legacy(dry_run: bool, yes: bool) {
    let dir = core::platform::backup::env_backup_dir();

    if !dry_run && !yes {
        // 文档承诺「未给 --yes 时只打印计划并以 1 退出」：这里真的打印计划，
        // 而不是只丢一句错误让用户再跑一次 --dry-run。
        let plan = scan_legacy_env_backups(&dir).unwrap_or_else(|e| exit_core_error(&e));
        println!(
            "═══ 将转换后删除的明文备份（{} 个）═══",
            plan.converted.len()
        );
        for (json, _) in &plan.converted {
            println!("  {}", json.display());
        }
        for (path, why) in &plan.skipped {
            println!("  跳过 {}: {why}", path.display());
        }
        eprintln!("错误: 这会删除明文备份。确认后加 --yes 重新执行。");
        std::process::exit(1);
    }

    let report = if dry_run {
        scan_legacy_env_backups(&dir).unwrap_or_else(|e| exit_core_error(&e))
    } else {
        let material = core::db::crypto::load_default_key().unwrap_or_else(|e| exit_core_error(&e));
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

/// `maintenance reconcile-backups [--json] [--purge-missing]`
///
/// 只对账与标记，**不删除磁盘文件**：`--purge-missing` 仅把「文件已丢失」
/// 的索引行标记 `deleted_at`。
pub(crate) fn maintenance_reconcile_backups(json_out: bool, purge_missing: bool) {
    let mut db = open_db_or_exit();
    let dir = core::platform::backup::env_backup_dir();
    if purge_missing {
        let n = core::db::backup_index::purge_missing(&mut db, &dir)
            .unwrap_or_else(|e| exit_core_error(&e));
        if !json_out {
            println!("已标记 {n} 条缺失索引");
        }
    }
    let report = core::db::backup_index::reconcile_backups(&db, &dir)
        .unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        crate::output::print_json(&report);
        return;
    }
    println!(
        "索引缺失 {} 条，目录中未入索引 {} 个",
        report.missing.len(),
        report.orphans.len()
    );
    for row in &report.missing {
        println!("  缺失: {}", row.file_path);
    }
    for path in &report.orphans {
        println!("  未入索引: {path}");
    }
}

/// `maintenance export --events|--changes [--format csv|json] [--out <file>]`
///
/// 导出审计数据。`--events` 支持 `--since/--until`（YYYY-MM-DD，按 UTC 日界）。
pub(crate) fn maintenance_export(
    events: bool,
    changes: bool,
    format: &str,
    out: Option<String>,
    since: Option<String>,
    until: Option<String>,
) {
    if events == changes {
        crate::runtime::exit_err("必须且只能指定 --events 或 --changes");
    }
    let db = open_db_or_exit();
    let content = if events {
        let mut filter = core::db::history::EventFilter::default();
        if let Some(raw) = since {
            filter.from_ms = Some(day_start_ms(&raw));
        }
        if let Some(raw) = until {
            filter.to_ms = Some(day_end_ms(&raw));
        }
        core::db::audit_export::export_events(&db, &filter, format)
            .unwrap_or_else(|e| exit_core_error(&e))
    } else {
        core::db::audit_export::export_changes(&db, false, format)
            .unwrap_or_else(|e| exit_core_error(&e))
    };

    match out {
        Some(path) => {
            core::platform::fs::write_export_file(&path, &content)
                .unwrap_or_else(|e| exit_core_error(&e));
            println!("已导出到 {path}");
        }
        None => print!("{content}"),
    }
}

/// 解析 `YYYY-MM-DD` 为当天 00:00:00.000 UTC 的毫秒时间戳。
fn day_start_ms(raw: &str) -> i64 {
    let date = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .unwrap_or_else(|_| crate::runtime::exit_err(&format!("日期必须是 YYYY-MM-DD: {raw}")));
    date.and_hms_opt(0, 0, 0)
        .expect("00:00:00 恒合法")
        .and_utc()
        .timestamp_millis()
}

/// 解析 `YYYY-MM-DD` 为当天 23:59:59.999 UTC 的毫秒时间戳。
fn day_end_ms(raw: &str) -> i64 {
    let date = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .unwrap_or_else(|_| crate::runtime::exit_err(&format!("日期必须是 YYYY-MM-DD: {raw}")));
    date.and_hms_milli_opt(23, 59, 59, 999)
        .expect("23:59:59.999 恒合法")
        .and_utc()
        .timestamp_millis()
}
