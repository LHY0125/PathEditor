//! 派生表水位与按需重建。
//!
//! `daily_stats` 与 `var_references` 都能从 `events` / `snapshots` 完整重建，
//! 但每次启动全量重建会随数据量线性变慢。这里用「事件/快照的计数 + 最大 id」
//! 作为水位：水位未变就跳过，变了才重建。
//!
//! 水位是**派生元数据**，不是真相源；文件损坏或丢失只会导致一次多余重建。

use crate::db::{refs, stats, Db};
use crate::domain::error::CoreError;
use std::path::Path;

/// 派生表水位：**任何影响 `daily_stats` / `var_references` 的写入都必须推进它**。
///
/// - `events` / `snapshots`：两张派生表的主数据源；
/// - `scan_runs`：`daily_stats.conflicts` 的数据源（`record_run` 不写 events，
///   旧水位因此看不到扫描历史的变化）；
/// - `app_state.current_snapshot_id`：`var_references` 的入口（引用图按当前快照建）。
///
/// 计数覆盖删除（保留策略清理），最大 id 覆盖新增；两者同时比较才能避免
/// 「删一条、加一条、计数不变」被误判为无变化。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Watermark {
    pub events_count: i64,
    pub events_max: i64,
    pub snapshots_count: i64,
    pub snapshots_max: i64,
    pub scan_runs_count: i64,
    pub scan_runs_max: i64,
    pub current_snapshot_id: i64,
}

/// 读取当前水位。
pub fn watermark(db: &Db) -> Result<Watermark, CoreError> {
    let (events_count, events_max) = db.query_row_with(
        "SELECT COUNT(*), COALESCE(MAX(id), 0) FROM events",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let (snapshots_count, snapshots_max) = db.query_row_with(
        "SELECT COUNT(*), COALESCE(MAX(id), 0) FROM snapshots",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let (scan_runs_count, scan_runs_max) = db.query_row_with(
        "SELECT COUNT(*), COALESCE(MAX(id), 0) FROM scan_runs",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let current_snapshot_id: i64 = db.query_row_with(
        "SELECT COALESCE((SELECT current_snapshot_id FROM app_state WHERE id = 1), 0)",
        [],
        |r| r.get(0),
    )?;
    Ok(Watermark {
        events_count,
        events_max,
        snapshots_count,
        snapshots_max,
        scan_runs_count,
        scan_runs_max,
        current_snapshot_id,
    })
}

/// 读取水位文件；不存在、格式损坏或读失败一律返回 `None`（触发一次重建）。
fn read_watermark(path: &Path) -> Option<Watermark> {
    let raw = std::fs::read_to_string(path).ok()?;
    let parts: Vec<i64> = raw
        .trim()
        .split(':')
        .map(|p| p.parse::<i64>())
        .collect::<Result<_, _>>()
        .ok()?;
    // 旧格式（4 段）按「scan_runs / current_snapshot 未知」处理：
    // 直接视为不匹配，触发一次重建即可，不做兼容解析。
    let [events_count, events_max, snapshots_count, snapshots_max, scan_runs_count, scan_runs_max, current_snapshot_id] =
        parts.as_slice()
    else {
        return None;
    };
    Some(Watermark {
        events_count: *events_count,
        events_max: *events_max,
        snapshots_count: *snapshots_count,
        snapshots_max: *snapshots_max,
        scan_runs_count: *scan_runs_count,
        scan_runs_max: *scan_runs_max,
        current_snapshot_id: *current_snapshot_id,
    })
}

/// 写入水位；失败只告警，不影响已完成的派生表重建结果。
fn write_watermark(path: &Path, value: Watermark) {
    let content = format!(
        "{}:{}:{}:{}:{}:{}:{}",
        value.events_count,
        value.events_max,
        value.snapshots_count,
        value.snapshots_max,
        value.scan_runs_count,
        value.scan_runs_max,
        value.current_snapshot_id
    );
    if let Err(e) = crate::platform::fs::atomic_write(path, &content) {
        log::warn!("派生表水位写入失败（下次启动会重建）: {}", e);
    }
}

/// 水位未变时跳过重建；变了才重建 `daily_stats` 与 `var_references`。
///
/// 返回 `None` 表示跳过；`Some((days, edges))` 表示重建完成，附带两张表的行数。
/// 任一重建失败都向上传播，且**不写水位**，下次启动会重试。
pub fn rebuild_if_stale(
    db: &mut Db,
    watermark_path: &Path,
) -> Result<Option<(usize, usize)>, CoreError> {
    let current = watermark(db)?;
    if read_watermark(watermark_path) == Some(current) {
        return Ok(None);
    }

    let days = stats::rebuild_daily(db)?;
    let edges = refs::rebuild_refs(db)?;
    write_watermark(watermark_path, current);
    Ok(Some((days, edges)))
}

/// 只重建指定的派生表（供手动重建命令使用）。
pub fn rebuild_all(db: &mut Db) -> Result<(usize, usize), CoreError> {
    Ok((stats::rebuild_daily(db)?, refs::rebuild_refs(db)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watermark_roundtrip_and_corruption_falls_back() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("derived.watermark");
        let value = Watermark {
            events_count: 1,
            events_max: 2,
            snapshots_count: 3,
            snapshots_max: 4,
            scan_runs_count: 5,
            scan_runs_max: 6,
            current_snapshot_id: 7,
        };
        write_watermark(&path, value);
        assert_eq!(read_watermark(&path), Some(value));

        std::fs::write(&path, "1:2:3").unwrap();
        assert_eq!(read_watermark(&path), None);

        // 旧的四段格式视为不匹配（触发一次重建），不做兼容解析
        std::fs::write(&path, "1:2:3:4").unwrap();
        assert_eq!(read_watermark(&path), None);

        std::fs::write(&path, "not-a-number").unwrap();
        assert_eq!(read_watermark(&path), None);
    }
}
