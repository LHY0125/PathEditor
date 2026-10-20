//! 冲突历史（spec §6.2）：`scan_runs` 持久化、两次扫描 diff、首次/最近出现。
//!
//! **`scan_runs` 不是派生表**：它是真实记录（每次扫描当时看到了什么），
//! 无法从别的表重建。`daily_stats.conflicts` 才是从它派生出来的。

use super::{stats, Db};
use crate::error::CoreError;
use rusqlite::params;
use uuid::Uuid;

/// 一次扫描的视图。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRunView {
    /// 扫描 uid
    pub uid: String,
    /// 扫描时间（Unix 毫秒）
    pub created_at: i64,
    /// 冲突路径（解析后的列表，不是原始 JSON）
    pub conflicts: Vec<String>,
    /// 工具目录数（解析后的计数，不是原始 JSON）
    pub tool_count: usize,
}

/// 「首次出现」的一条（N-5）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstSeen {
    /// 冲突路径
    pub conflict: String,
    /// 第一条包含它的 run uid
    pub run_uid: String,
    /// 该 run 的时间
    pub seen_at: i64,
}

/// 「最近出现」的一条（M-5）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastSeen {
    /// 冲突路径
    pub conflict: String,
    /// 最后一条包含它的 run uid
    pub run_uid: String,
    /// 该 run 的时间
    pub seen_at: i64,
}

/// 两次扫描的差异。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRunDiff {
    /// 本次有、上次没有
    pub appeared: Vec<String>,
    /// 上次有、本次没有
    pub disappeared: Vec<String>,
    /// 两次都有
    pub persistent: Vec<String>,
    /// 「最近出现」（M-5）：**全部**冲突在历史里最后一次出现的 run
    pub last_seen: Vec<LastSeen>,
    /// 「首次出现」（N-5）：**全部**冲突在历史里第一次出现的 run
    pub first_seen: Vec<FirstSeen>,
}

/// 记录一次扫描，返回其 uid。
///
/// **同时触发 `daily_stats` 重建**：冲突频率依赖 `scan_runs`（M-6）。
///
/// ⚠️ 事务纪律：本函数先用 `with_transaction` 写完并提交，**返回之后**才调
/// [`stats::rebuild_daily`]。两者不能嵌套——SQLite 不支持嵌套事务，
/// 在闭包内部再调 `with_transaction` 会 panic（rusqlite 的行为）。
pub fn record_run(
    db: &mut Db,
    conflicts_json: &str,
    tools_json: &str,
) -> Result<String, CoreError> {
    let uid = Uuid::now_v7().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    db.with_transaction(|tx| {
        tx.execute(
            "INSERT INTO scan_runs (uid, created_at, conflicts_json, tools_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![uid, now, conflicts_json, tools_json],
        )
        .map_err(|e| super::db_err("record_run", e))?;
        Ok(())
    })?;

    // 事务已提交，现在可以安全地开新事务重建派生表。
    // 失败不阻断 record_run：扫描记录本身已经落库，统计下次启动会重算。
    if let Err(e) = stats::rebuild_daily(db) {
        log::warn!(
            "record_run 后重建 daily_stats 失败（不影响扫描记录）: {}",
            e.message
        );
    }

    Ok(uid)
}

