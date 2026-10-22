//! `changes` 变更日志：写入、待导出查询、标记已导出。
//!
//! **本表是同步的真相源**：`export` 只导出 `origin='local' AND exported_at IS NULL`
//! 的行，`import` 把远端变更以 `origin='remote'` 写入。派生表（`search_fts` 等）
//! 可以从快照重建，但变更日志**不能**——它是「发生过什么」的唯一记录。
//!
//! ## 两个计数器
//!
//! - `seq`：**本设备**单调序号，从 1 开始，与 `device_id` 组成 `(device_id, seq)`；
//! - `lamport`：逻辑时钟，跨设备可比，用于 LWW 判定。
//!
//! 两者**同源推进**（每次写入都 +1）：若 `lamport` 落后于 `seq`，导入时的
//! LWW 会用错误的时间比较，导致「较新的本地修改」输给「较旧的远端修改」。

use rusqlite::{params, Transaction};

use crate::db::{db_err, Db};
use crate::error::CoreError;

/// 待写入的一条变更（调用方构造）。
#[derive(Debug, Clone)]
pub struct NewChange {
    /// 实体类型：`snapshot` / `profile` / `profile_revision` / `path_entry` / `env_value`
    pub entity_type: String,
    /// 实体 uid（**不是**本地整数主键）
    pub entity_uid: String,
    /// 字段名；`None` 表示整实体
    pub field: Option<String>,
    /// 旧值 JSON（**仅用于同步**，不写入 `events`，UI 展示须掩码）
    pub old_value: String,
    /// 新值 JSON
    pub new_value: String,
}

/// 已落库的一条变更（查询返回）。
#[derive(Debug, Clone)]
pub struct ChangeRow {
    /// 变更 UUID（幂等判据）
    pub uid: String,
    /// 产生变更的设备
    pub device_id: String,
    /// 本设备单调序号
    pub seq: i64,
    /// 逻辑时间（LWW 比较用）
    pub lamport: i64,
    /// `local` / `remote`
    pub origin: String,
    /// 实体类型
    pub entity_type: String,
    /// 实体 uid
    pub entity_uid: String,
    /// 字段名；`None` 表示整实体
    pub field: Option<String>,
    /// 旧值
    pub old_value: Option<String>,
    /// 新值
    pub new_value: Option<String>,
    /// 产生时间
    pub created_at: i64,
    /// 本地落地时间
    pub applied_at: i64,
    /// 导出时间；`None` = 待导出（仅 `origin='local'`）
    pub exported_at: Option<i64>,
}

/// 读本设备 id。
///
/// 行缺失时回落到 `"local"`——与 [`crate::db::history_sink`] 的 `COALESCE`
/// 兜底口径一致：同步是 sidecar 能力，不该因为一行元数据缺失就整体失败。
fn device_id(tx: &Transaction) -> Result<String, CoreError> {
    tx.query_row(
        "SELECT COALESCE((SELECT device_id FROM app_state WHERE id = 1), 'local')",
        [],
        |r| r.get::<_, String>(0),
    )
    .map_err(|e| db_err("device_id", e))
}

/// 读当前逻辑时钟。
pub fn current_lamport(db: &Db) -> Result<i64, CoreError> {
    db.query_row_with(
        "SELECT COALESCE((SELECT lamport FROM app_state WHERE id = 1), 0)",
        [],
        |r| r.get::<_, i64>(0),
    )
}

/// 「当前 + 1」的逻辑时钟，**不落库**。
///
/// 冲突解决时用：用户的选择是一个**更晚的决定**，必须胜过导致冲突的两个值，
/// 否则第三台设备导入时会按原 lamport 重新判一次，把用户刚做的选择覆盖掉。
pub fn next_lamport(db: &Db) -> Result<i64, CoreError> {
    Ok(current_lamport(db)? + 1)
}

/// 事务内写入一条变更，返回其 uid。
///
/// **不自开事务**：调用方持有事务（`save_profile_tx` / `insert_snapshot_tx` 等），
/// 与业务行同一事务提交，避免「业务写成、变更没写成」的半状态。
pub fn record_change_tx(tx: &Transaction, c: NewChange) -> Result<String, CoreError> {
    let device = device_id(tx)?;
    let uid = uuid::Uuid::now_v7().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    // seq 与 lamport 同源推进：两者都取「当前最大值 + 1」。
    //
    // 为什么 seq 用 `MAX(seq) WHERE device_id = ?` 而不是依赖 id 自增：
    // 导入会把**其它设备**的变更写进同一张表，自增 id 因此不等于本设备序号。
    let next_seq: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM changes WHERE device_id = ?1",
            params![device],
            |r| r.get(0),
        )
        .map_err(|e| db_err("next_seq", e))?;
    let next_lamport: i64 = tx
        .query_row(
            "SELECT COALESCE((SELECT lamport FROM app_state WHERE id = 1), 0) + 1",
            [],
            |r| r.get(0),
        )
        .map_err(|e| db_err("next_lamport", e))?;

    tx.execute(
        "INSERT INTO changes
           (uid, device_id, seq, lamport, origin, entity_type, entity_uid,
            field, old_value, new_value, created_at, applied_at, exported_at)
         VALUES (?1, ?2, ?3, ?4, 'local', ?5, ?6, ?7, ?8, ?9, ?10, ?10, NULL)",
        params![
            uid,
            device,
            next_seq,
            next_lamport,
            c.entity_type,
            c.entity_uid,
            c.field,
            c.old_value,
            c.new_value,
            now
        ],
    )
    .map_err(|e| db_err("insert_change", e))?;

    tx.execute(
        "UPDATE app_state SET lamport = ?1 WHERE id = 1",
        params![next_lamport],
    )
    .map_err(|e| db_err("bump_lamport", e))?;

    Ok(uid)
}

