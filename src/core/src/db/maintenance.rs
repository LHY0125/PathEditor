//! 维护任务。
//!
//! 两类职责：
//!
//! 1. **保留策略**：`changes` 中「已导出且超过 90 天」的变更、`sync_conflicts` 中
//!    「已解决且超过 30 天」的记录会被删除。待导出队列（`exported_at IS NULL`）
//!    永远不删——那里面是会永久丢失的变更。
//! 2. **遗留明文备份清理**：把 `env_backup_*.json` 转成 DPAPI 加密的
//!    `.pebak`，**回读校验通过后**才删除明文。

use std::path::{Path, PathBuf};

use super::{db_err, Db};
use crate::db::crypto::KeyMaterial;
use crate::domain::error::{CoreError, ErrorCode};
use crate::platform::backup::config::{config_file_path, read_backup_keep};
use crate::platform::backup::EnvBackupPayload;

/// 已导出 `changes` 的保留期：90 天。
pub const CHANGES_RETENTION_MS: i64 = 90 * 24 * 60 * 60 * 1000;
/// 已解决 `sync_conflicts` 的保留期：30 天。
pub const CONFLICTS_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// 快照默认保留天数（`config.ini: snapshot_keep_days`）。
///
/// 取 365 天而不是更短：快照是「恢复」的唯一来源，删掉即不可再生；
/// 需要更短保留期的用户显式配置即可。
pub const SNAPSHOT_KEEP_DAYS_DEFAULT: usize = 365;
/// 快照默认保留条数上限（`config.ini: snapshot_keep_max`）。
pub const SNAPSHOT_KEEP_MAX_DEFAULT: usize = 500;
/// 事件默认保留天数（`config.ini: event_keep_days`）。
pub const EVENT_KEEP_DAYS_DEFAULT: usize = 730;

/// 读取快照保留天数（非法值回落默认，与备份保留份数同口径）。
pub fn snapshot_keep_days() -> usize {
    read_backup_keep(
        &config_file_path(),
        "snapshot_keep_days",
        SNAPSHOT_KEEP_DAYS_DEFAULT,
    )
}

/// 读取快照保留条数上限。
pub fn snapshot_keep_max() -> usize {
    read_backup_keep(
        &config_file_path(),
        "snapshot_keep_max",
        SNAPSHOT_KEEP_MAX_DEFAULT,
    )
}

/// 读取事件保留天数。
pub fn event_keep_days() -> usize {
    read_backup_keep(
        &config_file_path(),
        "event_keep_days",
        EVENT_KEEP_DAYS_DEFAULT,
    )
}

/// 清理结果。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PruneReport {
    /// 删除的 changes 行数
    pub changes_deleted: u32,
    /// 删除的 sync_conflicts 行数
    pub conflicts_deleted: u32,
    /// 删除的 events 行数（到期事件）
    pub events_deleted: u32,
    /// 删除的 snapshots 行数（到期或超出条数上限的未保护快照）
    pub snapshots_deleted: u32,
}

/// 快照删除的**保护谓词**：命中任一条件的快照永不删。
///
/// - `pinned = 1`：用户显式固定；
/// - 当前快照：`app_state.current_snapshot_id` 指向它，删了基线就断了；
/// - 被 `profile_revisions` 引用：删快照会让版本记录悬空（外键会直接拦下）；
/// - 被 `backups` 引用：备份索引仍指向它，删了就无法从备份恢复。
///
/// 已在回收站的快照不重复统计。
const SNAPSHOT_PROTECTED: &str = "s.deleted_at IS NULL
   AND s.pinned = 0
   AND s.id <> COALESCE((SELECT current_snapshot_id FROM app_state WHERE id = 1), -1)
   AND NOT EXISTS (SELECT 1 FROM profile_revisions pr WHERE pr.snapshot_id = s.id)
   AND NOT EXISTS (SELECT 1 FROM backups b WHERE b.snapshot_id = s.id)";

/// 按保留期选出候选快照 id（按创建时间升序）。
const SNAPSHOT_EXPIRED_SQL: &str = "SELECT s.id FROM snapshots s
  WHERE {protected}
    AND s.created_at < ?1
  ORDER BY s.created_at ASC, s.id ASC";

