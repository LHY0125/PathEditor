//! 启动引导序列。
//!
//! 顺序固定：
//! 1. schema 建库/迁移（失败中止启动）；
//! 2. 补写上次未落盘的 pending 快照；
//! 3. 确保 `kind=current` 基线存在；
//! 4. 备份索引与保留策略清理；
//! 5. 派生表重建（FTS 首次构建 + `daily_stats` / `var_references` 水位判断）。
//!
//! **失败策略**：只有迁移失败会中止启动。其余步骤失败只是记入
//! [`BootstrapReport::warnings`]——DB 可用性与注册表编辑优先于派生数据新鲜度，
//! 且下次启动会重试。调用方（GUI/CLI）负责把 warnings 写入自己的日志系统。

use crate::app::service;
use crate::db::{self, Db};
use crate::domain::history::{EventSource, HistorySink};
use crate::{CoreError, ErrorCode};
use std::path::PathBuf;

/// 引导可选路径。`None` 走生产默认路径；测试注入 `TempDir` 避免触碰真实配置目录。
#[derive(Debug, Clone)]
pub struct BootstrapOptions {
    /// 备份索引目录；默认 `backup::env_backup_dir()`。
    pub backup_dir: Option<PathBuf>,
    /// pending 快照路径；默认 `disabled::default_pending_path()`。
    pub pending_path: Option<PathBuf>,
    /// 派生表水位文件；默认 `<config_dir>/derived.watermark`。
    pub derived_watermark: Option<PathBuf>,
    /// 是否在启动时比较注册表与 `current_snapshot`（漂移检测）。
    ///
    /// 生产默认 `true`；测试可关掉以保持不读真实注册表。
    pub detect_drift: bool,
    /// 维护元数据目录（`maintenance.json`）；默认数据目录。测试注入 `TempDir`。
    pub maintenance_meta_dir: Option<PathBuf>,
}

impl Default for BootstrapOptions {
    fn default() -> Self {
        Self {
            backup_dir: None,
            pending_path: None,
            derived_watermark: None,
            detect_drift: true,
            maintenance_meta_dir: None,
        }
    }
}

/// 一次引导各步骤的结果。
///
/// 字段为「实际发生的动作」，调用方据此决定日志级别：
/// - `baseline_created` / `fts_indexed` / `derived_rebuilt` 只在首次或数据变化时非空；
/// - `warnings` 非空表示有步骤失败但启动继续。
#[derive(Debug, Default)]
pub struct BootstrapReport {
    /// 是否成功补写了 pending 快照
    pub pending_retried: bool,
    /// 是否新建了 `kind=current` 基线
    pub baseline_created: bool,
    /// 备份目录扫描到的备份条数
    pub backups_indexed: usize,
    /// 保留策略删除的 changes 条数
    pub changes_pruned: u32,
    /// 保留策略删除的冲突条数
    pub conflicts_pruned: u32,
    /// FTS 首次构建的条目数（已非空时为 0）
    pub fts_indexed: usize,
    /// 派生表重建结果 `(daily_stats 天数, 引用图边数)`；跳过时为 `None`
    pub derived_rebuilt: Option<(usize, usize)>,
    /// 检测到的注册表漂移 hive（`system` / `user`）
    pub drift_fields: Vec<String>,
    /// 本次是否真的写入了 drift 事件（同一份外部状态只记一次）
    pub drift_recorded: bool,
    /// 失败但未阻断启动的步骤说明
    pub warnings: Vec<String>,
}

