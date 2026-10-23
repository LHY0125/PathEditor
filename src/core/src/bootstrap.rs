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

use crate::db::{self, Db};
use crate::history::EventSource;
use crate::service;
use crate::{CoreError, ErrorCode};
use std::path::PathBuf;

/// 引导可选路径。`None` 走生产默认路径；测试注入 `TempDir` 避免触碰真实配置目录。
#[derive(Debug, Default, Clone)]
pub struct BootstrapOptions {
    /// 备份索引目录；默认 `backup::env_backup_dir()`。
    pub backup_dir: Option<PathBuf>,
    /// pending 快照路径；默认 `disabled::default_pending_path()`。
    pub pending_path: Option<PathBuf>,
    /// 派生表水位文件；默认 `<config_dir>/derived.watermark`。
    pub derived_watermark: Option<PathBuf>,
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
        .unwrap_or_else(crate::disabled::default_pending_path);
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

    // ④ 备份索引
    let backup_dir = options
        .backup_dir
        .clone()
        .unwrap_or_else(crate::backup::env_backup_dir);
    match db::backups::index_backups(db, &backup_dir) {
        Ok(n) => report.backups_indexed = n,
        Err(e) => report
            .warnings
            .push(format!("备份索引失败（继续启动）: {}", e.message)),
    }

    // ④b 保留策略
    match db::maintenance::prune_now(db) {
        Ok(r) => {
            report.changes_pruned = r.changes_deleted;
            report.conflicts_pruned = r.conflicts_deleted;
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

/// 重建三张派生表；失败只记 warning。
fn rebuild_derived(db: &mut Db, options: &BootstrapOptions, report: &mut BootstrapReport) {
    // `search_fts`：仅当为空时才重建；写入路径会增量维护。
    match db::search::index_count(db) {
        Ok(0) => match db::search::rebuild_index(db) {
            Ok(n) => report.fts_indexed = n,
            Err(e) => report
                .warnings
                .push(format!("FTS 索引重建失败（继续启动）: {}", e.message)),
        },
        Ok(_) => {}
        Err(e) => report
            .warnings
            .push(format!("FTS 索引计数失败（继续启动）: {}", e.message)),
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
