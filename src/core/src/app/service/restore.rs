//! 历史快照恢复：读快照 → 走保存编排（append-only）。

use super::*;

/// 把历史快照恢复回注册表。
///
/// 「读快照 → 写注册表与当前快照 → 记录恢复事件」是产品语义，GUI/CLI 共用。
/// 恢复是 **append-only**：成功后追加新快照与事件，旧快照原样保留——
/// 这样「恢复」本身也可被撤销。
pub fn restore_snapshot(
    db: &mut Db,
    uid: &str,
    source: EventSource,
) -> Result<ApplyOutcome, CoreError> {
    restore_snapshot_io(
        &RealPathHiveIo,
        db,
        uid,
        source,
        &disabled::default_pending_path(),
    )
}

/// [`restore_snapshot`] 的可注入版本（测试用 fake 注册表与 `TempDir` pending 路径）。
pub fn restore_snapshot_io(
    io: &dyn PathHiveIo,
    db: &mut Db,
    uid: &str,
    source: EventSource,
    pending_path: &std::path::Path,
) -> Result<ApplyOutcome, CoreError> {
    // 恢复的是 PATH（不含 env 值），因此读取时不需要 reveal。
    let view = history::get_snapshot(db, uid, false)?;
    let outcome = save_path_with_db_io(
        io,
        db,
        source,
        Some(PathHiveWrite {
            entries: view.system,
            expected: None,
        }),
        Some(PathHiveWrite {
            entries: view.user,
            expected: None,
        }),
        pending_path,
    )?;
    // 调用方只按 `Result` 判成败（GUI 的「恢复快照」成功后直接清空详情），
    // 逐 hive 失败必须在这里抬成 Err，否则「恢复」在注册表没改时也报成功。
    super::fail_on_hive_error(&outcome)?;
    Ok(outcome)
}
