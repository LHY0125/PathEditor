# PathEditor v6.0.0 M3 同步 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 M1/M2 的加密存储与检索之上实现文件式跨设备同步：`changes` 变更日志、加密 bundle 导出/导入、字段级 LWW 合并、冲突记录与 UI。

**Architecture:** schema 升到 v3（新增 `changes` / `sync_conflicts` 两张表）。新增 `core/src/db/sync/` 模块（`mod` / `change_log` / `bundle` / `merge` / `conflict`）。bundle 用 Argon2id 派生密钥 + ChaCha20-Poly1305 加密，格式见 spec §6.4。同步**不含注册表写入**——导入只改 DB，用户随后手动应用。

**Tech Stack:** 同 M1/M2，**新增三个依赖**（均已验证可通过南大镜像获取）：`argon2`（口令派生）、`chacha20poly1305`（AEAD）、`rpassword`（CLI 隐藏口令输入，Task 8 需要）。

**Spec:** `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`（§4.1 表结构、§4.2 不变量 7/8/9、§5.2 同步包安全、§6.3 Tier 3、§6.4 Bundle Schema、§8 退出码）

**前置：**

- `docs/superpowers/plans/2026-10-20-v6-sqlite-m2-plan.md`（M2 已完成并合并）
- `docs/审核和开发/2026.10.21/PathEditor-v6-M2-开发回执.md`（M2 移交的 8 条未覆盖项）

---

## Global Constraints

- **一文件一测试**：任何使用 SQLCipher 的 `.rs` 测试文件**只放一个 `#[test]`**。同一进程内第二个 SQLCipher 测试会 `STATUS_STACK_OVERFLOW`（GNU/MSVC 均已复现，`--test-threads=1` 无效）。M2 结束时 `core/tests/` 有 31 个文件，M3 声明 **10 个**新文件（Task 1 两个、Task 2/2b/3/4/5/6/7 各一）。
- **`config_dir()` 无法被环境变量重定向**：`dirs::home_dir()` 在 Windows 上走 `SHGetKnownFolderPath`，`HOME` 与 `USERPROFILE` **都不读**。所有测试必须**显式传 `home: &Path` 参数**（`tempfile::TempDir`），不得依赖 `set_var`。M1 已因此出过一次写坏用户真实 `disabled.json` 的事故。
- **每个新增 `pub fn` 必须有生产调用者**。检查方法（**先验证工具本身有效**）：

  ```bash
  # 0. 先用已知有调用者的函数验证 grep 模式
  grep -rn "dashboard(" --include=*.rs core/src gui/src cli/src | grep -v "fn dashboard"
  #    必须 ≥1 命中，否则模式有问题

  # 1. 再查目标函数
  grep -rn "<fn_name>(" --include=*.rs core/src gui/src cli/src | grep -v "fn <fn_name>"
  ```

  ⚠️ 不要用 `\b` 词边界——它匹配不到 `refs::find_cycles` 这类带模块前缀的调用。这条约束来自 M1 的 V-3（`.pebak` 写入器零调用者）与 M2 的 `index_snapshot_tx`（同一缺陷形态重演），**已经发生过两次**。

- **`changes.old_value` / `new_value` 仅用于同步**（spec §4.2 第 7 条）：它们位于 SQLCipher 加密库与加密 bundle 内，**不得写入 `events`**，UI 展示敏感值时仍须掩码。
- **同步不写注册表**：导入只改 DB。用户要生效必须走既有 `apply_path_snapshot` / `apply_profile`。
- **禁止在 bundle 中出现本地整数主键**（spec §6.4）：所有引用用 uid。
- **导入失败必须整体回滚**（spec §8）：单事务，不产生半应用状态。
- **不提交、不推送、不升级版本号**，除非用户明确授权。
- 中文注释；Rust 4 空格；TS 2 空格、单引号、100 列。
- 质量门：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`npm run verify:all`。
- **schema 版本**：M3 引入 `CURRENT_SCHEMA_VERSION = 3`。迁移必须**幂等**且向前兼容（v2 库升级到 v3 不丢数据）。**注意 M2 的教训**：`PRAGMA user_version` 必须写在所有分支之后。

---

## File Structure

| 文件                                 | 责任                                              |
| ------------------------------------ | ------------------------------------------------- |
| `core/src/db/schema.rs`              | `DDL_V3`（`changes` + `sync_conflicts`）          |
| `core/src/db/mod.rs`                 | `migrate_to_current` 增加 `current < 3` 分支      |
| `core/src/db/sync/mod.rs`            | 模块入口、`SyncError` 辅助                        |
| `core/src/db/sync/change_log.rs`     | `changes` 写入/查询/标记已导出                    |
| `core/src/db/sync/bundle.rs`         | Argon2id 派生 + ChaCha20-Poly1305 加解密 + 包格式 |
| `core/src/db/sync/export.rs`         | 收集本地变更 → 构建 payload → 写文件              |
| `core/src/db/sync/import.rs`         | 解密验证 → 单事务逐条应用 → 冲突记录              |
| `core/src/db/sync/merge.rs`          | 字段级 LWW 判定（`lamport` + `device_id` 破平）   |
| `core/src/db/sync/conflict.rs`       | `sync_conflicts` 读写与解决                       |
| `gui/src/commands/sync.rs`           | Tauri 命令                                        |
| `cli/src/sync_ops.rs`                | CLI `sync export` / `sync import` / `sync status` |
| `src/services/backend.ts`            | 形状校验                                          |
| `src/store/sync-store.ts`            | 同步中心状态                                      |
| `src/components/sync/SyncCenter.tsx` | 导出/导入/冲突列表 UI                             |

