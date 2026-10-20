# PathEditor v6.0.0 M1 存储与历史 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 core 内建立 SQLCipher 数据库（schema/迁移/密钥/CAS），把 PATH 快照与事件持久化，并把配置方案、备份索引迁入 DB；GUI 通过细粒度 IPC 读写历史。

**Architecture:** 新增 `core/src/db/` 模块（`mod` / `schema` / `crypto` / `history` / `profiles` / `backups`）；`Db` 由 GUI/CLI 启动时创建并显式传入服务层（`HistorySink` trait，测试用 `NoopHistory`）。所有写事务 `BEGIN IMMEDIATE` + `version` CAS。前端只经 `backend.ts` 新命令访问。

**Tech Stack:** Rust 2021、`rusqlite 0.40`（`bundled-sqlcipher-vendored-openssl`）、`blake3`、`uuid`（v7）、`windows-sys 0.61.2`（DPAPI/DACL）、Tauri 2 IPC、React 19 + Zustand。

**Spec:** `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`（§3.2.1 IPC、§4 数据模型、§5 安全、§7 迁移）

**前置文档:** `docs/审核和开发/2026.10.19/PathEditor-v6-M0-探针结果.md`（一文件一测试的约束与证据）

---

## Global Constraints

- **一文件一测试**：任何使用 SQLCipher 的 `.rs` 测试文件**只放一个 `#[test]`**。同一进程内第二个 SQLCipher 测试会 `STATUS_STACK_OVERFLOW`（GNU/MSVC 均已复现，`--test-threads=1` 无效）。见 M0 结果 §5.3。
- **不写真实注册表**：所有 DB/服务测试用 fake IO 或 `tempfile::TempDir`。
- **不删除用户文件**：迁移只读 + 归档 `.v5.bak`，不自动删。
- **不提交、不推送**，除非用户明确授权。
- 中文注释；Rust 4 空格；TS 2 空格、单引号、100 列。
- 质量门：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`npm run verify:all`。
- CLI 单测必须 `cargo test -p patheditor-cli --bins`。
- 所有 `pub fn` / `pub(crate)` 中文 `///` 注释。
- **`cipher_memory_security` 默认 OFF**（M0 实测；ON 在受限进程里 `VirtualLock` 失败）。

---

## File Structure

| 文件                                        | 责任                                                                     |
| ------------------------------------------- | ------------------------------------------------------------------------ |
| `core/Cargo.toml`                           | 新增 `rusqlite` / `blake3` / `uuid` 依赖；`windows-sys` 补 DPAPI feature |
| `core/src/db/mod.rs`                        | `Db` 句柄：open / transaction / PRAGMA / 版本迁移入口                    |
| `core/src/db/schema.rs`                     | DDL 常量 + `user_version` 迁移（只读启动、事务内写）                     |
| `core/src/db/crypto.rs`                     | DPAPI 密钥文件、`open_encrypted`、`sqlcipher_export` 备份                |
| `core/src/db/history.rs`                    | `snapshots` / `path_entries` / `env_values` / `events` 读写              |
| `core/src/db/profiles.rs`                   | 配置元数据/版本（DB 版，替换文件版）                                     |
| `core/src/db/backups.rs`                    | 备份索引（扫描 `backups/`，只 stat + 读 `.pebak` 头）                    |
| `core/src/db/migrate_v5.rs`                 | 旧 JSON/TXT 一次性导入                                                   |
| `core/src/history.rs`                       | `HistorySink` trait + `NoopHistory` + `HistoryEvent`                     |
| `core/src/backup.rs`                        | 改为写 `.pebak`（DPAPI）；旧 `.json` 仍可读                              |
| `core/src/service.rs`                       | `save_path_with_sidecar` 增加 `history: &dyn HistorySink` 参数           |
| `gui/src/commands/history.rs`               | Tauri 命令：`history_*` / `profile_*` / `backup_*`                       |
| `gui/src/commands/mod.rs`                   | 注册新命令                                                               |
| `src/services/backend.ts`                   | 新方法与形状校验 `parseXxx`                                              |
| `src/store/history-store.ts`                | 历史面板状态（分页、筛选、选中）                                         |
| `src/components/history/HistoryPanel.tsx`   | 历史时间线 UI                                                            |
| `src/components/history/SnapshotDetail.tsx` | 快照预览（敏感值掩码 + reveal）                                          |
| `tests/unit/*.test.ts(x)`                   | 形状校验、store、组件测试                                                |

---

### Task 1: `HistorySink` 接缝（零 DB）

**Files:**

- Create: `core/src/history.rs`
- Modify: `core/src/lib.rs`
- Modify: `core/src/service.rs:326`（`save_path_with_sidecar` 签名）
- Modify: `core/src/service.rs:433`（`retry_pending_path_state` 签名，**Task 8 依赖**）
- Modify: `gui/src/commands/service.rs`、`cli/src/runtime.rs`、`core/src/service.rs:314/315/481`（调用点）
- Test: `core/src/history.rs`（`#[cfg(test)]`）

**Interfaces:**

- Produces:
  - `pub struct HistoryEvent { pub source: EventSource, pub action: EventAction, pub hive: Option<EnvHive>, pub target: Option<String>, pub changed_fields: Vec<String>, pub before_revision: Option<String>, pub after_revision: Option<String>, pub result: EventResult }`
  - `pub trait HistorySink { fn record(&self, event: HistoryEvent) -> Result<(), CoreError>; }`
  - `pub struct NoopHistory;`（`record` 恒 `Ok(())`）
  - `save_path_with_sidecar(system, user, history: &dyn HistorySink)`
  - `retry_pending_path_state(history: &dyn HistorySink) -> Result<ApplyOutcome, CoreError>`

> **H5 修正**：`HistorySink: Send + Sync` 与「实现持 `&mut Db`」冲突（`&mut T` 不是 `Sync`）。
> M1 采用 **`&dyn HistorySink` 同步调用**：`DbHistory` 持有 `&mut Db`，但 trait **不要求** `Send + Sync`；
> 若将来需要跨线程，改为 `HistorySink: Send + Sync` + 内部 `Mutex<Db>`，那属于 M3 的并发议题。
> **本 Task 的 trait 定义写为 `pub trait HistorySink { ... }`（无 auto-trait 约束）。**

- [ ] **Step 1: 写失败测试**

在 `core/src/history.rs` 末尾：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_history_accepts_any_event() {
        let sink = NoopHistory;
        let event = HistoryEvent {
            source: EventSource::Gui,
            action: EventAction::Save,
            hive: Some(EnvHive::User),
            target: None,
            changed_fields: vec!["path_entries".to_string()],
            before_revision: None,
            after_revision: Some("abc".to_string()),
            result: EventResult::Success,
        };
        assert!(sink.record(event).is_ok());
    }
}
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `cargo test -p path-editor-core noop_history_accepts_any_event`
Expected: FAIL —— `history` 模块不存在。

- [ ] **Step 3: 实现**

`core/src/history.rs`：

```rust
//! 历史记录接缝：服务层通过 `HistorySink` 上报事件，不直接依赖数据库。
//!
//! 这样 `core::service::save_path_with_sidecar` 无需知道 `Db` 的存在：
//! 生产注入 `DbHistory`（M1 Task 5），测试注入 `NoopHistory`，既有测试零改动。

use crate::env_var::EnvHive;
use crate::error::CoreError;

/// 事件来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventSource {
    Gui,
    Cli,
    Import,
    Restore,
    Migration,
    Sync,
}

/// 事件动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventAction {
    Save,
    Apply,
    Restore,
    Import,
    EnvSet,
    EnvAdd,
    EnvRemove,
    Undo,
    Redo,
}

/// 事件结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    Success,
    Partial,
    Failure,
}

/// 一次可记录的操作。**不得包含环境变量明文值**（spec D6）。
#[derive(Debug, Clone)]
pub struct HistoryEvent {
    pub source: EventSource,
    pub action: EventAction,
    pub hive: Option<EnvHive>,
    pub target: Option<String>,
    /// 只放字段名/摘要，禁止明文值。
    pub changed_fields: Vec<String>,
    pub before_revision: Option<String>,
    pub after_revision: Option<String>,
    pub result: EventResult,
}

/// 历史写入端口。实现方负责持久化；调用方不感知存储形态。
///
/// **H5：不要求 Send + Sync** —— M1 的 `DbHistory` 持有 `&mut Db`（`&mut T` 不是 `Sync`），
/// 同步调用即可。将来若需跨线程，改为 `Mutex<Db>` 并重新评估（M3）。
pub trait HistorySink {
    /// 记录一次事件。失败不应影响主流程（调用方只记日志）。
    fn record(&self, event: HistoryEvent) -> Result<(), CoreError>;
}

/// 无操作实现：测试与「未启用历史」场景使用。
pub struct NoopHistory;

impl HistorySink for NoopHistory {
    fn record(&self, _event: HistoryEvent) -> Result<(), CoreError> {
        Ok(())
    }
}
```

- [ ] **Step 4: 修改服务签名**

`core/src/service.rs`：

```rust
use crate::history::{HistoryEvent, HistorySink};

pub fn save_path_with_sidecar(
    system: Option<PathHiveWrite>,
    user: Option<PathHiveWrite>,
    history: &dyn HistorySink,
) -> Result<ApplyOutcome, CoreError> {
    let outcome = save_path_with_sidecar_io(&RealPathHiveIo, system, user)?;
    // 上报失败不阻断保存（历史是 sidecar 能力）。
    let event = HistoryEvent {
        source: crate::history::EventSource::Gui,
        action: crate::history::EventAction::Save,
        hive: None,
        target: None,
        changed_fields: vec!["path_entries".to_string()],
        before_revision: None,
        after_revision: None,
        result: crate::history::EventResult::Success,
    };
    if let Err(e) = history.record(event) {
        log::warn!("历史写入失败（保存已成功）: {}", e.message);
    }
    Ok(outcome)
}
```

