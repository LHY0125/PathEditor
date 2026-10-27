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

/// 当前水位：`(events_count, events_max_id, snapshots_count, snapshots_max_id)`。
///
/// 计数覆盖删除（保留策略清理），最大 id 覆盖新增；两者同时比较才能避免
/// 「删一条、加一条、计数不变」被误判为无变化。
pub fn watermark(db: &Db) -> Result<(i64, i64, i64, i64), CoreError> {
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
    Ok((events_count, events_max, snapshots_count, snapshots_max))
}

/// 读取水位文件；不存在、格式损坏或读失败一律返回 `None`（触发一次重建）。
fn read_watermark(path: &Path) -> Option<(i64, i64, i64, i64)> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut parts = raw.trim().split(':');
    let parsed = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    if parts.next().is_some() {
        return None;
    }
    Some(parsed)
}

/// 写入水位；失败只告警，不影响已完成的派生表重建结果。
fn write_watermark(path: &Path, value: (i64, i64, i64, i64)) {
    let content = format!("{}:{}:{}:{}", value.0, value.1, value.2, value.3);
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
        let value = (1, 2, 3, 4);
        write_watermark(&path, value);
        assert_eq!(read_watermark(&path), Some(value));

        std::fs::write(&path, "1:2:3").unwrap();
        assert_eq!(read_watermark(&path), None);

        std::fs::write(&path, "not-a-number").unwrap();
        assert_eq!(read_watermark(&path), None);
    }
}
