//! FTS5 全文检索。
//!
//! **索引范围**：PATH 条目串、env 变量名、配置名/描述/标签。
//! **绝不索引 env 变量的值** —— 明文值只存在于 `env_values.value`，
//! 由 SQLCipher 保护，且不进任何派生表。
//!
//! **派生表**：`search_fts` 可从 `path_entries` / `env_values.name` /
//! `profiles` 完整重建，见 [`rebuild_index`]。
//!
//! **查询转义**：
//!
//! FTS5 的 `MATCH` 把右边的值当**查询语法**解析。用户直接输入 `C:\Program Files`
//! 会被拆成两个 token 做隐式 AND，而输入一个裸 `"` 会直接报
//! `fts5: syntax error near """`。
//!
//! 本模块**在 core 侧**做转义，把整个查询包成 phrase（`"..."`，内部双引号翻倍）。
//! 选 core 而非 GUI 的理由：CLI 与未来的任何调用方都走同一个 `search()`，
//! 把转义放在调用方等于要求每个调用方各实现一遍，且忘掉的那一个不会被测试抓到。
//! 代价是失去 FTS5 的高级语法（`OR`、`NEAR`、前缀 `*`）——搜索框的用户输入
//! 本来也不该被当语法解析。

use super::Db;
use crate::error::CoreError;
use rusqlite::{params, Transaction};

/// 一条搜索命中。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    /// 来源类型：`path` / `envName` / `profile`
    pub kind: String,
    /// 指向的对象 uid（快照 uid / 变量名 / 配置 uid）
    pub ref_uid: String,
    /// 完整索引内容（路径串 / 变量名 / 配置名+描述+标签），供前端定位与复制。
    /// 不含环境变量的值。
    pub content: String,
    /// 高亮片段（FTS5 `snippet()`）
    pub snippet: String,
    /// 排名（越小越相关）
    pub rank: f64,
}

/// `search_fts` 当前行数；启动引导据此判断是否需要首次全量构建。
pub fn index_count(db: &Db) -> Result<i64, CoreError> {
    db.query_row_with("SELECT count(*) FROM search_fts", [], |r| r.get(0))
}

/// 全量重建索引，返回写入的行数。
///
/// 派生表纪律：本函数必须能从 `path_entries` /
/// `env_values` / `profiles` 完整重建 `search_fts`，不依赖任何其他派生表。
pub fn rebuild_index(db: &mut Db) -> Result<usize, CoreError> {
    db.with_transaction(|tx| {
        tx.execute("DELETE FROM search_fts", [])
            .map_err(|e| super::db_err("clear_fts", e))?;
        let n = 0usize;

        Ok(n + index_all_tx(tx)?)
    })
}

/// 事务内重建全部索引（供 [`rebuild_index`] 与测试复用）。
fn index_all_tx(tx: &Transaction) -> Result<usize, CoreError> {
    let mut n = 0usize;

    // PATH 条目
    let mut stmt = tx
        .prepare(
            "SELECT s.uid, p.hive, p.path FROM path_entries p
               JOIN snapshots s ON s.id = p.snapshot_id
              WHERE s.deleted_at IS NULL",
        )
        .map_err(|e| super::db_err("fts_paths", e))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| super::db_err("fts_paths", e))?;
    for row in rows {
        let (uid, hive, path) = row.map_err(|e| super::db_err("fts_paths", e))?;
        // ref_uid 带 hive 前缀，便于前端定位到「哪一侧的哪条」
        insert_tx(tx, &path, "path", &format!("{uid}#{hive}"))?;
        n += 1;
    }
    drop(stmt);

    // env 变量名（**不索引 value**）
    let mut stmt = tx
        .prepare(
            "SELECT s.uid, e.hive, e.name FROM env_values e
               JOIN snapshots s ON s.id = e.snapshot_id
              WHERE s.deleted_at IS NULL",
        )
        .map_err(|e| super::db_err("fts_env", e))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| super::db_err("fts_env", e))?;
    for row in rows {
        let (uid, hive, name) = row.map_err(|e| super::db_err("fts_env", e))?;
        insert_tx(tx, &name, "envName", &format!("{uid}#{hive}#{name}"))?;
        n += 1;
    }
    drop(stmt);

    // 配置名 / 描述 / 标签
    let mut stmt = tx
        .prepare(
            "SELECT uid, name, COALESCE(description,''), COALESCE(tags,'')
               FROM profiles WHERE deleted_at IS NULL",
        )
        .map_err(|e| super::db_err("fts_profiles", e))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| super::db_err("fts_profiles", e))?;
    for row in rows {
        let (uid, name, desc, tags) = row.map_err(|e| super::db_err("fts_profiles", e))?;
        let content = format!("{name} {desc} {tags}");
        insert_tx(tx, &content, "profile", &uid)?;
        n += 1;
    }

    Ok(n)
}

