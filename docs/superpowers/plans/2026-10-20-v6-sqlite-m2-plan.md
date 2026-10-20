# PathEditor v6.0.0 M2 检索与统计 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 M1 的加密存储之上补齐 Tier 2 能力：FTS5 全文检索、统计仪表盘、N 路快照对比、`%VAR%` 引用图、冲突历史持久化，并清偿 M1 移交给 M2 的 4 项欠账。

**Architecture:** 新增 `core/src/db/{search,stats,refs,scan_history}.rs` 四个派生数据模块 + `schema.rs` 升到 v2（新增 4 张表 + 1 张 FTS5 虚表）。派生表可随时重建，不是真相源（spec §4.2 第 5 条）。GUI 侧新增「统计」标签页与搜索框，沿用 M1 的细粒度 IPC + `backend.ts` 形状校验。

**Tech Stack:** 同 M1（`rusqlite 0.40` + SQLCipher、Tauri 2、React 19 + Zustand）。M2 **不引入新依赖**。

**Spec:** `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`（§4.1 数据模型、§6.2 Tier 2、§3.2.1 IPC 边界、§十三 M2 验证门）

**前置：**

- `docs/superpowers/plans/2026-10-19-v6-sqlite-m1-plan.md`（M1 已完成并合并）
- `docs/审核和开发/2026.10.20/PathEditor-v6-M1-缺口清单.md`（M2 接收的 4 项欠账）
- `docs/审核和开发/2026.10.19/PathEditor-v6-M0-探针结果.md`（FTS5/JSON1 已双平台验证；一文件一测试约束）

---

## Global Constraints

- **一文件一测试**：任何使用 SQLCipher 的 `.rs` 测试文件**只放一个 `#[test]`**。同一进程内第二个 SQLCipher 测试会 `STATUS_STACK_OVERFLOW`（GNU/MSVC 均已复现，`--test-threads=1` 无效）。见 M0 结果 §5.3。
- **`config_dir()` 无法被环境变量重定向**：`dirs::home_dir()` 在 Windows 上走 `SHGetKnownFolderPath`，`HOME` 与 `USERPROFILE` **都不读**。所有测试必须**显式传 `home: &Path` 参数**（`tempfile::TempDir`），不得依赖 `set_var`。M1 已因此出过一次写坏用户真实 `disabled.json` 的事故。
- **派生表不是真相**（spec §4.2 第 5 条）：`search_fts` / `daily_stats` / `var_references` 必须能从 `snapshots` / `events` / `path_entries` **完整重建**。每个派生模块都要有一个 `rebuild_*` 函数与对应的重建测试。
- **不写真实注册表**：所有测试用 fake IO 或 `TempDir`。
- **敏感值**：`search_fts` 的 `content` **绝不索引 env 变量的值**，只索引路径串、变量名、配置描述/标签（spec §6.2）。任何返回 `SnapshotView` / 对比结果的命令默认掩码。
- **不提交、不推送**，除非用户明确授权。
- 中文注释；Rust 4 空格；TS 2 空格、单引号、100 列。
- 质量门：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`npm run verify:all`。
- CLI 单测必须 `cargo test -p patheditor-cli --bins`。
- 所有 `pub fn` / `pub(crate)` 中文 `///` 注释。
- **schema 版本**：M2 引入 `CURRENT_SCHEMA_VERSION = 2`。迁移必须**幂等**且**向前兼容**（v1 库升级到 v2 不丢数据）。

---

## File Structure

| 文件                                       | 责任                                                          |
| ------------------------------------------ | ------------------------------------------------------------- |
| `core/src/db/schema.rs`                    | `DDL_V2`（4 表 + 1 虚表）+ 版本分支                           |
| `core/src/db/mod.rs`                       | `migrate_to_current` 增加 `current < 2` 分支                  |
| `core/src/db/search.rs`                    | FTS5 建表/重建/查询/高亮                                      |
| `core/src/db/stats.rs`                     | `daily_stats` 聚合与重建                                      |
| `core/src/db/refs.rs`                      | `%VAR%` 解析 + `var_references` + 递归 CTE                    |
| `core/src/db/scan_history.rs`              | `scan_runs` 持久化与 diff                                     |
| `core/src/db/compare.rs`                   | N 路快照对比（纯函数 + DB 读取）                              |
| `core/src/db/v5_export.rs`                 | `export_profile_v5`（M1 欠账 P2-2）                           |
| `gui/src/commands/history.rs`              | 新增 `history_reveal_env_value`（M1 欠账 P2-1）               |
| `gui/src/commands/m2.rs`                   | `search` / `stats` / `compare` / `refs` / `scan_history` 命令 |
| `src/services/backend.ts`                  | 新方法与 `parseXxx` 形状校验                                  |
| `src/store/search-store.ts`                | 搜索结果与筛选                                                |
| `src/components/search/SearchPanel.tsx`    | 搜索 UI（高亮）                                               |
| `src/components/stats/StatsPanel.tsx`      | 统计仪表盘                                                    |
| `src/components/history/CompareDialog.tsx` | N 路对比                                                      |
| `src/components/env/RefGraphDialog.tsx`    | 引用图与影响范围                                              |

---

## Task 1: schema v2 与派生表 DDL

**Files:**

- Modify: `core/src/db/schema.rs`
- Modify: `core/src/db/mod.rs`
- Test: `core/tests/db_schema_v2.rs`

**Interfaces:**

- Consumes: M1 的 `Db::migrate_to_current`、`with_transaction`
- Produces: `CURRENT_SCHEMA_VERSION = 2`、`DDL_V2` 常量