---

## Task 1: schema v3（`changes` + `sync_conflicts`）

**Files:**

- Modify: `core/src/db/schema.rs`
- Modify: `core/src/db/mod.rs`
- Test: `core/tests/db_schema_v3.rs`、`core/tests/db_schema_v2_to_v3.rs`

**Interfaces:**

- Consumes: M2 的 `migrate_to_current`
- Produces: `CURRENT_SCHEMA_VERSION = 3`、`DDL_V3` 常量

- [ ] **Step 1: 写失败测试（新库建到 v3）**

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn schema_v3_adds_sync_tables_and_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("v3.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    assert_eq!(db.schema_version().unwrap(), 3, "版本必须是 3");
    for t in ["changes", "sync_conflicts", "scan_runs", "search_fts"] {
        assert!(db.table_exists(t).unwrap(), "缺表: {t}");
    }
    db.migrate_to_current().unwrap();
    assert_eq!(db.schema_version().unwrap(), 3, "幂等");
}
```

- [ ] **Step 2: 写失败测试（v2 → v3 升级）**

```rust
// 一文件一测试：见 M0 §5.3。
//
// **v2 → v3 升级路径的回归守卫。**
// M2 的首版把 `PRAGMA user_version` 写在 `if current < 1` 内部，
// 导致 v1→v2 执行了 DDL 但不更新版本号。M3 必须避免同样的错误。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::{schema, Db};
use rusqlite::Connection;
use tempfile::TempDir;

#[test]
fn upgrading_a_v2_database_bumps_version_and_preserves_data() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("up.db");
    let key = load_or_create_key(dir.path()).unwrap();
    let kh = key_hex(&key);

    // 造一个真 v2 库：DDL_V1 + 种子 + DDL_V2 + user_version = 2
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(&format!("PRAGMA key = \"x'{kh}'\";")).unwrap();
        conn.execute_batch(schema::DDL_V1).unwrap();
        conn.execute(
            schema::SEED_APP_STATE_SQL,
            rusqlite::params![uuid::Uuid::now_v7().to_string()],
        )
        .unwrap();
        conn.execute_batch(schema::DDL_V2).unwrap();
        conn.execute_batch("PRAGMA user_version = 2;").unwrap();
        // 造一条 v2 时期的数据
        conn.execute(
            "INSERT INTO snapshots (uid, created_at, source, kind, content_hash, updated_at)
             VALUES ('v2-snap', 1, 'gui', 'current', 'h', 1)",
            [],
        )
        .unwrap();
    }

    let mut db = Db::open_encrypted(&path, &kh).unwrap();
    assert_eq!(db.schema_version().unwrap(), 2, "前置：确实是 v2 库");

    db.migrate_to_current().unwrap();
    assert_eq!(db.schema_version().unwrap(), 3, "升级后版本必须落盘");

    for t in ["changes", "sync_conflicts"] {
        assert!(db.table_exists(t).unwrap(), "升级路径缺表: {t}");
    }
    db.migrate_to_current().expect("二次迁移必须幂等");

    // v2 数据必须还在
    let n: i64 = db
        .query_row_with("SELECT count(*) FROM snapshots WHERE uid = 'v2-snap'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1, "升级不得丢数据");
}
```

- [ ] **Step 3: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_schema_v3`
Expected: FAIL — 版本是 2

- [ ] **Step 4: 写 DDL_V3**

```rust
/// 当前 schema 版本。
pub const CURRENT_SCHEMA_VERSION: u32 = 3;

/// v3 增量 DDL：M3 的同步表（spec §4.1）。
pub const DDL_V3: &str = r#"
CREATE TABLE changes (
  id           INTEGER PRIMARY KEY,
  uid          TEXT NOT NULL UNIQUE,
  device_id    TEXT NOT NULL,
  seq          INTEGER NOT NULL,
  lamport      INTEGER NOT NULL,
  origin       TEXT NOT NULL CHECK (origin IN ('local','remote')),
  entity_type  TEXT NOT NULL,
  entity_uid   TEXT NOT NULL,
  field        TEXT,
  old_value    TEXT,
  new_value    TEXT,
  created_at   INTEGER NOT NULL,
  applied_at   INTEGER NOT NULL,
  exported_at  INTEGER
);
CREATE INDEX idx_changes_seq ON changes(device_id, seq);
CREATE INDEX idx_changes_entity ON changes(entity_type, entity_uid);
CREATE INDEX idx_changes_pending_export ON changes(origin, exported_at);

CREATE TABLE sync_conflicts (
  id            INTEGER PRIMARY KEY,
  entity_type   TEXT NOT NULL,
  entity_uid    TEXT NOT NULL,
  field         TEXT,
  local_value   TEXT,
  remote_value  TEXT,
  remote_device TEXT NOT NULL,
  created_at    INTEGER NOT NULL,
  resolved_at   INTEGER,
  resolution    TEXT
);
CREATE INDEX idx_sync_conflicts_unresolved ON sync_conflicts(resolved_at);
"#;
```

- [ ] **Step 5: `migrate_to_current` 加 v3 分支**

```rust
// 在 if current < 2 { ... } 之后、PRAGMA 之前
if current < 3 {
    tx.execute_batch(schema::DDL_V3)
        .map_err(|e| db_err("ddl_v3", e))?;
}
// PRAGMA user_version 仍在**所有分支之后**（M2 的教训）
```

- [ ] **Step 6: 运行，两个测试都通过**

Run: `cargo test -p path-editor-core --test db_schema_v3 && cargo test -p path-editor-core --test db_schema_v2_to_v3`

- [ ] **Step 7: 提交**