/// 事务内写入一行 FTS。供 [`rebuild_index`] 与增量写入复用。
fn insert_tx(tx: &Transaction, content: &str, kind: &str, ref_uid: &str) -> Result<(), CoreError> {
    tx.execute(
        "INSERT INTO search_fts (content, kind, ref_uid) VALUES (?1, ?2, ?3)",
        params![content, kind, ref_uid],
    )
    .map_err(|e| super::db_err("fts_insert", e))?;
    Ok(())
}

/// 增量索引**一份快照**。
///
/// 与 [`rebuild_index`] 的区别：只处理指定的 `snapshot_id`，不清空全表。
/// 供写入路径在同一事务内调用（ 第 5 条：派生表可随时重建，
/// 增量只是性能优化）。
///
/// **幂等**：先删该快照的所有旧行再重插，重复调用不会产生重复行。
/// `ref_uid` 的格式是 `<snapshot_uid>#<hive>...`，因此用 `uid` 与 `uid#%` 两个条件覆盖。
///
/// **不索引 env 的值**：只取 `env_values.name`。
pub fn index_snapshot_tx(tx: &Transaction, snapshot_id: i64) -> Result<(), CoreError> {
    // 先取 uid，用于清理旧行（ref_uid 的格式是 `<snapshot_uid>#...`）
    let uid: String = tx
        .query_row(
            "SELECT uid FROM snapshots WHERE id = ?1",
            params![snapshot_id],
            |r| r.get(0),
        )
        .map_err(|e| super::db_err("index_snapshot_uid", e))?;

    // 幂等：先删该快照的所有旧索引行（profile 类 ref_uid 是裸 uid，也一并处理）
    tx.execute(
        "DELETE FROM search_fts WHERE ref_uid = ?1 OR ref_uid LIKE ?2",
        params![uid, format!("{uid}#%")],
    )
    .map_err(|e| super::db_err("index_snapshot_clear", e))?;

    // PATH 条目
    let mut stmt = tx
        .prepare("SELECT hive, path FROM path_entries WHERE snapshot_id = ?1")
        .map_err(|e| super::db_err("index_snapshot_paths", e))?;
    let rows = stmt
        .query_map(params![snapshot_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| super::db_err("index_snapshot_paths", e))?;
    for row in rows {
        let (hive, path) = row.map_err(|e| super::db_err("index_snapshot_paths", e))?;
        insert_tx(tx, &path, "path", &format!("{uid}#{hive}"))?;
    }
    drop(stmt);

    // env 变量名（**不索引 value**）
    let mut stmt = tx
        .prepare("SELECT hive, name FROM env_values WHERE snapshot_id = ?1")
        .map_err(|e| super::db_err("index_snapshot_env", e))?;
    let rows = stmt
        .query_map(params![snapshot_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| super::db_err("index_snapshot_env", e))?;
    for row in rows {
        let (hive, name) = row.map_err(|e| super::db_err("index_snapshot_env", e))?;
        insert_tx(tx, &name, "envName", &format!("{uid}#{hive}#{name}"))?;
    }

    Ok(())
}

/// 把用户输入转成 FTS5 的 phrase 查询。
///
/// 整个查询包成一对双引号，内部的双引号按 FTS5 规则翻倍。这样：
/// - `C:\Program Files` → 一个 phrase，按原文匹配，不会被拆成 AND；
/// - `"` → 外层一对引号 + 内部 `""` 转义，展开为 `""""`：一个**内容为一个双引号字符**
///   的 phrase（不是空 phrase），语法合法，不报 `unterminated string`；
/// - `a OR b` → 按字面量 `a OR b` 匹配，不触发布尔语法。
///
/// 为什么是这个写法而不是 `""` 空 phrase：空 phrase 匹配不到任何行，
/// 而用户输入一个引号时，期望的是「搜含引号的内容」而不是「搜不到任何东西」。
fn escape_query(q: &str) -> String {
    format!("\"{}\"", q.replace('"', "\"\""))
}

/// 搜索。空查询（含纯空白）返回空结果——**不得返回全量**。
///
/// 上限 200 条（`limit` 会被 clamp）。
pub fn search(db: &Db, query: &str, limit: u32) -> Result<Vec<SearchHit>, CoreError> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 200) as i64;

    let mut stmt = db.prepare(
        "SELECT kind, ref_uid, content,
                    snippet(search_fts, 0, '[', ']', '…', 12),
                    rank
               FROM search_fts
              WHERE search_fts MATCH ?1
              ORDER BY rank
              LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![escape_query(q), limit], |r| {
            Ok(SearchHit {
                kind: r.get(0)?,
                ref_uid: r.get(1)?,
                content: r.get(2)?,
                snippet: r.get(3)?,
                rank: r.get(4)?,
            })
        })
        .map_err(|e| super::db_err("fts_query", e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("fts_query", e))
}