- [ ] **Step 1: 写失败测试**

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn schema_v2_adds_derived_tables_and_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("v2.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    assert_eq!(db.schema_version().unwrap(), 2, "版本必须是 2");

    for t in ["scan_runs", "var_references", "daily_stats", "search_fts"] {
        assert!(db.table_exists(t).unwrap(), "缺表: {t}");
    }

    // 幂等：重复迁移不改版本、不报错
    db.migrate_to_current().unwrap();
    assert_eq!(db.schema_version().unwrap(), 2);
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_schema_v2`
Expected: FAIL — `assert_eq!(db.schema_version(), 2)` 得到 1

- [ ] **Step 3: 写 DDL_V2**

在 `core/src/db/schema.rs` 追加（**逐字照抄 spec §4.1**）：

```rust
/// 当前 schema 版本。
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// v2 增量 DDL：M2 的派生表（可随时重建，不是真相源）。
pub const DDL_V2: &str = r#"
CREATE TABLE scan_runs (
  id             INTEGER PRIMARY KEY,
  uid            TEXT NOT NULL UNIQUE,
  created_at     INTEGER NOT NULL,
  conflicts_json TEXT NOT NULL,
  tools_json     TEXT NOT NULL
);
CREATE INDEX idx_scan_runs_time ON scan_runs(created_at);

CREATE TABLE var_references (
  snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
  from_hive   TEXT NOT NULL,
  from_name   TEXT NOT NULL,
  to_name     TEXT NOT NULL,
  PRIMARY KEY (snapshot_id, from_hive, from_name, to_name)
);

CREATE TABLE daily_stats (
  day        TEXT PRIMARY KEY,
  path_items INTEGER NOT NULL,
  env_items  INTEGER NOT NULL,
  events     INTEGER NOT NULL,
  conflicts  INTEGER NOT NULL
);

CREATE VIRTUAL TABLE search_fts USING fts5(
  content, kind, ref_uid, tokenize='unicode61'
);
"#;
```

- [ ] **Step 4: migrate_to_current 增加 v2 分支**

```rust
pub fn migrate_to_current(&mut self) -> Result<(), CoreError> {
    let current = self.schema_version()?;
    if current > schema::CURRENT_SCHEMA_VERSION { /* 同 M1 */ }
    if current == schema::CURRENT_SCHEMA_VERSION { return Ok(()); }
    self.with_transaction(|tx| {
        if current < 1 {
            tx.execute_batch(schema::DDL_V1).map_err(|e| db_err("ddl_v1", e))?;
            tx.execute(schema::SEED_APP_STATE_SQL, params![Uuid::now_v7().to_string()])
                .map_err(|e| db_err("seed_app_state", e))?;
        }
        if current < 2 {
            tx.execute_batch(schema::DDL_V2).map_err(|e| db_err("ddl_v2", e))?;
        }
        tx.execute_batch(&format!("PRAGMA user_version = {}", schema::CURRENT_SCHEMA_VERSION))
            .map_err(|e| db_err("set_version", e))?;
        Ok(())
    })
}
```

- [ ] **Step 5: 运行，确认通过**

Run: `cargo test -p path-editor-core --test db_schema_v2`
Expected: PASS

- [ ] **Step 6: 提交**

```bash
git add core/src/db/schema.rs core/src/db/mod.rs core/tests/db_schema_v2.rs
git commit -m "feat(v6): schema v2 —— M2 派生表（scan_runs/var_references/daily_stats/search_fts）"
```

---

## Task 2: FTS5 检索与重建

**Files:**

- Create: `core/src/db/search.rs`
- Modify: `core/src/db/mod.rs`（`pub mod search;`）
- Test: `core/tests/db_search_fts.rs`

**Interfaces:**

- Consumes: `Db`、`snapshots` / `path_entries` / `profiles`
- Produces:
  - `pub fn rebuild_index(db: &mut Db) -> Result<usize, CoreError>` — 全量重建，返回行数
  - `pub fn index_snapshot_tx(tx: &Transaction, snapshot_id: i64) -> Result<(), CoreError>` — 增量
  - `pub fn search(db: &Db, query: &str, limit: u32) -> Result<Vec<SearchHit>, CoreError>`
  - `pub struct SearchHit { pub kind: String, pub ref_uid: String, pub snippet: String, pub rank: f64 }`

- [ ] **Step 1: 写失败测试**

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::search::{rebuild_index, search};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn fts_indexes_paths_and_never_indexes_sensitive_values() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("s.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let snap = NewSnapshot {
        kind: SnapshotKind::Current,
        source: EventSource::Gui,
        system: vec![PathEntry { path: "C:\\Program Files\\Java\\jdk21".into(), enabled: true }],
        user: vec![],
        env: vec![],
        parent_uid: None,
    };
    insert_snapshot(&mut db, snap).unwrap();
    let n = rebuild_index(&mut db).unwrap();
    assert!(n >= 1, "至少要索引一条路径");

    let hits = search(&db, "jdk21", 10).unwrap();
    assert!(!hits.is_empty(), "应能搜到 jdk21");
    assert_eq!(hits[0].kind, "path");

    // 空查询不得返回全量（避免「搜索框为空时列出全部」）
    assert!(search(&db, "   ", 10).unwrap().is_empty());

    // 派生表可重建：清空后重建，结果一致
    db.with_transaction(|tx| {
        tx.execute("DELETE FROM search_fts", []).map_err(|e| {
            path_editor_core::CoreError::new(
                path_editor_core::ErrorCode::Database, "clear", e.to_string(),
            )
        })?;
        Ok(())
    })
    .unwrap();
    assert!(search(&db, "jdk21", 10).unwrap().is_empty(), "清空后应为空");
    rebuild_index(&mut db).unwrap();
    assert!(!search(&db, "jdk21", 10).unwrap().is_empty(), "重建后恢复");
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_search_fts`
Expected: FAIL — `search` 模块不存在

- [ ] **Step 3: 实现 search.rs**

```rust
//! FTS5 全文检索（spec §6.2）。
//!
//! **索引范围**：PATH 条目串、env 变量名、配置名/描述/标签。
//! **绝不索引 env 变量的值** —— 明文值只存在于 `env_values.value`，
//! 由 SQLCipher 保护，且不进任何派生表（spec D6 + §6.2）。
//!
//! **派生表**：`search_fts` 可从 `path_entries` / `env_values.name` /
//! `profiles` 完整重建，见 [`rebuild_index`]。

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
    /// 高亮片段（FTS5 `snippet()`）
    pub snippet: String,
    /// 排名（越小越相关）
    pub rank: f64,
}

/// 全量重建索引，返回写入的行数。
pub fn rebuild_index(db: &mut Db) -> Result<usize, CoreError> {
    db.with_transaction(|tx| {
        tx.execute("DELETE FROM search_fts", [])
            .map_err(|e| super::db_err("clear_fts", e))?;
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
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
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
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
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
    })
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

/// 增量索引**一份快照**（C-2）。
///
/// 与 [`rebuild_index`] 的区别：只处理指定的 `snapshot_id`，不清空全表。
/// 供写入路径在同一事务内调用（spec：派生表可随时重建，增量只是性能优化）。
///
/// **调用方必须保证同一快照不被索引两次** —— 重复调用会产生重复行。
/// 调用前先删该快照的旧行（`DELETE ... WHERE ref_uid LIKE '<uid>#%'`），
/// 本函数内部会做这一步。
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

/// 搜索。空查询（含纯空白）返回空结果——**不得返回全量**。
pub fn search(db: &Db, query: &str, limit: u32) -> Result<Vec<SearchHit>, CoreError> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 200) as i64;

    let mut stmt = db
        .prepare(
            "SELECT kind, ref_uid,
                    snippet(search_fts, 0, '[', ']', '…', 12),
                    rank
               FROM search_fts
              WHERE search_fts MATCH ?1
              ORDER BY rank
              LIMIT ?2",
        )?;
    let rows = stmt
        .query_map(params![q, limit], |r| {
            Ok(SearchHit {
                kind: r.get(0)?,
                ref_uid: r.get(1)?,
                snippet: r.get(2)?,
                rank: r.get(3)?,
            })
        })
        .map_err(|e| super::db_err("fts_query", e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("fts_query", e))
}
```

> **注意**：`Db::prepare` 在 M1 里是 `pub(crate)`。本 Task 需要把它用到 `search.rs`（同 crate，可访问）。

- [ ] **Step 4: 运行，确认通过**

Run: `cargo test -p path-editor-core --test db_search_fts`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add core/src/db/search.rs core/src/db/mod.rs core/tests/db_search_fts.rs
git commit -m "feat(v6): FTS5 检索与索引重建（不索引敏感值）"
```

---

## Task 3: 统计聚合（`daily_stats`）

**Files:**

- Create: `core/src/db/stats.rs`
- Modify: `core/src/db/mod.rs`
- Test: `core/tests/db_stats_daily.rs`

**Interfaces:**

- Consumes: `events` / `snapshots` / `path_entries` / `profiles`
- Produces:
  - `pub fn rebuild_daily(db: &mut Db) -> Result<usize, CoreError>` — 重建 `daily_stats` + 打耗时日志（T-1）
  - `pub fn dashboard(db: &Db, days: u32) -> Result<Dashboard, CoreError>`
  - `pub struct Dashboard { days, top_targets, profile_applies, current_avg_len, current_path_items }`（完整定义见 Step 3）
  - `pub struct DailyPoint { day, path_items, env_items, events, conflicts }`
  - `pub struct TargetCount { target, count }` / `pub struct ProfileApply { name, apply_count, last_applied_at }`

- [ ] **Step 1: 写失败测试**

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::history_sink::DbHistory;
use path_editor_core::db::stats::{dashboard, rebuild_daily};
use path_editor_core::db::Db;
use path_editor_core::history::{EventAction, EventResult, EventSource, HistoryEvent, HistorySink};
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn daily_stats_aggregates_events_and_is_rebuildable() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("st.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![
                PathEntry { path: "C:\\A".into(), enabled: true },
                PathEntry { path: "C:\\B".into(), enabled: true },
            ],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();

    for _ in 0..3 {
        DbHistory::new(&mut db)
            .record(HistoryEvent {
                source: EventSource::Gui,
                action: EventAction::Save,
                hive: None,
                target: Some("Path".into()),
                changed_fields: vec![],
                before_revision: None,
                after_revision: None,
                result: EventResult::Success,
                snapshot_uid: None,
            })
            .unwrap();
    }

    let n = rebuild_daily(&mut db).unwrap();
    assert_eq!(n, 1, "同一天只应有一行");

    let d = dashboard(&db, 7).unwrap();
    assert_eq!(d.days.len(), 1);
    assert_eq!(d.days[0].events, 3, "当天 3 条事件");
    assert_eq!(d.days[0].path_items, 2, "当前快照 2 条 PATH");

    // 可重建
    db.with_transaction(|tx| {
        tx.execute("DELETE FROM daily_stats", []).map_err(|e| {
            path_editor_core::CoreError::new(
                path_editor_core::ErrorCode::Database, "clear", e.to_string(),
            )
        })?;
        Ok(())
    })
    .unwrap();
    assert!(dashboard(&db, 7).unwrap().days.is_empty());
    rebuild_daily(&mut db).unwrap();
    assert_eq!(dashboard(&db, 7).unwrap().days.len(), 1);
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_stats_daily`
Expected: FAIL — `stats` 模块不存在

- [ ] **Step 3: 实现 stats.rs**

```rust
//! 统计聚合（spec §6.2）。
//!
//! `daily_stats` 是**派生表**：从 `events` / `snapshots` 重算，可随时重建。

use super::Db;
use crate::error::CoreError;
use rusqlite::params;

/// 单日聚合点。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPoint {
    /// `YYYY-MM-DD`
    pub day: String,
    /// 当日快照的 PATH 条目数（取当日最后一条快照）
    pub path_items: i64,
    /// 当日快照的环境变量数
    pub env_items: i64,
    /// 当日事件数
    pub events: i64,
    /// 当日冲突扫描次数
    pub conflicts: i64,
}

/// 统计面板数据。
///
/// **口径说明（M-1）**：`days` 是按天聚合的**历史**序列，某一天的 `path_items`
/// 是「那天最后一次保存时的条目数」，不是「现在有多少条」。
/// 「现在」单独由 `current_path_items` / `current_avg_len` 给出，
/// 取自 `app_state.current_snapshot_id`（与 GUI 的 `READ_SNAPSHOT_CACHE` 同源）。
/// UI 上必须把这两个概念分开显示，否则用户会把「今天最后一次保存」误读成「当前」。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    /// 按日期升序的时间序列
    pub days: Vec<DailyPoint>,
    /// 最常改变的变量（来自 `events.target` 聚合，spec §6.2）
    pub top_targets: Vec<TargetCount>,
    /// 配置应用次数（直接读 M1 的 `profiles.apply_count`）
    pub profile_applies: Vec<ProfileApply>,
    /// **当前**快照的 PATH 平均长度（M-2：与 `days` 的口径不同，见上）
    pub current_avg_len: f64,
    /// **当前**快照的 PATH 条目数
    pub current_path_items: i64,
}

/// 变量被修改的次数。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetCount {
    /// 变量名（`events.target`）
    pub target: String,
    /// 出现次数
    pub count: i64,
}

/// 配置的应用统计。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileApply {
    /// 配置名
    pub name: String,
    /// 应用次数
    pub apply_count: i64,
    /// 最近一次应用时间（Unix 毫秒；从未应用为 `None`）
    pub last_applied_at: Option<i64>,
}

/// 全量重建 `daily_stats`，返回写入的天数。
pub fn rebuild_daily(db: &mut Db) -> Result<usize, CoreError> {
    // T-1：**只观测，不设阈值**。当前 events 规模下全表扫描是毫秒级，
    // 定一个现在跑不到的阈值等于埋一条永远不会被测到的分支。
    // 等真有启动瓶颈时，这里的日志会提供决策所需的基线数据。
    let started = std::time::Instant::now();
    let n = db.with_transaction(|tx| {
        tx.execute("DELETE FROM daily_stats", [])
            .map_err(|e| super::db_err("clear_daily", e))?;

        // events 按天聚合；path/env 取当天**最后一条快照**的条目数
        tx.execute(
            "INSERT INTO daily_stats (day, path_items, env_items, events, conflicts)
             WITH ev AS (
               SELECT date(created_at / 1000, 'unixepoch', 'localtime') AS day, count(*) AS c
                 FROM events GROUP BY day
             ),
             last_snap AS (
               SELECT date(s.created_at / 1000, 'unixepoch', 'localtime') AS day,
                      s.id AS sid,
                      ROW_NUMBER() OVER (
                        PARTITION BY date(s.created_at / 1000, 'unixepoch', 'localtime')
                        ORDER BY s.created_at DESC, s.id DESC
                      ) AS rn
                 FROM snapshots s WHERE s.deleted_at IS NULL
             ),
             snap_counts AS (
               SELECT l.day,
                      (SELECT count(*) FROM path_entries p WHERE p.snapshot_id = l.sid) AS paths,
                      (SELECT count(*) FROM env_values e WHERE e.snapshot_id = l.sid) AS envs
                 FROM last_snap l WHERE l.rn = 1
             ),
             scans AS (
               SELECT date(created_at / 1000, 'unixepoch', 'localtime') AS day, count(*) AS c
                 FROM scan_runs GROUP BY day
             )
             SELECT COALESCE(ev.day, snap_counts.day, scans.day) AS day,
                    COALESCE(snap_counts.paths, 0),
                    COALESCE(snap_counts.envs, 0),
                    COALESCE(ev.c, 0),
                    COALESCE(scans.c, 0)
               FROM ev
               FULL OUTER JOIN snap_counts ON ev.day = snap_counts.day
               FULL OUTER JOIN scans ON COALESCE(ev.day, snap_counts.day) = scans.day",
            [],
        )
        .map_err(|e| super::db_err("rebuild_daily", e))?;

        let n: i64 = tx
            .query_row("SELECT count(*) FROM daily_stats", [], |r| r.get(0))
            .map_err(|e| super::db_err("count_daily", e))?;
        Ok(n as usize)
    })?;
    log::info!(
        "daily_stats 重建完成: {} 天, 耗时 {:?}",
        n,
        started.elapsed()
    );
    Ok(n)
}

/// 读取最近 `days` 天的统计。
///
/// 返回两部分（口径不同，UI 必须分开显示，见 `Dashboard` 的文档注释）：
/// - `days`：按天的**历史**序列；
/// - `current_*`：**现在**的状态，取自 `app_state.current_snapshot_id`。
pub fn dashboard(db: &Db, days: u32) -> Result<Dashboard, CoreError> {
    let days = days.clamp(1, 365) as i64;
    let mut stmt = db.prepare(
        "SELECT day, path_items, env_items, events, conflicts
           FROM daily_stats ORDER BY day DESC LIMIT ?1",
    )?;
    let mut rows = stmt
        .query_map(params![days], |r| {
            Ok(DailyPoint {
                day: r.get(0)?,
                path_items: r.get(1)?,
                env_items: r.get(2)?,
                events: r.get(3)?,
                conflicts: r.get(4)?,
            })
        })
        .map_err(|e| super::db_err("dashboard", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("dashboard", e))?;
    rows.reverse(); // 前端按时间升序画图

    // 最常改变的变量：从 events.target 聚合（spec §6.2，不需要新表）
    let mut stmt = db.prepare(
        "SELECT target, count(*) AS c FROM events
          WHERE target IS NOT NULL AND target <> ''
          GROUP BY target ORDER BY c DESC, target LIMIT 10",
    )?;
    let top_targets = stmt
        .query_map([], |r| {
            Ok(TargetCount {
                target: r.get(0)?,
                count: r.get(1)?,
            })
        })
        .map_err(|e| super::db_err("dashboard_targets", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("dashboard_targets", e))?;

    // 配置应用次数：直接读 M1 的 profiles
    let mut stmt = db.prepare(
        "SELECT name, apply_count, last_applied_at FROM profiles
          WHERE deleted_at IS NULL ORDER BY apply_count DESC, name LIMIT 10",
    )?;
    let profile_applies = stmt
        .query_map([], |r| {
            Ok(ProfileApply {
                name: r.get(0)?,
                apply_count: r.get(1)?,
                last_applied_at: r.get(2)?,
            })
        })
        .map_err(|e| super::db_err("dashboard_profiles", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("dashboard_profiles", e))?;

    // 当前快照的条目数与平均长度（B-2：daily_stats 无长度列，这里直接查 path_entries）
    let (current_path_items, current_avg_len): (i64, f64) = db
        .query_row_with(
            "SELECT count(*), COALESCE(AVG(length(path)), 0.0)
               FROM path_entries
              WHERE snapshot_id = (SELECT current_snapshot_id FROM app_state WHERE id = 1)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| super::db_err("dashboard_current", e))?;

    Ok(Dashboard {
        days: rows,
        top_targets,
        profile_applies,
        current_avg_len,
        current_path_items,
    })
}
```

> **SQLite 版本注意**：`FULL OUTER JOIN` 需要 SQLite ≥ 3.39。**已实测确认**：本仓库捆绑的
> SQLite 为 **3.51.3**，`ENABLE_FTS5 = 1`，`FULL OUTER JOIN = true`。可直接使用，无需降级写法。
> （验证方式：`SELECT sqlite_version()` / `sqlite_compileoption_used('ENABLE_FTS5')`）

- [ ] **Step 4: 运行，确认通过**

Run: `cargo test -p path-editor-core --test db_stats_daily`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add core/src/db/stats.rs core/src/db/mod.rs core/tests/db_stats_daily.rs
git commit -m "feat(v6): daily_stats 聚合与统计面板数据"
```

---

## Task 4: `%VAR%` 引用图（`var_references`）

**Files:**

- Create: `core/src/db/refs.rs`
- Modify: `core/src/db/mod.rs`
- Test: `core/tests/db_var_references.rs`

**Interfaces:**

- Consumes: `path_entries` / `env_values`（只读名字，不读值）
- Produces:
  - `pub fn rebuild_refs(db: &mut Db) -> Result<usize, CoreError>`
  - `pub fn dependents(db: &Db, name: &str) -> Result<Vec<RefEdge>, CoreError>` — 谁引用了它
  - `pub fn dependencies(db: &Db, name: &str) -> Result<Vec<RefEdge>, CoreError>` — 它引用了谁（递归 CTE）
  - `pub fn find_cycles(db: &Db) -> Result<Vec<Vec<String>>, CoreError>`
  - `pub fn parse_refs(text: &str) -> Vec<String>` — 纯函数，抽 `%NAME%`

- [ ] **Step 1: 写失败测试**

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::refs::{dependencies, dependents, find_cycles, parse_refs, rebuild_refs};
use path_editor_core::db::history::{insert_snapshot, EnvValueRow, NewSnapshot, SnapshotKind};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn var_references_parses_edges_and_detects_cycles() {
    // 纯函数：解析 %VAR%
    let refs = parse_refs("%JAVA_HOME%\\bin;%M2_HOME%\\bin;C:\\plain");
    assert_eq!(refs, vec!["JAVA_HOME", "M2_HOME"], "只认 %NAME%，忽略普通目录");

    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("r.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![PathEntry { path: "%JAVA_HOME%\\bin".into(), enabled: true }],
            user: vec![],
            env: vec![EnvValueRow {
                hive: "system".into(),
                name: "PATH".into(),   // 被 insert_snapshot 拒绝，见下
                kind: "string".into(),
                value: Some(String::new()),
                revision: "r".into(),
                sensitive: false,
                restorable: true,
            }],
            parent_uid: None,
        },
    )
    .ok(); // Path 不得进 env_values；此处只验证 refs 不依赖它

    let n = rebuild_refs(&mut db).unwrap();
    assert!(n >= 1);

    let deps = dependents(&db, "JAVA_HOME").unwrap();
    assert!(!deps.is_empty(), "JAVA_HOME 应被系统 PATH 引用");

    // 无环
    assert!(find_cycles(&db).unwrap().is_empty());
}
```

> **注意**：`insert_snapshot` 会拒绝名为 `Path` 的 `env_values`（DDL CHECK）。测试里那条 `env` 用 `.ok()` 忽略结果，**或者**换成 `JAVA_HOME` 之类合法名字并把引用关系写在 PATH 串里。实现时按后者写更清晰——本测试只关心 `path_entries` 里的 `%VAR%`。

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_var_references`
Expected: FAIL — `refs` 模块不存在

- [ ] **Step 3: 实现 refs.rs**

```rust
//! `%VAR%` 引用图（spec §6.2）。
//!
//! **只解析名字，不解析值**：`var_references` 存的是「A 引用了 B」这条边，
//! 边由 `%NAME%` 语法得出，与变量值无关。因此不含任何明文。

use super::Db;
use crate::error::CoreError;
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
    /// 距离（`dependents` / `dependencies` 的 BFS 层数）
    pub depth: i64,
}