```bash
git commit -m "feat(v6): schema v3 —— 同步表（changes / sync_conflicts）"
```

---

## Task 2: 变更日志（`change_log`）

**Files:**

- Create: `core/src/db/sync/mod.rs`、`core/src/db/sync/change_log.rs`
- Modify: `core/src/db/mod.rs`
- Test: `core/tests/db_change_log.rs`

**Interfaces:**

- Produces:
  - `pub fn record_change_tx(tx: &Transaction, c: NewChange) -> Result<String, CoreError>` — 事务内写入
  - `pub fn pending_export(db: &Db) -> Result<Vec<ChangeRow>, CoreError>` — `origin='local' AND exported_at IS NULL`
  - `pub fn mark_exported(db: &mut Db, uids: &[String], at: i64) -> Result<usize, CoreError>`
  - `pub fn current_lamport(db: &Db) -> Result<i64, CoreError>`
  - `pub fn next_lamport(db: &Db) -> Result<i64, CoreError>` — `current_lamport() + 1`（冲突解决追加用，见 Task 6）

  ```rust
  /// 待写入的一条变更（调用方构造）。
  #[derive(Debug, Clone)]
  pub struct NewChange {
      /// 实体类型：`snapshot` / `profile` / `profile_revision` / `path_entry` / `env_value`
      pub entity_type: String,
      /// 实体 uid（**不是**本地整数主键）
      pub entity_uid: String,
      /// 字段名；`None` 表示整实体
      pub field: Option<String>,
      /// 旧值 JSON（**仅用于同步**，不写入 events，UI 展示须掩码）
      pub old_value: String,
      /// 新值 JSON
      pub new_value: String,
  }

  /// 已落库的一条变更（查询返回）。
  ///
  /// **S-2 补正**：本类型在首版计划里从未定义，却被 `pending_export` 的返回类型、
  /// Task 5 的 `resolve` 参数、Task 6 的导入逻辑三处引用。此处补齐。
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
  ```

- [ ] **Step 1: 写失败测试**

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::sync::change_log::{
    current_lamport, mark_exported, pending_export, record_change_tx, NewChange,
};
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn change_log_records_assigns_seq_and_marks_exported() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("cl.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let uid1 = db
        .with_transaction(|tx| {
            record_change_tx(
                tx,
                NewChange {
                    entity_type: "profile".into(),
                    entity_uid: "p1".into(),
                    field: Some("name".into()),
                    old_value: "old".into(),
                    new_value: "new".into(),
                },
            )
        })
        .unwrap();
    let _uid2 = db
        .with_transaction(|tx| {
            record_change_tx(
                tx,
                NewChange {
                    entity_type: "profile".into(),
                    entity_uid: "p1".into(),
                    field: Some("tags".into()),
                    old_value: "".into(),
                    new_value: "a,b".into(),
                },
            )
        })
        .unwrap();

    let pending = pending_export(&db).unwrap();
    assert_eq!(pending.len(), 2, "两条都待导出");
    assert!(pending[0].seq < pending[1].seq, "seq 必须单调递增");
    assert_eq!(pending[0].origin, "local");

    // lamport 必须与 seq 同步推进（LWW 依赖它）
    assert!(current_lamport(&db).unwrap() >= 2);

    let marked = mark_exported(&mut db, &[uid1], 999).unwrap();
    assert_eq!(marked, 1);
    assert_eq!(pending_export(&db).unwrap().len(), 1, "标记后只剩一条待导出");
}
```

- [ ] **Step 2: 实现 `change_log.rs`**

要点：

- `seq` = `MAX(seq) + 1`（按 `device_id` 分组）；`lamport` 与 `seq` 同源推进，并更新 `app_state.lamport`。
- `device_id` 从 `app_state.device_id` 读（M1 已建库即写）。
- `origin='local'`、`applied_at = created_at`、`exported_at = NULL`。
- **`record_change_tx` 在事务内调用**，不自开事务（避免嵌套）。

- [ ] **Step 3–5**: 运行通过 → 提交

```bash
git commit -m "feat(v6): changes 变更日志（seq/lamport 单调、待导出查询）"
```

---

## Task 2b: 把写操作接进变更日志（**S-1 阻塞项**）

> **为什么这是一个独立的 Task**：首版计划的 11 个 Task 里**没有一个**负责「把写操作接进
> `changes`」，导致 `record_change_tx` 零生产调用者。后果是 `changes` 表永远为空 →
> `pending_export()` 恒空 → `export_bundle` 导出 0 条 → **整条同步链路空转，而所有单测都会通过**
> （测试自己调 `record_change_tx` 造数据）。
>
> 这是同一形态缺陷的**第三次**：M1 的 `.pebak` 写入器、M2 的 `index_snapshot_tx`、
> M3 的 `record_change_tx`。前两次是「实现了没接」，这次是「**计划里就没有那个 Task**」。

**Files:**

- Modify: `core/src/db/profiles.rs`、`core/src/db/history.rs`（快照写入路径）
- Test: `core/tests/db_change_log_wiring.rs`

**Interfaces:**

- Consumes: Task 2 的 `record_change_tx` / `NewChange`
- Produces: 无新公开 API——本 Task 只增加调用点

### 写入口 → 要记的变更（**对照表，必须逐条落实**）

| 写入口                            | 位置                      | 记什么变更               | `entity_type`      | `field`          |
| --------------------------------- | ------------------------- | ------------------------ | ------------------ | ---------------- |
| `db::profiles::save_profile_tx`   | `core/src/db/profiles.rs` | 新建/更新配置时记元数据  | `profile`          | `None`（整实体） |
| `db::profiles::delete_profile`    | 同上                      | 软删除                   | `profile`          | `deleted_at`     |
| `db::profiles::rename_profile`    | 同上                      | 改名                     | `profile`          | `name`           |
| `db::history::insert_snapshot_tx` | `core/src/db/history.rs`  | 新快照 + 其子行          | `snapshot`         | `None`           |
| `path_entries` 写入               | 随快照                    | 整快照语义，**不单独记** | —                  | —                |
| `env_values` 写入                 | 随快照                    | 整快照语义，**不单独记** | —                  | —                |
| `profile_revisions` 写入          | 随配置保存                | 新版本                   | `profile_revision` | `None`           |

> **为什么不给 `path_entries` / `env_values` 单独记变更**：它们没有 `deleted_at`，
> 且总是随父快照整体写入（`insert_snapshot_tx` 一次写全部子行）。
> 「整快照」是一条变更，子行由它携带。这与 Task 6 的墓碑规则一致。

- [ ] **Step 1: 写失败测试**

```rust
// 一文件一测试：见 M0 §5.3。
//
// **S-1 的回归守卫**：写操作必须真的产生 `changes` 行。
// 首版计划里 `record_change_tx` 只有测试调用者，这个测试守的是「生产路径也在调」。
//
// 注意：本测试调的是 core 的写函数（save_profile / insert_snapshot），
// 然后断言 `changes` 表**自己多出了行**——不手写 `record_change_tx`。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::profiles;
use path_editor_core::db::sync::change_log::pending_export;
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn write_paths_populate_change_log_without_manual_recording() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("w.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    // 前置：空库没有任何待导出变更
    assert!(pending_export(&db).unwrap().is_empty());

    // ① 存一个配置 → 必须产生变更
    profiles::save_profile_tx_helper(&mut db, "Work", None, &[], None).unwrap();
    let after_profile = pending_export(&db).unwrap();
    assert!(
        !after_profile.is_empty(),
        "保存配置必须产生 changes 行（S-1）"
    );
    assert!(after_profile.iter().any(|c| c.entity_type == "profile"));

    // ② 插入快照 → 必须再产生变更
    let before = pending_export(&db).unwrap().len();
    insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![PathEntry { path: "C:\\X".into(), enabled: true }],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();
    let after_snap = pending_export(&db).unwrap().len();
    assert!(after_snap > before, "插入快照必须产生 changes 行（S-1）");
    assert!(pending_export(&db)
        .unwrap()
        .iter()
        .any(|c| c.entity_type == "snapshot"));
}
```

> **注意**：`save_profile_tx_helper` 是占位名，按 `core/src/db/profiles.rs` 的实际签名调整
> （M1 的实现是 `save_profile(db, name, description, tags, snap)`）。

- [ ] **Step 2: 运行，确认失败**

Run: `cargo test -p path-editor-core --test db_change_log_wiring`
Expected: FAIL — `pending_export` 为空（写路径没记变更）

- [ ] **Step 3: 在写路径里调用 `record_change_tx`**

每个写入口在**它自己的事务内**调 `record_change_tx(tx, ...)`。不要新开事务。

- [ ] **Step 4: 运行，确认通过**

- [ ] **Step 5: 验证接线完整（**必须执行，不能只靠测试**）**

```bash
# 先验证 grep 模式本身有效
grep -rn "record_change_tx(" --include=*.rs core/src | grep -v "fn record_change_tx"
#     必须 ≥1 命中且不在 sync/change_log.rs 里