/// 按条数上限选出候选快照 id：保留最新 `keep_max` 条，其余先删。
const SNAPSHOT_OVERFLOW_SQL: &str = "SELECT s.id FROM snapshots s
  WHERE {protected}
  ORDER BY s.created_at DESC, s.id DESC
  LIMIT -1 OFFSET ?1";

fn expired_sql() -> String {
    SNAPSHOT_EXPIRED_SQL.replace("{protected}", SNAPSHOT_PROTECTED)
}

fn overflow_sql() -> String {
    SNAPSHOT_OVERFLOW_SQL.replace("{protected}", SNAPSHOT_PROTECTED)
}

/// 删除给定快照 id，并先把 `events.snapshot_id` 置空。
///
/// **为什么置空而不是连事件一起删**：事件是审计记录，保留期由
/// `event_keep_days` 单独决定；快照被清理后事件仍然有审计价值，只是不再能跳转预览。
fn delete_snapshots_tx(tx: &rusqlite::Transaction<'_>, ids: &[i64]) -> Result<u32, CoreError> {
    if ids.is_empty() {
        return Ok(0);
    }
    let mut deleted = 0u32;
    for id in ids {
        tx.execute(
            "UPDATE events SET snapshot_id = NULL WHERE snapshot_id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| db_err("prune_events_unlink", e))?;
        deleted += tx
            .execute("DELETE FROM snapshots WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| db_err("prune_snapshots", e))? as u32;
    }
    Ok(deleted)
}

/// 执行清理，返回删除行数。
///
/// **绝不删除 `exported_at IS NULL`**：那是待导出队列，删了就永久丢变更。
///
/// 快照清理有两条独立规则（都受 [`SNAPSHOT_PROTECTED`] 保护）：
/// 超过 `snapshot_keep_days` 天的，以及总数超过 `snapshot_keep_max` 条后最旧的。
/// 事件按 `event_keep_days` 单独删除；被删快照的事件只在 `snapshot_id` 上解链。
pub fn prune(db: &mut Db, now_ms: i64) -> Result<PruneReport, CoreError> {
    let snapshot_cutoff = now_ms - (snapshot_keep_days() as i64) * 24 * 60 * 60 * 1000;
    let event_cutoff = now_ms - (event_keep_days() as i64) * 24 * 60 * 60 * 1000;

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
        let events_deleted = tx
            .execute(
                "DELETE FROM events WHERE created_at < ?1",
                rusqlite::params![event_cutoff],
            )
            .map_err(|e| db_err("prune_events", e))?;

        let mut candidate_ids: Vec<i64> = {
            let mut stmt = tx
                .prepare(&expired_sql())
                .map_err(|e| db_err("prune_snapshot_plan", e))?;
            let ids = stmt
                .query_map(rusqlite::params![snapshot_cutoff], |r| r.get(0))
                .map_err(|e| db_err("prune_snapshot_plan", e))?
                .collect::<Result<Vec<i64>, _>>()
                .map_err(|e| db_err("prune_snapshot_plan", e))?;
            ids
        };
        {
            let mut stmt = tx
                .prepare(&overflow_sql())
                .map_err(|e| db_err("prune_snapshot_overflow", e))?;
            let overflow = stmt
                .query_map(rusqlite::params![snapshot_keep_max() as i64], |r| r.get(0))
                .map_err(|e| db_err("prune_snapshot_overflow", e))?
                .collect::<Result<Vec<i64>, _>>()
                .map_err(|e| db_err("prune_snapshot_overflow", e))?;
            candidate_ids.extend(overflow);
        }
        candidate_ids.sort_unstable();
        candidate_ids.dedup();
        let snapshots_deleted = delete_snapshots_tx(tx, &candidate_ids)?;

        Ok(PruneReport {
            changes_deleted: changes_deleted as u32,
            conflicts_deleted: conflicts_deleted as u32,
            events_deleted: events_deleted as u32,
            snapshots_deleted,
        })
    })
}