/// 从文本中抽取 `%NAME%`，返回**大写**名字（去重、保序）。
pub fn parse_refs(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(end) = text[i + 1..].find('%') {
                let name = &text[i + 1..i + 1 + end];
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    let up = name.to_ascii_uppercase();
                    if !out.contains(&up) {
                        out.push(up);
                    }
                    i = i + 1 + end + 1;
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
/// **不变量（M-3）**：本表**只保存当前快照的边**，由「每次全量清空 + 只插当前快照」
/// 保证。因此 [`dependents`] / [`dependencies`] / [`find_cycles`] 可以省略
/// `snapshot_id` 过滤。
///
/// ⚠️ 若将来改为增量（只删当前快照的边），**必须同时**给那三条查询补上
/// `WHERE snapshot_id = (SELECT current_snapshot_id FROM app_state WHERE id = 1)`，
/// 否则会静默混入历史快照的边。
pub fn rebuild_refs(db: &mut Db) -> Result<usize, CoreError> {
    db.with_transaction(|tx| {
        tx.execute("DELETE FROM var_references", [])
            .map_err(|e| super::db_err("clear_refs", e))?;

        // 只处理当前快照（引用图是「现在」的形状，不是历史）
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
                tx.execute(
                    "INSERT OR IGNORE INTO var_references (snapshot_id, from_hive, from_name, to_name)
                     VALUES (?1, ?2, 'PATH', ?3)",
                    params![sid, hive, to],
                )
                .map_err(|e| super::db_err("refs_insert", e))?;
                n += 1;
            }
        }
        drop(stmt);

        // env 变量的值里也可能有 %VAR%（只读名字，不建立到值的引用）
        let mut stmt = tx
            .prepare(
                "SELECT hive, name, COALESCE(value,'') FROM env_values WHERE snapshot_id = ?1",
            )
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
                tx.execute(
                    "INSERT OR IGNORE INTO var_references (snapshot_id, from_hive, from_name, to_name)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![sid, hive, name, to],
                )
                .map_err(|e| super::db_err("refs_insert", e))?;
                n += 1;
            }
        }
        Ok(n)
    })
}