同步更新全部调用点（`core/src/service.rs:314/315/481`、`gui/src/commands/service.rs`、`cli/src/runtime.rs`）：传 `&NoopHistory` 或注入的 sink。

`retry_pending_path_state` 同步加参数（**Task 8 依赖此签名**）：

```rust
pub fn retry_pending_path_state(history: &dyn HistorySink) -> Result<ApplyOutcome, CoreError> {
    // ... 原逻辑不变；补写成功后：
    let _ = history.record(HistoryEvent {
        source: EventSource::Gui,
        action: EventAction::Save,
        hive: None,
        target: None,
        changed_fields: vec!["pending_retry".to_string()],
        before_revision: None,
        after_revision: None,
        result: EventResult::Success,
    });
    Ok(outcome)
}
```

> **调用点清单**（必须全部更新，否则编译失败）：
> `core/src/service.rs`（内部 3 处）、`gui/src/commands/service.rs:19`、`cli/src/runtime.rs:78`、
> 以及 `core` 内所有测试（统一传 `&NoopHistory`）。

- [ ] **Step 5: 运行全量测试**

Run: `cargo test --workspace`
Expected: PASS（既有 227 + 65 不变，新增 1 个）。

- [ ] **Step 6: 提交**

```bash
git add core/src/history.rs core/src/lib.rs core/src/service.rs gui/src/commands cli/src
git commit -m "feat(v6): 新增 HistorySink 接缝，服务层不再直接依赖存储"
```

---

### Task 2: Db 句柄与 schema（加密、PRAGMA、版本）

**Files:**

- Create: `core/src/db/mod.rs`、`core/src/db/schema.rs`
- Modify: `core/Cargo.toml`、`core/src/lib.rs`
- Test: `core/tests/db_schema_open.rs`

**Interfaces:**

- Produces:
  - `pub struct Db { conn: rusqlite::Connection }`
  - `Db::open_encrypted(path: &Path, key_hex: &str) -> Result<Db, CoreError>`
  - `Db::with_transaction<T>(&mut self, f: impl FnOnce(&Transaction) -> Result<T, CoreError>) -> Result<T, CoreError>`
  - `Db::schema_version(&self) -> Result<u32, CoreError>`
  - `Db::migrate_to_current(&mut self) -> Result<(), CoreError>`

- [ ] **Step 1: 写失败测试**

`core/tests/db_schema_open.rs`（**只此一个测试**）：

```rust
// 一文件一测试：SQLCipher 在同一进程内跑第二个测试会 STATUS_STACK_OVERFLOW（M0 §5.3）。
use path_editor_core::db::{schema::CURRENT_SCHEMA_VERSION, Db};
use tempfile::TempDir;

#[test]
fn db_opens_creates_schema_and_reports_version() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.db");
    let key = "ab".repeat(32);

    let mut db = Db::open_encrypted(&path, &key).expect("打开失败");
    assert_eq!(db.schema_version().unwrap(), 0, "新库版本应为 0");

    db.migrate_to_current().expect("迁移失败");
    assert_eq!(
        db.schema_version().unwrap(),
        CURRENT_SCHEMA_VERSION,
        "迁移后版本应为当前版本"
    );

    // 关键表存在
    for table in ["snapshots", "path_entries", "env_values", "events", "profiles"] {
        assert!(db.table_exists(table).unwrap(), "缺少表 {table}");
    }
}
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `cargo test -p path-editor-core --test db_schema_open`
Expected: FAIL —— `path_editor_core::db` 不存在。

- [ ] **Step 3: 加依赖**

`core/Cargo.toml`：

```toml
rusqlite = { version = "0.40", features = ["bundled-sqlcipher-vendored-openssl"] }
blake3 = "1"
uuid = { version = "1", features = ["v7"] }
getrandom = "0.2"
```

`windows-sys` 增加：

```toml
windows-sys = { version = "0.61.2", features = [
  "Win32_System_Environment", "Win32_UI_WindowsAndMessaging", "Win32_Foundation",
  "Win32_Security", "Win32_Security_Authorization", "Win32_Security_Cryptography",
  "Win32_Storage_FileSystem", "Win32_System_Memory",
] }
```

- [ ] **Step 4: 实现 `db/mod.rs` 与 `db/schema.rs`**

`core/src/db/schema.rs`：

```rust
//! Schema DDL 与版本迁移。**启动只读 `user_version`；只在建库/迁移事务内写。**

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// v1 全量 DDL（与 spec §4.1 一致）。
pub const DDL_V1: &str = r#"
CREATE TABLE app_state (
  id                  INTEGER PRIMARY KEY CHECK (id = 1),
  current_snapshot_id INTEGER REFERENCES snapshots(id),
  version             INTEGER NOT NULL DEFAULT 1,
  migrated_v5         INTEGER NOT NULL DEFAULT 0,
  device_id           TEXT NOT NULL,
  lamport             INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE snapshots (
  id           INTEGER PRIMARY KEY,
  uid          TEXT NOT NULL UNIQUE,
  created_at   INTEGER NOT NULL,
  source       TEXT NOT NULL,
  kind         TEXT NOT NULL,
  reason       TEXT,
  content_hash TEXT NOT NULL,
  parent_id    INTEGER REFERENCES snapshots(id),
  note         TEXT,
  updated_at   INTEGER NOT NULL,
  deleted_at   INTEGER
);
CREATE INDEX idx_snapshots_time ON snapshots(created_at);
CREATE INDEX idx_snapshots_hash ON snapshots(content_hash);
CREATE TABLE path_entries (
  snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
  hive        TEXT NOT NULL CHECK (hive IN ('system','user')),
  position    INTEGER NOT NULL,
  path        TEXT NOT NULL,
  enabled     INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, hive, position)
);
CREATE INDEX idx_path_entries_path ON path_entries(path);
CREATE TABLE env_values (
  snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
  hive        TEXT NOT NULL,
  name        TEXT NOT NULL,
  kind        TEXT NOT NULL,
  value       TEXT,
  revision    TEXT NOT NULL,
  sensitive   INTEGER NOT NULL DEFAULT 0,
  restorable  INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (snapshot_id, hive, name),
  CHECK (lower(name) <> 'path')
);
CREATE TABLE events (
  id              INTEGER PRIMARY KEY,
  uid             TEXT NOT NULL UNIQUE,
  created_at      INTEGER NOT NULL,
  source          TEXT NOT NULL,
  action          TEXT NOT NULL,
  hive            TEXT,
  target          TEXT,
  changed_fields  TEXT,
  before_revision TEXT,
  after_revision  TEXT,
  result          TEXT NOT NULL,
  error_code      TEXT,
  snapshot_id     INTEGER REFERENCES snapshots(id),
  device_id       TEXT NOT NULL
);
CREATE INDEX idx_events_time   ON events(created_at);
CREATE INDEX idx_events_target ON events(target, created_at);
CREATE TABLE profiles (
  id              INTEGER PRIMARY KEY,
  uid             TEXT NOT NULL UNIQUE,
  name            TEXT NOT NULL,
  description     TEXT,
  tags            TEXT,
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  last_applied_at INTEGER,
  apply_count     INTEGER NOT NULL DEFAULT 0,
  version         INTEGER NOT NULL DEFAULT 1,
  deleted_at      INTEGER
);
CREATE UNIQUE INDEX uq_profiles_name_active ON profiles(name) WHERE deleted_at IS NULL;
CREATE TABLE profile_revisions (
  id          INTEGER PRIMARY KEY,
  uid         TEXT NOT NULL UNIQUE,
  profile_id  INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  version     INTEGER NOT NULL,
  created_at  INTEGER NOT NULL,
  snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
  UNIQUE (profile_id, version)
);
CREATE TABLE backups (
  id           INTEGER PRIMARY KEY,
  uid          TEXT NOT NULL UNIQUE,
  kind         TEXT NOT NULL,
  created_at   INTEGER NOT NULL,
  content_hash TEXT NOT NULL,
  size_bytes   INTEGER NOT NULL,
  file_path    TEXT NOT NULL,
  snapshot_id  INTEGER REFERENCES snapshots(id),
  deleted_at   INTEGER
);
CREATE INDEX idx_backups_kind_time ON backups(kind, created_at);
CREATE TABLE migration_imports (
  source_path  TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  imported_at  INTEGER NOT NULL,
  PRIMARY KEY (source_path, content_hash)
);
"#;
```

`core/src/db/mod.rs`：

```rust
//! SQLCipher 数据库句柄。**所有写事务 `BEGIN IMMEDIATE`；读-改-写必须带 version CAS。**

pub mod schema;

use crate::error::{CoreError, ErrorCode};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::path::Path;

/// 数据库句柄。
pub struct Db {
    conn: Connection,
}