# 逐条对照上表的 7 个写入口
```

- [ ] **Step 6: 提交**

```bash
git commit -m "feat(v6): 把写路径接进变更日志（S-1）"
```

---

## Task 3: Bundle 加密格式（Argon2id + ChaCha20-Poly1305）

**Files:**

- Create: `core/src/db/sync/bundle.rs`
- Modify: `core/Cargo.toml`（加 `argon2`、`chacha20poly1305`）
- Test: `core/tests/db_bundle_crypto.rs`

**Interfaces:**

- Produces:
  - `pub fn derive_key(passphrase: &str, salt: &[u8; 16]) -> Result<[u8; 32], CoreError>`
  - `pub fn seal(passphrase: &str, header: &BundleHeader, payload: &[u8]) -> Result<Vec<u8>, CoreError>`
  - `pub fn open(passphrase: &str, bytes: &[u8]) -> Result<(BundleHeader, Vec<u8>), CoreError>`
  - `pub struct BundleHeader { pub schema_version: u32, pub device_id: String, pub exported_at: i64, pub base_lamport: i64, pub change_count: u32 }`

**包格式（spec §5.2 / §6.4）**：

```
magic "PESYNC1" (8) | version u32 (4) | header_len u32 (4) | header JSON | salt (16) | nonce (12) | ciphertext | tag (16)
```

**AAD = magic || version || header_len || header JSON || salt || nonce**

- [ ] **Step 1: 写失败测试**

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::sync::bundle::*;   // C-1：按 File Structure 的 core/src/db/sync/bundle.rs
use path_editor_core::ErrorCode;

#[test]
fn bundle_roundtrips_and_rejects_wrong_passphrase_and_tampering() {
    let header = BundleHeader {
        schema_version: 1,
        device_id: "dev-1".into(),
        exported_at: 1_700_000_000,
        base_lamport: 5,
        change_count: 1,
    };
    let payload = br#"{"changes":[]}"#;

    let sealed = seal("correct horse", &header, payload).unwrap();
    assert!(sealed.starts_with(b"PESYNC1"), "必须带 magic");

    // 口令错误 → Crypto 错误（不是 panic，不是返回空）
    let err = open("wrong", &sealed).unwrap_err();
    assert_eq!(err.code, ErrorCode::Crypto, "错误口令必须是 Crypto");

    // 篡改密文任一字节 → 拒绝
    let mut tampered = sealed.clone();
    let n = tampered.len();
    tampered[n - 20] ^= 0xFF;
    assert_eq!(open("correct horse", &tampered).unwrap_err().code, ErrorCode::Crypto);

    // 篡改 header（影响 AAD）→ 拒绝
    let mut header_tampered = sealed.clone();
    // 改 header JSON 里的一个字符（位置依赖实际布局，用 magic+8 之后找 '{' ）
    let start = header_tampered.windows(1).position(|w| w == b"{").unwrap();
    header_tampered[start + 1] ^= 0x20;
    assert!(open("correct horse", &header_tampered).is_err(), "header 被改必须拒绝");

    // 正确口令 → 原样返回
    let (h2, p2) = open("correct horse", &sealed).unwrap();
    assert_eq!(h2.device_id, "dev-1");
    assert_eq!(p2, payload);
}
```