/// 列出最近的扫描，按 `created_at DESC, id DESC`。
pub fn list_runs(db: &Db, limit: u32) -> Result<Vec<ScanRunView>, CoreError> {
    let limit = limit.clamp(1, 200) as i64;
    let mut stmt = db.prepare(
        "SELECT uid, created_at, conflicts_json, tools_json FROM scan_runs
          ORDER BY created_at DESC, id DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |r| {
            let conflicts_json: String = r.get(2)?;
            let tools_json: String = r.get(3)?;
            Ok(ScanRunView {
                uid: r.get(0)?,
                created_at: r.get(1)?,
                conflicts: parse_string_list(&conflicts_json),
                tool_count: parse_string_list(&tools_json).len(),
            })
        })
        .map_err(|e| super::db_err("list_runs", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("list_runs", e))?;
    Ok(rows)
}

/// 比较两次扫描。
///
/// `a` 是「上次」、`b` 是「本次」。`appeared` 表示本次新出现。
pub fn diff_runs(db: &Db, a: &str, b: &str) -> Result<ScanRunDiff, CoreError> {
    let conflicts_a = load_conflicts(db, a)?;
    let conflicts_b = load_conflicts(db, b)?;

    let mut appeared: Vec<String> = conflicts_b
        .iter()
        .filter(|c| !conflicts_a.contains(c))
        .cloned()
        .collect();
    let mut disappeared: Vec<String> = conflicts_a
        .iter()
        .filter(|c| !conflicts_b.contains(c))
        .cloned()
        .collect();
    let mut persistent: Vec<String> = conflicts_b
        .iter()
        .filter(|c| conflicts_a.contains(c))
        .cloned()
        .collect();

    appeared.sort();
    appeared.dedup();
    disappeared.sort();
    disappeared.dedup();
    persistent.sort();
    persistent.dedup();

    // 「首次出现」与「最近出现」基于**全历史**，不只这两次——
    // spec §6.2 说的是冲突在历史中的首末，与本次是否 persistent 无关。
    let history = load_history(db)?;
    let (first_seen, last_seen) = first_and_last_seen(&history);

    Ok(ScanRunDiff {
        appeared,
        disappeared,
        persistent,
        last_seen,
        first_seen,
    })
}

/// 全部扫描的历史（正序：最旧在前）。
fn load_history(db: &Db) -> Result<Vec<(String, i64, Vec<String>)>, CoreError> {
    let mut stmt = db
        .prepare("SELECT uid, created_at, conflicts_json FROM scan_runs ORDER BY created_at, id")?;
    let rows = stmt
        .query_map([], |r| {
            let json: String = r.get(2)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                parse_string_list(&json),
            ))
        })
        .map_err(|e| super::db_err("load_history", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("load_history", e))?;
    Ok(rows)
}

/// 从正序历史里算出每个冲突的首次与最近出现。
fn first_and_last_seen(history: &[(String, i64, Vec<String>)]) -> (Vec<FirstSeen>, Vec<LastSeen>) {
    use std::collections::BTreeMap;
    // BTreeMap 保证输出顺序稳定（按冲突路径字典序），避免测试与 UI 抖动
    let mut first: BTreeMap<&str, (&str, i64)> = BTreeMap::new();
    let mut last: BTreeMap<&str, (&str, i64)> = BTreeMap::new();

    for (uid, at, conflicts) in history {
        for c in conflicts {
            // 只在第一次见到时插入 → 得到首次
            first.entry(c.as_str()).or_insert((uid.as_str(), *at));
            // 每次覆盖 → 循环结束后是最后一次
            last.insert(c.as_str(), (uid.as_str(), *at));
        }
    }

    let first_seen = first
        .into_iter()
        .map(|(conflict, (run_uid, seen_at))| FirstSeen {
            conflict: conflict.to_string(),
            run_uid: run_uid.to_string(),
            seen_at,
        })
        .collect();
    let last_seen = last
        .into_iter()
        .map(|(conflict, (run_uid, seen_at))| LastSeen {
            conflict: conflict.to_string(),
            run_uid: run_uid.to_string(),
            seen_at,
        })
        .collect();
    (first_seen, last_seen)
}

/// 读取一次扫描的冲突列表。
fn load_conflicts(db: &Db, uid: &str) -> Result<Vec<String>, CoreError> {
    let json: String = db
        .query_row_with(
            "SELECT conflicts_json FROM scan_runs WHERE uid = ?1",
            params![uid],
            |r| r.get(0),
        )
        .map_err(|e| {
            if e.code == crate::error::ErrorCode::Database {
                CoreError::new(
                    crate::error::ErrorCode::NotFound,
                    "load_conflicts",
                    format!("扫描记录不存在: {uid}"),
                )
            } else {
                e
            }
        })?;
    Ok(parse_string_list(&json))
}

/// 解析 JSON 字符串数组。损坏时返回空列表并记警告——历史数据不该阻断 UI。
fn parse_string_list(json: &str) -> Vec<String> {
    match serde_json::from_str::<Vec<String>>(json) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("scan_runs 的 JSON 解析失败（按空列表处理）: {e}");
            Vec::new()
        }
    }
}