/// 谁引用了 `name`（反向边，深度 1）。
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

/// `name` 传递依赖了谁（递归 CTE，spec §6.2 明确要求）。
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
          ORDER BY depth, from_name",
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
         SELECT DISTINCT path FROM walk WHERE cur = start",
    )?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| super::db_err("find_cycles", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("find_cycles", e))?;
    Ok(rows
        .into_iter()
        .map(|p| p.split('>').map(|s| s.to_string()).collect())
        .collect())
}
```

- [ ] **Step 4: 运行，确认通过**

Run: `cargo test -p path-editor-core --test db_var_references`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add core/src/db/refs.rs core/src/db/mod.rs core/tests/db_var_references.rs
git commit -m "feat(v6): %VAR% 引用图（解析/递归依赖/环检测）"
```

---

## Task 5: N 路快照对比

**Files:**

- Create: `core/src/db/compare.rs`
- Modify: `core/src/db/mod.rs`
- Test: `core/tests/db_compare.rs`

**Interfaces:**

- Consumes: `history::get_snapshot`
- Produces:
  - `pub fn compare(db: &Db, left: &str, right: &str) -> Result<CompareResult, CoreError>`
  - `pub struct CompareResult { pub path_added, path_removed, path_reordered, env_added, env_removed, env_modified }`
  - **T-3**：`env_added` / `env_removed` / `env_modified` 本轮**恒为空数组**，
    UI 必须显式显示「环境变量对比将在 M3 提供」。只有 PATH 结果而字段缺失时，
    用户会以为「env 没变化」，而不是「env 没对比」——空数组 + 显式文案比字段不存在诚实。
  - 敏感值在结果里**恒为掩码**（`Option<String>` 仅对非敏感项有值）