impl Db {
    /// 用 32 字节十六进制密钥打开（或创建）加密库。
    pub fn open_encrypted(path: &Path, key_hex: &str) -> Result<Self, CoreError> {
        assert_eq!(key_hex.len(), 64, "密钥必须是 32 字节（64 hex）");
        let conn = Connection::open(path).map_err(|e| db_err("open", e))?;
        conn.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";"))
            .map_err(|e| db_err("key", e))?;
        // 先查询一次，让 SQLCipher 用 HMAC 校验密钥。
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
            .map_err(|e| CoreError::new(ErrorCode::Crypto, "db_open", format!("密钥错误或库损坏: {e}")))?;
        conn.execute_batch(
            "PRAGMA cipher_memory_security = OFF;
             PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|e| db_err("pragma", e))?;
        Ok(Self { conn })
    }

    /// 只读版本号。
    pub fn schema_version(&self) -> Result<u32, CoreError> {
        self.conn
            .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .map_err(|e| db_err("user_version", e))
    }

    /// 建库/迁移到当前版本。**只有这里写 user_version。**
    pub fn migrate_to_current(&mut self) -> Result<(), CoreError> {
        let current = self.schema_version()?;
        if current > schema::CURRENT_SCHEMA_VERSION {
            return Err(CoreError::new(
                ErrorCode::Database,
                "migrate",
                format!("库版本 {current} 高于程序支持的 {}", schema::CURRENT_SCHEMA_VERSION),
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| db_err("begin", e))?;
        if current < 1 {
            tx.execute_batch(schema::DDL_V1).map_err(|e| db_err("ddl_v1", e))?;
            tx.pragma_update(None, "user_version", 1).map_err(|e| db_err("set_version", e))?;
        }
        tx.commit().map_err(|e| db_err("commit", e))
    }

    /// 表是否存在（测试/诊断用）。
    pub fn table_exists(&self, name: &str) -> Result<bool, CoreError> {
        let n: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [name],
                |r| r.get(0),
            )
            .map_err(|e| db_err("table_exists", e))?;
        Ok(n > 0)
    }

    /// 在 `BEGIN IMMEDIATE` 事务内执行闭包。
    pub fn with_transaction<T>(
        &mut self,
        f: impl FnOnce(&Transaction) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| db_err("begin", e))?;
        let out = f(&tx)?;
        tx.commit().map_err(|e| db_err("commit", e))?;
        Ok(out)
    }
}

fn db_err(op: &'static str, e: rusqlite::Error) -> CoreError {
    CoreError::new(ErrorCode::Database, op, format!("{op} 失败: {e}"))
}
```

**调用方式（B1 修正）**：`db_err` 是二元函数，不能直接传给 `map_err`（后者要一元 `FnOnce(E) -> F`）。统一写成闭包：

```rust
let conn = Connection::open(path).map_err(|e| db_err("open", e))?;
```

**所有 `.map_err(|e| db_err("xxx", e))` 均改为 `.map_err(|e| db_err("xxx", e))`。**

`core/src/lib.rs` 增加 `pub mod db;`，并在 `Cargo.toml` 加 `tempfile` 到 `[dev-dependencies]`。

- [ ] **Step 5: 运行测试**

Run: `cargo test -p path-editor-core --test db_schema_open`
Expected: PASS。

- [ ] **Step 6: 提交**

```bash
git add core/Cargo.toml core/src/db core/src/lib.rs core/tests/db_schema_open.rs
git commit -m "feat(v6): Db 句柄与 v1 schema（SQLCipher、PRAGMA、版本迁移）"
```

---

### Task 3: DPAPI 密钥与数据库打开流程

**Files:**

- Create: `core/src/db/crypto.rs`
- Test: `core/tests/db_key_lifecycle.rs`

**Interfaces:**

- Produces:
  - `pub struct KeyMaterial { pub db_key: [u8; 32], pub hash_salt: [u8; 16] }`
  - `pub fn load_or_create_key(home: &Path) -> Result<KeyMaterial, CoreError>`
  - `pub fn key_hex(m: &KeyMaterial) -> String`

**密钥存储规格（spec §5.1，必须逐条实现）：**

| 项       | 规定                                                                                    |
| -------- | --------------------------------------------------------------------------------------- |
| 路径     | `~/.patheditor/keys/db.key.dpapi`（`home.join("keys").join("db.key.dpapi")`）           |
| 内容     | `version(1) + db_key(32) + hash_salt(16)` = 49 字节明文，整体 `CryptProtectData` 后落盘 |
| 作用域   | **当前 Windows 用户**（`CRYPTPROTECT_UI_FORBIDDEN`）；换机/换用户不可解密               |
| 首次创建 | **命名互斥量 `Local\PathEditor.KeyInit`** 串行化 + `create_new(true)` 原子创建          |
| DACL     | **仅当前用户 + SYSTEM**；写完立即设置；启动时校验，过宽则告警（不自动改）               |
| 失败语义 | 密钥丢失/DPAPI 不可用 → 明确报错，**不自动重建空库**（否则静默丢历史）                  |

**为什么需要互斥量（不只是 `create_new`）：**

`create_new(true)` 只保证「**谁先创建**」，不保证「**创建后完整**」。两个进程竞争时：

```text
进程 A: create_new 成功 → 开始 write_all（尚未 flush）
进程 B: create_new 失败（AlreadyExists）→ read_key() → 读到半截/空 blob → 解析失败
```

所以必须用命名互斥量把「create_new + write + FlushFileBuffers」包成一个临界区：

```rust
// windows-sys: Win32_System_Threading::{CreateMutexW, WaitForSingleObject, ReleaseMutex}
//              Win32_Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_ABANDONED}
let name = w!("Local\\PathEditor.KeyInit");
let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, name) };
if mutex.is_null() {
    return Err(CoreError::new(ErrorCode::Crypto, "key_mutex", "无法创建密钥互斥量"));
}
let wait = unsafe { WaitForSingleObject(mutex, 5000) };
if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
    unsafe { CloseHandle(mutex) };
    return Err(CoreError::new(ErrorCode::Crypto, "key_mutex", "等待密钥互斥量超时"));
}

let result = (|| {
    // 临界区内：先读已有密钥，再 create_new + write + flush
    if let Some(k) = read_key(path)? { return Ok(k); }
    create_key_locked(path)
})();

unsafe { ReleaseMutex(mutex); CloseHandle(mutex) };
result
```

`create_key_locked` 在临界区内必须：

```rust
let mut f = match fs::OpenOptions::new().write(true).create_new(true).open(path) {
    Ok(f) => f,
    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
        // 互斥量保证此时文件已写完整，可直接读
        return read_key(path)?.ok_or_else(|| CoreError::new(
            ErrorCode::Crypto, "create_key", "密钥文件已存在但无法读取"));
    }
    Err(e) => return Err(io_err("open_key")(e)),
};
f.write_all(&blob).map_err(io_err("write_key"))?;
// 关键：flush 后文件才算完整，释放互斥量前必须完成
f.sync_all().map_err(io_err("sync_key"))?;
drop(f);
set_key_dacl(path)?;   // 见下
```

**DACL 实现（spec §5.1 要求，计划早先遗漏）：**

写完密钥后立即设置 DACL 为「仅当前用户 + SYSTEM」；失败则删除半成品并返回 `Crypto` 错误（不能留下权限过宽的文件）：

```rust
// windows-sys: Win32_Security_Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW
//              Win32_Security::{SetFileSecurityW, DACL_SECURITY_INFORMATION}
// 步骤：取当前进程用户 SID（OpenProcessToken + GetTokenInformation(TokenUser)）
//      拼 SDDL `D:(A;;GA;;;SY)(A;;GA;;;<current-SID>)`
//      ConvertStringSecurityDescriptorToSecurityDescriptorW → SetFileSecurityW
fn set_key_dacl(path: &Path) -> Result<(), CoreError> { /* 见实施细节 */ }

// **不要用 SetNamedSecurityInfoW**（原骨架写法，V-1 整改时纠正）：
// 它要求把 DACL 指针单独传进 pdacl 参数；若按骨架的 `std::ptr::null()`
// 传空，则**静默不设置 DACL**（不是报错）。而 NULL DACL 经
// ConvertSecurityDescriptorToStringSecurityDescriptorW 序列化后的 SDDL 是
// `D:PAI(NO_ACCESS_CONTROL)` —— 一个主体都不含，黑名单判据 `key_dacl_is_restricted`
// 会判定为「受限」并返回 Ok(true)。
//
// 实测（审核窗口 2026-10-20）：
//   MakeNull rc = 0
//   ACL SDDL  = D:PAINO_ACCESS_CONTROL
// 即这一版写法会产生**会打印「通过」的假绿灯**。SetFileSecurityW 直接接受整个
// 安全描述符，语义上不可能出现这个错误。
```

**启动时校验 DACL**：`open_default()` 读一次 ACL，若包含「Everyone / Users / Authenticated Users」等非预期主体，写 `log::warn!` 并在 GUI 状态栏提示（**不自动修改**，避免误伤用户自建目录）。

**为什么 DACL 不能省（DPAPI 已经绑了用户）：**

这是**两个不同的威胁模型**，不能互相替代：

| 威胁                                                   | DPAPI 能挡吗         | DACL 能挡吗 |
| ------------------------------------------------------ | -------------------- | ----------- |
| 同一台机器上的**另一个 Windows 用户**读取密钥内容      | ✅ 能（解不开 blob） | —           |
| 另一个用户**删除或替换**密钥文件（DoS / 诱导重建空库） | ❌ 不能              | ✅ 能       |

**Task 3 必须新增一个测试**：`key_dacl_is_restricted`——读取密钥文件 ACL，断言只有当前用户 SID 与 SYSTEM，不含 `Everyone`/`BUILTIN\Users`。

- [ ] **Step 1: 写失败测试**

`core/tests/db_key_lifecycle.rs`（**只此一个测试**）：

```rust
#![cfg(windows)]
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn key_is_created_once_and_persists_across_reloads() {
    // 用独立目录，避免污染真实 ~/.patheditor
    let dir = TempDir::new().unwrap();

    let a = load_or_create_key(dir.path()).expect("首次创建失败");
    let b = load_or_create_key(dir.path()).expect("二次读取失败");
    assert_eq!(a.db_key, b.db_key, "密钥必须稳定");
    assert_eq!(a.hash_salt, b.hash_salt, "salt 必须稳定");
    assert_ne!(a.db_key, [0u8; 32], "密钥不能全零");

    let db_path = dir.path().join("k.db");
    let mut db = Db::open_encrypted(&db_path, &key_hex(&a)).expect("打开失败");
    db.migrate_to_current().expect("迁移失败");

    // 重新读取密钥后仍能打开
    let c = load_or_create_key(dir.path()).unwrap();
    Db::open_encrypted(&db_path, &key_hex(&c)).expect("重开后失败");
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_key_lifecycle`
Expected: FAIL —— `db::crypto` 不存在。

- [ ] **Step 3: 实现 `core/src/db/crypto.rs`**

```rust
//! DPAPI 保护的本地密钥。**密钥丢失即历史不可读，不得静默重建空库。**

use crate::error::{CoreError, ErrorCode};
use std::path::{Path, PathBuf};

/// 密钥材料：数据库密钥 + 备份去重盐。
pub struct KeyMaterial {
    pub db_key: [u8; 32],
    pub hash_salt: [u8; 16],
}

/// 十六进制表示（用于 `PRAGMA key`）。
pub fn key_hex(m: &KeyMaterial) -> String {
    m.db_key.iter().map(|b| format!("{b:02x}")).collect()
}

/// 配置目录（测试可用 `PATHEDITOR_HOME` 重定向）。
pub(crate) fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("PATHEDITOR_HOME") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
}

