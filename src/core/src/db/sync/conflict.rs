//! `sync_conflicts` 读写与解决。
//!
//! 冲突是**待人工决定的记录**，不是错误：LWW 在平局时无法自动化，
//! 保守做法是保留双方、让用户选。

use crate::db::sync::change_log::{record_change_tx, ChangeRow, NewChange};
use crate::db::{db_err, Db};
use crate::domain::error::{CoreError, ErrorCode};
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
    /// 字段名；`None` 表示非 profile 的整实体
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
///
/// `remote_content` 是「采纳远端」的落点：导入的冲突分支不写实体，远端内容
/// 必须在那一刻留下，否则用户选择 `remote` 后无从应用（旧实现只记 new_value）。
pub(crate) fn record_conflict_tx(
    tx: &rusqlite::Transaction,
    local: &ChangeRow,
    remote: &ChangeRow,
    remote_content: Option<&serde_json::Value>,
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

    let content_json = remote_content
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| {
            CoreError::new(
                ErrorCode::SyncFormat,
                "record_conflict",
                format!("冲突内容序列化失败: {e}"),
            )
        })?;

    tx.execute(
        "INSERT INTO sync_conflicts
           (entity_type, entity_uid, field, local_value, remote_value,
            remote_device, created_at, resolved_at, resolution, remote_content)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8)",
        params![
            remote.entity_type,
            remote.entity_uid,
            remote.field,
            local.new_value,
            remote.new_value,
            remote.device_id,
            chrono::Utc::now().timestamp_millis(),
            content_json
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
/// **追加一条新 change**：`lamport = next_lamport()`，
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
    /// 产生冲突的远端设备（profile 同名改名的后缀来源）。
    remote_device: String,
    /// 导入时留下的远端实体内容（JSON）；旧库记录为 `None`。
    remote_content: Option<String>,
}

impl ConflictTarget {
    fn load(tx: &rusqlite::Transaction, id: i64) -> Result<Option<Self>, CoreError> {
        tx.query_row(
            "SELECT entity_type, entity_uid, field, local_value, remote_value,
                    resolved_at, remote_device, remote_content
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
                    remote_device: r.get(6)?,
                    remote_content: r.get(7)?,
                })
            },
        )
        .optional()
        .map_err(|e| db_err("get_conflict", e))
    }
}

pub fn resolve_conflict(db: &mut Db, id: i64, resolution: Resolution) -> Result<(), CoreError> {
    db.with_transaction(|tx| resolve_one_tx(tx, id, resolution))
}

/// 批量解决：单事务，任一失败整批回滚。
pub fn resolve_conflicts(
    db: &mut Db,
    ids: &[i64],
    resolution: Resolution,
) -> Result<(), CoreError> {
    if ids.is_empty() {
        return Ok(());
    }
    db.with_transaction(|tx| {
        for id in ids {
            resolve_one_tx(tx, *id, resolution)?;
        }
        Ok(())
    })
}

fn resolve_one_tx(
    tx: &rusqlite::Transaction<'_>,
    id: i64,
    resolution: Resolution,
) -> Result<(), CoreError> {
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

    if resolution == Resolution::Remote {
        if target.field.as_deref() == Some("deleted_at") {
            super::import::apply_tombstone_tx(tx, &target.entity_type, &target.entity_uid)?;
        } else if let Some(raw) = &target.remote_content {
            let content: serde_json::Value = serde_json::from_str(raw).map_err(|e| {
                CoreError::new(
                    ErrorCode::SyncFormat,
                    "resolve_conflict",
                    format!("冲突 {id} 的远端内容无法解析: {e}"),
                )
            })?;
            // profile 的字段级冲突只写该字段；整体写回会把用户已解决的
            // 其它字段一起覆盖，正是 D4 要消除的问题。
            if target.entity_type == "profile" {
                let field = target.field.as_deref().ok_or_else(|| {
                    CoreError::new(
                        ErrorCode::Internal,
                        "resolve_conflict",
                        "profile 冲突缺少字段名，无法按字段应用远端内容",
                    )
                })?;
                // `deleted_at` 已在上面处理，这里只剩普通字段。
                crate::db::profiles::apply_profile_field_tx(
                    tx,
                    &target.entity_uid,
                    field,
                    &content,
                    &target.remote_device,
                )?;
            } else {
                super::import::apply_entity_tx(
                    tx,
                    &target.entity_type,
                    &target.entity_uid,
                    &content,
                    &target.remote_device,
                )?;
            }
        } else {
            log::warn!("冲突 {id} 没有远端内容（旧版本记录），仅记录解决结果");
        }
    }

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
}

/// 单条冲突的详情（含远端原始内容，供并排 diff）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflictDetail {
    /// 冲突本身
    pub conflict: SyncConflictView,
    /// 远端实体内容（JSON 字符串；旧记录可能为空）
    pub remote_content: Option<String>,
}

/// 读取单条冲突详情。
pub fn conflict_detail(db: &Db, id: i64) -> Result<SyncConflictDetail, CoreError> {
    let conflict = list_conflicts(db, true)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| {
            CoreError::new(
                ErrorCode::NotFound,
                "conflict_detail",
                format!("冲突 {id} 不存在"),
            )
        })?;
    let remote_content: Option<String> = db.query_row_with(
        "SELECT remote_content FROM sync_conflicts WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    Ok(SyncConflictDetail {
        conflict,
        remote_content,
    })
}