/// 待导出变更：`origin='local' AND exported_at IS NULL`，按 `seq` 升序。
pub fn pending_export(db: &Db) -> Result<Vec<ChangeRow>, CoreError> {
    let mut stmt = db.prepare(
        "SELECT uid, device_id, seq, lamport, origin, entity_type, entity_uid,
                field, old_value, new_value, created_at, applied_at, exported_at
           FROM changes
          WHERE origin = 'local' AND exported_at IS NULL
          ORDER BY seq ASC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ChangeRow {
                uid: r.get(0)?,
                device_id: r.get(1)?,
                seq: r.get(2)?,
                lamport: r.get(3)?,
                origin: r.get(4)?,
                entity_type: r.get(5)?,
                entity_uid: r.get(6)?,
                field: r.get(7)?,
                old_value: r.get(8)?,
                new_value: r.get(9)?,
                created_at: r.get(10)?,
                applied_at: r.get(11)?,
                exported_at: r.get(12)?,
            })
        })
        .map_err(|e| db_err("pending_export", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("pending_export", e))?;
    Ok(rows)
}

/// 标记若干变更已导出，返回**实际更新**的行数。
///
/// 只更新 `exported_at IS NULL` 的行：重复标记同一条不计数，因此返回值可以
/// 直接用来断言「这次真的导出了几条」。
pub fn mark_exported(db: &mut Db, uids: &[String], at: i64) -> Result<usize, CoreError> {
    if uids.is_empty() {
        return Ok(0);
    }
    db.with_transaction(|tx| {
        let mut n = 0usize;
        for uid in uids {
            n += tx
                .execute(
                    "UPDATE changes SET exported_at = ?1
                      WHERE uid = ?2 AND exported_at IS NULL",
                    params![at, uid],
                )
                .map_err(|e| db_err("mark_exported", e))?;
        }
        Ok(n)
    })
}

/// 按 uid 查一条变更是否已存在（导入幂等判据）。
///
/// **接 `&Transaction` 而非 `&Db`**：导入在单事务里跑，用 `&Db` 会借用冲突。
pub(crate) fn change_exists_tx(tx: &Transaction, uid: &str) -> Result<bool, CoreError> {
    use rusqlite::OptionalExtension as _;
    let id: Option<i64> = tx
        .query_row("SELECT id FROM changes WHERE uid = ?1", params![uid], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| db_err("change_exists", e))?;
    Ok(id.is_some())
}

/// 事务内写入一条**远端**变更（导入用）。
///
/// 与 [`record_change_tx`] 的区别：
/// - `origin='remote'`，因此不会进入待导出队列；
/// - `seq` / `lamport` / `device_id` 全部**沿用远端值**（不重新分配）——
/// 它们是远端设备的事实，改写会破坏跨设备的 LWW 比较；
/// - 不推进本地 `app_state.lamport`（由导入方在整包结束时统一 `max` 提升）。
#[allow(clippy::too_many_arguments)]
pub fn record_remote_change_tx(
    tx: &Transaction,
    uid: &str,
    device_id: &str,
    seq: i64,
    lamport: i64,
    entity_type: &str,
    entity_uid: &str,
    field: Option<&str>,
    old_value: Option<&str>,
    new_value: Option<&str>,
) -> Result<(), CoreError> {
    let now = chrono::Utc::now().timestamp_millis();
    tx.execute(
        "INSERT INTO changes
           (uid, device_id, seq, lamport, origin, entity_type, entity_uid,
            field, old_value, new_value, created_at, applied_at, exported_at)
         VALUES (?1, ?2, ?3, ?4, 'remote', ?5, ?6, ?7, ?8, ?9, ?10, ?10, ?10)",
        params![
            uid,
            device_id,
            seq,
            lamport,
            entity_type,
            entity_uid,
            field,
            old_value,
            new_value,
            now
        ],
    )
    .map_err(|e| db_err("insert_remote_change", e))?;
    Ok(())
}
