//! `%VAR%` 引用图。
//!
//! **只解析名字，不解析值**：`var_references` 存的是「A 引用了 B」这条边，
//! 边由 `%NAME%` 语法得出，与变量值无关。因此本模块的派生表不含任何明文。
//!
//! **不变量**：本表**只保存当前快照的边**，由「每次全量清空 + 只插当前快照」
//! 保证。因此 [`dependents`] / [`dependencies`] / [`find_cycles`] 可以省略
//! `snapshot_id` 过滤。
//!
//! ⚠️ 若将来改为增量（只删当前快照的边），**必须同时**给那三条查询补上
//! `WHERE snapshot_id = (SELECT current_snapshot_id FROM app_state WHERE id = 1)`，
//! 否则会静默混入历史快照的边——而且不会报错，只会给出「看起来合理」的错答案。

use super::Db;
use crate::domain::error::CoreError;
use rusqlite::params;

/// 一条引用边。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefEdge {
    /// 引用方 hive
    pub from_hive: String,
    /// 引用方名字（PATH 用字面量 `PATH`）
    pub from_name: String,
    /// 被引用方名字（统一大写）
    pub to_name: String,
    /// 距离（`dependents` 恒为 1；`dependencies` 为递归层数）
    pub depth: i64,
}

/// 从文本中抽取 `%NAME%`，返回**大写**名字（去重、保序）。
///
/// 合法的变量名字符：字母、数字、下划线。其余（含连字符、空格、反斜杠）
/// 视为不构成引用——`%A-B%` 不产生边，因为它不是合法的 Windows 变量名。
///
/// 相邻引用 `%A%%B%` 两个都能抽出：匹配成功后从**闭合的 `%`** 继续扫描，
/// 而它同时是下一个引用的开引号。不合法时只前进 1 个字符，
/// 保证 `%` 不会同时被当开引号与闭引号重复消费。
pub fn parse_refs(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(end_rel) = text[i + 1..].find('%') {
                let name = &text[i + 1..i + 1 + end_rel];
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    let up = name.to_ascii_uppercase();
                    if !out.contains(&up) {
                        out.push(up);
                    }
                    i = i + 1 + end_rel + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// 全量重建 `var_references`，返回写入的边数。
///
/// 只处理 `app_state.current_snapshot_id` 指向的快照（引用图是「现在」的形状，
/// 不是历史）。无当前快照时清空并返回 0。
pub fn rebuild_refs(db: &mut Db) -> Result<usize, CoreError> {
    db.with_transaction(|tx| {
        tx.execute("DELETE FROM var_references", [])
            .map_err(|e| super::db_err("clear_refs", e))?;

        let sid: Option<i64> = tx
            .query_row(
                "SELECT current_snapshot_id FROM app_state WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        let Some(sid) = sid else { return Ok(0) };

        let mut n = 0usize;

        // PATH 条目：from_name 统一记为 `PATH`，按 hive 区分
        let mut stmt = tx
            .prepare("SELECT hive, path FROM path_entries WHERE snapshot_id = ?1")
            .map_err(|e| super::db_err("refs_paths", e))?;
        let rows = stmt
            .query_map(params![sid], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| super::db_err("refs_paths", e))?;
        for row in rows {
            let (hive, path) = row.map_err(|e| super::db_err("refs_paths", e))?;
            for to in parse_refs(&path) {
                n += tx
                    .execute(
                        "INSERT OR IGNORE INTO var_references
                           (snapshot_id, from_hive, from_name, to_name)
                         VALUES (?1, ?2, 'PATH', ?3)",
                        params![sid, hive, to],
                    )
                    .map_err(|e| super::db_err("refs_insert", e))?;
            }
        }
        drop(stmt);

        // env 变量的值里也可能有 %VAR%（只取名字，不把值写进任何派生表）
        let mut stmt = tx
            .prepare("SELECT hive, name, COALESCE(value,'') FROM env_values WHERE snapshot_id = ?1")
            .map_err(|e| super::db_err("refs_env", e))?;
        let rows = stmt
            .query_map(params![sid], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| super::db_err("refs_env", e))?;
        for row in rows {
            let (hive, name, value) = row.map_err(|e| super::db_err("refs_env", e))?;
            for to in parse_refs(&value) {
                if to.eq_ignore_ascii_case(&name) {
                    continue; // 自引用不算边
                }
                n += tx
                    .execute(
                        "INSERT OR IGNORE INTO var_references
                           (snapshot_id, from_hive, from_name, to_name)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![sid, hive, name, to],
                    )
                    .map_err(|e| super::db_err("refs_insert", e))?;
            }
        }

        Ok(n)
    })
}

/// 谁引用了 `name`（反向边，深度恒为 1）。
pub fn dependents(db: &Db, name: &str) -> Result<Vec<RefEdge>, CoreError> {
    let mut stmt = db.prepare(
        "SELECT from_hive, from_name, to_name FROM var_references
          WHERE to_name = ?1 ORDER BY from_hive, from_name",
    )?;
    let rows = stmt
        .query_map(params![name.to_ascii_uppercase()], |r| {
            Ok(RefEdge {
                from_hive: r.get(0)?,
                from_name: r.get(1)?,
                to_name: r.get(2)?,
                depth: 1,
            })
        })
        .map_err(|e| super::db_err("dependents", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("dependents", e))?;
    Ok(rows)
}

/// `name` 传递依赖了谁。
///
/// 深度上限 10：防止环导致无限递归。环由 [`find_cycles`] 单独报告。
pub fn dependencies(db: &Db, name: &str) -> Result<Vec<RefEdge>, CoreError> {
    let mut stmt = db.prepare(
        "WITH RECURSIVE dep(from_hive, from_name, to_name, depth) AS (
           SELECT from_hive, from_name, to_name, 1
             FROM var_references WHERE from_name = ?1
           UNION
           SELECT v.from_hive, v.from_name, v.to_name, d.depth + 1
             FROM var_references v JOIN dep d ON v.from_name = d.to_name
            WHERE d.depth < 10
         )
         SELECT from_hive, from_name, to_name, depth FROM dep
          ORDER BY depth, from_name, to_name",
    )?;
    let rows = stmt
        .query_map(params![name.to_ascii_uppercase()], |r| {
            Ok(RefEdge {
                from_hive: r.get(0)?,
                from_name: r.get(1)?,
                to_name: r.get(2)?,
                depth: r.get(3)?,
            })
        })
        .map_err(|e| super::db_err("dependencies", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("dependencies", e))?;
    Ok(rows)
}

/// 找出引用环。无环返回空。
///
/// 判据：任一节点能从自己出发经递归回到自己。
///
/// 返回每条环的节点序列（首尾**不重复**，例如 `["A", "B"]` 表示 A→B→A）。
/// 同一对节点的正反两条路径会被归一化为同一个环，避免 `A>B>A` 与 `B>A>B` 重复出现。
pub fn find_cycles(db: &Db) -> Result<Vec<Vec<String>>, CoreError> {
    let mut stmt = db.prepare(
        "WITH RECURSIVE walk(start, cur, path, depth) AS (
           SELECT from_name, to_name, from_name || '>' || to_name, 1
             FROM var_references
           UNION ALL
           SELECT w.start, v.to_name, w.path || '>' || v.to_name, w.depth + 1
             FROM walk w JOIN var_references v ON v.from_name = w.cur
            WHERE w.depth < 10
         )
         SELECT path FROM walk WHERE cur = start",
    )?;
    let raw = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| super::db_err("find_cycles", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("find_cycles", e))?;

    // 归一化：`A>B>A` 的节点序列是 [A, B, A]，去掉末尾重复的首节点得到 [A, B]。
    // 环的旋转视为同一个环（A,B 与 B,A），按字典序取最小旋转作为键去重。
    let mut seen: Vec<Vec<String>> = Vec::new();
    for p in raw {
        let mut nodes: Vec<String> = p.split('>').map(|s| s.to_string()).collect();
        // 首尾相同：去掉末尾那个（它是回到起点的重复）
        if nodes.len() > 1 && nodes.first() == nodes.last() {
            nodes.pop();
        }
        if nodes.is_empty() {
            continue;
        }
        let key = canonical_rotation(&nodes);
        if !seen.contains(&key) {
            seen.push(key);
        }
    }
    Ok(seen)
}

/// 把环旋转到字典序最小的起点，用于去重（`A,B` 与 `B,A` 是同一个环）。
fn canonical_rotation(nodes: &[String]) -> Vec<String> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let n = nodes.len();
    let mut best: Option<Vec<String>> = None;
    for shift in 0..n {
        let rotated: Vec<String> = (0..n).map(|i| nodes[(i + shift) % n].clone()).collect();
        if best.as_ref().is_none_or(|b| &rotated < b) {
            best = Some(rotated);
        }
    }
    best.unwrap_or_else(|| nodes.to_vec())
}