/// 执行完整启动引导。
///
/// `source` 只用于 pending 补写的事件来源；基线快照固定记为
/// [`EventSource::Migration`]（与历史行为一致，不进入同步队列的语义由
/// `insert_snapshot` 内部处理）。
pub fn initialize(
    db: &mut Db,
    source: EventSource,
    options: &BootstrapOptions,
) -> Result<BootstrapReport, CoreError> {
    db.migrate_to_current()?;
    let mut report = BootstrapReport::default();

    // ② 补写上次未落盘的快照
    let pending_path = options
        .pending_path
        .clone()
        .unwrap_or_else(crate::app::disabled::default_pending_path);
    match service::retry_pending_path_state_db_at(db, source, &pending_path) {
        Ok(_) => report.pending_retried = true,
        Err(e) => report
            .warnings
            .push(format!("补写 pending 失败（继续启动）: {}", e.message)),
    }

    // ③ 当前状态基线
    match ensure_current_snapshot_baseline(db) {
        Ok(created) => report.baseline_created = created,
        Err(e) => report
            .warnings
            .push(format!("建立当前状态基线失败: {}", e.message)),
    }

    // ③b 注册表漂移检测：只记事件，不自动建快照
    if options.detect_drift {
        match service::load_path_snapshot_db(db) {
            Ok(live) => match record_drift(db, source, &live) {
                Ok(Some(outcome)) => {
                    report.drift_fields = outcome.fields;
                    report.drift_recorded = outcome.recorded;
                }
                Ok(None) => {}
                Err(e) => report
                    .warnings
                    .push(format!("注册表漂移检测失败（继续启动）: {}", e.message)),
            },
            Err(e) => report.warnings.push(format!(
                "读取注册表做漂移检测失败（继续启动）: {}",
                e.message
            )),
        }
    }

    // ④ 备份索引
    let backup_dir = options
        .backup_dir
        .clone()
        .unwrap_or_else(crate::platform::backup::env_backup_dir);
    match db::backup_index::index_backups(db, &backup_dir) {
        Ok(n) => report.backups_indexed = n,
        Err(e) => report
            .warnings
            .push(format!("备份索引失败（继续启动）: {}", e.message)),
    }

    // ④b 保留策略
    let prune_at = chrono::Utc::now().timestamp_millis();
    match db::maintenance::prune(db, prune_at) {
        Ok(r) => {
            report.changes_pruned = r.changes_deleted;
            report.conflicts_pruned = r.conflicts_deleted;
            // 时间戳 best-effort：清理已成功，记录失败只告警
            let meta_dir = options
                .maintenance_meta_dir
                .clone()
                .unwrap_or_else(db::crypto::data_home);
            if let Err(e) = db::maintenance::record_prune_time(&meta_dir, prune_at) {
                report
                    .warnings
                    .push(format!("记录清理时间失败（继续启动）: {}", e.message));
            }
        }
        Err(e) => report
            .warnings
            .push(format!("保留策略清理失败（继续启动）: {}", e.message)),
    }

    // ⑤ 派生表
    rebuild_derived(db, options, &mut report);

    Ok(report)
}

/// 确保 DB 里有一条 `kind=current` 快照；没有就用注册表当前状态建一条。
///
/// 幂等：已有 current 时返回 `Ok(false)`。
fn ensure_current_snapshot_baseline(db: &mut Db) -> Result<bool, CoreError> {
    if db::history::has_current_snapshot(db)? {
        return Ok(false);
    }
    let snapshot = service::load_path_snapshot_db(db)?;
    ensure_baseline_from(db, snapshot)
}

/// 用调用方给定的快照建立 `kind=current` 基线。
///
/// 独立于注册表读取，供测试与需要显式快照的入口复用。
/// 幂等：已有 current 时返回 `Ok(false)`。
pub fn ensure_baseline_from(db: &mut Db, snapshot: crate::PathSnapshot) -> Result<bool, CoreError> {
    if db::history::has_current_snapshot(db)? {
        return Ok(false);
    }

    let uid = db::history::insert_snapshot(
        db,
        db::history::NewSnapshot {
            kind: db::history::SnapshotKind::Current,
            source: EventSource::Migration,
            system: snapshot.system,
            user: snapshot.user,
            env: Vec::new(),
            parent_uid: None,
        },
    )?;
    let version = db::history::app_state_version(db)?;
    if !db::history::set_current_snapshot(db, &uid, version)? {
        return Err(CoreError::new(
            ErrorCode::Conflict,
            "bootstrap",
            "建立基线时当前快照 version CAS 冲突，读路径将回落到注册表",
        ));
    }
    Ok(true)
}

/// 漂移检测结果。
#[derive(Debug, Clone)]
pub struct DriftOutcome {
    /// 发生差异的 hive（`system` / `user`）
    pub fields: Vec<String>,
    /// 是否写入了新事件（同一外部状态重复启动只记一次）
    pub recorded: bool,
}