/// dry-run：只统计将被删除的行数，**不写库**。
pub fn prune_plan(db: &Db, now_ms: i64) -> Result<PruneReport, CoreError> {
    let snapshot_cutoff = now_ms - (snapshot_keep_days() as i64) * 24 * 60 * 60 * 1000;
    let event_cutoff = now_ms - (event_keep_days() as i64) * 24 * 60 * 60 * 1000;
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
    let events_deleted: i64 = db.query_row_with(
        "SELECT COUNT(*) FROM events WHERE created_at < ?1",
        rusqlite::params![event_cutoff],
        |r| r.get(0),
    )?;
    // 到期与超限可能有交集：用 UNION 去重后计数，dry-run 才与实际删除数一致。
    // 两个分支各自使用 ?1 / ?2（不能用同一编号，否则绑定两个值会报参数数量不符）。
    let union_sql = format!(
        "SELECT COUNT(*) FROM (
           SELECT s.id FROM snapshots s
            WHERE {protected} AND s.created_at < ?1
           UNION
           SELECT id FROM (
             SELECT s.id FROM snapshots s
              WHERE {protected}
              ORDER BY s.created_at DESC, s.id DESC
              LIMIT -1 OFFSET ?2
           )
         )",
        protected = SNAPSHOT_PROTECTED
    );
    let snapshots_deleted: i64 = db.query_row_with(
        &union_sql,
        rusqlite::params![snapshot_cutoff, snapshot_keep_max() as i64],
        |r| r.get(0),
    )?;
    Ok(PruneReport {
        changes_deleted: changes_deleted as u32,
        conflicts_deleted: conflicts_deleted as u32,
        events_deleted: events_deleted as u32,
        snapshots_deleted: snapshots_deleted as u32,
    })
}

/// 以当前时间执行清理（GUI 启动序列用）。
///
/// 时间源收在 core：GUI crate 没有直接依赖 `chrono`，也不该为一次清理新增依赖。
pub fn prune_now(db: &mut Db) -> Result<PruneReport, CoreError> {
    prune(db, chrono::Utc::now().timestamp_millis())
}

/// 维护元数据文件名（与 `config.ini` 同目录）。
const MAINTENANCE_META_FILE: &str = "maintenance.json";

/// 写入「最近一次保留策略清理」时间。
///
/// 独立元数据文件而不是 `app_state` 新列：避免为一行时间戳升 schema。
pub fn record_prune_time(dir: &Path, at_ms: i64) -> Result<(), CoreError> {
    std::fs::create_dir_all(dir).map_err(|e| io_err("record_prune", e))?;
    let path = dir.join(MAINTENANCE_META_FILE);
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::json!({ "lastPruneAt": at_ms });
    std::fs::write(&tmp, body.to_string()).map_err(|e| io_err("record_prune", e))?;
    std::fs::rename(&tmp, &path).map_err(|e| io_err("record_prune", e))
}