- [ ] **Step 2: 实现 `bundle.rs`**

要点：

- Argon2id 参数：**m=65536 KiB（64 MiB）, t=3, p=1, salt 16 B**（spec §5.2 写死）
- `derive_key` 用 `argon2::Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::new(65536, 3, 1, Some(32))?)`
- `seal`：随机 salt（16）+ 随机 nonce（12），用 `getrandom`（已是依赖）
- **AAD 必须覆盖 header**，否则 header 可被篡改而不被发现
- 口令**不落盘**；错误文案**不得包含口令**

- [ ] **Step 3–5**: 运行通过 → 提交

```bash
git commit -m "feat(v6): 同步包加密（Argon2id + ChaCha20-Poly1305、AAD 覆盖 header）"
```

---

## Task 4: 导出（收集变更 → 加密 → 写文件）

**Files:**

- Create: `core/src/db/sync/export.rs`
- Test: `core/tests/db_sync_export.rs`

**Interfaces:**

- Produces: `pub fn export_bundle(db: &mut Db, passphrase: &str, path: &Path) -> Result<ExportReport, CoreError>`
- `pub struct ExportReport { pub change_count: u32, pub path: PathBuf }`

**要点**：

1. `pending_export()` 取待导出变更
2. 构建 payload：`{ "changes": [...] }`，**所有引用用 uid，禁止本地整数主键**
3. `seal()` 加密
4. **写文件成功后才**在同一事务内 `mark_exported`（顺序不能反：先标记后写文件,写失败会导致变更永久丢失）

- [ ] **Step 1–5**: 标准 TDD 循环。测试断言：
  - 导出后 `pending_export` 为空
  - 导出文件能被 `open()` 读回
  - **写文件失败时 `pending_export` 不变**（用只读目录或非法路径构造）

```bash
git commit -m "feat(v6): 同步包导出（先写文件后标记，避免丢变更）"
```

---

## Task 5: 字段级 LWW 合并

**Files:**

- Create: `core/src/db/sync/merge.rs`
- Test: `core/tests/db_sync_merge.rs`

**Interfaces:**

- Produces: `pub fn resolve(local: &ChangeRow, remote: &ChangeRow) -> MergeDecision`
- `pub enum MergeDecision { TakeRemote, KeepLocal, Conflict }`

**判定规则（spec §6.3）**：

1. `remote.lamport > local.lamport` → `TakeRemote`
2. `remote.lamport < local.lamport` → `KeepLocal`
3. 相等 → 比较 `device_id`（字典序大的胜）；**若是同一个 `device_id`** → 视为同源，`KeepLocal`
4. `TakeRemote` 与 `KeepLocal` **都不产生冲突记录**；只有「同 lamport、不同 device、值不同」才记 `Conflict`

- [ ] **Step 1: 写失败测试**（纯函数，不碰 DB，**可以不用 SQLCipher**）

```rust
// 一文件一测试：见 M0 §5.3。
// 本文件测试纯函数，不建库——因此不受 SQLCipher 栈溢出约束，但仍遵守一文件一测试。
use path_editor_core::db::sync::merge::{resolve, MergeDecision};

fn change(lamport: i64, device: &str, value: &str) -> ChangeRow { /* 构造辅助 */ }

#[test]
fn lww_prefers_higher_lamport_then_device_id() {
    // 高 lamport 胜
    assert!(matches!(resolve(&change(1, "a", "x"), &change(2, "b", "y")), MergeDecision::TakeRemote));
    // 低 lamport 负
    assert!(matches!(resolve(&change(2, "a", "x"), &change(1, "b", "y")), MergeDecision::KeepLocal));
    // 同 lamport、不同 device：字典序大的胜
    assert!(matches!(resolve(&change(5, "aaa", "x"), &change(5, "bbb", "y")), MergeDecision::TakeRemote));
    assert!(matches!(resolve(&change(5, "bbb", "x"), &change(5, "aaa", "y")), MergeDecision::KeepLocal));
    // 同 lamport、同 device → 同源，保持本地
    assert!(matches!(resolve(&change(5, "a", "x"), &change(5, "a", "y")), MergeDecision::KeepLocal));
}
```

- [ ] **Step 2–5**: 实现 → 通过 → 提交

```bash
git commit -m "feat(v6): 字段级 LWW 合并（lamport + device_id 破平）"
```

---

## Task 6: 导入（解密 → 单事务应用 → 冲突记录）

**Files:**

- Create: `core/src/db/sync/import.rs`、`core/src/db/sync/conflict.rs`
- Test: `core/tests/db_sync_import.rs`、`core/tests/db_sync_conflict.rs`

**Interfaces:**