/// 比较实时快照与 `current_snapshot`，有差异时写一条 `drift` 事件。
///
/// **不建快照**：漂移是「外部改动发生」的信号，是否留档由用户决定
/// （历史面板本来就能对当前状态手动建快照）。事件通过 `before_revision`
/// 存放实时状态的哈希，用于去重——每次启动都重复记一条会让历史面板失去可读性。
pub fn record_drift(
    db: &mut Db,
    source: EventSource,
    live: &crate::PathSnapshot,
) -> Result<Option<DriftOutcome>, CoreError> {
    let Some(current) = db::history::current_snapshot(db)? else {
        return Ok(None);
    };
    let mut fields = Vec::new();
    if path_lists_differ(&current.system, &live.system) {
        fields.push("system".to_string());
    }
    if path_lists_differ(&current.user, &live.user) {
        fields.push("user".to_string());
    }
    if fields.is_empty() {
        return Ok(None);
    }

    let hash = live_hash(live);
    let last: Option<String> = db.query_row_optional(
        "SELECT before_revision FROM events WHERE action = 'drift' ORDER BY id DESC LIMIT 1",
        [],
        |r| r.get(0),
    )?;
    if last.as_deref() == Some(hash.as_str()) {
        return Ok(Some(DriftOutcome {
            fields,
            recorded: false,
        }));
    }

    db::history_sink::DbHistory::new(db).record(crate::domain::history::HistoryEvent {
        source,
        action: crate::domain::history::EventAction::Drift,
        hive: None,
        target: Some(fields.join(",")),
        changed_fields: fields.clone(),
        before_revision: Some(hash),
        after_revision: None,
        result: crate::domain::history::EventResult::Partial,
        snapshot_uid: Some(current.uid),
    })?;
    Ok(Some(DriftOutcome {
        fields,
        recorded: true,
    }))
}

/// PATH 列表是否不同：逐项比顺序、启用状态与路径（Windows 大小写不敏感）。
fn path_lists_differ(a: &[crate::PathEntry], b: &[crate::PathEntry]) -> bool {
    a.len() != b.len()
        || a.iter()
            .zip(b)
            .any(|(x, y)| x.enabled != y.enabled || !x.path.eq_ignore_ascii_case(&y.path))
}

/// 实时状态的稳定哈希（大小写归一，含启用状态与顺序）。
fn live_hash(live: &crate::PathSnapshot) -> String {
    let mut hasher = blake3::Hasher::new();
    for (hive, entries) in [("system", &live.system), ("user", &live.user)] {
        hasher.update(hive.as_bytes());
        hasher.update(&[0]);
        for entry in entries {
            hasher.update(&[u8::from(entry.enabled)]);
            hasher.update(entry.path.to_ascii_lowercase().as_bytes());
            hasher.update(&[0]);
        }
    }
    hasher.finalize().to_hex().to_string()
}

/// 重建三张派生表；失败只记 warning。
fn rebuild_derived(db: &mut Db, options: &BootstrapOptions, report: &mut BootstrapReport) {
    // `search_fts`：行数与实体表不一致就重建；一致时写入路径的增量维护已足够。
    // 旧行为是「仅当为空才重建」，旧库升级后（索引非空但缺配置行）永远补不上缺口。
    match db::search::index_needs_rebuild(db) {
        Ok(true) => match db::search::rebuild_index(db) {
            Ok(n) => report.fts_indexed = n,
            Err(e) => report
                .warnings
                .push(format!("FTS 索引重建失败（继续启动）: {}", e.message)),
        },
        Ok(false) => {}
        Err(e) => report
            .warnings
            .push(format!("FTS 索引一致性检查失败（继续启动）: {}", e.message)),
    }

    // `daily_stats` / `var_references`：事件/快照水位未变则跳过。
    let watermark_path = options
        .derived_watermark
        .clone()
        .unwrap_or_else(|| db::crypto::config_dir().join("derived.watermark"));
    match db::derived::rebuild_if_stale(db, &watermark_path) {
        Ok(result) => report.derived_rebuilt = result,
        Err(e) => report
            .warnings
            .push(format!("派生表重建失败（继续启动）: {}", e.message)),
    }
}
