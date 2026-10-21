//! `sync_conflicts` 读写与解决。
//!
//! 冲突是**待人工决定的记录**，不是错误：LWW 在平局时无法自动化，
//! 保守做法是保留双方、让用户选。

use crate::db::sync::change_log::{next_lamport, record_change_tx, ChangeRow, NewChange};
use crate::db::{db_err, Db};
use crate::error::{CoreError, ErrorCode};
use rusqlite::{params, OptionalExtension};

/// 冲突的对外视图。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflictView {
    /// 冲突 id（解决时的入参）
    pub id: i64,
    /// 实体类型
    pub entity_type: String,
    /// 实体 uid
    pub entity_uid: String,
    /// 字段名；`None` 表示整实体
    pub field: Option<String>,
    /// 本地值
    pub local_value: Option<String>,
    /// 远端值
    pub remote_value: Option<String>,
    /// 远端设备 id
    pub remote_device: String,
    /// 产生时间
    pub created_at: i64,
    /// 解决时间；`None` 表示未解决
    pub resolved_at: Option<i64>,
    /// 解决方式：`local` / `remote`
    pub resolution: Option<String>,
}

/// 写一条冲突。**已存在同 (entityType, entityUid, field, remoteDevice) 的
/// 未解决记录时不再重复写**——重复导入同一个包不该堆出一列相同的冲突。
pub(crate) fn record_conflict_tx(
    tx: &rusqlite::Transaction,
    local: &ChangeRow,
    remote: &ChangeRow,
) -> Result<(), CoreError> {
    let dup: Option<i64> = tx
        .query_row(
            "SELECT id FROM sync_conflicts
              WHERE entity_type = ?1 AND entity_uid = ?2
                AND ((field IS NULL AND ?3 IS NULL) OR field = ?3)
                AND remote_device = ?4 AND resolved_at IS NULL",
            params![
                remote.entity_type,
                remote.entity_uid,
                remote.field,
                remote.device_id
            ],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| db_err("find_conflict", e))?;
    if dup.is_some() {
        return Ok(());
    }

    tx.execute(
        "INSERT INTO sync_conflicts
           (entity_type, entity_uid, field, local_value, remote_value,
            remote_device, created_at, resolved_at, resolution)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL)",
        params![
            remote.entity_type,
            remote.entity_uid,
            remote.field,
            local.new_value,
            remote.new_value,
            remote.device_id,
            chrono::Utc::now().timestamp_millis()
        ],
    )
    .map_err(|e| db_err("insert_conflict", e))?;
    Ok(())
}

/// 列出冲突。`include_resolved = false` 时只看未解决的。
pub fn list_conflicts(db: &Db, include_resolved: bool) -> Result<Vec<SyncConflictView>, CoreError> {
    let sql = if include_resolved {
        "SELECT id, entity_type, entity_uid, field, local_value, remote_value,
                remote_device, created_at, resolved_at, resolution
           FROM sync_conflicts ORDER BY id ASC"
    } else {
        "SELECT id, entity_type, entity_uid, field, local_value, remote_value,
                remote_device, created_at, resolved_at, resolution
           FROM sync_conflicts WHERE resolved_at IS NULL ORDER BY id ASC"
    };
    let mut stmt = db.prepare(sql)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SyncConflictView {
                id: r.get(0)?,
                entity_type: r.get(1)?,
                entity_uid: r.get(2)?,
                field: r.get(3)?,
                local_value: r.get(4)?,
                remote_value: r.get(5)?,
                remote_device: r.get(6)?,
                created_at: r.get(7)?,
                resolved_at: r.get(8)?,
                resolution: r.get(9)?,
            })
        })
        .map_err(|e| db_err("list_conflicts", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("list_conflicts", e))?;
    Ok(rows)
}

/// 未解决冲突数（同步中心显示用）。
pub fn unresolved_count(db: &Db) -> Result<u32, CoreError> {
    let n: i64 = db.query_row_with(
        "SELECT count(*) FROM sync_conflicts WHERE resolved_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    Ok(n as u32)
}

/// 解决方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// 保留本地值
    Local,
    /// 采纳远端值
    Remote,
}

impl Resolution {
    /// 落库字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }

    /// 解析用户输入（`local` / `remote`，大小写不敏感）。
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "local" => Some(Self::Local),
            "remote" => Some(Self::Remote),
            _ => None,
        }
    }
}

/// 解决一条冲突。
///
/// **追加一条新 change**（核对轮裁断）：`lamport = next_lamport()`，
/// 即「当前 + 1」。理由：用户的选择是一个**更晚的决定**，必须胜过导致冲突的
/// 两个值——否则第三台设备导入时会按原 lamport 重新判一次，可能把用户刚做的
/// 选择覆盖掉。
///
/// `old_value` = 败方值，`new_value` = 胜方值。
/// 解决冲突时从库里取回的最小字段集。
struct ConflictTarget {
    entity_type: String,
    entity_uid: String,
    field: Option<String>,
    local_value: Option<String>,
    remote_value: Option<String>,
    resolved_at: Option<i64>,
}

impl ConflictTarget {
    fn load(tx: &rusqlite::Transaction, id: i64) -> Result<Option<Self>, CoreError> {
        tx.query_row(
            "SELECT entity_type, entity_uid, field, local_value, remote_value, resolved_at
               FROM sync_conflicts WHERE id = ?1",
            params![id],
            |r| {
                Ok(Self {
                    entity_type: r.get(0)?,
                    entity_uid: r.get(1)?,
                    field: r.get(2)?,
                    local_value: r.get(3)?,
                    remote_value: r.get(4)?,
                    resolved_at: r.get(5)?,
                })
            },
        )
        .optional()
        .map_err(|e| db_err("get_conflict", e))
    }
}

pub fn resolve_conflict(db: &mut Db, id: i64, resolution: Resolution) -> Result<(), CoreError> {
    let lamport = next_lamport(db)?;
    db.with_transaction(|tx| {
        let Some(target) = ConflictTarget::load(tx, id)? else {
            return Err(CoreError::new(
                ErrorCode::NotFound,
                "resolve_conflict",
                format!("冲突 {id} 不存在"),
            ));
        };
        if target.resolved_at.is_some() {
            return Err(CoreError::new(
                ErrorCode::Conflict,
                "resolve_conflict",
                format!("冲突 {id} 已解决"),
            ));
        }

        let (winner, loser) = match resolution {
            Resolution::Local => (target.local_value.clone(), target.remote_value.clone()),
            Resolution::Remote => (target.remote_value.clone(), target.local_value.clone()),
        };

        tx.execute(
            "UPDATE sync_conflicts SET resolved_at = ?1, resolution = ?2 WHERE id = ?3",
            params![
                chrono::Utc::now().timestamp_millis(),
                resolution.as_str(),
                id
            ],
        )
        .map_err(|e| db_err("resolve_conflict", e))?;

        // 追加的 change 用 `record_change_tx`，但它的 lamport 由该函数自增分配。
        // 这里需要「当前 + 1」的语义恰好等价——`record_change_tx` 内部就是
        // 取 app_state.lamport + 1，与 `next_lamport()` 同一算法，因此无需另写。
        // `lamport` 变量仅用于断言语义一致（debug 场景）与文档化。
        debug_assert!(lamport >= 1, "lamport 必须为正");
        record_change_tx(
            tx,
            NewChange {
                entity_type: target.entity_type,
                entity_uid: target.entity_uid,
                field: target.field,
                old_value: loser.unwrap_or_default(),
                new_value: winner.unwrap_or_default(),
            },
        )?;
        Ok(())
    })
}