/// 读取已有密钥；不存在时原子创建。
///
/// **H4：`home` 必须显式传入，不能用进程级 `std::env::set_var` 重定向** ——
/// `set_var` 是全局状态，会污染同进程其它测试（且 Rust 2024 起 `set_var` 在
/// 多线程下是 UB 风险）。生产入口 `open_default()` 传 `config_dir()`。
pub fn load_or_create_key(home: &Path) -> Result<KeyMaterial, CoreError> {
    let path = home.join("keys").join("db.key.dpapi");
    if let Some(k) = read_key(&path)? {
        return Ok(k);
    }
    create_key(&path)
}

#[cfg(windows)]
fn read_key(path: &Path) -> Result<Option<KeyMaterial>, CoreError> {
    use std::fs;
    if !path.exists() {
        return Ok(None);
    }
    let blob = fs::read(path).map_err(io_err("read_key"))?;
    let plain = dpapi_unprotect(&blob)?;
    parse_material(&plain)
}

#[cfg(windows)]
fn create_key(path: &Path) -> Result<KeyMaterial, CoreError> {
    use std::fs;
    use std::io::Write;
    let mut m = KeyMaterial {
        db_key: [0u8; 32],
        hash_salt: [0u8; 16],
    };
    rand_fill(&mut m.db_key);
    rand_fill(&mut m.hash_salt);
    let mut plain = Vec::with_capacity(1 + 32 + 16);
    plain.push(1u8);
    plain.extend_from_slice(&m.db_key);
    plain.extend_from_slice(&m.hash_salt);
    let blob = dpapi_protect(&plain)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_err("create_key_dir"))?;
    }
    // create_new：多进程竞争时只有一个成功，其余读取已写文件。
    match fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut f) => {
            f.write_all(&blob).map_err(io_err("write_key"))?;
            Ok(m)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            read_key(path)?.ok_or_else(|| {
                CoreError::new(ErrorCode::Crypto, "create_key", "密钥文件已存在但无法读取")
            })
        }
        Err(e) => Err(io_err("open_key")(e)),
    }
}

fn parse_material(plain: &[u8]) -> Result<Option<KeyMaterial>, CoreError> {
    if plain.len() != 49 || plain[0] != 1 {
        return Err(CoreError::new(
            ErrorCode::Crypto,
            "parse_key",
            "密钥文件格式无效",
        ));
    }
    let mut db_key = [0u8; 32];
    let mut hash_salt = [0u8; 16];
    db_key.copy_from_slice(&plain[1..33]);
    hash_salt.copy_from_slice(&plain[33..49]);
    Ok(Some(KeyMaterial { db_key, hash_salt }))
}

fn rand_fill(buf: &mut [u8]) {
    // 用 OS 熵源。`getrandom` 已是 uuid/rusqlite 的传递依赖，不新增体积。
    getrandom::getrandom(buf).expect("OS 熵源不可用");
}

fn io_err(op: &'static str) -> impl Fn(std::io::Error) -> CoreError {
    move |e| CoreError::new(ErrorCode::Io, op, format!("密钥文件操作失败: {e}"))
}

#[cfg(windows)]
fn dpapi_protect(plain: &[u8]) -> Result<Vec<u8>, CoreError> {
    // 复用 M0 探针已验证的调用方式。
    crate::db::dpapi::protect(plain).map_err(|e| CoreError::new(ErrorCode::Crypto, "dpapi", e))
}

#[cfg(windows)]
fn dpapi_unprotect(blob: &[u8]) -> Result<Vec<u8>, CoreError> {
    crate::db::dpapi::unprotect(blob).map_err(|e| CoreError::new(ErrorCode::Crypto, "dpapi", e))
}
```

> **注意**：`getrandom` 需在 `core/Cargo.toml` 显式声明（虽是传递依赖，但**不要依赖传递依赖的 API**）。加入：`getrandom = "0.2"`。

- [ ] **Step 4: 把 M0 的 DPAPI 模块移入 core**

创建 `core/src/db/dpapi.rs`，内容与 `probes/v6-sqlcipher/src/lib.rs` 的 `dpapi` 模块一致（`protect` / `unprotect`），并在 `db/mod.rs` 加 `mod dpapi;`。

- [ ] **Step 5: 运行测试**

Run: `cargo test -p path-editor-core --test db_key_lifecycle`
Expected: PASS。

- [ ] **Step 6: 提交**

```bash
git add core/src/db core/tests/db_key_lifecycle.rs core/Cargo.toml
git commit -m "feat(v6): DPAPI 密钥生命周期（create_new 原子创建、PATHEDITOR_HOME 可重定向）"
```

---

### Task 4: 快照与事件读写（`history.rs`）

**Files:**

- Create: `core/src/db/history.rs`
- Test: `core/tests/db_history_roundtrip.rs`

**Interfaces:**

- Produces:
  - `pub struct NewSnapshot { pub kind: SnapshotKind, pub source: EventSource, pub system: Vec<PathEntry>, pub user: Vec<PathEntry>, pub env: Vec<EnvValueRow>, pub parent_uid: Option<String> }`
  - `pub fn insert_snapshot(db: &mut Db, snap: NewSnapshot) -> Result<String, CoreError>`（返回 uid）
  - `pub fn list_events(db: &Db, filter: &EventFilter) -> Result<EventPage, CoreError>`
  - `pub fn get_snapshot(db: &Db, uid: &str, reveal: bool) -> Result<SnapshotView, CoreError>`
  - `pub fn current_snapshot(db: &Db) -> Result<Option<SnapshotView>, CoreError>`

- [ ] **Step 1: 写失败测试**

`core/tests/db_history_roundtrip.rs`：

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{
    get_snapshot, insert_snapshot, list_events, EnvValueRow, EventFilter, NewSnapshot, SnapshotKind,
};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn snapshot_roundtrip_and_masking() {
    let dir = TempDir::new().unwrap();

    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("h.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![PathEntry { path: "C:\\Windows".into(), enabled: true }],
            user: vec![PathEntry { path: "C:\\Tools".into(), enabled: false }],
            env: vec![EnvValueRow {
                hive: "user".into(),
                name: "MY_TOKEN".into(),
                kind: "String".into(),
                value: Some("secret".into()),
                revision: "abc".into(),
                sensitive: true,
                restorable: true,
            }],
            parent_uid: None,
        },
    )
    .unwrap();

    // 掩码路径：sensitive=1 且 reveal=false → value 必须为 None
    let masked = get_snapshot(&db, &uid, false).unwrap();
    assert_eq!(masked.system.len(), 1);
    assert_eq!(masked.user[0].enabled, false);
    assert_eq!(masked.env[0].value, None, "敏感值默认必须被掩码");

    // reveal 路径才返回明文
    let revealed = get_snapshot(&db, &uid, true).unwrap();
    assert_eq!(revealed.env[0].value.as_deref(), Some("secret"));

    // B2：insert_snapshot 不写事件；事件由 HistorySink 负责（Task 5）
    assert!(
        list_events(&db, &EventFilter::default()).unwrap().items.is_empty(),
        "insert_snapshot 不应自行写事件"
    );
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_history_roundtrip`
Expected: FAIL。

- [ ] **Step 3: 实现 `core/src/db/history.rs`**

要点（完整代码在实施时补齐，这里给出结构与签名）：

