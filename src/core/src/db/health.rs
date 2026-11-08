//! 数据库自检：文件体积、schema、完整性、行数与最近清理时间。
//!
//! 只读；任何查询失败都原样上报（不吞错），调用方决定如何呈现。

use super::{crypto, maintenance, Db};
use crate::domain::error::CoreError;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// 单表行数。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRows {
    /// 表名
    pub name: String,
    /// 行数
    pub rows: i64,
}

/// 健康快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbHealth {
    /// 数据库文件路径
    pub path: String,
    /// 主文件字节数
    pub file_bytes: u64,
    /// WAL 字节数（不存在时为 0）
    pub wal_bytes: u64,
    /// SHM 字节数（不存在时为 0）
    pub shm_bytes: u64,
    /// 当前 schema 版本
    pub schema_version: u32,
    /// 程序支持的 schema 版本
    pub expected_schema_version: u32,
    /// `PRAGMA integrity_check(1)` 的结果（`ok` 或第一条错误）
    pub integrity: String,
    /// `integrity == "ok"`
    pub integrity_ok: bool,
    /// 关键表行数
    pub table_rows: Vec<TableRows>,
    /// 最近一次保留策略清理时间（Unix 毫秒）
    pub last_prune_at: Option<i64>,
}

/// 参与行数统计的表（顺序即展示顺序）。
const COUNTED_TABLES: &[&str] = &[
    "snapshots",
    "path_entries",
    "env_values",
    "events",
    "changes",
    "sync_conflicts",
    "scan_runs",
    "profiles",
    "backups",
];

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("patheditor.db");
    path.with_file_name(format!("{name}{suffix}"))
}

fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// 读取数据库健康状态。
pub fn health(db: &Db) -> Result<DbHealth, CoreError> {
    let path = db.path();
    let integrity: String = db.query_row_with("PRAGMA integrity_check(1)", [], |r| r.get(0))?;

    let mut table_rows = Vec::with_capacity(COUNTED_TABLES.len());
    for name in COUNTED_TABLES {
        // 表名来自常量白名单，不参与用户输入拼接
        let rows: i64 =
            db.query_row_with(&format!("SELECT COUNT(*) FROM \"{name}\""), [], |r| {
                r.get(0)
            })?;
        table_rows.push(TableRows {
            name: (*name).to_string(),
            rows,
        });
    }

    let last_prune_at = maintenance::last_prune_time(&crypto::data_home());
    Ok(DbHealth {
        path: path.display().to_string(),
        file_bytes: file_len(path),
        wal_bytes: file_len(&sidecar(path, "-wal")),
        shm_bytes: file_len(&sidecar(path, "-shm")),
        schema_version: db.schema_version()?,
        expected_schema_version: super::schema::CURRENT_SCHEMA_VERSION,
        integrity_ok: integrity == "ok",
        integrity,
        table_rows,
        last_prune_at,
    })
}