- Produces:
  - `pub fn import_bundle(db: &mut Db, passphrase: &str, path: &Path) -> Result<ImportReport, CoreError>`
  - `pub struct ImportReport { pub applied, pub skipped, pub conflicts }`
  - `conflict.rs`: `pub fn list_conflicts(db, include_resolved) -> ...`、`pub fn resolve_conflict(db, id, resolution) -> ...`
    - **解决冲突时追加一条新 change**（核对轮裁断）：`entity_type` / `entity_uid` / `field` 与冲突记录一致，
      `old_value` = 败方值，`new_value` = 胜方值，**`lamport` = `next_lamport()`（= 当前 + 1）**。
      理由：「用户的选择」是一个**更晚的决定**，应该胜过导致冲突的两个值——否则第三台设备
      导入时会按原 lamport 重新判一次，可能把用户刚做的选择覆盖掉。

**要点（spec §6.3 / §6.4）**：

1. `open()` 解密验证；失败 → `Sync` 错误，**不产生任何写入**
2. **整个导入在单个 `BEGIN IMMEDIATE` 事务内**
3. 按依赖序应用：`snapshots → path_entries/env_values → profiles → profile_revisions → events`
4. 每条变更：
   - `changes.uid` 已存在 → 跳过（幂等）
   - 调用 `merge::resolve`；`TakeRemote` → 应用并记 `origin='remote'`
   - `Conflict` → 写 `sync_conflicts`，**不改实体**
5. **悬空 uid 的处理分两类**（C-3 裁断）：
   - **实体引用悬空**（`parentUid` / `profileUid` / `snapshotUid` 指向包内不存在的实体）→ **整体回滚**，返回 `SyncFormat` 错误。理由：这些是结构关系，悬空会导致数据不一致。
   - **`events.snapshot_id` 悬空** → **置 NULL，不回滚**。理由：它是可空的历史关联（M1 的 DDL 就是可空），事件本身有价值，不该因为快照没同步过来就整包失败。**注意：M3 已决定不同步 `events`（见 Self-Review #2），因此这条只在将来恢复 events 同步时适用**——但规则先写下来。
6. 更新 `app_state.lamport = max(local, remote)`

- [ ] **Step 1: 写失败测试（导入幂等 + 冲突记录）**

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::sync::{export_bundle, import_bundle};
// ... 造两个独立库，A 导出、B 导入、B 再导入（幂等）

#[test]
fn import_is_idempotent_and_records_conflicts() {
    // 库 A：写一条 profile 变更 → 导出
    // 库 B：导入 → applied=1
    // 库 B：再导入同一包 → skipped=1, applied=0（幂等）
    // 构造冲突：B 本地同 lamport 不同 device 改了同一字段 → 导入后 conflicts=1
}
```

- [ ] **Step 2: 写第二个测试（篡改拒绝）**

```rust
// 一文件一测试：见 M0 §5.3。
// 篡改的包必须整体拒绝，且库内无任何变化（不是部分导入）。
```

- [ ] **Step 3–6**: 实现 → 通过 → 提交

```bash
git commit -m "feat(v6): 同步包导入（单事务、幂等、冲突记录、篡改拒绝）"
```

---

## Task 7: 退出码与错误映射

**Files:**

- Modify: `core/src/error.rs`
- Test: `core/tests/error_exit_codes.rs`

**要求（spec §8，核对轮 C-2 已裁断）**：

| `ErrorCode`  | 退出码 | 语义                                                                                 | 能否自动恢复                  |
| ------------ | ------ | ------------------------------------------------------------------------------------ | ----------------------------- |
| `Sync`       | **4**  | 合并冲突（LWW 平局、字段级并发修改）                                                 | ❌ 需要人工选择 local/remote  |
| `SyncFormat` | **1**  | 包格式错误：magic 不符 / 版本未知 / AEAD 验证失败 / `entityType` 未知 / 实体引用悬空 | ❌ 重试无用，文件或口令有问题 |

> **为什么拆成两个码**（C-2 裁断）：spec §8 原文写「4 同步/合并冲突」，字面只覆盖前者。
> 但若「文件坏了/口令错了」也返回 4，脚本无法区分「需要人工介入」与「重试无用」。
> 两者的脚本处理完全不同：前者应提示用户解决冲突，后者应报告文件损坏。

- [ ] **Step 1: 写失败测试**

```rust
// 一文件一测试：见 M0 §5.3。
use path_editor_core::ErrorCode;

