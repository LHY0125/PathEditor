//! 把 [`HistorySink`] 落到数据库。
//!
//! **`record` 失败不阻断主流程** —— 调用方（服务层）只记 warn（spec D16 fail-open）。

use super::history::{action_str, result_str, source_str};
use super::Db;
use crate::error::{CoreError, ErrorCode};
use crate::history::{HistoryEvent, HistorySink};
use rusqlite::{params, OptionalExtension};
use std::cell::RefCell;
use uuid::Uuid;

/// 数据库实现的历史端口。
///
/// **为什么内部是 `RefCell<&mut Db>` 而不是裸 `&mut Db`**：
/// `HistorySink::record` 的签名是 `&self`（服务层以 `&dyn HistorySink` 持有它），
/// 而从 `&self` 无法借出 `&mut Db`（计划核对 H5 暴露的同类问题）。
/// `RefCell` 把「共享引用 + 内部可变」这件事显式化，且它**不是 `Sync`**，
/// 恰好与「trait 不要求 `Send + Sync`、同步单线程调用」的 M1 前提一致。
///
/// 运行期借用冲突（同一 sink 上重入 `record`）会 panic —— 但调用链里
/// `record` 只在服务层操作结束时调用一次，不存在重入路径。
pub struct DbHistory<'a>(pub RefCell<&'a mut Db>);

impl<'a> DbHistory<'a> {
    /// 构造：接管一个可变的 `Db` 引用。
    pub fn new(db: &'a mut Db) -> Self {
        Self(RefCell::new(db))
    }
}

impl HistorySink for DbHistory<'_> {
    fn record(&self, event: HistoryEvent) -> Result<(), CoreError> {
        let mut guard = self.0.borrow_mut();
        // `guard` 是 `RefMut<&'a mut Db>`：`*guard` 是 `&mut Db`（不能移出），
        // `**guard` 是 `Db`，因此这里是对内层 `Db` 的**重借用**而非多余借用。
        let db = &mut **guard;
        let now = chrono::Utc::now().timestamp_millis();
        let changed_fields =
            serde_json::to_string(&event.changed_fields).unwrap_or_else(|_| "[]".to_string());

        // 事件行不携带任何明文值（spec D6）：只有字段名、revision 与结果。
        db.with_transaction(|tx| {
            // J-8：把快照 uid 解析成本地整数主键。找不到时写 NULL 而不是让整条
            // 事件失败 —— 事件本身仍有价值，不该因为快照被删就一并丢弃。
            let snapshot_id: Option<i64> = match event.snapshot_uid.as_deref() {
                Some(uid) => tx
                    .query_row("SELECT id FROM snapshots WHERE uid = ?1", [uid], |r| {
                        r.get(0)
                    })
                    .optional()
                    .map_err(|e| super::db_err("record_find_snapshot", e))?,
                None => None,
            };

            tx.execute(
                "INSERT INTO events
                   (uid, created_at, source, action, hive, target, changed_fields,
                    before_revision, after_revision, result, snapshot_id, device_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                         COALESCE((SELECT device_id FROM app_state WHERE id = 1), 'local'))",
                params![
                    Uuid::now_v7().to_string(),
                    now,
                    source_str(event.source),
                    action_str(event.action),
                    event.hive.map(|h| match h {
                        crate::env_var::EnvHive::System => "system",
                        crate::env_var::EnvHive::User => "user",
                    }),
                    event.target,
                    changed_fields,
                    event.before_revision,
                    event.after_revision,
                    result_str(event.result),
                    snapshot_id,
                ],
            )
            .map_err(|e| CoreError::new(ErrorCode::Database, "record", e.to_string()))?;
            Ok(())
        })
    }
}