/// 读取最近一次清理时间；文件不存在或内容非法时返回 `None`。
pub fn last_prune_time(dir: &Path) -> Option<i64> {
    let content = std::fs::read_to_string(dir.join(MAINTENANCE_META_FILE)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    value.get("lastPruneAt")?.as_i64()
}

/// 默认数据目录下的清理时间记录。
pub fn record_prune_time_default(at_ms: i64) -> Result<(), CoreError> {
    record_prune_time(&crate::db::crypto::data_home(), at_ms)
}

fn io_err(operation: &'static str, e: std::io::Error) -> CoreError {
    CoreError::new(ErrorCode::Io, operation, e.to_string())
}

/// 压缩数据库：先 `wal_checkpoint(TRUNCATE)`，再 `VACUUM`。
///
/// `VACUUM` 不能在事务内执行，因此这里用 `execute_batch` 直接下发。
/// 大库会短暂锁库，调用方应在维护命令里显式触发，不放进启动路径。
pub fn vacuum(db: &Db) -> Result<(), CoreError> {
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")
}

/// `changes` 的删除谓词。
///
/// `applied_at > 0` 排除「从未落地」的行；`exported_at IS NOT NULL` 排除待导出队列。
/// 任务本身产生的本地变更在 `record_change_tx` 里就把 `applied_at` 写成当时时间，
/// 因此本地导出过的变更同样会到期清理。
///
/// **额外两条保护**（否则保留策略会破坏合并语义）：
/// - 每个 `(entity_type, entity_uid)` 保留 lamport 最新的那一行 —— 它是 LWW 的
///   本地基线，删掉后导入旧包会走 `None => TakeRemote`，把已删除/旧版本实体复活；
/// - 保留**全部墓碑**（`field = 'deleted_at'`）——“删除”这个意图不能随保留期消失。
const CHANGES_PREDICATE: &str = "exported_at IS NOT NULL
   AND applied_at > 0
   AND applied_at < ?1
   AND (field IS NULL OR field <> 'deleted_at')
   AND EXISTS (
     SELECT 1 FROM changes d
      WHERE d.entity_type = changes.entity_type
        AND d.entity_uid = changes.entity_uid
        AND (d.lamport > changes.lamport
             OR (d.lamport = changes.lamport AND d.id > changes.id))
   )";

/// `sync_conflicts` 的删除谓词：只删**已解决**且到期的。
const CONFLICTS_PREDICATE: &str = "resolved_at IS NOT NULL AND resolved_at < ?1";

/// 遗留明文备份清理结果。
#[derive(Debug, Default)]
pub struct CleanupReport {
    /// 已转换（dry-run 时是「将转换」）的 `(json, pebak)` 对；dry-run 时 pebak 为空路径
    pub converted: Vec<(PathBuf, PathBuf)>,
    /// 跳过项与原因
    pub skipped: Vec<(PathBuf, String)>,
    /// 是否为 dry-run
    pub dry_run: bool,
}

/// 把遗留 `env_backup_*.json` 转成 `.pebak` 后删除明文。
///
/// **顺序不可颠倒**：先写 `.pebak` 并回读校验，成功后才删除 `.json`。
/// 任一失败都保留原文件——这是「转换成功才删除」的硬要求。
///
/// 解析**不使用** `backup::read_env_backup`：后者在解析失败时会经
/// `persist::parse_error_quarantined` 把文件重命名为 `.corrupt-<ts>`，
/// 清理命令不能在 dry-run 时改动用户文件。这里直接反序列化 + [`crate::platform::persist::migrate`]。
///
/// `dry_run = true` 时不使用 `material`；只想预览的调用方应改用
/// [`scan_legacy_env_backups`]，免得为一次 dry-run 去创建密钥文件。
pub fn cleanup_legacy_env_backups(
    dir: &Path,
    material: &KeyMaterial,
    dry_run: bool,
) -> Result<CleanupReport, CoreError> {
    cleanup_legacy_impl(dir, if dry_run { None } else { Some(material) })
}

/// dry-run：只解析并列出将被转换的文件，**不写、不删、不需要密钥**。
///
/// 与 [`cleanup_legacy_env_backups`] 共用同一份解析与筛选逻辑，区别只在
/// `material = None`——这保证「预览」与「执行」看的是同一批文件。
pub fn scan_legacy_env_backups(dir: &Path) -> Result<CleanupReport, CoreError> {
    cleanup_legacy_impl(dir, None)
}

/// 转换实现。`material` 为 `None` 即 dry-run。
///
/// **类型上就不可能「忘了传密钥却真的去写」**：写路径只在 `Some` 分支里。
fn cleanup_legacy_impl(
    dir: &Path,
    material: Option<&KeyMaterial>,
) -> Result<CleanupReport, CoreError> {
    let dry_run = material.is_none();
    let mut report = CleanupReport {
        dry_run,
        ..Default::default()
    };
    if !dir.is_dir() {
        return Ok(report);
    }

    let entries = std::fs::read_dir(dir).map_err(|e| {
        CoreError::new(
            crate::domain::error::ErrorCode::Io,
            "cleanup_legacy",
            format!("读取 {} 失败: {e}", dir.display()),
        )
    })?;

    for entry in entries {
        let path = entry
            .map_err(|e| {
                CoreError::new(
                    crate::domain::error::ErrorCode::Io,
                    "cleanup_legacy",
                    format!("读取目录项失败: {e}"),
                )
            })?
            .path();
        if !is_legacy_env_backup(&path) {
            continue;
        }

        let payload = match read_legacy_backup_no_quarantine(&path) {
            Ok(p) => p,
            Err(e) => {
                report
                    .skipped
                    .push((path, format!("无法解析: {}", e.message)));
                continue;
            }
        };

        let Some(material) = material else {
            // dry-run：只报告「会转换哪些」，pebak 路径未知（写入时刻才定名）
            report.converted.push((path, PathBuf::new()));
            continue;
        };

        let pebak = match crate::db::backup_index::write_env_backup_pebak(dir, &payload, material) {
            Ok(p) => p,
            Err(e) => {
                report
                    .skipped
                    .push((path, format!("写加密备份失败: {}", e.message)));
                continue;
            }
        };
        // **回读校验**：写成功不等于写对了（DPAPI 作用域、磁盘满都可能让文件不可读）。
        if let Err(e) = crate::db::backup_index::read_env_backup_any(&pebak, material) {
            report
                .skipped
                .push((path, format!("加密备份回读失败，已保留明文: {}", e.message)));
            continue;
        }
        if let Err(e) = std::fs::remove_file(&path) {
            report.skipped.push((path, format!("删除明文失败: {e}")));
            continue;
        }
        report.converted.push((path, pebak));
    }
    Ok(report)
}

/// 文件名是否形如本工具生成的 `env_backup_YYYYMMDD_HHMMSS_mmm.json`。
///
/// **不能只匹配 `env_backup_*.json`**：那个前缀可能命中用户自己放的文件
/// （例如 `env_backup_x.json`），清理命令不得碰它们——口径与 `backup.rs` 的轮换一致。
fn is_legacy_env_backup(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    // 与轮换共用同一套时间戳规则，避免两处判断漂移。
    name.ends_with(".json") && crate::platform::backup::is_env_backup_name(name)
}

/// 不做损坏隔离的遗留备份解析。
///
/// 与 `backup::read_env_backup` 同口径（`Versioned` 信封 + `persist::migrate`），
/// 唯一区别是解析失败时**只返回错误、不动文件**。
fn read_legacy_backup_no_quarantine(path: &Path) -> Result<EnvBackupPayload, CoreError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        CoreError::new(
            crate::domain::error::ErrorCode::Io,
            "cleanup_legacy",
            format!("读取 {} 失败: {e}", path.display()),
        )
    })?;
    let versioned: crate::platform::persist::Versioned<EnvBackupPayload> =
        serde_json::from_str(&content).map_err(|e| {
            CoreError::new(
                crate::domain::error::ErrorCode::Parse,
                "cleanup_legacy",
                format!("解析 {} 失败: {e}", path.display()),
            )
        })?;
    crate::platform::persist::migrate(versioned, "env 备份")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::crypto::load_or_create_key;
    use crate::domain::env_var::EnvValueKind;
    use crate::platform::backup::{EnvBackupHives, EnvBackupVar};
    use crate::platform::persist::{Versioned, PERSIST_SCHEMA_VERSION};

    /// 只用 DPAPI、不建数据库，因此可以放在单元测试模块里。
    #[test]
    fn legacy_json_is_converted_to_pebak_before_deletion() {
        let dir = tempfile::TempDir::new().unwrap();
        let material = load_or_create_key(dir.path()).unwrap();

        let payload = EnvBackupPayload {
            captured_at: 1_700_000_000_000,
            hives: EnvBackupHives {
                system: vec![EnvBackupVar {
                    name: "TOKEN".to_string(),
                    kind: EnvValueKind::String,
                    value: "secret".to_string(),
                    revision: "0123456789abcdef".to_string(),
                }],
                user: vec![],
            },
        };
        let legacy = dir.path().join("env_backup_20260101_000000_000.json");
        std::fs::write(
            &legacy,
            serde_json::to_string(&Versioned {
                schema_version: PERSIST_SCHEMA_VERSION,
                inner: payload.clone(),
            })
            .unwrap(),
        )
        .unwrap();

        // dry-run 不得写文件、不得删除
        let plan = cleanup_legacy_env_backups(dir.path(), &material, true).unwrap();
        assert_eq!(plan.converted.len(), 1);
        assert!(legacy.exists(), "dry-run 不得删除");

        // CLI 的预览走 `scan_legacy_env_backups`（不加载密钥）：两者必须看同一批文件
        let scanned = scan_legacy_env_backups(dir.path()).unwrap();
        assert!(scanned.dry_run);
        assert_eq!(
            scanned.converted.len(),
            plan.converted.len(),
            "预览与 dry-run 必须给出同一份计划"
        );

        let report = cleanup_legacy_env_backups(dir.path(), &material, false).unwrap();
        assert_eq!(report.converted.len(), 1);
        assert_eq!(report.skipped.len(), 0, "转换不应产生跳过项");
        assert!(!legacy.exists(), "转换成功后删除明文");

        let (_, pebak) = &report.converted[0];
        let read_back = crate::db::backup_index::read_env_backup_any(pebak, &material).unwrap();
        assert_eq!(read_back, payload, ".pebak 必须可回读");
    }
}
