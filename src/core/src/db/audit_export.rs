//! 审计导出：`events` / `changes` → CSV 或 JSON。
//!
//! **不导出变量值**：`events` 本就没有值列；`changes.old_value/new_value` 是
//! 同步载荷，敏感变量的值在采集阶段已为空字符串。

use super::history::{EventFilter, EVENT_WHERE};
use super::{db_err, Db};
use crate::domain::error::{CoreError, ErrorCode};
use crate::platform::fs::csv_escape;
use rusqlite::params;

/// 单次导出的行数上限。审计导出是「留档」而不是「全库转储」，
/// 超过上限请用时间范围分段；避免一次把数年事件读进内存。
const EXPORT_MAX: u32 = 10_000;

/// 校验导出格式并返回规范化名。
fn validate_format(format: &str) -> Result<&'static str, CoreError> {
    match format {
        "csv" => Ok("csv"),
        "json" => Ok("json"),
        other => Err(CoreError::new(
            ErrorCode::InvalidValue,
            "export",
            format!("不支持的导出格式: {other}（可选 csv / json）"),
        )),
    }
}

/// 导出事件。
///
/// 采用与 [`super::history::list_events`] **同一份 WHERE 条件**（[`EVENT_WHERE`]），
/// 保证「列表筛选」与「导出筛选」一致。按 `created_at DESC` 排序，最多 1 万条。
pub fn export_events(db: &Db, filter: &EventFilter, format: &str) -> Result<String, CoreError> {
    let format = validate_format(format)?;
    let mut stmt = db.prepare(&format!(
        "SELECT e.uid, e.created_at, e.source, e.action, e.hive, e.target,
                e.changed_fields, e.before_revision, e.after_revision, e.result,
                e.error_code, e.device_id, s.uid
           FROM events e LEFT JOIN snapshots s ON s.id = e.snapshot_id
          {EVENT_WHERE}
          ORDER BY e.created_at DESC, e.id DESC
          LIMIT ?8"
    ))?;

    let rows = stmt
        .query_map(
            params![
                filter.action,
                filter.source,
                filter.hive,
                filter.target,
                filter.result,
                filter.from_ms,
                filter.to_ms,
                EXPORT_MAX as i64
            ],
            |r| {
                Ok(EventExportRow {
                    uid: r.get(0)?,
                    created_at: r.get(1)?,
                    source: r.get(2)?,
                    action: r.get(3)?,
                    hive: r.get(4)?,
                    target: r.get(5)?,
                    changed_fields: r.get(6)?,
                    before_revision: r.get(7)?,
                    after_revision: r.get(8)?,
                    result: r.get(9)?,
                    error_code: r.get(10)?,
                    device_id: r.get(11)?,
                    snapshot_uid: r.get(12)?,
                })
            },
        )
        .map_err(|e| db_err("export_events", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("export_events", e))?;

    if format == "json" {
        serde_json::to_string_pretty(&rows)
            .map_err(|e| CoreError::new(ErrorCode::Internal, "export_events", e.to_string()))
    } else {
        let mut out = String::from(
            "uid,createdAt,source,action,hive,target,changedFields,beforeRevision,afterRevision,result,errorCode,deviceId,snapshotUid\n",
        );
        for row in &rows {
            out.push_str(
                &[
                    row.uid.as_str(),
                    &row.created_at.to_string(),
                    row.source.as_str(),
                    row.action.as_str(),
                    row.hive.as_deref().unwrap_or(""),
                    row.target.as_deref().unwrap_or(""),
                    row.changed_fields.as_str(),
                    row.before_revision.as_deref().unwrap_or(""),
                    row.after_revision.as_deref().unwrap_or(""),
                    row.result.as_str(),
                    row.error_code.as_deref().unwrap_or(""),
                    row.device_id.as_str(),
                    row.snapshot_uid.as_deref().unwrap_or(""),
                ]
                .iter()
                .map(|v| csv_escape(v))
                .collect::<Vec<_>>()
                .join(","),
            );
            out.push('\n');
        }
        Ok(out)
    }
}

/// 导出一条事件的可导出字段（形状与 `EventView` 对齐，额外含审计列）。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EventExportRow {
    uid: String,
    created_at: i64,
    source: String,
    action: String,
    hive: Option<String>,
    target: Option<String>,
    changed_fields: String,
    before_revision: Option<String>,
    after_revision: Option<String>,
    result: String,
    error_code: Option<String>,
    device_id: String,
    snapshot_uid: Option<String>,
}

/// 导出变更日志。
///
/// `exported_only = true` 时只导已导出过的行（用于对端排查）；
/// `false` 时导出全部，包含待导出队列。
pub fn export_changes(db: &Db, exported_only: bool, format: &str) -> Result<String, CoreError> {
    let format = validate_format(format)?;
    let mut stmt = db.prepare(
        "SELECT uid, device_id, seq, lamport, origin, entity_type, entity_uid,
                field, old_value, new_value, created_at, applied_at, exported_at
           FROM changes
          WHERE (?1 = 0 OR exported_at IS NOT NULL)
          ORDER BY id DESC
          LIMIT ?2",
    )?;

    let rows = stmt
        .query_map(params![i64::from(exported_only), EXPORT_MAX as i64], |r| {
            Ok(ChangeExportRow {
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
        .map_err(|e| db_err("export_changes", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("export_changes", e))?;

    if format == "json" {
        serde_json::to_string_pretty(&rows)
            .map_err(|e| CoreError::new(ErrorCode::Internal, "export_changes", e.to_string()))
    } else {
        let mut out = String::from(
            "uid,deviceId,seq,lamport,origin,entityType,entityUid,field,oldValue,newValue,createdAt,appliedAt,exportedAt\n",
        );
        for row in &rows {
            out.push_str(
                &[
                    row.uid.as_str(),
                    row.device_id.as_str(),
                    &row.seq.to_string(),
                    &row.lamport.to_string(),
                    row.origin.as_str(),
                    row.entity_type.as_str(),
                    row.entity_uid.as_str(),
                    row.field.as_deref().unwrap_or(""),
                    row.old_value.as_deref().unwrap_or(""),
                    row.new_value.as_deref().unwrap_or(""),
                    &row.created_at.to_string(),
                    &row.applied_at.to_string(),
                    &row.exported_at.map(|v| v.to_string()).unwrap_or_default(),
                ]
                .iter()
                .map(|v| csv_escape(v))
                .collect::<Vec<_>>()
                .join(","),
            );
            out.push('\n');
        }
        Ok(out)
    }
}

/// 一条变更的可导出字段。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ChangeExportRow {
    uid: String,
    device_id: String,
    seq: i64,
    lamport: i64,
    origin: String,
    entity_type: String,
    entity_uid: String,
    field: Option<String>,
    old_value: Option<String>,
    new_value: Option<String>,
    created_at: i64,
    applied_at: i64,
    exported_at: Option<i64>,
}