- [ ] **Step 1: 写失败测试**

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::compare::compare;
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn compare_reports_added_removed_and_reorder() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("c.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let mk = |paths: &[&str]| NewSnapshot {
        kind: SnapshotKind::History,
        source: EventSource::Gui,
        system: paths
            .iter()
            .map(|p| PathEntry { path: p.to_string(), enabled: true })
            .collect(),
        user: vec![],
        env: vec![],
        parent_uid: None,
    };

    let a = insert_snapshot(&mut db, mk(&["C:\\A", "C:\\B"])).unwrap();
    let b = insert_snapshot(&mut db, mk(&["C:\\B", "C:\\A", "C:\\C"])).unwrap();

    let r = compare(&db, &a, &b).unwrap();
    assert_eq!(r.path_added.len(), 1, "C 是新增");
    assert_eq!(r.path_added[0].path, "C:\\C");
    assert!(r.path_removed.is_empty(), "没有删除");
    assert!(!r.path_reordered.is_empty(), "A/B 顺序变了");

    // 自比对：全为空
    let same = compare(&db, &a, &a).unwrap();
    assert!(same.path_added.is_empty() && same.path_removed.is_empty() && same.path_reordered.is_empty());
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_compare`
Expected: FAIL — `compare` 模块不存在

- [ ] **Step 3: 实现 compare.rs**

核心算法（按路径小写做键）：

1. `left_only` = left 有、right 无 → `path_removed`
2. `right_only` = right 有、left 无 → `path_added`
3. `both` 的交集里，比较各自在 hive 内的**相对顺序**；若顺序不同 → `path_reordered`（记录 `path` + `left_index` + `right_index`）

```rust
//! N 路快照对比（spec §6.2）。
//!
//! **敏感值恒掩码**：对比结果里 env 的 `value` 只对非敏感项填充。

use super::{history, Db};
use crate::error::CoreError;
use crate::path_entry::PathEntry;
use std::collections::HashMap;

/// 顺序变化的一条。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderEntry {
    /// 路径
    pub path: String,
    /// 在 left 里的位置
    pub left_index: usize,
    /// 在 right 里的位置
    pub right_index: usize,
}

/// 对比结果。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareResult {
    /// right 有、left 无
    pub path_added: Vec<PathEntry>,
    /// left 有、right 无
    pub path_removed: Vec<PathEntry>,
    /// 两边都有但相对顺序变了
    pub path_reordered: Vec<ReorderEntry>,
}

/// 比较两个快照（按 uid）。
pub fn compare(db: &Db, left_uid: &str, right_uid: &str) -> Result<CompareResult, CoreError> {
    // 对比只看 PATH；env 对比涉及掩码语义，留给 M2 的第二轮（见计划 §未覆盖项）
    let left = history::get_snapshot(db, left_uid, false)?;
    let right = history::get_snapshot(db, right_uid, false)?;
    Ok(compare_paths(&left.system, &right.system))
}

/// 纯函数：对比两侧 PATH。
pub fn compare_paths(left: &[PathEntry], right: &[PathEntry]) -> CompareResult {
    let key = |p: &str| p.trim().to_lowercase();

    let left_map: HashMap<String, (usize, &PathEntry)> = left
        .iter()
        .enumerate()
        .map(|(i, e)| (key(&e.path), (i, e)))
        .collect();
    let right_map: HashMap<String, (usize, &PathEntry)> = right
        .iter()
        .enumerate()
        .map(|(i, e)| (key(&e.path), (i, e)))
        .collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut reordered = Vec::new();

    for (k, (ri, e)) in &right_map {
        match left_map.get(k) {
            None => added.push((*e).clone()),
            Some((li, _)) => {
                if li != ri {
                    reordered.push(ReorderEntry {
                        path: e.path.clone(),
                        left_index: *li,
                        right_index: *ri,
                    });
                }
            }
        }
    }
    for (k, (_, e)) in &left_map {
        if !right_map.contains_key(k) {
            removed.push((*e).clone());
        }
    }

    // 稳定排序，避免 HashMap 迭代顺序影响输出
    added.sort_by(|a, b| key(&a.path).cmp(&key(&b.path)));
    removed.sort_by(|a, b| key(&a.path).cmp(&key(&b.path)));
    reordered.sort_by_key(|r| r.right_index);

    CompareResult {
        path_added: added,
        path_removed: removed,
        path_reordered: reordered,
    }
}

```

> **实现时删掉上面那行占位注释**，并去掉未使用的 `import`（否则 clippy `-D warnings` 会红）。

- [ ] **Step 4: 运行，确认通过**

Run: `cargo test -p path-editor-core --test db_compare`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add core/src/db/compare.rs core/src/db/mod.rs core/tests/db_compare.rs
git commit -m "feat(v6): N 路快照对比（新增/删除/顺序变化）"
```