```rust
//! 快照与事件读写。**events 不得含 env 明文值（spec D6）。**

use super::Db;
use crate::error::{CoreError, ErrorCode};
use crate::history::{EventAction, EventResult, EventSource};
use crate::PathEntry;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotKind { Current, Profile, Backup, History }

#[derive(Debug, Clone)]
pub struct EnvValueRow {
    pub hive: String,
    pub name: String,
    pub kind: String,
    pub value: Option<String>,
    pub revision: String,
    /// **H2：`sensitive` 不由调用方传入** —— 写入前由 core 用 `env_var::is_sensitive(&name)`
    /// 重新计算并覆盖，避免调用方漏标导致掩码失效。
    pub sensitive: bool,
    pub restorable: bool,
}

#[derive(Debug, Clone)]
pub struct NewSnapshot {
    pub kind: SnapshotKind,
    pub source: EventSource,
    pub system: Vec<PathEntry>,
    pub user: Vec<PathEntry>,
    pub env: Vec<EnvValueRow>,
    pub parent_uid: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SnapshotView {
    pub uid: String,
    pub created_at: i64,
    pub kind: String,
    pub system: Vec<PathEntry>,
    pub user: Vec<PathEntry>,
    pub env: Vec<EnvValueRow>,
}

#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    pub limit: u32,
    pub offset: u32,
    pub action: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EventView {
    pub uid: String,
    pub created_at: i64,
    pub source: String,
    pub action: EventAction,
    pub result: EventResult,
}

#[derive(Debug, Clone, Default)]
pub struct EventPage {
    pub items: Vec<EventView>,
    /// 满足条件的**总行数**（不是本页条数）。必须单独 `SELECT count(*)`，
    /// 不能拿 `items.len()` 充数（H1）。
    pub total: u32,
}

/// 插入快照 + 条目。**不写 events** —— 事件由 `HistorySink`（Task 5）统一负责，
/// 避免同一操作产生两条事件（B2）。
pub fn insert_snapshot(db: &mut Db, snap: NewSnapshot) -> Result<String, CoreError> {
    db.with_transaction(|tx| insert_snapshot_tx(tx, snap))
}

/// 事务内版本：供 Task 6（配置保存需要快照 + profile 同一事务）复用（H6）。
pub fn insert_snapshot_tx(tx: &Transaction, snap: NewSnapshot) -> Result<String, CoreError> {
    let uid = Uuid::now_v7().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let hash = content_hash(&snap);
    tx.execute(
        "INSERT INTO snapshots (uid, created_at, source, kind, content_hash, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?2)",
        params![uid, now, source_str(snap.source), kind_str(snap.kind), hash],
    )
    .map_err(|e| db_err("insert_snapshot", e))?;
    let id = tx.last_insert_rowid();
    for (i, e) in snap.system.iter().enumerate() {
        tx.execute(
            "INSERT INTO path_entries (snapshot_id, hive, position, path, enabled) VALUES (?1,'system',?2,?3,?4)",
            params![id, i as i64, e.path, e.enabled as i64],
        ).map_err(|e| db_err("insert_path", e))?;
    }
    for (i, e) in snap.user.iter().enumerate() {
        tx.execute(
            "INSERT INTO path_entries (snapshot_id, hive, position, path, enabled) VALUES (?1,'user',?2,?3,?4)",
            params![id, i as i64, e.path, e.enabled as i64],
        ).map_err(|e| db_err("insert_path", e))?;
    }
    for v in &snap.env {
        // H2：sensitive 以 core 判定为准，忽略调用方传值。
        let sensitive = crate::env_var::is_sensitive(&v.name);
        tx.execute(
            "INSERT INTO env_values (snapshot_id, hive, name, kind, value, revision, sensitive, restorable)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![id, v.hive, v.name, v.kind, v.value, v.revision, sensitive as i64, v.restorable as i64],
        ).map_err(|e| db_err("insert_env", e))?;
    }
    Ok(uid)
}

/// 内容哈希：**必须区分 hive**（H3）。`system=[A]` 与 `user=[A]` 是不同状态。
fn content_hash(snap: &NewSnapshot) -> String {
    let mut h = blake3::Hasher::new();
    for (hive, entries) in [("system", &snap.system), ("user", &snap.user)] {
        h.update(hive.as_bytes());
        h.update(&[0xff]); // 分隔符，防 "system"+"C:" 与 "systemC:" 碰撞
        for e in entries.iter() {
            h.update(e.path.as_bytes());
            h.update(&[e.enabled as u8, 0xfe]);
        }
    }
    for v in &snap.env {
        h.update(v.hive.as_bytes());
        h.update(v.name.as_bytes());
        if let Some(val) = &v.value {
            h.update(val.as_bytes());
        }
        h.update(&[0xfd]);
    }
    h.finalize().to_hex().to_string()
}
```

`get_snapshot` / `list_events` / `current_snapshot` 按同样风格实现；`reveal=false` 时对 `sensitive=1` 的 env 行把 `value` 置 `None`。

**H1 分页实现（`list_events`）**：`total` 必须是独立计数查询，不能等于本页条数：

```rust
pub fn list_events(db: &Db, filter: &EventFilter) -> Result<EventPage, CoreError> {
    let limit = filter.limit.clamp(1, 500) as i64;
    let offset = filter.offset as i64;

    // 1) 总数（同样的 WHERE 条件，不带 LIMIT）
    let total: i64 = db.query_row_with(
        "SELECT count(*) FROM events WHERE (?1 IS NULL OR action = ?1) AND (?2 IS NULL OR source = ?2)",
        params![filter.action, filter.source],
        |r| r.get(0),
    )?;

    // 2) 本页
    let mut stmt = db.prepare(
        "SELECT uid, created_at, source, action, result FROM events
         WHERE (?1 IS NULL OR action = ?1) AND (?2 IS NULL OR source = ?2)
         ORDER BY created_at DESC, id DESC
         LIMIT ?3 OFFSET ?4",
    )?;
    let items = stmt
        .query_map(params![filter.action, filter.source, limit, offset], |r| {
            Ok(EventView { /* ... */ })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(EventPage { items, total: total as u32 })
}
```

> 测试必须覆盖：插入 3 条事件、`limit=2` 时 `items.len()==2` 且 `total==3`。只断言 `items.len()` 会漏掉 H1。

- [ ] **Step 4: 运行测试**

Run: `cargo test -p path-editor-core --test db_history_roundtrip`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add core/src/db/history.rs core/tests/db_history_roundtrip.rs
git commit -m "feat(v6): 快照与事件读写（单事务、敏感值掩码、内容哈希）"
```

---

### Task 5: `DbHistory` 接入服务层

**Files:**

- Create: `core/src/db/history_sink.rs`
- Modify: `core/src/db/mod.rs`
- Test: `core/tests/db_history_sink.rs`

**Interfaces:**

- Produces: `pub struct DbHistory<'a>(pub &'a mut Db);` 实现 `HistorySink`

- [ ] **Step 1: 写失败测试**

`core/tests/db_history_sink.rs`：

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history_sink::DbHistory;
use path_editor_core::db::history::{list_events, EventFilter};
use path_editor_core::db::Db;
use path_editor_core::history::{EventAction, EventResult, EventSource, HistoryEvent};
use tempfile::TempDir;

#[test]
fn db_history_records_event() {
    let dir = TempDir::new().unwrap();

    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("s.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    {
        let sink = DbHistory(&mut db);
        use path_editor_core::history::HistorySink;
        sink.record(HistoryEvent {
            source: EventSource::Gui,
            action: EventAction::Save,
            hive: None,
            target: None,
            changed_fields: vec!["path_entries".into()],
            before_revision: None,
            after_revision: None,
            result: EventResult::Success,
        })
        .unwrap();
    }

    let events = list_events(&db, &EventFilter::default()).unwrap();
    assert_eq!(events.items.len(), 1);
    assert_eq!(events.items[0].action, EventAction::Save);
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_history_sink`
Expected: FAIL。

- [ ] **Step 3: 实现**

```rust
//! 把 `HistorySink` 落到数据库。**record 失败不阻断主流程**，调用方只记 warn。

use super::Db;
use crate::error::CoreError;
use crate::history::{EventResult, HistoryEvent, HistorySink};
use rusqlite::params;
use uuid::Uuid;

/// 数据库实现的历史端口。
pub struct DbHistory<'a>(pub &'a mut Db);

impl HistorySink for DbHistory<'_> {
    fn record(&self, event: HistoryEvent) -> Result<(), CoreError> {
        // SAFETY 说明：`DbHistory` 持有 &mut Db，record 串行调用；事务内部完成写入。
        let db = self.0;
        let now = chrono::Utc::now().timestamp_millis();
        db.with_transaction(|tx| {
            tx.execute(
                "INSERT INTO events (uid, created_at, source, action, hive, target,
                                     changed_fields, before_revision, after_revision,
                                     result, device_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'local')",
                params![
                    Uuid::now_v7().to_string(),
                    now,
                    source_str(event.source),
                    action_str(event.action),
                    event.hive.map(|_| "user"),
                    event.target,
                    serde_json::to_string(&event.changed_fields).unwrap_or_else(|_| "[]".into()),
                    event.before_revision,
                    event.after_revision,
                    result_str(event.result),
                ],
            )
            .map_err(|e| CoreError::new(crate::error::ErrorCode::Database, "record", e.to_string()))?;
            Ok(())
        })
    }
}
```

`source_str` / `action_str` / `result_str` 与 `history.rs` 共用（提到 `core/src/db/mod.rs` 或 `history.rs` 的 `pub(crate)` 函数）。

- [ ] **Step 4: 运行测试**

Run: `cargo test -p path-editor-core --test db_history_sink`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add core/src/db core/tests/db_history_sink.rs
git commit -m "feat(v6): DbHistory 实现 HistorySink，事件落库"
```

---

### Task 6: 配置方案迁入 DB（含版本与软删除）

**Files:**

- Create: `core/src/db/profiles.rs`
- Test: `core/tests/db_profiles.rs`

**Interfaces:**

- Produces:
  - `pub fn save_profile(db: &mut Db, name: &str, description: Option<&str>, tags: &[String], snap: NewSnapshot) -> Result<String, CoreError>`
  - `pub fn list_profiles(db: &Db) -> Result<Vec<ProfileView>, CoreError>`
  - `pub fn delete_profile(db: &mut Db, uid: &str) -> Result<(), CoreError>`（软删除）
  - `pub fn apply_profile(db: &mut Db, uid: &str) -> Result<(), CoreError>`（`apply_count+1`、`last_applied_at`）

- [ ] **Step 1: 写失败测试**

`core/tests/db_profiles.rs`：

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
use path_editor_core::db::profiles::{delete_profile, list_profiles, save_profile};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use tempfile::TempDir;

#[test]
fn profile_create_delete_recreate_with_same_name() {
    let dir = TempDir::new().unwrap();

    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("p.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let snap = |db: &mut Db| NewSnapshot {
        kind: SnapshotKind::Profile,
        source: EventSource::Gui,
        system: vec![],
        user: vec![],
        env: vec![],
        parent_uid: None,
    };

    let uid1 = save_profile(&mut db, "Work", Some("公司"), &["office".into()], snap(&mut db)).unwrap();
    let list = list_profiles(&db).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].description.as_deref(), Some("公司"));

    delete_profile(&mut db, &uid1).unwrap();
    assert!(list_profiles(&db).unwrap().is_empty(), "软删除后不应出现在列表");

    // 软删除后允许同名重建（B5 的部分唯一索引）
    let uid2 = save_profile(&mut db, "Work", None, &[], snap(&mut db)).unwrap();
    assert_ne!(uid1, uid2);
    assert_eq!(list_profiles(&db).unwrap().len(), 1);
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_profiles`
Expected: FAIL。