#[test]
fn sync_exit_codes_are_distinguished() {
    assert_eq!(ErrorCode::Sync.exit_code(), 4, "合并冲突 → 4");
    assert_eq!(ErrorCode::SyncFormat.exit_code(), 1, "包格式错误 → 1（一般错误）");
    // 既有码不受影响
    assert_eq!(ErrorCode::Conflict.exit_code(), 3);
    assert_eq!(ErrorCode::Crypto.exit_code(), 5);
}
```

- [ ] **Step 2: 加 `SyncFormat` 码并映射**

`core/src/error.rs`：`ErrorCode` 枚举新增 `SyncFormat`；`exit_code()` 分支返回 `1`。
**`SyncFormat` 的 serde 名必须是 `syncFormat`**（camelCase，与前端 `ERROR_CODES` 白名单一致）。

- [ ] **Step 3: 运行通过 → 提交**

```bash
git commit -m "feat(v6): 拆分 Sync/SyncFormat 退出码（4 / 1）"
```

---

## Task 8: IPC 与 CLI

**Files:**

- Create: `gui/src/commands/sync.rs`、`cli/src/sync_ops.rs`
- Modify: `gui/src/lib.rs`、`cli/src/main.rs`

**命令清单**：

```text
sync_export(passphrase, path)        -> ExportReport
sync_import(passphrase, path)        -> ImportReport
sync_status()                        -> SyncStatus    # 待导出数、最近同步时间、未解决冲突数
sync_list_conflicts(include_resolved) -> SyncConflictView[]
sync_resolve_conflict(id, resolution) -> void
```

**CLI**：

```text
patheditor sync export <FILE>   # 交互式输入口令（两次确认）
patheditor sync import <FILE>   # 交互式输入口令
patheditor sync status
patheditor sync conflicts [--all]
patheditor sync resolve <ID> --take=local|remote
```

**安全要求**：口令用 `rpassword` 风格的隐藏输入（CLI 不落盘、不进 shell history、不出现在 `ps` 输出）；错误文案**不得包含口令**。

- [ ] 标准 TDD 循环 + 提交

---

## Task 9: 前端形状校验与同步中心 UI

**Files:**

- Modify: `src/services/backend.ts`
- Create: `src/store/sync-store.ts`、`src/components/sync/SyncCenter.tsx`
- Test: `tests/unit/backend-sync-shapes.test.ts`、`tests/unit/sync-center.test.tsx`

**要求**：

- 口令输入用 `type="password"`，**不写入 store**（与 M2 的 reveal 同口径：组件局部 state）
- 冲突列表按 `entity_type` 分组；解决后刷新
- 敏感值在冲突展示时**掩码**（spec §4.2 第 9 条）
- 所有文案走 `t()`，locale 补齐

- [ ] 标准 TDD 循环 + 提交

---

## Task 10: E2E 与文档

**Files:**

- Create: `e2e/tests/sync.spec.ts`
- Modify: `README.md`、`CHANGELOG.md`

**E2E（spec §3.2.1 要求 3–4 条主路径）**：

1. 同步中心打开 → 显示待导出数
2. 导出 → 提示成功
3. 导入错误口令 → 显示错误、不崩溃
4. 冲突列表 → 解决一条 → 列表更新

- [ ] 标准 TDD 循环 + 提交

---

## Self-Review

**规格覆盖**（对照 spec §6.3 / §6.4）：

| spec 要求                                                                                                  | Task       |
| ---------------------------------------------------------------------------------------------------------- | ---------- |
| 导出 `origin='local' AND exported_at IS NULL`，成功后同一事务标记                                          | Task 4     |
| 导入单事务 → 远端以 `origin='remote'` 写入 → 冲突写 `sync_conflicts` → 更新 lamport                        | Task 6     |
| 字段级 LWW（lamport + device_id 破平）                                                                     | Task 5     |
| 范围：profiles/profile_revisions/snapshots/path_entries/env_values/events；**不同步** backups 与派生表     | Task 4 / 6 |
| bundle schema（magic/schemaVersion/deviceId/kdf/nonce）                                                    | Task 3     |
| 引用映射用 uid，禁止本地主键                                                                               | Task 4     |
| 导入顺序 + 悬空 uid 整体回滚                                                                               | Task 6     |
| 幂等（`changes.uid` 已存在则跳过）、墓碑                                                                   | Task 6     |
| 错误：未知 schemaVersion / entityType / 悬空实体引用 → `SyncFormat`（exit 1）；合并冲突 → `Sync`（exit 4） | Task 6 / 7 |
| UI：同步中心（导出/导入/冲突列表/最近同步时间）                                                            | Task 9     |

**核对轮已裁断的三条（2026-10-21）**：

1. **墓碑规则**（已采纳，含核对轮的补充）：

   | 表                            | 墓碑处理                                                            |
   | ----------------------------- | ------------------------------------------------------------------- |
   | `snapshots`                   | 有 `deleted_at` → 软删除                                            |
   | `profiles`                    | 有 `deleted_at` → 软删除                                            |
   | `profile_revisions`           | **无 `deleted_at`** → 随 `profiles` 软删除走（父被删则子不可见）    |
   | `path_entries` / `env_values` | **无 `deleted_at`** → 随 `snapshots` 走（外键 `ON DELETE CASCADE`） |

   > 核对轮指出：首版只写了「`snapshots` / `profiles` 有 `deleted_at`」，但范围表里列了
   > `profile_revisions`，而它也没有该列——规则未覆盖。已补全。

2. **`events` 不同步 —— 已改 spec §6.3**（核对轮建议，审核窗口采纳）。

   核对轮的理由：它是「操作日志」不是「用户数据」；UI 不展示 `device_id`（已核实 `EventView` 有该字段但前端未渲染）；bundle 体积随时间膨胀且同步过去没有应用场景。

   **spec §6.3 已修订**：同步范围改为 `profiles`/`profile_revisions`/`snapshots`/`path_entries`/`env_values`，移除 `events`。

3. **冲突解决追加 change 的 lamport**：用 **`next_lamport()` = `current_lamport() + 1`**。
   理由：「用户的选择」是一个**更晚的决定**，应该胜过导致冲突的两个值。已加进 Task 2 的 Interfaces。

**类型一致性**：`NewChange` / `ChangeRow` / `MergeDecision` / `BundleHeader` / `ExportReport` / `ImportReport` 在 Task 2–6 定义，Task 8/9 引用同一形状。

---

## 与 M1/M2 遗留项的关系

| M2 未覆盖项                          | M3 是否处理                                          |
| ------------------------------------ | ---------------------------------------------------- |
| 1. env 的 N 路对比                   | ✅ Task 5 的对比扩展（**见下**）                     |
| 2. CLI 不重建派生表                  | ❌ 仍不处理（需 CLI 接 DB current 快照，属范围扩张） |
| 3. 真实注册表闭环                    | ❌ 需用户授权                                        |
| 4. CI（MSVC）                        | ⏳ 推送后自动跑                                      |
| 5. `rebuild_derived_tables` 编排测试 | ❌ 仍是 gui 私有函数，测不到                         |
| 6. `db_search_write_path` 的边界     | ❌ 已在注释里写明                                    |
| 7. 统计/搜索面板交互细节             | ❌ E2E 覆盖主路径                                    |
| 8. 全局环列表无独立入口              | ❌ M3 不加                                           |

> **未覆盖项 #1#1（env 的 N 路对比）**：M2 推迟的原因是「掩码值在对比结果里如何呈现」。M3 有了加密 bundle 的经验，可以定义：
>
> - 两边都非敏感 → 显示明文
> - 任一边敏感 → 显示「已修改（敏感）」+ 是否相等
> - 这是 Task 11（可选，见下）

**Task 11（可选）：env 的 N 路对比**

若时间允许，在 M3 补上 M2 的未覆盖项 #1。不在验收门内。

---

## 核对轮裁断（2026-10-21）

开发窗口提交 `PathEditor-v6-M3-核对轮交接.md`（315 行），提出 2 阻塞 + 4 矛盾 + 3 答复 + 1 依赖遗漏。
审核窗口逐条复核后**全部采纳**。

| 编号         | 主张                                                             | 复核                                                                                                                                   | 裁断                                                                                                          |
| ------------ | ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| **S-1**      | `record_change_tx` 零生产调用者                                  | ✅ 成立。5 处引用全在 Task 2 自己的接口与测试里；**11 个 Task 里没有一个负责接线**                                                     | **阻塞，已修**：新增 **Task 2b**，含「写入口 → 要记的变更」7 行对照表 + `db_change_log_wiring.rs` 回归测试    |
| **S-2**      | `ChangeRow` 从未定义                                             | ✅ 成立。被 3 处引用（`pending_export` 返回类型、`resolve` 参数 ×2），Interfaces 里只有 `NewChange`；Task 5 的 `/* 构造辅助 */` 是注释 | **阻塞，已修**：Task 2 的 Interfaces 补齐 `ChangeRow` 完整定义（14 字段）                                     |
| **C-1**      | Task 3 测试用 `db::bundle`，File Structure 写 `db::sync::bundle` | ✅ 成立                                                                                                                                | **已修**：改为 `db::sync::bundle`                                                                             |
| **C-2**      | exit 4 语义边界含糊（合并冲突 vs 包格式错误）                    | ✅ 成立。spec §8 字面只覆盖前者                                                                                                        | **裁断：拆成两个码**。`Sync` → 4（需人工解决）；`SyncFormat` → 1（重试无用）。**已改 spec §8**                |
| **C-3**      | `events.snapshot_id` 悬空未定义                                  | ✅ 成立                                                                                                                                | **裁断：置 NULL 不回滚**。实体引用悬空 → 回滚；可空历史关联 → 置 NULL。理由：M1 的 `history_sink.rs` 已有先例 |
| **C-4**      | 预估 12–15 个测试文件，实际声明 9 个                             | ✅ 成立                                                                                                                                | **已修**：改为「10 个」（含 Task 2b）                                                                         |
| **答复 1**   | 墓碑规则未覆盖 `profile_revisions`                               | ✅ 成立（它也没有 `deleted_at`）                                                                                                       | **采纳**：补全四行规则表                                                                                      |
| **答复 2**   | 建议 `events` 不同步，改 spec                                    | ✅ 成立，理由比计划的默认更强                                                                                                          | **采纳**：**已改 spec §6.3**，从同步范围移除 `events`                                                         |
| **答复 3**   | 冲突解决追加 change 的 lamport 未写                              | ✅ 成立                                                                                                                                | **采纳**：用 `next_lamport()` = 当前 + 1                                                                      |
| **依赖遗漏** | `rpassword` 不在 Cargo.lock                                      | ✅ 成立（`grep` 零命中）                                                                                                               | **采纳**：依赖清单改为三个；已实测 `rpassword v7.5.4` 可通过南大镜像获取                                      |

### S-1 值得单独记：同一形态缺陷的第三次

| 轮次 | 缺陷                         | 后果                 | 发现时机            |
| ---- | ---------------------------- | -------------------- | ------------------- |
| M1   | `.pebak` 写入器零调用者      | 备份仍写明文         | 审查（第三轮）      |
| M2   | `index_snapshot_tx` 零调用者 | 搜索要等重启         | **写 CHANGELOG 时** |
| M3   | `record_change_tx` 零调用者  | **整条同步链路空转** | **计划核对轮**      |

**三次的严重性递增，发现时机逐次提前。** 这次是「计划里就没有那个 Task」——不是实现漏了，
是**计划漏了**。开发窗口在写代码前抓到，避免了第三次返工。

**已写进 Global Constraints 的检查方法**（含「先验证 grep 模式本身」这一步）：

```bash
# 0. 先用已知有调用者的函数验证 grep 模式有效
grep -rn "dashboard(" --include=*.rs core/src gui/src cli/src | grep -v "fn dashboard"
#    必须 ≥1 命中，否则模式本身有问题

# 1. 再查目标函数
grep -rn "<fn_name>(" --include=*.rs core/src gui/src cli/src | grep -v "fn <fn_name>"
```

> ⚠️ 不要用 `\b` 词边界——它匹配不到 `refs::find_cycles` 这类带模块前缀的调用。

---

## Execution Notes

**（留给开发窗口回填）**

- 偏离记录：
- 未覆盖项：
- 残余风险：
- 实际测试数：