---

## Task 6: 冲突历史（`scan_runs`）

**Files:**

- Create: `core/src/db/scan_history.rs`
- Modify: `core/src/db/mod.rs`
- Test: `core/tests/db_scan_runs.rs`

**Interfaces:**

- Consumes: `core::scanner::scan_conflicts` 的输出
- Produces:
  - `pub fn record_run(db: &mut Db, conflicts_json: &str, tools_json: &str) -> Result<String, CoreError>`
  - `pub fn list_runs(db: &Db, limit: u32) -> Result<Vec<ScanRunView>, CoreError>`
  - `pub fn diff_runs(db: &Db, a: &str, b: &str) -> Result<ScanRunDiff, CoreError>`
  - `pub struct ScanRunDiff { pub appeared: Vec<String>, pub disappeared: Vec<String>, pub persistent: Vec<String>, pub last_seen: Vec<LastSeen> }`
  - `pub struct LastSeen { pub conflict: String, pub run_uid: String, pub seen_at: i64 }`（M-5）

- [ ] **Step 1: 写失败测试**

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::scan_history::{diff_runs, list_runs, record_run};
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn scan_runs_persist_and_diff_between_two_scans() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("sr.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let a = record_run(&mut db, r#"["C:\\dup","C:\\old"]"#, "[]").unwrap();
    let b = record_run(&mut db, r#"["C:\\dup","C:\\new"]"#, "[]").unwrap();

    let list = list_runs(&db, 10).unwrap();
    assert_eq!(list.len(), 2, "应记录两次扫描");
    assert_eq!(list[0].uid, b, "最新在前");

    let d = diff_runs(&db, &a, &b).unwrap();
    assert_eq!(d.appeared, vec!["C:\\new".to_string()]);
    assert_eq!(d.disappeared, vec!["C:\\old".to_string()]);
    assert_eq!(d.persistent, vec!["C:\\dup".to_string()]);
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_scan_runs`
Expected: FAIL — `scan_history` 模块不存在

- [ ] **Step 3: 实现 scan_history.rs**

要点：

- `record_run` 写 `scan_runs`（`uid` 用 `Uuid::now_v7()`）；**同时**触发 `stats::rebuild_daily`（冲突频率依赖它）。
  ⚠️ **不能嵌套事务**：`record_run` 用 `with_transaction` 写完后，再单独调 `rebuild_daily`。
- `list_runs` 按 `created_at DESC, id DESC`。
- `diff_runs` 解析两份 `conflicts_json`（`Vec<String>`），做三集合运算：`appeared` / `disappeared` / `persistent`，全部排序后返回。
- **M-5「最近出现」**：spec §6.2 原文要求「首次出现/最近出现」。`last_seen` 字段承载后者——
  对 `persistent` 里的每一项，从 `list_runs` 的历史里**倒序找第一条不含它的 run**，
  它之后的那条就是「最近一次出现」。实现上遍历 `scan_runs` 即可，**不需要改表**。
  若该冲突在最近一次运行里出现（即 `b`），`last_seen` 就是 `b` 本身。

**M-6 测试要求**：Step 1 的测试末尾**必须**再加一条断言，验证 `record_run` 真的触发了
`daily_stats` 重建——否则「触发重建」这件事没有任何证据，接没接上都会「测试全绿」。

```rust
// 在同一函数、同一个 db 上追加（不新开 SQLCipher 连接，不违反一文件一测试）
let rows: i64 = db
    .query_row_with("SELECT COALESCE(SUM(conflicts),0) FROM daily_stats", [], |r| r.get(0))
    .unwrap();
assert!(rows >= 2, "record_run 必须触发 daily_stats 重建（M-6），实际 conflicts 合计 {rows}");
```

- [ ] **Step 4: 运行，确认通过**

Run: `cargo test -p path-editor-core --test db_scan_runs`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add core/src/db/scan_history.rs core/src/db/mod.rs core/tests/db_scan_runs.rs
git commit -m "feat(v6): 冲突历史持久化与两次扫描 diff"
```

---

## Task 7: M1 欠账 —— `history_reveal_env_value`

**Files:**

- Modify: `gui/src/commands/history.rs`
- Modify: `gui/src/lib.rs`
- Modify: `src/services/backend.ts`
- Modify: `src/components/history/SnapshotDetail.tsx`
- Test: `core/tests/db_reveal_value.rs` + `tests/unit/backend-reveal.test.ts`

**Interfaces:**

- Consumes: `env_values.value`
- Produces: `history_reveal_env_value(uid, hive, name) -> String`（明文，语义对齐 `reveal_env_var`）

- [ ] **Step 1: 写失败测试（core 侧）**

```rust
// 一文件一测试：见 M0 §5.3。
// 断言：非敏感变量的 reveal 返回明文；敏感变量在 reveal=true 时也返回明文，
// 但**调用方必须显式传 reveal**，默认路径不暴露。
```

- [ ] **Step 2: 实现**

```rust
/// 显式 reveal 一个历史快照里的 env 变量值（spec §3.2.1）。
///
/// **独立命令**：与 `history_get_snapshot` 分离，让「掩码」成为默认，
/// 「看明文」成为一次可审计的显式动作。
#[tauri::command]
pub fn history_reveal_env_value(
    uid: String,
    hive: String,
    name: String,
    state: State<'_, DbState>,
) -> Result<String, CoreError> {
    with_db(&state, "history_reveal_env_value", |db| {
        history::reveal_env_value(db, &uid, &hive, &name)
    })
}
```

core 侧 `history::reveal_env_value` 直接 `SELECT value FROM env_values WHERE snapshot_id = (SELECT id FROM snapshots WHERE uid=?) AND hive=? AND name=?`；`restorable = 0` 时返回 `NotFound`（Unsupported 类型没有值可 reveal）。

- [ ] **Step 3: 前端**

`SnapshotDetail.tsx`：每个被掩码的变量旁加「显示」按钮 → 调 `backend.historyRevealEnvValue(...)` → 只在该次渲染里显示明文，**不写入 store**（避免明文进入 DevTools / 持久状态）。

- [ ] **Step 4: 提交**

```bash
git commit -m "feat(v6): history_reveal_env_value —— 敏感值显式 reveal"
```

---

## Task 8: M1 欠账 —— `profile_export_v5`

**Files:**

- Create: `core/src/db/v5_export.rs`
- Modify: `core/src/profiles.rs`（**新增 `load_profile_from(path)`，B-1 整改**）
- Modify: `gui/src/commands/history.rs`、`cli/src/profile_ops.rs`
- Test: `core/tests/db_profile_export_v5.rs`

**Interfaces:**

- Produces: `pub fn export_profile_v5(db: &Db, uid: &str) -> Result<String, CoreError>`（返回 v5 `ProfileData` 的 JSON 字符串）

- 前置（**B-1：必须先做，否则测试会写进用户真实目录**）：
  `core/src/profiles.rs` 现有 `load_profile(name)` 内部走 `profiles_dir()`，而 `profiles_dir()` 基于
  `dirs::home_dir()`，**无法被环境变量重定向**。`core/tests/*.rs` 是外部集成测试，
  用不到 `#[cfg(test)]` 的 temp 特判，照计划写成「export → 写文件 → `load_profile` 读回」
  会在 `C:\Users\<用户>\.patheditor\profiles\` 下留下测试文件。

  **整改**：把路径解析与解析逻辑拆开——

  ```rust
  /// 从**指定路径**读取配置（供测试与 v5 导入使用，B-1）。
  ///
  /// [`load_profile`] 是「按名字算路径 + 调本函数」的薄封装。
  /// 拆分理由：`profiles_dir()` 基于 `dirs::home_dir()`，无法重定向；
  /// 外部集成测试必须能指定路径，否则会写进用户真实配置目录。
  pub fn load_profile_from(path: &Path) -> Result<ProfileData, CoreError> {
      if !path.exists() {
          return Err(CoreError::new(ErrorCode::NotFound, "load_profile", "配置文件不存在"));
      }
      let content = fs::read_to_string(path).map_err(|e| {
          CoreError::new(ErrorCode::Io, "load_profile", format!("无法读取配置文件: {e}"))
      })?;
      serde_json::from_str::<Versioned<ProfileData>>(&content)
          .map_err(|e| parse_error_quarantined(path, "配置文件", e))
          .and_then(|v| migrate(v, "配置文件"))
  }

  /// 按名字读取配置（路径由 `profiles_dir()` 决定）。
  pub fn load_profile(name: &str) -> Result<ProfileData, CoreError> {
      validate_profile_name(name)?;
      load_profile_from(&profile_path(name))
  }
  ```

  **这是 M1 遗留的可测性缺口**：`profiles.rs` 的所有读写都硬绑 `profiles_dir()`，
  导致任何依赖它的集成测试都无法隔离。本 Task 顺带修掉。

**要点**：输出必须能被 `core::profiles::load_profile_from(path)` 读回（降级到 v5 时的退路）。
测试断言**往返一致**：export → 写成 `TempDir` 下的文件 → `load_profile_from(&path)` 读回 →
与 DB 里的 PATH 相等。**测试全程只碰 `TempDir`，不调 `load_profile(name)`。**

- [ ] **Step 1–5**（同标准 TDD 循环）

```bash
git commit -m "feat(v6): profile_export_v5 —— 降级到 v5 文件的退路"
```

---

## Task 9: IPC 与前端形状校验

**Files:**

- Create: `gui/src/commands/m2.rs`
- Modify: `gui/src/lib.rs`、`src/services/backend.ts`
- Test: `tests/unit/backend-m2-shapes.test.ts`

**命令清单**（继承 M1 的 `{ limit, offset }` 契约，spec §3.2.1 规则 1）：

```text
search(query, limit)                        -> SearchHit[]
stats_dashboard(days)                       -> Dashboard
compare_snapshots(left_uid, right_uid)      -> CompareResult
refs_dependents(name)                       -> RefEdge[]
refs_dependencies(name)                     -> RefEdge[]
refs_cycles()                               -> string[][]
scan_history_list(limit)                    -> ScanRunView[]
scan_history_diff(a_uid, b_uid)             -> ScanRunDiff
scan_history_record(conflicts_json)         -> string
```

- [ ] **Step 1: 写失败测试** — 每个 `parseXxx` 覆盖：正常、缺字段、类型错、未知 code
- [ ] **Step 2–5**（同标准 TDD 循环）

```bash
git commit -m "feat(v6): M2 IPC 命令与前端形状校验"
```

---

## Task 10: 搜索 UI 与统计面板

**Files:**

- Create: `src/store/search-store.ts`、`src/components/search/SearchPanel.tsx`、`src/components/stats/StatsPanel.tsx`
- Modify: `src/components/layout/AppShell.tsx`、`src/core/path-capabilities.ts`（**在既有 `TabId` 联合类型里追加** `'search' | 'stats'`，不是新建类型；`app-store.ts:16` 已 re-export，无需改动）、`src/i18n/locales/{zh-CN,en}.json`（补 `tab.search` / `tab.stats`）
- Modify: `src/i18n/locales/{zh-CN,en}.json`

**要求**：

- **所有文案走 `t()`，locale 里补齐键**（M1 曾因 17 个硬编码中文标签被审查打回）
- 搜索框为空时不发请求（与 `search()` 的「空查询返回空」口径一致）
- 统计用现有依赖画图；**不引入新库**——用简单的 bar/line 元素配 `--app-*` 语义 token
- 降级模式下两个面板都显示「不可用」，复用 `DegradedBanner` 的判定

- [ ] **Step 1–5**（同标准 TDD 循环）

```bash
git commit -m "feat(v6): 搜索与统计面板"
```

---

## Task 11: 对比与引用图 UI

**Files:**

- Create: `src/components/history/CompareDialog.tsx`、`src/components/env/RefGraphDialog.tsx`
- Modify: `src/components/history/HistoryPanel.tsx`（多选 + 「对比」按钮）

**要求**：

- 对比结果按 `added` / `removed` / `reordered` 三色区分，用 `--app-*` token
- **删除变量前**展示影响范围（`refs_dependents`），这是 spec §6.2 明确的产品语义
- 引用图展示环（`refs_cycles`）时高亮

- [ ] **Step 1–5**（同标准 TDD 循环）

```bash
git commit -m "feat(v6): 快照对比与引用图 UI"
```

---

## Task 12: 派生表重建策略与启动接线

**Files:**

- Modify: `gui/src/lib.rs`、`cli/src/main.rs`
- Test: `core/tests/db_derived_rebuild.rs`

**要点**：三个派生表（`search_fts` / `daily_stats` / `var_references`）必须有明确的**重建时机**：

| 派生表           | 何时重建                                                                      |
| ---------------- | ----------------------------------------------------------------------------- |
| `search_fts`     | 启动时若 `count(*) = 0` 则全量重建；每次写入快照后增量（`index_snapshot_tx`） |
| `daily_stats`    | 启动时全量重建（成本是一次全表扫描，数据量小）                                |
| `var_references` | 当前快照变化时重建（`update_current_snapshot` 之后）                          |

**验收判据**：`db_derived_rebuild` 测试断言「清空三张派生表 → 重启序列 → 数据恢复」。

- [ ] **Step 1–5**（同标准 TDD 循环）

```bash
git commit -m "feat(v6): 派生表重建策略与启动接线"
```

---

## Task 13: E2E 与文档

**Files:**

- Create: `e2e/tests/search.spec.ts`、`e2e/tests/stats.spec.ts`
- Modify: `README.md`、`CHANGELOG.md`

**E2E 覆盖（spec §3.2.1 要求 3–4 条主路径）**：

1. 搜索框输入 → 结果高亮
2. 搜索框为空 → 无结果、无请求
3. 统计面板显示当日数据
4. 降级模式下两个面板显示「不可用」

- [ ] **Step 1–5**（同标准 TDD 循环）

```bash
git commit -m "test(v6): 搜索与统计 E2E；文档同步"
```

---

## Self-Review

**规格覆盖**（对照 spec §6.2 Tier 2 逐条）：

| spec 要求                                                                     | Task                                      |
| ----------------------------------------------------------------------------- | ----------------------------------------- |
| FTS5 索引路径串、变量名、配置描述/标签；`search(query)` 返回排名与高亮        | Task 2                                    |
| 统计：PATH 条目数/长度趋势、最常改变量、配置应用次数、冲突频率；`daily_stats` | Task 3（**长度趋势与最常改变量见下）**    |
| N 路对比：新增/删除/修改/顺序变化；敏感值掩码                                 | Task 5（**仅 PATH；env 对比见未覆盖项**） |
| 引用图：`%VAR%` 边；递归 CTE；删除前显示影响范围                              | Task 4 + Task 11                          |
| 冲突历史：`scan_runs` 持久化；两次扫描 diff；首次/最近出现                    | Task 6                                    |

**发现的规格缺口（已于核对轮全部落到具体 Task）**：

1. **「PATH 长度趋势」** — `daily_stats` 表结构无长度字段（spec §4.1 未定义）。
   **裁决（T-2）**：不改表结构，`dashboard()` 直接查 `path_entries` 算 `current_avg_len`。
   理由：spec §4.2 第 5 条写明派生表可重建、不是真相；为一个展示字段改权威表结构不划算。
   ⚠️ 口径：`current_avg_len` 是**当前**快照的平均长度，与 `days` 的历史序列不是一回事
   （见 `Dashboard` 的文档注释 M-1）。
2. **「最常改变量」** — 从 `events.target` 的 `GROUP BY` 直接算，不需要新表。已落 Task 3。
3. **「配置应用次数」** — 已在 M1 的 `profiles.apply_count`，Task 3 直接读。已落 Task 3。
4. **env 的 N 路对比** — 推迟到 M3。**范围缩减，见下方「未覆盖项」表，回执必须照抄。**

**「未覆盖项」清单（开发窗口回执必须原样带过去，不得省略）**：

| #   | 未覆盖项                                                                   | 归属              | 原因                                                                                                                                  |
| --- | -------------------------------------------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | env 变量的 N 路对比（`env_added`/`env_removed`/`env_modified` 恒为空数组） | M3                | 掩码值在对比结果里如何呈现没有定义（显示 `••••`？显示「已修改但不可见」？两边都敏感且都改了无法判断是否相同时怎么办），硬做会误导用户 |
| 2   | CLI 不重建派生表；CLI 改 PATH 后 `var_references` / `daily_stats` 保持陈旧 | 未排期（M1 R-02） | 需 CLI 也接 DB 的 current 快照，属范围扩张；**M2 不顺手修**                                                                           |
| 3   | 真实注册表闭环测试                                                         | 用户授权后        | E2E 用 mock IPC                                                                                                                       |
| 4   | CI（MSVC）验证                                                             | 推送后            | 本机为 GNU                                                                                                                            |

**M-2（时区）约束**：`date(..., 'localtime')` 取 C 运行时时区，代码里没有任何地方强制
`Asia/Shanghai`。**Task 3 的测试必须避开跨日边界**——不要在 23:59 附近写事件，
断言只比较「天数」不比较具体日期。

**类型一致性**：`SearchHit` / `DailyPoint` / `TargetCount` / `ProfileApply` / `RefEdge` /
`CompareResult` / `ScanRunDiff` / `LastSeen` 在 Task 2/3/4/5/6 定义，Task 9 的 `parseXxx`
与 Task 10/11 的组件都引用同一形状，无重定义。

---

## 核对轮裁断（2026-10-20）

开发窗口提交 `PathEditor-v6-M2-核对轮交接.md`（339 行），提出 1 阻塞 + 4 矛盾 + 6 口径 + 3 待裁断。
审核窗口逐条复核后**全部采纳**，修订如下。

| 编号    | 主张                                          | 复核                                                                                                                   | 裁断                                                                                                                                 |
| ------- | --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| **B-1** | Task 8 的测试会写进 `~/.patheditor/profiles/` | ✅ 成立。`load_profile` 走 `profiles_dir()` = `dirs::home_dir()`，不可重定向；`#[cfg(test)]` 特判只对 crate 内单测生效 | **采纳方案 A**：新增 `profiles::load_profile_from(path)`，`load_profile(name)` 变为薄封装。Task 8 已改写                             |
| **C-1** | `Dashboard` 三处定义不一致                    | ✅ 成立。接口 3 字段、实现 1 字段、Self-Review 又提 `top_targets`                                                      | **采纳**：定死为 `{ days, top_targets, profile_applies, current_avg_len, current_path_items }`，三处统一                             |
| **B-2** | `avg_len` 在 `dashboard()` 里算不出来         | ✅ 成立。`daily_stats` 无长度列，`dashboard()` 不接触 `path_entries`                                                   | **采纳方案 B**（T-2）：改查 `path_entries`，不改 spec 表结构                                                                         |
| **C-2** | `index_snapshot_tx` 声明未实现                | ✅ 成立。Interfaces 有、代码块无、Task 12 依赖它                                                                       | **采纳**：Task 2 补齐实现（含幂等清理）                                                                                              |
| **C-3** | Task 5 引入未使用的 `ErrorCode`               | ✅ 成立，**且是两处**（Task 2 第 272 行也有）                                                                          | **采纳**：两处都改为 `use crate::error::CoreError;`，删占位注释                                                                      |
| **C-4** | `TabId` 是「扩展」不是「新增」                | ✅ 成立。`path-capabilities.ts:3` 已有                                                                                 | **采纳**：措辞已改                                                                                                                   |
| **M-1** | `daily_stats.path_items` 口径                 | ✅ 成立                                                                                                                | **采纳**：`Dashboard` 文档注释写清「历史 vs 当前」，并新增 `current_*` 字段                                                          |
| **M-2** | `'localtime'` 依赖进程时区                    | ✅ 成立                                                                                                                | **采纳**：写入 Self-Review 的测试约束                                                                                                |
| **M-3** | 引用查询无快照过滤的不变量是隐式的            | ✅ 成立                                                                                                                | **采纳**：`rebuild_refs` 文档注释写死不变量与破坏后果                                                                                |
| **M-4** | CLI 侧无派生表重建点                          | ✅ 成立（M1 遗留 R-02）                                                                                                | **采纳**：登记为未覆盖项 #2，**M2 不顺手修**                                                                                         |
| **M-5** | spec「首次出现/最近出现」无对应实现           | ✅ 成立。`ScanRunDiff` 只有三字段                                                                                      | **采纳**：增加 `last_seen: Vec<LastSeen>`，不改表                                                                                    |
| **M-6** | Task 6 测试没验证 `daily_stats` 被重建        | ✅ 成立                                                                                                                | **采纳**：测试追加 `SUM(conflicts)` 断言                                                                                             |
| **T-1** | `daily_stats` 重建阈值                        | 开发窗口建议「现在不定阈值，只加耗时日志」                                                                             | **采纳开发窗口建议**：`rebuild_daily` 加 `Instant` + `log::info!` 耗时。理由：定一个现在跑不到的阈值，等于埋一条永远不会被测到的分支 |
| **T-2** | `avg_len` 落点                                | 开发窗口推荐方案 B                                                                                                     | **采纳**：不改 spec §4.1                                                                                                             |
| **T-3** | env 对比推迟的呈现方式                        | 开发窗口建议保留空数组 + 显式文案                                                                                      | **采纳**：比「字段不存在」诚实                                                                                                       |

**未采纳/未处理的**：无。核对轮 14 条全部采纳。

**对核对工具的肯定**：开发窗口在 §9 记录了一版作废稿——三条「阻塞项」是凭印象写的，
逐字比对原文后**全部不成立**。这个自我更正比条目本身更有价值：它展示了一个可用的
判据（「打开文件逐字比对」而不是「凭这类计划容易犯什么错」）。本计划后续修订也应遵守同一判据。

---

## Execution Notes

**（留给开发窗口回填）**

- 偏离记录：
- 未覆盖项：
- 残余风险：
- 实际测试数：