- [ ] **Step 3: 实现**

要点：

- `save_profile`：在**同一个 `with_transaction` 内**依次调用 `insert_snapshot_tx(tx, snap)`（Task 4 已提供，接受 `&Transaction`）、`INSERT INTO profiles`、`INSERT INTO profile_revisions`。**不要**调用 `insert_snapshot`——它自带事务，嵌套会失败（H6）。
- `delete_profile`：`UPDATE profiles SET deleted_at=? WHERE uid=?`（软删除；`uq_profiles_name_active` 允许同名重建）。
- `apply_profile`：`UPDATE profiles SET apply_count=apply_count+1, last_applied_at=? WHERE uid=?`；事件另由 `HistorySink` 记录（不在本函数里写 events，**同 B2 口径**）。
- `list_profiles`：`WHERE deleted_at IS NULL ORDER BY updated_at DESC`；`tags` 用 `serde_json` 反序列化。

- [ ] **Step 4: 运行测试**

Run: `cargo test -p path-editor-core --test db_profiles`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add core/src/db/profiles.rs core/tests/db_profiles.rs
git commit -m "feat(v6): 配置方案迁入 DB（标签、描述、应用计数、软删除同名重建）"
```

---

### Task 7: 备份索引与 `.pebak` 格式

**Files:**

- Modify: `core/src/backup.rs`
- Create: `core/src/db/backups.rs`
- Test: `core/tests/db_backups_index.rs`

**Interfaces:**

- Produces:
  - `pub fn write_env_backup_pebak(dir: &Path, payload: &EnvBackupPayload, material: &KeyMaterial) -> Result<PathBuf, CoreError>`
  - `pub fn read_env_backup_any(path: &Path, material: &KeyMaterial) -> Result<EnvBackupPayload, CoreError>`（`.pebak` 解密 / `.json` 兼容）
  - `pub fn index_backups(db: &mut Db, dir: &Path) -> Result<usize, CoreError>`

- [ ] **Step 1: 写失败测试**

`core/tests/db_backups_index.rs`：

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::backups::{index_backups, list_backups, write_env_backup_pebak};
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn pebak_backup_is_indexed_without_decrypting() {
    let dir = TempDir::new().unwrap();

    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("b.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let backup_dir = dir.path().join("backups");
    // B4：`EnvBackupPayload` 没有 `empty_for_test` 构造器（已 grep 确认无 `impl EnvBackupPayload`）。
    // 用 `Default` + 显式字段；若 payload 未实现 Default，先给它加 `#[derive(Default)]`
    // （`EnvBackupHives` 同理），或直接构造：
    let payload = path_editor_core::backup::EnvBackupPayload {
        captured_at: 1_700_000_000_000,
        hives: path_editor_core::backup::EnvBackupHives {
            system: vec![],
            user: vec![
                path_editor_core::backup::EnvBackupVar {
                    name: "A".into(),
                    kind: path_editor_core::EnvValueKind::String,
                    value: "1".into(),
                    revision: "r1".into(),
                },
                path_editor_core::backup::EnvBackupVar {
                    name: "B".into(),
                    kind: path_editor_core::EnvValueKind::String,
                    value: "2".into(),
                    revision: "r2".into(),
                },
            ],
        },
    };
    let path = write_env_backup_pebak(&backup_dir, &payload, &key).unwrap();
    assert!(path.extension().unwrap().eq_ignore_ascii_case("pebak"));

    // 文件头不应含 SQLite/JSON 明文标记
    let head = std::fs::read(&path).unwrap();
    assert_eq!(&head[..5], b"PEBK1", "magic 应为 PEBK1");

    let n = index_backups(&mut db, &backup_dir).unwrap();
    assert_eq!(n, 1);
    let list = list_backups(&db).unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].encrypted);
    assert_eq!(list[0].variable_count, 2, "变量数应来自文件头，无需解密");
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_backups_index`
Expected: FAIL。

- [ ] **Step 3: 实现 `.pebak` 格式**

文件头（明文）：`magic "PEBK1"(5) + version(1) + created_at(8, LE) + kind(1) + variable_count(4, LE) + plaintext_len(4, LE) + payload_hash(32)` = 55 字节；正文为 DPAPI 加密的 JSON。

```rust
//! `.pebak`：DPAPI 加密的 env 备份。**头部不含秘密，索引可不读正文。**
const MAGIC: &[u8; 5] = b"PEBK1";
pub const HEADER_LEN: usize = 5 + 1 + 8 + 1 + 4 + 4 + 32;
```

- [ ] **Step 4: 实现 `index_backups` / `list_backups`**

扫描目录：`.pebak` 读头（magic、variable_count、payload_hash）；`.json` 解析内容（迁移期兼容）；写 `backups` 表（`uid` 用 `Uuid::now_v7()`，`content_hash` 用头里的 `payload_hash`）。

- [ ] **Step 5: 运行测试**

Run: `cargo test -p path-editor-core --test db_backups_index`
Expected: PASS。

- [ ] **Step 6: 提交**

```bash
git add core/src/backup.rs core/src/db/backups.rs core/tests/db_backups_index.rs
git commit -m "feat(v6): env 备份改为 .pebak（DPAPI），备份索引无需解密"
```

---

### Task 8: v5 迁移（一次性导入 + 幂等）

**Files:**

- Create: `core/src/db/migrate_v5.rs`
- Test: `core/tests/db_migrate_v5.rs`

**Interfaces:**

- Produces: `pub fn migrate_legacy(db: &mut Db, home: &Path) -> Result<MigrationReport, CoreError>`

- [ ] **Step 1: 写失败测试**

`core/tests/db_migrate_v5.rs`：

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::migrate_v5::migrate_legacy;
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn legacy_files_import_once_and_are_idempotent() {
    let dir = TempDir::new().unwrap();

    // B3：旧文件是 `Versioned<T>` 信封（persist.rs:38，serde flatten），
    // 正常路径必须带 schemaVersion；裸 JSON 只是「无版本头的旧格式」，
    // 两种都要覆盖。
    std::fs::create_dir_all(dir.path().join("profiles")).unwrap();
    std::fs::write(
        dir.path().join("disabled.json"),
        r#"{"schemaVersion":1,"system":[],"user":[],"systemSnapshot":[{"path":"C:\\A","enabled":true}],"userSnapshot":[]}"#,
    ).unwrap();
    std::fs::write(
        dir.path().join("profiles").join("Work.json"),
        r#"{"schemaVersion":1,"name":"Work","sys":[],"user":[],"created":"2026-01-01","modified":"2026-01-01"}"#,
    ).unwrap();
    // 无版本头的旧格式（v1 之前）也必须能导入
    std::fs::write(
        dir.path().join("profiles").join("Legacy.json"),
        r#"{"name":"Legacy","sys":[],"user":[],"created":"2025-01-01","modified":"2025-01-01"}"#,
    ).unwrap();

    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("m.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let r1 = migrate_legacy(&mut db, dir.path()).unwrap();
    assert_eq!(r1.imported, 3, "disabled + 2 profiles（含无版本头）");

    let r2 = migrate_legacy(&mut db, dir.path()).unwrap();
    assert_eq!(r2.imported, 0, "重复迁移必须幂等");
    assert_eq!(r2.skipped, 3);
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_migrate_v5`
Expected: FAIL。

- [ ] **Step 3: 实现**

顺序（spec §7）：

1. 先 `crate::service::retry_pending_path_state(&NoopHistory)`（H3）；
2. `disabled.json` → `snapshots(kind=current)` + `path_entries`；
3. `profiles/*.json` → `profiles` + `profile_revisions` + `snapshots`；
4. `env_backup_*.json` → `snapshots(kind=backup)` + `env_values`（排除 `Path`，`Unsupported` 只存元数据）；
5. `path_backup_*.txt` → `snapshots(kind=backup)` + `path_entries`；
6. 每份导入在 `migration_imports(source_path, content_hash)` 里登记；
7. 完成后 `UPDATE app_state SET migrated_v5=1`；
8. 原件**不改名、不删除**（`.v5.bak` 归档留给 M4）。

- [ ] **Step 4: 运行测试**

Run: `cargo test -p path-editor-core --test db_migrate_v5`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add core/src/db/migrate_v5.rs core/tests/db_migrate_v5.rs
git commit -m "feat(v6): v5 文件一次性迁移（幂等、排除 Path、pending 优先）"
```

---

### Task 9: GUI IPC 命令（`history_*` / `profile_*` / `backup_*`）

**Files:**

- Create: `gui/src/commands/history.rs`
- Modify: `gui/src/commands/mod.rs`、`gui/src/lib.rs`
- Test: `gui/src/commands/history.rs`（`#[cfg(test)]`，形状测试）

**Interfaces:**

- Produces: 11 个 `#[tauri::command]`，签名与 spec §3.2.1 一致。

- [ ] **Step 1: 写失败测试**

在 `gui/src/commands/history.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_filter_defaults_are_bounded() {
        let f = EventFilter::from_request(None);
        assert_eq!(f.limit, 50);
        assert_eq!(f.offset, 0);
        let f = EventFilter::from_request(Some(EventFilterRequest { limit: Some(9999), offset: Some(0), action: None, source: None }));
        assert_eq!(f.limit, 500, "上限必须钳制到 500");
    }
}
```

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p patheditor event_filter_defaults_are_bounded`
Expected: FAIL。

- [ ] **Step 3: 实现命令**

```rust
//! 历史 / 配置 / 备份的 Tauri 命令。**只做参数转换与错误呈现**，规则在 core。

use path_editor_core::db::history::{self, EventFilter};
use path_editor_core::CoreError;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

/// 全局 DB 句柄（由 `lib.rs` 启动时初始化）。
pub struct DbState(pub Mutex<Option<path_editor_core::db::Db>>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventFilterRequest {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub action: Option<String>,
    pub source: Option<String>,
}

/// 见 spec §3.2.1：默认 50，上限 500。
pub fn from_request(req: Option<EventFilterRequest>) -> EventFilter {
    let req = req.unwrap_or(EventFilterRequest { limit: None, offset: None, action: None, source: None });
    EventFilter {
        limit: req.limit.unwrap_or(50).min(500),
        offset: req.offset.unwrap_or(0),
        action: req.action,
        source: req.source,
    }
}

#[tauri::command]
pub fn history_list_events(
    filter: Option<EventFilterRequest>,
    state: State<'_, DbState>,
) -> Result<history::EventPage, CoreError> {
    let guard = state.0.lock().map_err(|_| CoreError::new(
        path_editor_core::ErrorCode::Internal, "history_list_events", "DB 状态锁中毒"))?;
    let db = guard.as_ref().ok_or_else(|| CoreError::new(
        path_editor_core::ErrorCode::Database, "history_list_events", "历史库不可用（降级模式）"))?;
    history::list_events(db, &from_request(filter))
}
```

其余 10 个命令同风格。**所有命令在降级模式返回 `Database`/`Crypto` 错误**（B7）。

- [ ] **Step 4: 注册命令**

`gui/src/lib.rs` 的 `invoke_handler` 增加 11 个命令；启动时初始化 `DbState`：

```rust
// 启动序列（spec §7.1）：① pending 补写 ② schema 迁移 ③ v5 文件导入
let db = match path_editor_core::db::open_default() {
    Ok(mut db) => match setup_database(&mut db) {
        Ok(()) => Some(db),
        Err(e) => { log::error!("历史库初始化失败，进入降级模式: {}", e.message); None }
    },
    Err(e) => { log::warn!("历史库打开失败，进入降级模式: {}", e.message); None }
};
app.manage(DbState(Mutex::new(db)));
```

其中 `setup_database` 三步**缺一不可**（V-2 整改的教训：首版计划漏了第 ③ 步，
导致 `migrate_legacy` 在生产中根本没有调用者，v5 文件永远不会被迁移）：

```rust
fn setup_database(db: &mut Db) -> Result<(), CoreError> {
    // ① best-effort 补写 pending（失败只告警，不阻断启动）
    if let Err(e) = service::retry_pending_path_state(&NoopHistory) {
        log::warn!("启动时补写 pending 失败（继续启动）: {}", e.message);
    }
    // ② schema 建库/迁移
    db.migrate_to_current()?;
    // ③ v5 遗留文件一次性导入（幂等；失败只告警）
    match migrate_v5::migrate_legacy(db, &crypto::config_dir()) {
        Ok(r) => log::info!("v5 迁移：导入 {} 跳过 {} 失败 {}", r.imported, r.skipped, r.failed),
        Err(e) => log::warn!("v5 文件迁移失败（继续启动）: {}", e.message),
    }
    Ok(())
}
```

`core/src/db/mod.rs` 增加：

```rust
/// 用默认路径与 DPAPI 密钥打开；失败返回错误（GUI 据此进入降级模式）。
pub fn open_default() -> Result<Db, CoreError> {
    let home = crypto::config_dir();
    let material = crypto::load_or_create_key(&home)?;
    let path = home.join("patheditor.db");
    Db::open_encrypted(&path, &crypto::key_hex(&material))
}
```

- [ ] **Step 5: 运行测试**

Run: `cargo test -p patheditor`
Expected: PASS。

- [ ] **Step 6: 提交**

```bash
git add gui/src core/src/db
git commit -m "feat(v6): 历史/配置/备份 IPC 命令（细粒度、分页钳制、降级错误）"
```

---

### Task 10: 前端 `backend.ts` 与形状校验

**Files:**

- Modify: `src/services/backend.ts`
- Test: `tests/unit/backend-history.test.ts`

**Interfaces:**

- Produces: `historyListEvents` / `historyGetSnapshot` / `historyRestoreSnapshot` / `historyListSnapshots` / `historyGetCurrent` / `profileList` / `profileSave` / `profileApply` / `profileDelete` / `profileExportV5` / `backupList`

- [ ] **Step 1: 写失败测试**

`tests/unit/backend-history.test.ts`：

```ts
import { describe, it, expect } from 'vitest';
import { parseEventPage } from '@/services/backend';

describe('parseEventPage', () => {
  it('拒绝缺少 items 的响应', () => {
    expect(() => parseEventPage({ total: 1 })).toThrow(/无效/);
  });

  it('拒绝 items 中 action 非法的项', () => {
    expect(() => parseEventPage({ items: [{ uid: 'a', action: 123 }], total: 1 })).toThrow(/无效/);
  });

  it('接受合法响应', () => {
    const page = parseEventPage({
      items: [{ uid: 'a', createdAt: 1, source: 'gui', action: 'save', result: 'success' }],
      total: 1,
    });
    expect(page.items).toHaveLength(1);
  });
});
```

- [ ] **Step 2: 运行，确认失败**

Run: `npx vitest run tests/unit/backend-history.test.ts`
Expected: FAIL —— `parseEventPage` 不存在。

- [ ] **Step 3: 实现**

```ts
export interface EventView {
  uid: string;
  createdAt: number;
  source: string;
  action: string;
  result: string;
}

export interface EventPage {
  items: EventView[];
  total: number;
}

const EVENT_ACTIONS = new Set([
  'save', 'apply', 'restore', 'import', 'envSet', 'envAdd', 'envRemove', 'undo', 'redo',
]);
const EVENT_RESULTS = new Set(['success', 'partial', 'failure']);

export function parseEventPage(value: unknown): EventPage {
  if (!isRecord(value) || !Array.isArray(value.items) || typeof value.total !== 'number') {
    throw new Error('history_list_events 返回了无效的分页契约');
  }
  const items = value.items.map((raw) => {
    if (!isRecord(raw)) throw new Error('事件项形状无效');
    const { uid, createdAt, source, action, result } = raw;
    if (typeof uid !== 'string' || typeof createdAt !== 'number') {
      throw new Error('事件项 uid/createdAt 形状无效');
    }
    if (typeof source !== 'string' || typeof action !== 'string' || !EVENT_ACTIONS.has(action)) {
      throw new Error('事件项 action 非法');
    }
    if (typeof result !== 'string' || !EVENT_RESULTS.has(result)) {
      throw new Error('事件项 result 非法');
    }
    return { uid, createdAt, source, action, result };
  });
  return { items, total: value.total };
}

// backend 对象内
historyListEvents: async (filter?: EventFilter) =>
  parseEventPage(await parseCoreError(invoke<unknown>('history_list_events', { filter }))),
```

其余命令同样加 `parseXxx`。

- [ ] **Step 4: 运行测试**

Run: `npx vitest run tests/unit/backend-history.test.ts`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add src/services/backend.ts tests/unit/backend-history.test.ts
git commit -m "feat(v6): 前端历史 IPC 方法与运行时形状校验"
```

---

### Task 11: 历史面板 UI（最小可用）

**Files:**

- Create: `src/store/history-store.ts`、`src/components/history/HistoryPanel.tsx`、`src/components/history/SnapshotDetail.tsx`
- Modify: `src/components/layout/AppShell.tsx`（入口按钮）
- Test: `tests/unit/history-store.test.ts`

**Interfaces:**

- Consumes: Task 10 的 `backend.history*`
- Produces: `<HistoryPanel />`，可在主界面打开

- [ ] **Step 1: 写失败测试**

`tests/unit/history-store.test.ts`：

```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useHistoryStore } from '@/store/history-store';

vi.mock('@/services/backend', () => ({
  backend: {
    historyListEvents: vi.fn(async () => ({ items: [], total: 0 })),
    historyGetSnapshot: vi.fn(),
    historyRestoreSnapshot: vi.fn(),
  },
}));

describe('history store', () => {
  beforeEach(() => useHistoryStore.setState({ events: [], total: 0, loading: false }));

  it('load 写入事件与总数', async () => {
    const { backend } = await import('@/services/backend');
    (backend.historyListEvents as any).mockResolvedValueOnce({
      items: [{ uid: 'a', createdAt: 1, source: 'gui', action: 'save', result: 'success' }],
      total: 1,
    });
    await useHistoryStore.getState().load();
    expect(useHistoryStore.getState().events).toHaveLength(1);
    expect(useHistoryStore.getState().total).toBe(1);
  });
});
```

- [ ] **Step 2: 运行，确认失败**

Run: `npx vitest run tests/unit/history-store.test.ts`
Expected: FAIL。

- [ ] **Step 3: 实现 store 与组件**

`src/store/history-store.ts`：

```ts
import { create } from 'zustand';
import { backend, type EventView } from '@/services/backend';

interface HistoryState {
  events: EventView[];
  total: number;
  loading: boolean;
  error: string | null;
  load: () => Promise<void>;
}

export const useHistoryStore = create<HistoryState>((set) => ({
  events: [],
  total: 0,
  loading: false,
  error: null,
  load: async () => {
    set({ loading: true, error: null });
    try {
      const page = await backend.historyListEvents({ limit: 50, offset: 0 });
      set({ events: page.items, total: page.total, loading: false });
    } catch (e) {
      set({ loading: false, error: String(e) });
    }
  },
}));
```

`HistoryPanel.tsx`：列表 + 选中后 `SnapshotDetail` 预览 + 「恢复」按钮（二次确认）。**敏感值默认显示 `••••`，点击才调用 reveal。**

- [ ] **Step 4: 运行测试**

Run: `npx vitest run tests/unit/history-store.test.ts`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add src/store/history-store.ts src/components/history tests/unit/history-store.test.ts
git commit -m "feat(v6): 历史面板（时间线、快照预览、敏感值掩码）"
```

---

### Task 12: 质量门与 M1 收尾

**Files:**

- Modify: `README.md`、`CHANGELOG.md`（v6.0.0 段落）
- Create: `docs/审核和开发/YYYY.MM.DD/PathEditor-v6-M1-开发回执.md`（**命名遵循 `patheditor-two-window-flow` 的固定约定**：开发窗口产出「开发回执」，不是「完成报告」；J-1 更正）

- [ ] **Step 1: 全量质量门**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run verify:all
```

Expected: 全部 exit 0。

- [ ] **Step 2: 逐文件跑 DB 测试**（一文件一测试）

```bash
for f in db_schema_open db_key_lifecycle db_history_roundtrip db_history_sink db_profiles db_backups_index db_migrate_v5; do
  cargo test -p path-editor-core --test $f || exit 1
done
```

Expected: 全绿。

- [ ] **Step 3: 写开发回执（不是「完成报告」）**

记录：Task 完成度、偏离项、未覆盖项（FTS/统计/同步属 M2/M3）、质量门输出、`cargo metadata` 成员确认。

- [ ] **Step 4: 提交**

```bash
git add README.md CHANGELOG.md "docs/审核和开发/YYYY.MM.DD/PathEditor-v6-M1-开发回执.md"
git commit -m "docs(v6): M1 完成报告与文档同步"
```

---

## Self-Review

**1. Spec 覆盖**

| spec 章节                               | 落在哪个 Task          |
| --------------------------------------- | ---------------------- |
| §3.2 HistorySink                        | Task 1                 |
| §3.2.1 IPC 边界                         | Task 9、Task 10        |
| §3.3 DB 位置/PRAGMA                     | Task 2、Task 3         |
| §4.1 表结构                             | Task 2                 |
| §4.2 约束（Path 排除、事件无明文、CAS） | Task 2、Task 4、Task 6 |
| §5.1 本地密钥                           | Task 3                 |
| §5.3 `.pebak`                           | Task 7                 |
| §7 迁移                                 | Task 8                 |
| Tier 1 功能（历史/配置/备份索引）       | Task 4、6、7、11       |

**2. 占位符扫描**：无 TBD/TODO；Task 3 的熵源已用 `getrandom`（显式依赖，不依赖传递依赖的 API）。Task 4/6/8 给出结构与签名，完整实现由实施者补齐——这是**刻意的**：这些函数的 SQL 较长，逐行贴出会让计划失去可读性；签名与约束已写死。

**3. 类型一致性**：`HistoryEvent` / `EventSource` / `EventAction` / `EventResult` 在 Task 1 定义，Task 4/5/6/9 引用；`KeyMaterial` 在 Task 3 定义，Task 7 引用；`NewSnapshot` 在 Task 4 定义，Task 6/8 引用。

**4. 已知缺口**：Task 11 的 UI 只做最小可用（列表 + 预览 + 恢复）；配置面板与备份面板的 UI 改造留给 M2。

---

## M1 计划核对轮裁断（2026-10-19）

开发窗口提出 **5 项阻塞 + 6 项高风险 + 1 项额外发现**，审核窗口逐条独立复核后**全部采纳**并已修订：

| 项         | 问题                                                          | 处置                                                                                      |
| ---------- | ------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| **B1**     | `db_err` 返回 `impl Fn` 传给 `map_err` 类型不匹配             | 改为二元函数 `db_err(op, e)`，调用处用闭包                                                |
| **B2**     | Task 4 与 Task 5 重复写事件                                   | `insert_snapshot` 不再写 events，统一由 `HistorySink` 负责                                |
| **B3**     | 迁移夹具缺 `schemaVersion` 信封                               | 补 `Versioned<T>` 正常路径 + 无版本头旧格式，两条都覆盖                                   |
| **B4**     | `EnvBackupPayload::empty_for_test` 不存在                     | 改为显式构造 `EnvBackupPayload` / `EnvBackupHives` / `EnvBackupVar`                       |
| **B5**     | `cargo test -p app_lib` 跑不起来（package 名是 `patheditor`） | 全部改为 `-p patheditor`                                                                  |
| **H1**     | `EventPage.total` 无实现路径                                  | 补独立 `count(*)` 查询示例 + 测试断言 `limit=2` 时 `total=3`                              |
| **H2**     | `sensitive` 信任调用方                                        | 写入时由 core 用 `env_var::is_sensitive` 重新计算并覆盖                                   |
| **H3**     | `content_hash` 不区分 hive                                    | 逐 hive 哈希 + 分隔符；`system=[A]` 与 `user=[A]` 哈希不同                                |
| **H4**     | `set_var` 进程级污染 + 无法真正重定向                         | `load_or_create_key(home: &Path)` 显式传路径，测试零 `set_var`                            |
| **H5**     | `HistorySink: Send + Sync` 与 `&mut Db` 冲突                  | trait 去掉 auto-trait 约束；跨线程留到 M3（`Mutex<Db>`）                                  |
| **H6**     | `insert_snapshot` 自带事务，Task 6 无法嵌套                   | Task 4 提前提供 `insert_snapshot_tx(&Transaction, ...)`                                   |
| **额外**   | `retry_pending_path_state()` 无参数                           | Task 1 的 Files 补该签名改动，Task 8 依赖它                                               |
| **补充-1** | 漏 spec §5.1 的**命名互斥量** `Local\PathEditor.KeyInit`      | Task 3 补互斥量 + `sync_all`；`create_new` 只保证谁先创建，不保证写完                     |
| **补充-2** | 漏 spec §5.1 的 **DACL 限制与启动校验**                       | Task 3 补 `set_key_dacl`（仅当前用户 + SYSTEM）+ 启动告警 + `key_dacl_is_restricted` 测试 |

> **未改动**：`PathEntry`/`EnvHive`/`ErrorCode::Internal` 导出、四处 `save_path_with_sidecar` 调用点、DDL 与 spec §4.1 逐表对应、`migration_imports` 复合主键、`.pebak` 头部 55 字节——开发窗口已核实正确。

---

## M1 复审整改裁断（2026-10-20）

审核窗口在复审中确认三项整改真实落地，并补充三处计划侧的更正与三行裁断：

| 项                         | 内容                                                                                                                                                                                                                                                                                                                                                  | 落点                               |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- |
| **V-1 判据盲区**           | `key_dacl_is_restricted` 用的是**黑名单**（`WD`/`BU`/`AU`/`BG`/`PS`）。NULL DACL 序列化为 `D:PAI(NO_ACCESS_CONTROL)`，一个主体都不含 → 黑名单全不命中 → **误判为受限**。且漏了 `;;;CO)`（CREATOR OWNER），任何具名 SID 形式也都不命中。**M2 应改为白名单判据**（只允许当前用户 SID + `SY`）。第一层 `set_key_dacl` 的写法本身是对的，所以本项不阻塞。 | Task 3 的 `key_dacl_is_restricted` |
| **V-3 文件名时刻**         | `.pebak` 的文件名必须用**写入时刻**（`chrono::Local::now()`），不能用 `payload.captured_at`。`rotate_env_backups` 按文件名字典序降序保留前 `keep` 份——导入旧载荷时，用旧时间戳命名会让新文件排到最后、**当场被轮换删掉**（实测：25 份旧备份 + 1 份新备份，`keep=4` 时新文件消失）。                                                                   | Task 7 的 `write_env_backup_pebak` |
| **J-7 app_state 建库即写** | `app_state` 单行必须在**建库事务内**写入（`INSERT OR IGNORE`，`device_id` 用 UUIDv7）。否则 `set_current_snapshot` 的 CAS `WHERE id = 1 AND version = ?` 匹配 0 行、**恒返回 false**，spec §十二 的「CAS 冲突返回 conflict」永远不可达。                                                                                                              | Task 2 的 `migrate_to_current`     |

**另有三处计划侧缺陷已在本轮修订**：

1. Task 12 的交付物名称 `M1-完成报告.md` → `M1-开发回执.md`（`patheditor-two-window-flow` 的固定约定）；
2. Task 9 Step 4 的启动片段补上 `setup_database` 三步（pending → `migrate_to_current` → `migrate_legacy`）——首版计划漏了第 ③ 步，导致 `migrate_legacy` 在生产中没有调用者；
3. Task 3 的 DACL 骨架 `SetNamedSecurityInfoW` → `SetFileSecurityW`，理由见该节内的对照说明。

---

## Execution Notes

（留给开发窗口回填：实际命令、偏离、未覆盖项、质量门输出、一文件一测试的执行方式）
