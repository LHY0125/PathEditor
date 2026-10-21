# PathEditor v6.0.0 M4（配置同步与清理发布）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 补齐 M3 遗留的配置同步（`profile` / `profile_revision` 落库、同名自动改名、墓碑），完成 v6.0.0 的保留策略清理、`.v5.bak` 归档、遗留明文备份清理、v5 配置导入与版本发布准备。

**Architecture:** 所有业务规则在 `core`；GUI/CLI 只做参数转换与错误呈现。配置同步沿用 M3 的单事务导入管线，新增按实体依赖的稳定排序与实体级 LWW；清理与归档是显式维护任务，路径全部由调用方传入，测试用 `TempDir`。

**Tech Stack:** Rust（core/gui/cli workspace）、rusqlite + SQLCipher、serde、chrono、React 19 + Zustand（提示文案）、Clap（CLI）。

**Spec:** `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` —— §4.2 第 10 条、§5.3、§6.3、§6.4、§6.5、§7.4、§7.6、§十二、§十三 M4。

## Global Constraints

- 分支 `main`；每个 Task 一个 Conventional Commit。**授权：提交 + 推送 + 升级版本号到 6.0.0；不打 tag、不创建 Release。**
- 未经用户书面同意不删除任何文件。开发窗口**不得**对真实 `~/.patheditor` 执行归档/清理；测试全部用 `TempDir`。
- `config_dir()` 走 `SHGetKnownFolderPath`，`HOME` / `USERPROFILE` 都不可重定向——所有涉及用户目录的函数必须显式接收路径参数。
- **一文件一测试**：任何使用 SQLCipher 的 `.rs` 测试文件只放一个 `#[test]`（同进程第二个 SQLCipher 测试会 `STATUS_STACK_OVERFLOW`）。
- 每个新增 `pub fn` / `pub(crate) fn` 必须有生产调用者；先跑一次已知有调用者的 grep 验证模式（不要用 `\b`）。
- E2E 用 mock IPC，永远不碰真实注册表；本轮所有改动都不写注册表。
- 注释用中文；Rust 4 空格；TS 2 空格、单引号、100 列；所有 `pub fn` 有中文 `///`。
- 每个 Task 结束跑：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`npx tsc --noEmit`、`npx vitest run`。不要攒到最后。
- 基线测试数（M4 开工时）：Rust **48 binaries / 340 passed**；前端 **33 files / 397 passed**；E2E **54 passed**。任何下降都要当场解释。
- `npm run verify:all` 只在 Task 11 跑一次全量。

---

### Task 1: profile 落库 + 依赖排序

**Files:**

- Modify: `core/src/db/sync/import.rs`
- Modify: `core/src/db/profiles.rs`
- Test: `core/tests/db_sync_profile_order.rs`（新建，1 个 `#[test]`）

**Interfaces:**

- Consumes: `InboundChange { entity_type, entity_uid, device_id, field, content, .. }`、`profiles::ProfileView`
- Produces:
  - `profiles::ProfileSyncContent`（`pub(crate)`，接受侧形状）
  - `profiles::import_profile_content_tx(tx, uid, content, device_id) -> Result<(), CoreError>`（`pub(crate)`）
  - `import.rs` 内部：`entity_priority(entity_type) -> u8`、`latest_for_entity_tx(tx, entity_type, entity_uid)`

- [ ] **Step 1: 写失败测试**

创建 `core/tests/db_sync_profile_order.rs`：

```rust
//! 配置同步：profile content 落库，且 profile 在 bundle 中先于 snapshot 时仍能应用。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::Db;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join(name), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    db
}

fn write_bundle(path: &std::path::Path, changes_json: &str, count: u32) {
    let header = BundleHeader {
        schema_version: 3,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 9,
        change_count: count,
    };
    let payload = format!("{{\"changes\":{changes_json}}}");
    std::fs::write(path, seal("pw", &header, payload.as_bytes()).unwrap()).unwrap();
}

#[test]
fn profile_is_applied_after_its_snapshot_even_if_bundle_order_is_reversed() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "profile_order.db");

    // **profile 在前、snapshot 在后**：不按依赖排序时，profile 的 snapshotUid
    // 查不到本地快照，必须报 SyncFormat 并整包回滚。
    let changes = r#"[
      {
        "uid": "c-prof", "deviceId": "dev-x", "seq": 1, "lamport": 1,
        "entityType": "profile", "entityUid": "p-1", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": {
          "uid": "p-1", "name": "Work", "description": null, "tags": ["a"],
          "createdAt": 1, "updatedAt": 2, "version": 1, "snapshotUid": "snap-1"
        }
      },
      {
        "uid": "c-snap", "deviceId": "dev-x", "seq": 2, "lamport": 2,
        "entityType": "snapshot", "entityUid": "snap-1", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": {
          "uid": "snap-1", "createdAt": 1, "kind": "current",
          "system": [{"path": "C:\\A", "enabled": true}], "user": [], "env": []
        }
      }
    ]"#;
    let path = dir.path().join("profile_order.pesync");
    write_bundle(&path, changes, 2);

    let report = import_bundle(&mut db, "pw", &path).unwrap();
    assert_eq!(report.applied, 2, "快照与配置都必须落库");
    assert_eq!(report.skipped_unsupported, 0, "M4 起已知类型全部可落库");

    let name: String = db
        .query_row_with("SELECT name FROM profiles WHERE uid = 'p-1'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(name, "Work");

    let revisions: i64 = db
        .query_row_with(
            "SELECT count(*) FROM profile_revisions r
               JOIN snapshots s ON s.id = r.snapshot_id
              WHERE s.uid = 'snap-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(revisions, 1, "profile_revision 必须指向导入的快照");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path_editor_core --test db_sync_profile_order`
Expected: FAIL —— `profiles` 表无 `p-1`（当前 `APPLICABLE_ENTITY_TYPES` 只有 `snapshot`）。

- [ ] **Step 3: 实现依赖排序与可落库类型**

`core/src/db/sync/import.rs`：

```rust
/// 当前版本能真正写入数据库的实体类型（M4：全部已知类型）。
///
/// `profile_revision` / `path_entry` / `env_value` 的 `content` 为 `null`
/// （内容随父实体），`apply_entity_tx` 对它们是显式 no-op。
const APPLICABLE_ENTITY_TYPES: &[&str] = &[
    "snapshot",
    "profile",
    "profile_revision",
    "path_entry",
    "env_value",
];

/// 导入时的依赖优先级：数值小的先应用。
///
/// profile 的 `content.snapshotUid` 引用快照，必须等快照落库后才能解析。
fn entity_priority(entity_type: &str) -> u8 {
    match entity_type {
        "snapshot" => 0,
        "profile" => 1,
        "profile_revision" => 2,
        "path_entry" => 3,
        "env_value" => 4,
        _ => 5,
    }
}
```

在 `import_bundle` 的白名单校验之后、`db.with_transaction` 之前加入：

```rust
    // ④ 依赖排序：`sort_by_key` 是稳定排序，同类型内部维持包内原有顺序。
    let mut ordered: Vec<&InboundChange> = payload.changes.iter().collect();
    ordered.sort_by_key(|c| entity_priority(&c.entity_type));
```

把事务里的 `for c in &payload.changes` 改成 `for c in ordered`。

新增实体级查询（放在 `latest_for_tx` 旁边）：

```rust
/// 取某实体**最新的一条变更（不限 field）**。
///
/// `snapshot` / `profile` 用实体级 LWW：墓碑是 `field = "deleted_at"` 的变更，
/// 若按 field 比较，它和内容变更（`field = null`）会落在两条互不可见的时间线上，
/// 旧内容会在删除之后「复活」（spec §6.5.1）。
fn latest_for_entity_tx(
    tx: &Transaction,
    entity_type: &str,
    entity_uid: &str,
) -> Result<Option<ChangeRow>, CoreError> {
    use rusqlite::OptionalExtension as _;

    tx.query_row(
        "SELECT uid, device_id, seq, lamport, origin, entity_type, entity_uid,
                field, old_value, new_value, created_at, applied_at, exported_at
           FROM changes
          WHERE entity_type = ?1 AND entity_uid = ?2
          ORDER BY lamport DESC, device_id DESC
          LIMIT 1",
        rusqlite::params![entity_type, entity_uid],
        |r| {
            Ok(ChangeRow {
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
        },
    )
    .optional()
    .map_err(|e| crate::db::db_err("latest_entity", e))
}
```

把循环里的 LWW 本地值选择改成：

```rust
            let local = match c.entity_type.as_str() {
                // 实体级：内容变更与墓碑共用一条时间线
                "snapshot" | "profile" => {
                    latest_for_entity_tx(tx, &c.entity_type, &c.entity_uid)?
                }
                _ => latest_for_tx(tx, &c.entity_type, &c.entity_uid, c.field.as_deref())?,
            };
```

`apply_entity_tx` 增加 `device_id` 参数并按类型分派：

```rust
fn apply_entity_tx(
    tx: &Transaction,
    entity_type: &str,
    entity_uid: &str,
    content: &serde_json::Value,
    device_id: &str,
) -> Result<(), CoreError> {
    match entity_type {
        "snapshot" => {
            let snap = snapshot_from_content(content, entity_uid)?;
            crate::db::history::insert_snapshot_with_uid_tx(tx, snap, entity_uid, false)?;
            Ok(())
        }
        "profile" => crate::db::profiles::import_profile_content_tx(
            tx, entity_uid, content, device_id,
        ),
        // 内容随父实体：变更本身已落 `changes`，这里不单独写实体。
        "profile_revision" | "path_entry" | "env_value" => Ok(()),
        other => Err(CoreError::new(
            ErrorCode::SyncFormat,
            "import",
            format!("内部错误：实体类型 {other} 未实现落库"),
        )),
    }
}
```

调用处改为：

```rust
                    apply_entity_tx(tx, &c.entity_type, &c.entity_uid, content, &c.device_id)?;
```

- [ ] **Step 4: 实现 `import_profile_content_tx`**

`core/src/db/profiles.rs` 增加：

```rust
/// 同步包里的配置内容（接受侧子集）。
///
/// **不含** `applyCount` / `lastAppliedAt`：使用统计是本机数据，不跨设备同步。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileSyncContent {
    /// 配置名称
    pub name: String,
    /// 描述
    pub description: Option<String>,
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 创建时间（Unix 毫秒）
    pub created_at: i64,
    /// 更新时间（Unix 毫秒）
    pub updated_at: i64,
    /// CAS 版本号，同时是 revision 的版本号
    pub version: i64,
    /// 该版本指向的快照 uid
    pub snapshot_uid: Option<String>,
}

/// 按**远端 uid** 落库一条配置（同步导入用）。
///
/// 保留远端 uid：bundle 内所有引用都指向它。`profile_revisions.uid` 由本地生成
/// （bundle 不引用 revision uid）；`(profile_id, version)` 用 upsert 更新 `snapshot_id`。
///
/// 已存在同 uid 的行时更新元数据并**复活**（`deleted_at = NULL`）——是否符合预期
/// 由调用方的 LWW 决定，本函数只负责按内容写入。
pub(crate) fn import_profile_content_tx(
    tx: &Transaction,
    uid: &str,
    content: &serde_json::Value,
    _device_id: &str,
) -> Result<(), CoreError> {
    let c: ProfileSyncContent = serde_json::from_value(content.clone()).map_err(|e| {
        CoreError::new(
            ErrorCode::SyncFormat,
            "import_profile",
            format!("配置内容无法解析: {e}"),
        )
    })?;

    // 悬空 snapshotUid → 整包回滚（spec §6.5.1）
    let snapshot_id: Option<i64> = match &c.snapshot_uid {
        Some(s) => {
            let id = tx
                .query_row("SELECT id FROM snapshots WHERE uid = ?1", params![s], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(|e| db_err("import_profile_snapshot", e))?;
            Some(id.ok_or_else(|| {
                CoreError::new(
                    ErrorCode::SyncFormat,
                    "import_profile",
                    format!("配置引用的快照不存在: {s}"),
                )
            })?)
        }
        None => None,
    };

    let existing: Option<i64> = tx
        .query_row("SELECT id FROM profiles WHERE uid = ?1", params![uid], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| db_err("find_profile_uid", e))?;

    let tags_json = serde_json::to_string(&c.tags).unwrap_or_else(|_| "[]".to_string());
    let version = c.version.max(1);

    match existing {
        Some(id) => {
            tx.execute(
                "UPDATE profiles
                    SET name = ?1, description = ?2, tags = ?3, created_at = ?4,
                        updated_at = ?5, version = ?6, deleted_at = NULL
                  WHERE id = ?7",
                params![
                    c.name,
                    c.description,
                    tags_json,
                    c.created_at,
                    c.updated_at,
                    version,
                    id
                ],
            )
            .map_err(|e| db_err("import_profile_update", e))?;
            if let Some(sid) = snapshot_id {
                upsert_revision_tx(tx, id, version, c.updated_at, sid)?;
            }
        }
        None => {
            tx.execute(
                "INSERT INTO profiles
                   (uid, name, description, tags, created_at, updated_at, apply_count, version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
                params![
                    uid,
                    c.name,
                    c.description,
                    tags_json,
                    c.created_at,
                    c.updated_at,
                    version
                ],
            )
            .map_err(|e| db_err("import_profile_insert", e))?;
            let id = tx.last_insert_rowid();
            if let Some(sid) = snapshot_id {
                upsert_revision_tx(tx, id, version, c.updated_at, sid)?;
            }
        }
    }
    Ok(())
}

/// 插入/更新一条 revision（`(profile_id, version)` 唯一）。
fn upsert_revision_tx(
    tx: &Transaction,
    profile_id: i64,
    version: i64,
    created_at: i64,
    snapshot_id: i64,
) -> Result<(), CoreError> {
    tx.execute(
        "INSERT INTO profile_revisions (uid, profile_id, version, created_at, snapshot_id)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(profile_id, version) DO UPDATE SET snapshot_id = excluded.snapshot_id",
        params![
            Uuid::now_v7().to_string(),
            profile_id,
            version,
            created_at,
            snapshot_id
        ],
    )
    .map_err(|e| db_err("upsert_revision", e))?;
    Ok(())
}
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p path_editor_core --test db_sync_profile_order`
Expected: PASS（1 passed）。

- [ ] **Step 6: 跑既有同步测试**

Run: `cargo test -p path_editor_core --test db_sync_import --test db_sync_import_limits --test db_sync_conflict --test db_sync_merge --test db_sync_export`
Expected: 全部 passed。

- [ ] **Step 7: 质量门 + 提交**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add core/src/db/sync/import.rs core/src/db/profiles.rs core/tests/db_sync_profile_order.rs
git commit -m "feat(v6): M4 配置同步落库与依赖排序"
```

---

### Task 2: 同名自动改名 + `renamedProfiles` 计数

**Files:**

- Modify: `core/src/db/profiles.rs`
- Modify: `core/src/db/sync/import.rs`
- Modify: `gui/src/commands/sync.rs`（若导入报告在此序列化则无需改；只确认）
- Modify: `src/services/backend.ts`、`src/store/sync-store.ts`、`src/components/sync/SyncCenter.tsx`、`src/i18n/locales/zh-CN.json`、`src/i18n/locales/en.json`
- Modify: `cli/src/sync_ops.rs`、`e2e/mocks/ipc.ts`
- Test: `core/tests/db_sync_profile_name_conflict.rs`（新建）
- Test: `tests/unit/backend-sync-shapes.test.ts`、`tests/unit/sync-center.test.tsx`（改）

**Interfaces:**

- Consumes: Task 1 的 `import_profile_content_tx`
- Produces:
  - `ImportReport { applied, skipped, conflicts, skipped_unsupported, renamed_profiles }`
  - `profiles::unique_name_tx(tx, desired, self_uid, device_id) -> Result<String, CoreError>`（`pub(crate)`）
  - `apply_entity_tx(...) -> Result<bool, CoreError>`（true = 发生了改名）

- [ ] **Step 1: 写失败测试**

创建 `core/tests/db_sync_profile_name_conflict.rs`：

```rust
//! 同名不同 uid 的配置：自动改名导入，本地配置不受影响。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::{profiles, Db};
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use rusqlite::params;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join(name), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    db
}

#[test]
fn same_name_different_uid_is_imported_with_device_suffix() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "name_conflict.db");

    // 本地已有 "Work"
    let snap = NewSnapshot {
        kind: SnapshotKind::Current,
        source: EventSource::Gui,
        system: vec![PathEntry { path: "C:\\LOCAL".to_string(), enabled: true }],
        user: vec![],
        env: vec![],
        parent_uid: None,
    };
    let local_uid = profiles::save_profile(&mut db, "Work", None, &[], snap).unwrap();

    // 远端也有一个 "Work"，uid 不同
    let changes = r#"[
      {
        "uid": "c-snap", "deviceId": "dev-x", "seq": 1, "lamport": 10,
        "entityType": "snapshot", "entityUid": "snap-remote", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": {
          "uid": "snap-remote", "createdAt": 1, "kind": "current",
          "system": [{"path": "C:\\REMOTE", "enabled": true}], "user": [], "env": []
        }
      },
      {
        "uid": "c-prof", "deviceId": "dev-x", "seq": 2, "lamport": 11,
        "entityType": "profile", "entityUid": "p-remote", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": {
          "uid": "p-remote", "name": "Work", "description": null, "tags": [],
          "createdAt": 1, "updatedAt": 2, "version": 1, "snapshotUid": "snap-remote"
        }
      }
    ]"#;
    let header = BundleHeader {
        schema_version: 3,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 10,
        change_count: 2,
    };
    let path = dir.path().join("conflict.pesync");
    std::fs::write(
        &path,
        seal("pw", &header, format!("{{\"changes\":{changes}}}").as_bytes()).unwrap(),
    )
    .unwrap();

    let report = import_bundle(&mut db, "pw", &path).unwrap();
    assert_eq!(report.renamed_profiles, 1, "必须报告改名数量");
    assert_eq!(report.applied, 2);

    let local_name: String = db
        .query_row_with(
            "SELECT name FROM profiles WHERE uid = ?1",
            params![local_uid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(local_name, "Work", "本地配置不得被改名");

    let imported_name: String = db
        .query_row_with("SELECT name FROM profiles WHERE uid = 'p-remote'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(imported_name, "Work (来自 dev-x)");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path_editor_core --test db_sync_profile_name_conflict`
Expected: 编译失败（`renamed_profiles` 字段不存在）/ 运行失败（名称冲突触发唯一索引）。

- [ ] **Step 3: 实现唯一名称生成**

`core/src/db/profiles.rs`：

```rust
/// 为导入的配置生成一个本机唯一的未删除名称。
///
/// 冲突时改为 `"{desired} (来自 {device_id 前 8 位})"`；仍冲突则追加 ` 2`、` 3`…
/// 只影响本地行，不改变包内 uid（spec §6.5.1）。
fn unique_name_tx(
    tx: &Transaction,
    desired: &str,
    self_uid: &str,
    device_id: &str,
) -> Result<String, CoreError> {
    let taken = |name: &str| -> Result<bool, CoreError> {
        let n: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM profiles
                  WHERE name = ?1 AND deleted_at IS NULL AND uid <> ?2",
                params![name, self_uid],
                |r| r.get(0),
            )
            .map_err(|e| db_err("name_taken", e))?;
        Ok(n > 0)
    };

    if !taken(desired)? {
        return Ok(desired.to_string());
    }

    // 名称上限 255 字节（validate_name）；给后缀留出空间。
    let base_name = truncate_name(desired, 200);
    // 按字符取前 8 个：device_id 正常是 UUID，但包里是外部输入，
    // 直接按字节切片会在多字节字符中间 panic。
    let suffix: String = device_id.chars().take(8).collect();
    let base = format!("{base_name} (来自 {suffix})");
    if !taken(&base)? {
        return Ok(base);
    }
    for i in 2..100 {
        let candidate = format!("{base} {i}");
        if !taken(&candidate)? {
            return Ok(candidate);
        }
    }
    Err(CoreError::new(
        ErrorCode::SyncFormat,
        "import_profile",
        "同名配置过多，无法生成唯一名称",
    ))
}

/// 按字节上限截断名称（不切断 UTF-8 字符）。
fn truncate_name(name: &str, max_bytes: usize) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if out.len() + ch.len_utf8() > max_bytes {
            break;
        }
        out.push(ch);
    }
    out
}
```

`import_profile_content_tx` 中把 `c.name` 的写入替换为：

```rust
    let name = unique_name_tx(tx, &c.name, uid, device_id)?;
```

把 `_device_id` 改名为 `device_id`。函数返回 `Result<bool, CoreError>`（true = 改名）：

```rust
    let renamed = name != c.name;
    // …用 name 写入…
    Ok(renamed)
```

- [ ] **Step 4: 把改名计数接进 `ImportReport`**

`core/src/db/sync/import.rs`：

```rust
pub struct ImportReport {
    pub applied: u32,
    pub skipped: u32,
    pub conflicts: u32,
    pub skipped_unsupported: u32,
    /// 因同名而自动改名的配置数（M4）
    pub renamed_profiles: u32,
}
```

`apply_entity_tx` 返回 `Result<bool, CoreError>`；非 profile 分支 `Ok(false)`，profile 分支透传。
循环里：

```rust
                    if apply_entity_tx(tx, &c.entity_type, &c.entity_uid, content, &c.device_id)? {
                        report.renamed_profiles += 1;
                    }
```

- [ ] **Step 5: 前端形状与文案**

`src/services/backend.ts`：

```ts
export interface SyncImportReport {
  applied: number;
  skipped: number;
  conflicts: number;
  skippedUnsupported: number;
  /** 因同名而自动改名的配置数（M4） */
  renamedProfiles: number;
}
```

`parseSyncImportReport` 增加 `typeof value.renamedProfiles !== 'number'` 校验，并回填字段。

`src/components/sync/SyncCenter.tsx` 的 `renderNotice`：

```tsx
    case 'imported':
      if (notice.report.skippedUnsupported > 0) {
        return t('sync.noticeImportedUnsupported', {
          applied: notice.report.applied,
          skipped: notice.report.skipped,
          conflicts: notice.report.conflicts,
          unsupported: notice.report.skippedUnsupported,
        });
      }
      if (notice.report.renamedProfiles > 0) {
        return t('sync.noticeImportedRenamed', {
          applied: notice.report.applied,
          skipped: notice.report.skipped,
          conflicts: notice.report.conflicts,
          renamed: notice.report.renamedProfiles,
        });
      }
      return t('sync.noticeImported', {
        applied: notice.report.applied,
        skipped: notice.report.skipped,
        conflicts: notice.report.conflicts,
      });
```

locale 各加一键：

```json
"noticeImportedRenamed": "导入完成：应用 {{applied}}，跳过 {{skipped}}，冲突 {{conflicts}}；{{renamed}} 条配置因重名已自动改名"
```

```json
"noticeImportedRenamed": "Import done: applied {{applied}}, skipped {{skipped}}, conflicts {{conflicts}}; {{renamed}} profile(s) were renamed to avoid a name clash"
```

`cli/src/sync_ops.rs` 在导入输出后追加：

```rust
    if report.renamed_profiles > 0 {
        eprintln!(
            "提示: {} 条配置因与本地重名，已自动改名为「原名 (来自 设备)」",
            report.renamed_profiles
        );
    }
```

`e2e/mocks/ipc.ts` 的返回改为 `{ applied: 2, skipped: 0, conflicts: 0, skippedUnsupported: 0, renamedProfiles: 0 }`。

- [ ] **Step 6: 更新前端测试**

`tests/unit/backend-sync-shapes.test.ts`：所有 `parseSyncImportReport` 入参补 `renamedProfiles`；新增用例「缺 `renamedProfiles` → 抛错」。
`tests/unit/sync-center.test.tsx`：现有 report 字面量补 `renamedProfiles: 0`；新增一条：

```tsx
it('重名改名时提示必须说出来', () => {
  mockState.notice = {
    kind: 'imported',
    report: { applied: 1, skipped: 0, conflicts: 0, skippedUnsupported: 0, renamedProfiles: 1 },
  };
  render(<SyncCenter />);
  expect(screen.getByRole('status').textContent).toContain('1 条配置因重名已自动改名');
});
```

- [ ] **Step 7: 跑测试**

```powershell
cargo test -p path_editor_core --test db_sync_profile_name_conflict
npx tsc --noEmit
npx vitest run tests/unit/backend-sync-shapes.test.ts tests/unit/sync-center.test.tsx
```

Expected: 全部通过。

- [ ] **Step 8: 质量门 + 提交**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add core/src/db/profiles.rs core/src/db/sync/import.rs core/tests/db_sync_profile_name_conflict.rs cli/src/sync_ops.rs src/services/backend.ts src/components/sync/SyncCenter.tsx src/i18n/locales/zh-CN.json src/i18n/locales/en.json e2e/mocks/ipc.ts tests/unit/backend-sync-shapes.test.ts tests/unit/sync-center.test.tsx
git commit -m "feat(v6): 配置同步遇到同名时自动改名并在导入提示中计数"
```

---

### Task 3: 墓碑软删除

**Files:**

- Modify: `core/src/db/sync/import.rs`
- Test: `core/tests/db_sync_profile_tombstone.rs`（新建）

**Interfaces:**

- Consumes: Task 1 的实体级 LWW
- Produces: `import.rs` 内部 `apply_tombstone_tx(tx, entity_type, entity_uid)`

- [ ] **Step 1: 写失败测试**

创建 `core/tests/db_sync_profile_tombstone.rs`：

```rust
//! 墓碑：`content = null` 且 `field = "deleted_at"` → 本地软删除。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::{profiles, Db};
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join(name), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    db
}

#[test]
fn tombstone_soft_deletes_profile_and_frees_the_name() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "tombstone.db");

    let snap = NewSnapshot {
        kind: SnapshotKind::Current,
        source: EventSource::Gui,
        system: vec![PathEntry { path: "C:\\A".to_string(), enabled: true }],
        user: vec![],
        env: vec![],
        parent_uid: None,
    };
    profiles::save_profile(&mut db, "Work", None, &[], snap).unwrap();
    let uid: String = db
        .query_row_with("SELECT uid FROM profiles WHERE name = 'Work'", [], |r| r.get(0))
        .unwrap();

    let changes = format!(
        r#"[{{
          "uid": "c-tomb", "deviceId": "dev-x", "seq": 9, "lamport": 999,
          "entityType": "profile", "entityUid": "{uid}", "field": "deleted_at",
          "oldValue": "null", "newValue": "2000000000000", "createdAt": 0,
          "content": null
        }}]"#
    );
    let header = BundleHeader {
        schema_version: 3,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 999,
        change_count: 1,
    };
    let path = dir.path().join("tomb.pesync");
    std::fs::write(
        &path,
        seal("pw", &header, format!("{{\"changes\":{changes}}}").as_bytes()).unwrap(),
    )
    .unwrap();

    let report = import_bundle(&mut db, "pw", &path).unwrap();
    assert_eq!(report.applied, 1);

    let deleted: Option<i64> = db
        .query_row_with(
            "SELECT deleted_at FROM profiles WHERE uid = ?1",
            rusqlite::params![uid],
            |r| r.get(0),
        )
        .unwrap();
    assert!(deleted.is_some(), "墓碑必须写入 deleted_at");

    let active: i64 = db
        .query_row_with(
            "SELECT COUNT(*) FROM profiles WHERE name = 'Work' AND deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(active, 0, "名称必须被释放");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path_editor_core --test db_sync_profile_tombstone`
Expected: FAIL —— `deleted_at` 仍为 NULL。

- [ ] **Step 3: 实现墓碑分支**

`core/src/db/sync/import.rs` 的循环中，在「应用实体内容」之前加入：

```rust
            // 墓碑：`content = null` 且 field 明确是 deleted_at。
            //
            // 与内容变更共用 seen_entities 去重：同一实体在一包里只处理一次。
            if c.content.is_none() && c.field.as_deref() == Some("deleted_at") {
                let key = (c.entity_type.clone(), c.entity_uid.clone());
                if seen_entities.insert(key) {
                    apply_tombstone_tx(tx, &c.entity_type, &c.entity_uid)?;
                }
            }
```

新增函数：

```rust
/// 应用墓碑：按 uid 软删除实体。
///
/// 只支持有 `deleted_at` 列的实体；其它类型说明对端用了本版本没有的语义，
/// 返回 `SyncFormat` 整包回滚，而不是静默忽略。
fn apply_tombstone_tx(
    tx: &Transaction,
    entity_type: &str,
    entity_uid: &str,
) -> Result<(), CoreError> {
    let now = chrono::Utc::now().timestamp_millis();
    let sql = match entity_type {
        "profile" => {
            "UPDATE profiles SET deleted_at = ?1, updated_at = ?1, version = version + 1
              WHERE uid = ?2 AND deleted_at IS NULL"
        }
        "snapshot" => {
            "UPDATE snapshots SET deleted_at = ?1, updated_at = ?1
              WHERE uid = ?2 AND deleted_at IS NULL"
        }
        other => {
            return Err(CoreError::new(
                ErrorCode::SyncFormat,
                "import",
                format!("实体类型 {other} 不支持墓碑"),
            ))
        }
    };
    tx.execute(sql, rusqlite::params![now, entity_uid])
        .map_err(|e| crate::db::db_err("apply_tombstone", e))?;
    Ok(())
}
```

- [ ] **Step 4: 跑测试**

Run: `cargo test -p path_editor_core --test db_sync_profile_tombstone`
Expected: PASS。

- [ ] **Step 5: 质量门 + 提交**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add core/src/db/sync/import.rs core/tests/db_sync_profile_tombstone.rs
git commit -m "feat(v6): 同步导入支持配置墓碑软删除"
```

---

### Task 4: 墓碑与旧内容的时序回归

**Files:**

- Test: `core/tests/db_sync_profile_resurrect.rs`（新建）
- 可能修改: `core/src/db/sync/import.rs`（若测试暴露实体级 LWW 未生效）

**Interfaces:**

- Consumes: Task 1 的 `latest_for_entity_tx`、Task 3 的墓碑

- [ ] **Step 1: 写失败测试**

创建 `core/tests/db_sync_profile_resurrect.rs`：

```rust
//! 墓碑与旧内容：删除之后，低 lamport 的内容变更不得让配置「复活」。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::Db;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join(name), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    db
}

fn import_changes(db: &mut Db, dir: &TempDir, file: &str, changes: &str, count: u32) -> path_editor_core::db::sync::ImportReport {
    let header = BundleHeader {
        schema_version: 3,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 100,
        change_count: count,
    };
    let path = dir.path().join(file);
    std::fs::write(
        &path,
        seal("pw", &header, format!("{{\"changes\":{changes}}}").as_bytes()).unwrap(),
    )
    .unwrap();
    import_bundle(db, "pw", &path).unwrap()
}

const SNAPSHOT: &str = r#"{
  "uid": "c-snap", "deviceId": "dev-x", "seq": 1, "lamport": 1,
  "entityType": "snapshot", "entityUid": "snap-1", "field": null,
  "oldValue": null, "newValue": "{}", "createdAt": 0,
  "content": { "uid": "snap-1", "createdAt": 1, "kind": "current",
               "system": [], "user": [], "env": [] }
}"#;

#[test]
fn older_content_does_not_resurrect_a_deleted_profile() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "resurrect.db");

    // 1) 建立配置（lamport 1）
    let create = format!(
        r#"[{SNAPSHOT}, {{
          "uid": "c-prof-1", "deviceId": "dev-x", "seq": 2, "lamport": 1,
          "entityType": "profile", "entityUid": "p-1", "field": null,
          "oldValue": null, "newValue": "{{}}", "createdAt": 0,
          "content": {{ "uid": "p-1", "name": "Work", "description": null, "tags": [],
                       "createdAt": 1, "updatedAt": 2, "version": 1, "snapshotUid": "snap-1" }}
        }}]"#
    );
    import_changes(&mut db, &dir, "create.pesync", &create, 2);

    // 2) 删除（lamport 5）
    let tomb = r#"[{
      "uid": "c-tomb", "deviceId": "dev-x", "seq": 3, "lamport": 5,
      "entityType": "profile", "entityUid": "p-1", "field": "deleted_at",
      "oldValue": "null", "newValue": "2000000000000", "createdAt": 0, "content": null
    }]"#;
    import_changes(&mut db, &dir, "tomb.pesync", tomb, 1);

    // 3) 旧的 profile 内容（lamport 3）必须被 KeepLocal 跳过
    let older = r#"[{
      "uid": "c-prof-old", "deviceId": "dev-x", "seq": 4, "lamport": 3,
      "entityType": "profile", "entityUid": "p-1", "field": null,
      "oldValue": null, "newValue": "{}", "createdAt": 0,
      "content": { "uid": "p-1", "name": "Work", "description": null, "tags": [],
                   "createdAt": 1, "updatedAt": 2, "version": 1, "snapshotUid": "snap-1" }
    }]"#;
    let report = import_changes(&mut db, &dir, "older.pesync", older, 1);
    assert_eq!(report.skipped, 1, "低 lamport 的旧内容必须跳过");

    let deleted: Option<i64> = db
        .query_row_with("SELECT deleted_at FROM profiles WHERE uid = 'p-1'", [], |r| r.get(0))
        .unwrap();
    assert!(deleted.is_some(), "删除状态不得被旧内容覆盖");
}
```

- [ ] **Step 2: 跑测试**

Run: `cargo test -p path_editor_core --test db_sync_profile_resurrect`
Expected: PASS（若 Task 1 的实体级 LWW 正确）。若 FAIL，检查 `latest_for_entity_tx` 是否真的被用于 `profile`。

- [ ] **Step 3: 质量门 + 提交（测试即交付）**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add core/tests/db_sync_profile_resurrect.rs
git commit -m "test(v6): 墓碑不会被低 lamport 的旧内容复活"
```

---

### Task 5: 幂等回归

**Files:**

- Test: `core/tests/db_sync_profile_idempotent.rs`（新建）
- 可能修改: `core/src/db/sync/import.rs`

- [ ] **Step 1: 写测试**

创建 `core/tests/db_sync_profile_idempotent.rs`：

```rust
//! 同一包重复导入：配置与 revision 不得重复，第二次全部走幂等跳过。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn reimporting_the_same_bundle_does_not_duplicate_profiles_or_revisions() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("idem.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let changes = r#"[
      { "uid": "c-snap", "deviceId": "dev-x", "seq": 1, "lamport": 1,
        "entityType": "snapshot", "entityUid": "snap-1", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": { "uid": "snap-1", "createdAt": 1, "kind": "current",
                     "system": [], "user": [], "env": [] } },
      { "uid": "c-prof", "deviceId": "dev-x", "seq": 2, "lamport": 2,
        "entityType": "profile", "entityUid": "p-1", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": { "uid": "p-1", "name": "Work", "description": null, "tags": [],
                     "createdAt": 1, "updatedAt": 2, "version": 1, "snapshotUid": "snap-1" } }
    ]"#;
    let header = BundleHeader {
        schema_version: 3,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 10,
        change_count: 2,
    };
    let path = dir.path().join("idem.pesync");
    std::fs::write(
        &path,
        seal("pw", &header, format!("{{\"changes\":{changes}}}").as_bytes()).unwrap(),
    )
    .unwrap();

    let first = import_bundle(&mut db, "pw", &path).unwrap();
    assert_eq!(first.applied, 2);

    let second = import_bundle(&mut db, "pw", &path).unwrap();
    assert_eq!(second.applied, 0);
    assert_eq!(second.skipped, 2);
    assert_eq!(second.renamed_profiles, 0);

    let profiles: i64 = db
        .query_row_with("SELECT COUNT(*) FROM profiles WHERE uid = 'p-1'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(profiles, 1);

    let revisions: i64 = db
        .query_row_with(
            "SELECT COUNT(*) FROM profile_revisions WHERE profile_id =
               (SELECT id FROM profiles WHERE uid = 'p-1')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(revisions, 1, "重复导入不得生成第二个 revision");
}
```

- [ ] **Step 2: 跑测试**

Run: `cargo test -p path_editor_core --test db_sync_profile_idempotent`
Expected: PASS。

- [ ] **Step 3: 质量门 + 提交**

```powershell
cargo test --workspace
git add core/tests/db_sync_profile_idempotent.rs
git commit -m "test(v6): 配置同步重复导入保持幂等"
```

---

### Task 6: 保留策略清理

**Files:**

- Create: `core/src/db/maintenance.rs`
- Modify: `core/src/db/mod.rs`（`pub mod maintenance;`）
- Modify: `gui/src/lib.rs`（启动序列调用）
- Modify: `cli/src/main.rs`、`cli/src/maintenance_ops.rs`（新建）
- Test: `core/tests/db_maintenance_prune.rs`（新建）

**Interfaces:**

- Produces:
  - `maintenance::CHANGES_RETENTION_MS` / `CONFLICTS_RETENTION_MS`
  - `maintenance::prune(db, now_ms) -> Result<PruneReport, CoreError>`
  - `maintenance::prune_plan(db, now_ms) -> Result<PruneReport, CoreError>`（dry-run 计数）
  - `maintenance::prune_now(db) -> Result<PruneReport, CoreError>`（GUI 用；时间源收在 core，
    避免 GUI crate 新增 `chrono` 依赖）
  - `PruneReport { changes_deleted, conflicts_deleted }`（camelCase 序列化）

- [ ] **Step 1: 写失败测试**

创建 `core/tests/db_maintenance_prune.rs`：

```rust
//! 保留策略：只删「已导出且超过 90 天」的 changes 与「已解决超过 30 天」的冲突。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::maintenance::{prune, CONFLICTS_RETENTION_MS, CHANGES_RETENTION_MS};
use path_editor_core::db::Db;
use rusqlite::params;
use tempfile::TempDir;

#[test]
fn prune_deletes_only_expired_and_exported_rows() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("prune.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let now: i64 = 1_800_000_000_000;
    let old = now - CHANGES_RETENTION_MS - 1;
    let recent = now - 1_000;

    db.with_transaction(|tx| {
        let insert = |uid: &str, applied: i64, exported: Option<i64>| {
            tx.execute(
                "INSERT INTO changes
                   (uid, device_id, seq, lamport, origin, entity_type, entity_uid,
                    field, old_value, new_value, created_at, applied_at, exported_at)
                 VALUES (?1, 'd', ?2, ?2, 'remote', 'snapshot', 's-1', NULL, NULL, NULL, ?2, ?2, ?3)",
                params![uid, applied, exported],
            )
            .unwrap();
        };
        insert("old-exported", old, Some(old));
        insert("old-pending", old, None);
        insert("recent-exported", recent, Some(recent));

        tx.execute(
            "INSERT INTO sync_conflicts
               (entity_type, entity_uid, field, local_value, remote_value, remote_device,
                created_at, resolved_at, resolution)
             VALUES ('snapshot', 's-1', NULL, NULL, NULL, 'd', ?1, ?2, 'local')",
            params![old, Some(now - CONFLICTS_RETENTION_MS - 1)],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO sync_conflicts
               (entity_type, entity_uid, field, local_value, remote_value, remote_device,
                created_at, resolved_at, resolution)
             VALUES ('snapshot', 's-1', NULL, NULL, NULL, 'd', ?1, NULL, NULL)",
            params![old],
        )
        .unwrap();
        Ok::<_, path_editor_core::CoreError>(())
    })
    .unwrap();

    let report = prune(&mut db, now).unwrap();
    assert_eq!(report.changes_deleted, 1);
    assert_eq!(report.conflicts_deleted, 1);

    let remaining: i64 = db
        .query_row_with("SELECT COUNT(*) FROM changes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(remaining, 2, "待导出与未到期记录必须保留");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path_editor_core --test db_maintenance_prune`
Expected: 编译失败（`maintenance` 模块不存在）。

- [ ] **Step 3: 实现 `maintenance.rs`**

```rust
//! 维护任务：保留策略清理（spec §4.2 第 10 条、§6.5.2）。

use super::{db_err, Db};
use crate::error::CoreError;

/// 已导出 `changes` 的保留期：90 天。
pub const CHANGES_RETENTION_MS: i64 = 90 * 24 * 60 * 60 * 1000;
/// 已解决 `sync_conflicts` 的保留期：30 天。
pub const CONFLICTS_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// 清理结果。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PruneReport {
    /// 删除的 changes 行数
    pub changes_deleted: u32,
    /// 删除的 sync_conflicts 行数
    pub conflicts_deleted: u32,
}

const CHANGES_PREDICATE: &str =
    "exported_at IS NOT NULL AND applied_at > 0 AND applied_at < ?1";
const CONFLICTS_PREDICATE: &str = "resolved_at IS NOT NULL AND resolved_at < ?1";

/// 执行清理。
///
/// **绝不删除 `exported_at IS NULL`**：那是待导出队列，删了就永久丢变更。
pub fn prune(db: &mut Db, now_ms: i64) -> Result<PruneReport, CoreError> {
    db.with_transaction(|tx| {
        let changes_deleted = tx
            .execute(
                &format!("DELETE FROM changes WHERE {CHANGES_PREDICATE}"),
                rusqlite::params![now_ms - CHANGES_RETENTION_MS],
            )
            .map_err(|e| db_err("prune_changes", e))?;
        let conflicts_deleted = tx
            .execute(
                &format!("DELETE FROM sync_conflicts WHERE {CONFLICTS_PREDICATE}"),
                rusqlite::params![now_ms - CONFLICTS_RETENTION_MS],
            )
            .map_err(|e| db_err("prune_conflicts", e))?;
        Ok(PruneReport {
            changes_deleted: changes_deleted as u32,
            conflicts_deleted: conflicts_deleted as u32,
        })
    })
}

/// dry-run：只统计将被删除的行数，不写库。
pub fn prune_plan(db: &Db, now_ms: i64) -> Result<PruneReport, CoreError> {
    let changes_deleted: i64 = db
        .query_row_with(
            &format!("SELECT COUNT(*) FROM changes WHERE {CHANGES_PREDICATE}"),
            rusqlite::params![now_ms - CHANGES_RETENTION_MS],
            |r| r.get(0),
        )?;
    let conflicts_deleted: i64 = db
        .query_row_with(
            &format!("SELECT COUNT(*) FROM sync_conflicts WHERE {CONFLICTS_PREDICATE}"),
            rusqlite::params![now_ms - CONFLICTS_RETENTION_MS],
            |r| r.get(0),
        )?;
    Ok(PruneReport {
        changes_deleted: changes_deleted as u32,
        conflicts_deleted: conflicts_deleted as u32,
    })
}
/// 以当前时间执行清理（GUI 启动序列用）。
///
/// GUI crate 没有直接依赖 `chrono`，时间源收在 core 内。
pub fn prune_now(db: &mut Db) -> Result<PruneReport, CoreError> {
    prune(db, chrono::Utc::now().timestamp_millis())
}
```

`core/src/db/mod.rs` 加 `pub mod maintenance;`。

- [ ] **Step 4: 接进 GUI 启动序列**

`gui/src/lib.rs` 的 `setup_database`，在备份索引（④）之后：

```rust
    // ④b 保留策略清理（spec §6.5.2）：失败只告警，不阻断启动。
    match path_editor_core::db::maintenance::prune_now(db) {
        Ok(r) if r.changes_deleted > 0 || r.conflicts_deleted > 0 => log::info!(
            "保留策略清理：changes {} 条，冲突 {} 条",
            r.changes_deleted,
            r.conflicts_deleted
        ),
        Ok(_) => {}
        Err(e) => log::warn!("保留策略清理失败（继续启动）: {}", e.message),
    }
```

- [ ] **Step 5: CLI 命令**

`cli/src/main.rs` 的 `Command` 加：

```rust
    /// 维护任务（清理、归档）
    #[command(subcommand)]
    Maintenance(MaintenanceCmd),
```

```rust
#[derive(Subcommand)]
enum MaintenanceCmd {
    /// 清理过期的变更与冲突（90 / 30 天）
    Prune {
        /// 只统计，不删除
        #[arg(long)]
        dry_run: bool,
    },
}
```

`cli/src/maintenance_ops.rs`：

```rust
//! `maintenance` 子命令。

use crate::runtime::exit_core_error;
use path_editor_core as core;
use path_editor_core::db::maintenance::{prune, prune_plan};

fn open_db() -> core::db::Db {
    core::db::open_and_migrate().unwrap_or_else(|e| exit_core_error(&e))
}

/// `maintenance prune [--dry-run]`
pub(crate) fn maintenance_prune(dry_run: bool) {
    let mut db = open_db();
    let now = chrono::Utc::now().timestamp_millis();
    let report = if dry_run {
        prune_plan(&db, now).unwrap_or_else(|e| exit_core_error(&e))
    } else {
        prune(&mut db, now).unwrap_or_else(|e| exit_core_error(&e))
    };
    println!(
        "{}将删除 changes {} 条，冲突 {} 条",
        if dry_run { "（dry-run）" } else { "" },
        report.changes_deleted,
        report.conflicts_deleted
    );
}
```

`main.rs` 分派：

```rust
        Command::Maintenance(MaintenanceCmd::Prune { dry_run }) => {
            maintenance_ops::maintenance_prune(dry_run)
        }
```

并在 `mod` 列表加 `mod maintenance_ops;`。

- [ ] **Step 6: 跑测试与 CLI 冒烟**

```powershell
cargo test -p path_editor_core --test db_maintenance_prune
cargo run -p patheditor-cli -- maintenance prune --dry-run
```

Expected: 测试 PASS；CLI 输出 dry-run 计数（不写库）。

- [ ] **Step 7: 质量门 + 提交**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add core/src/db/maintenance.rs core/src/db/mod.rs core/tests/db_maintenance_prune.rs gui/src/lib.rs cli/src/main.rs cli/src/maintenance_ops.rs
git commit -m "feat(v6): 保留策略清理（changes 90 天 / 冲突 30 天）"
```

---

### Task 7: `.v5.bak` 归档

**Files:**

- Create: `core/src/db/archive_v5.rs`
- Modify: `core/src/db/mod.rs`
- Modify: `gui/src/lib.rs`（迁移成功且无失败后调用）
- Modify: `cli/src/main.rs`、`cli/src/maintenance_ops.rs`
- Test: `core/tests/db_archive_v5.rs`（新建）

**Interfaces:**

- Produces: `archive_v5::archive_v5_sources(home: &Path, dry_run: bool) -> Result<ArchiveReport, CoreError>`
- `ArchiveReport { archived: Vec<PathBuf>, skipped: Vec<(PathBuf, String)> }`

- [ ] **Step 1: 写失败测试**

创建 `core/tests/db_archive_v5.rs`：

```rust
//! `.v5.bak` 归档：只改名、幂等、不覆盖已有备份。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::archive_v5::archive_v5_sources;
use tempfile::TempDir;

#[test]
fn archives_v5_sources_once_and_never_overwrites() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    std::fs::create_dir_all(home.join("profiles")).unwrap();
    std::fs::write(home.join("disabled.json"), "{}").unwrap();
    std::fs::write(home.join("profiles").join("Work.json"), "{}").unwrap();
    // 已存在的备份：必须跳过
    std::fs::write(home.join("disabled.json.v5.bak"), "old").unwrap();

    let dry = archive_v5_sources(home, true).unwrap();
    assert_eq!(dry.archived.len(), 1, "dry-run 只报告 profiles/Work.json");
    assert!(home.join("profiles").join("Work.json").exists(), "dry-run 不得改名");

    let real = archive_v5_sources(home, false).unwrap();
    assert_eq!(real.archived.len(), 1);
    assert_eq!(real.skipped.len(), 1, "disabled.json 因目标已存在而跳过");

    assert!(home.join("profiles").join("Work.json.v5.bak").exists());
    assert!(!home.join("profiles").join("Work.json").exists());
    assert_eq!(
        std::fs::read_to_string(home.join("disabled.json.v5.bak")).unwrap(),
        "old",
        "已有备份不得被覆盖"
    );

    let again = archive_v5_sources(home, false).unwrap();
    assert!(again.archived.is_empty(), "第二次运行无候选");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path_editor_core --test db_archive_v5`
Expected: 编译失败（模块不存在）。

- [ ] **Step 3: 实现 `archive_v5.rs`**

```rust
//! v5 原件归档（spec §7.4 / §6.5.2）。
//!
//! 把 `disabled.json` 与 `profiles/*.json` 重命名为 `<原名>.v5.bak`。
//! **只改名，不删除**；目标已存在时跳过并记录，绝不覆盖。

use crate::error::{CoreError, ErrorCode};
use std::path::{Path, PathBuf};

/// 归档结果。
#[derive(Debug, Default)]
pub struct ArchiveReport {
    /// 成功改名的目标路径（dry-run 时是「将要改名」的目标）
    pub archived: Vec<PathBuf>,
    /// 跳过项与原因
    pub skipped: Vec<(PathBuf, String)>,
}

/// 归档 v5 原件。
///
/// `home` 由调用方显式传入（`config_dir()` 不可被环境变量重定向，测试必须用 TempDir）。
pub fn archive_v5_sources(home: &Path, dry_run: bool) -> Result<ArchiveReport, CoreError> {
    let mut report = ArchiveReport::default();
    let mut candidates = Vec::new();

    let disabled = home.join("disabled.json");
    if disabled.is_file() {
        candidates.push(disabled);
    }
    let profiles = home.join("profiles");
    if profiles.is_dir() {
        let entries = std::fs::read_dir(&profiles).map_err(|e| {
            CoreError::new(
                ErrorCode::Io,
                "archive_v5",
                format!("读取 {} 失败: {e}", profiles.display()),
            )
        })?;
        for entry in entries {
            let path = entry
                .map_err(|e| {
                    CoreError::new(ErrorCode::Io, "archive_v5", format!("读取目录项失败: {e}"))
                })?
                .path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                candidates.push(path);
            }
        }
    }

    for src in candidates {
        let mut name = src
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default();
        name.push(".v5.bak");
        let dst = src.with_file_name(name);

        if dst.exists() {
            report
                .skipped
                .push((src, format!("目标已存在: {}", dst.display())));
            continue;
        }
        if !dry_run {
            std::fs::rename(&src, &dst).map_err(|e| {
                CoreError::new(
                    ErrorCode::Io,
                    "archive_v5",
                    format!("归档 {} 失败: {e}", src.display()),
                )
            })?;
        }
        report.archived.push(dst);
    }
    Ok(report)
}
```

`core/src/db/mod.rs` 加 `pub mod archive_v5;`。

- [ ] **Step 4: 接进 GUI 启动序列（仅在迁移零失败时）**

`gui/src/lib.rs` 的迁移分支改为记录成功状态：

```rust
    let migration_clean = match path_editor_core::db::migrate_v5::migrate_legacy(db, &home) {
        Ok(report) => {
            if report.imported > 0 || report.failed > 0 {
                log::info!(
                    "v5 文件迁移完成：导入 {}，跳过 {}，失败 {}",
                    report.imported,
                    report.skipped,
                    report.failed
                );
            }
            report.failed == 0
        }
        Err(e) => {
            log::warn!("v5 文件迁移失败（继续启动）: {}", e.message);
            false
        }
    };
```

在 `ensure_current_snapshot_baseline(db);` 之后：

```rust
    // ③c v5 原件归档（spec §7.4）：只在迁移零失败时执行；失败只告警。
    if migration_clean {
        match path_editor_core::db::archive_v5::archive_v5_sources(&home, false) {
            Ok(r) if !r.archived.is_empty() => {
                log::info!("v5 原件归档：{} 个文件", r.archived.len())
            }
            Ok(_) => {}
            Err(e) => log::warn!("v5 原件归档失败（继续启动）: {}", e.message),
        }
    }
```

- [ ] **Step 5: CLI 命令**

`MaintenanceCmd` 增加：

```rust
    /// 把 v5 原件归档为 .v5.bak（只改名，不删除）
    ArchiveV5 {
        /// 只显示将归档的文件，不改名
        #[arg(long)]
        dry_run: bool,
    },
```

`maintenance_ops.rs`：

```rust
use path_editor_core::db::archive_v5::archive_v5_sources;

/// `maintenance archive-v5 [--dry-run]`
pub(crate) fn maintenance_archive_v5(dry_run: bool) {
    let home = core::db::crypto::config_dir();
    let report = archive_v5_sources(&home, dry_run).unwrap_or_else(|e| exit_core_error(&e));
    for path in &report.archived {
        println!("{} {}", if dry_run { "将归档" } else { "已归档" }, path.display());
    }
    for (path, why) in &report.skipped {
        eprintln!("跳过 {}：{why}", path.display());
    }
    println!(
        "{}共 {} 个，跳过 {} 个",
        if dry_run { "（dry-run）" } else { "" },
        report.archived.len(),
        report.skipped.len()
    );
}
```

分派：

```rust
        Command::Maintenance(MaintenanceCmd::ArchiveV5 { dry_run }) => {
            maintenance_ops::maintenance_archive_v5(dry_run)
        }
```

- [ ] **Step 6: 跑测试与 CLI 冒烟**

```powershell
cargo test -p path_editor_core --test db_archive_v5
cargo run -p patheditor-cli -- maintenance archive-v5 --dry-run
```

Expected: 测试 PASS；CLI dry-run 列出候选（本机若已归档则 0 个）。

- [ ] **Step 7: 质量门 + 提交**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add core/src/db/archive_v5.rs core/src/db/mod.rs core/tests/db_archive_v5.rs gui/src/lib.rs cli/src/main.rs cli/src/maintenance_ops.rs
git commit -m "feat(v6): v5 原件归档为 .v5.bak"
```

---

### Task 8: 遗留明文备份清理（转 `.pebak` 后删除）

**Files:**

- Modify: `core/src/db/maintenance.rs`
- Modify: `cli/src/main.rs`、`cli/src/maintenance_ops.rs`
- Test: `core/src/db/maintenance.rs` 内 `#[cfg(test)]`（1 个测试；DPAPI，不建 SQLCipher）

**Interfaces:**

- Produces: `maintenance::cleanup_legacy_env_backups(dir, material, dry_run) -> Result<CleanupReport, CoreError>`
- `CleanupReport { converted: Vec<(PathBuf, PathBuf)>, skipped: Vec<(PathBuf, String)>, dry_run: bool }`

- [ ] **Step 1: 写失败测试**

在 `core/src/db/maintenance.rs` 末尾加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::{EnvBackupHives, EnvBackupPayload, EnvBackupVar};
    use crate::db::crypto::load_or_create_key;
    use crate::env_var::EnvValueKind;
    use crate::persist::{Versioned, PERSIST_SCHEMA_VERSION};

    #[test]
    fn legacy_json_is_converted_to_pebak_before_deletion() {
        let dir = tempfile::TempDir::new().unwrap();
        let material = load_or_create_key(dir.path()).unwrap();

        let payload = EnvBackupPayload {
            captured_at: 1_700_000_000_000,
            hives: EnvBackupHives {
                system: vec![EnvBackupVar {
                    name: "TOKEN".to_string(),
                    kind: EnvValueKind::String,
                    value: "secret".to_string(),
                    revision: "0123456789abcdef".to_string(),
                }],
                user: vec![],
            },
        };
        let legacy = dir.path().join("env_backup_20260101_000000_000.json");
        std::fs::write(
            &legacy,
            serde_json::to_string(&Versioned {
                schema_version: PERSIST_SCHEMA_VERSION,
                inner: payload.clone(),
            })
            .unwrap(),
        )
        .unwrap();

        // dry-run 不得写文件、不得删除
        let plan = cleanup_legacy_env_backups(dir.path(), &material, true).unwrap();
        assert_eq!(plan.converted.len(), 1);
        assert!(legacy.exists(), "dry-run 不得删除");

        let report = cleanup_legacy_env_backups(dir.path(), &material, false).unwrap();
        assert_eq!(report.converted.len(), 1);
        assert!(!legacy.exists(), "转换成功后删除明文");

        let (_, pebak) = &report.converted[0];
        let read_back = crate::db::backups::read_env_backup_any(pebak, &material).unwrap();
        assert_eq!(read_back, payload, ".pebak 必须可回读");
    }
}
```

> 说明：此测试用 DPAPI、不开 SQLCipher，因此可以放在单元测试模块里；该文件仍只有这一个测试。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path_editor_core db::maintenance::tests`
Expected: 编译失败（`cleanup_legacy_env_backups` 不存在）。

- [ ] **Step 3: 实现清理函数**

在 `maintenance.rs` 增加：

```rust
use crate::backup::EnvBackupPayload;
use crate::db::crypto::KeyMaterial;
use std::path::{Path, PathBuf};

/// 遗留明文备份清理结果。
#[derive(Debug, Default)]
pub struct CleanupReport {
    /// 已转换（dry-run 时是「将转换」）的 `(json, pebak)` 对
    pub converted: Vec<(PathBuf, PathBuf)>,
    /// 跳过项与原因
    pub skipped: Vec<(PathBuf, String)>,
    /// 是否为 dry-run
    pub dry_run: bool,
}

/// 把遗留 `env_backup_*.json` 转成 `.pebak` 后删除明文。
///
/// **顺序不可颠倒**：先写 `.pebak` 并回读校验，成功后才删除 `.json`。
/// 任一失败都保留原文件——这是 spec §5.3 / §十二 的「用户确认 + 回滚副本」。
///
/// 解析**不使用** `backup::read_env_backup`：后者在解析失败时会经
/// `persist::parse_error_quarantined` 重命名文件，清理命令不能在 dry-run 时
/// 改动用户文件。这里直接反序列化 + [`crate::persist::migrate`]。
pub fn cleanup_legacy_env_backups(
    dir: &Path,
    material: &KeyMaterial,
    dry_run: bool,
) -> Result<CleanupReport, CoreError> {
    let mut report = CleanupReport {
        dry_run,
        ..Default::default()
    };
    if !dir.is_dir() {
        return Ok(report);
    }

    let entries = std::fs::read_dir(dir).map_err(|e| {
        CoreError::new(
            crate::error::ErrorCode::Io,
            "cleanup_legacy",
            format!("读取 {} 失败: {e}", dir.display()),
        )
    })?;

    for entry in entries {
        let path = entry
            .map_err(|e| {
                CoreError::new(
                    crate::error::ErrorCode::Io,
                    "cleanup_legacy",
                    format!("读取目录项失败: {e}"),
                )
            })?
            .path();
        if !is_legacy_env_backup(&path) {
            continue;
        }

        let payload = match read_legacy_backup_no_quarantine(&path) {
            Ok(p) => p,
            Err(e) => {
                report.skipped.push((path, format!("无法解析: {}", e.message)));
                continue;
            }
        };

        if dry_run {
            report.converted.push((path, PathBuf::new()));
            continue;
        }

        let pebak = match crate::db::backups::write_env_backup_pebak(dir, &payload, material) {
            Ok(p) => p,
            Err(e) => {
                report.skipped.push((path, format!("写加密备份失败: {}", e.message)));
                continue;
            }
        };
        if let Err(e) = crate::db::backups::read_env_backup_any(&pebak, material) {
            report.skipped.push((path, format!("加密备份回读失败: {}", e.message)));
            continue;
        }
        if let Err(e) = std::fs::remove_file(&path) {
            report.skipped.push((path, format!("删除明文失败: {e}")));
            continue;
        }
        report.converted.push((path, pebak));
    }
    Ok(report)
}

/// 文件名是否形如本工具生成的 `env_backup_YYYYMMDD_HHMMSS_mmm.json`。
///
/// **不能只匹配 `env_backup_*.json`**：那个前缀可能命中用户自己放的文件，
/// 清理命令不得碰它们（与 `backup.rs` 轮换的判定口径一致）。
fn is_legacy_env_backup(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Some(stem) = name
        .strip_prefix("env_backup_")
        .and_then(|s| s.strip_suffix(".json"))
    else {
        return false;
    };
    let parts: Vec<&str> = stem.split('_').collect();
    parts.len() == 3
        && parts[0].len() == 8
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].len() == 6
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[2].len() == 3
        && parts[2].chars().all(|c| c.is_ascii_digit())
}

/// 不做损坏隔离的遗留备份解析。
fn read_legacy_backup_no_quarantine(path: &Path) -> Result<EnvBackupPayload, CoreError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        CoreError::new(
            crate::error::ErrorCode::Io,
            "cleanup_legacy",
            format!("读取 {} 失败: {e}", path.display()),
        )
    })?;
    let versioned: crate::persist::Versioned<EnvBackupPayload> =
        serde_json::from_str(&content).map_err(|e| {
            CoreError::new(
                crate::error::ErrorCode::Parse,
                "cleanup_legacy",
                format!("解析 {} 失败: {e}", path.display()),
            )
        })?;
    crate::persist::migrate(versioned, "env 备份")
}
```

- [ ] **Step 4: CLI 命令**

`MaintenanceCmd` 增加：

```rust
    /// 把遗留 env_backup_*.json 转为 .pebak 后删除（必须先 --yes）
    CleanupLegacyBackups {
        /// 只显示计划，不改动任何文件
        #[arg(long)]
        dry_run: bool,
        /// 确认执行
        #[arg(long)]
        yes: bool,
    },
```

`maintenance_ops.rs`：

```rust
use path_editor_core::db::maintenance::cleanup_legacy_env_backups;

/// `maintenance cleanup-legacy-backups [--dry-run] [--yes]`
pub(crate) fn maintenance_cleanup_legacy(dry_run: bool, yes: bool) {
    let dir = core::backup::env_backup_dir();
    let material = core::db::crypto::load_or_create_key(&core::db::crypto::config_dir())
        .unwrap_or_else(|e| exit_core_error(&e));

    if !dry_run && !yes {
        eprintln!("错误: 这会删除明文备份。先运行 --dry-run 查看计划，确认后加 --yes。");
        std::process::exit(1);
    }
    let report = cleanup_legacy_env_backups(&dir, &material, dry_run)
        .unwrap_or_else(|e| exit_core_error(&e));
    for (json, pebak) in &report.converted {
        if dry_run {
            println!("将转换 {}", json.display());
        } else {
            println!("已转换 {} -> {}", json.display(), pebak.display());
        }
    }
    for (path, why) in &report.skipped {
        eprintln!("跳过 {}：{why}", path.display());
    }
    println!(
        "{}共 {} 个，跳过 {} 个",
        if dry_run { "（dry-run）" } else { "" },
        report.converted.len(),
        report.skipped.len()
    );
}
```

分派：

```rust
        Command::Maintenance(MaintenanceCmd::CleanupLegacyBackups { dry_run, yes }) => {
            maintenance_ops::maintenance_cleanup_legacy(dry_run, yes)
        }
```

- [ ] **Step 5: 跑测试与 CLI 冒烟**

```powershell
cargo test -p path_editor_core db::maintenance::tests
cargo run -p patheditor-cli -- maintenance cleanup-legacy-backups --dry-run
cargo run -p patheditor-cli -- maintenance cleanup-legacy-backups
```

Expected: 测试 PASS；第二条命令（无 `--yes`）以 1 退出并提示。

- [ ] **Step 6: 质量门 + 提交**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add core/src/db/maintenance.rs cli/src/main.rs cli/src/maintenance_ops.rs
git commit -m "feat(v6): 遗留明文 env 备份先转 .pebak 再删除"
```

---

### Task 9: `import_profile_v5`

**Files:**

- Create: `core/src/db/v5_import.rs`
- Modify: `core/src/db/mod.rs`
- Modify: `cli/src/main.rs`、`cli/src/profile_ops.rs`
- Test: `core/tests/db_v5_import.rs`（新建）

**Interfaces:**

- Produces: `v5_import::import_profile_v5(db: &mut Db, path: &Path) -> Result<String, CoreError>`（返回新配置 uid）

- [ ] **Step 1: 写失败测试**

创建 `core/tests/db_v5_import.rs`：

```rust
//! v5 配置导入：把 `profiles/*.json` 读回 DB，并保留 PATH 条目。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::v5_import::import_profile_v5;
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn imports_a_v5_profile_json_into_the_database() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("v5import.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let file = dir.path().join("Work.json");
    std::fs::write(
        &file,
        r#"{
          "schemaVersion": 1,
          "name": "Work",
          "sys": [{"path": "C:\\A", "enabled": true}],
          "user": [{"path": "D:\\B", "enabled": false}],
          "created": "2026-01-01T00:00:00",
          "modified": "2026-01-02T00:00:00"
        }"#,
    )
    .unwrap();

    let uid = import_profile_v5(&mut db, &file).unwrap();
    assert!(!uid.is_empty());

    let name: String = db
        .query_row_with(
            "SELECT name FROM profiles WHERE uid = ?1",
            rusqlite::params![uid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(name, "Work");

    let entries: i64 = db
        .query_row_with(
            "SELECT COUNT(*) FROM path_entries pe
               JOIN profile_revisions r ON r.snapshot_id = pe.snapshot_id
              WHERE r.profile_id = (SELECT id FROM profiles WHERE uid = ?1)",
            rusqlite::params![uid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(entries, 2, "两个 hive 的条目都必须落库");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path_editor_core --test db_v5_import`
Expected: 编译失败（模块不存在）。

- [ ] **Step 3: 实现 `v5_import.rs`**

```rust
//! v5 配置导入（M1 欠账 P2-2 的导入侧）：`profiles/*.json` → DB。
//!
//! 读取复用 [`crate::profiles::load_profile_from`]——它是 v5 侧唯一的解析入口，
//! 带 `schemaVersion` 信封与损坏隔离语义，不在这里复制。

use super::{profiles, Db};
use crate::db::history::{NewSnapshot, SnapshotKind};
use crate::error::CoreError;
use crate::history::EventSource;
use std::path::Path;

/// 把一个 v5 配置文件导入 DB，返回新配置的 uid。
///
/// 名称冲突返回 `NameExists`（与 `save_profile` 同口径），由用户先改名再导入。
/// v5 的 `created` / `modified` 是展示字段，不写回 DB（导入时间即创建时间）。
pub fn import_profile_v5(db: &mut Db, path: &Path) -> Result<String, CoreError> {
    let data = crate::profiles::load_profile_from(path)?;
    let snap = NewSnapshot {
        kind: SnapshotKind::Current,
        source: EventSource::Import,
        system: data.sys,
        user: data.user,
        env: vec![],
        parent_uid: None,
    };
    profiles::save_profile(db, &data.name, None, &[], snap)
}
```

`core/src/db/mod.rs` 加 `pub mod v5_import;`。

- [ ] **Step 4: CLI 命令**

`ProfileCmd` 增加：

```rust
    /// 从 v5 `profiles/*.json` 导入配置
    Import { file: String },
```

`profile_ops.rs`：

```rust
use path_editor_core::db::v5_import::import_profile_v5;

pub(crate) fn profile_import(file: String) {
    let mut db = open_db();
    let uid = import_profile_v5(&mut db, std::path::Path::new(&file))
        .unwrap_or_else(|e| exit_core_error(&e));
    let view = db_profiles::get_profile(&db, &uid).unwrap_or_else(|e| exit_core_error(&e));
    match view {
        Some(v) => println!("已导入配置「{}」（{}）", v.name, v.uid),
        None => println!("已导入配置（{uid}）"),
    }
}
```

分派：

```rust
        ProfileCmd::Import { file } => profile_ops::profile_import(file),
```

- [ ] **Step 5: 跑测试与 CLI 冒烟**

```powershell
cargo test -p path_editor_core --test db_v5_import
```

Expected: PASS。

- [ ] **Step 6: 质量门 + 提交**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add core/src/db/v5_import.rs core/src/db/mod.rs core/tests/db_v5_import.rs cli/src/main.rs cli/src/profile_ops.rs
git commit -m "feat(v6): 支持从 v5 profiles/*.json 导入配置"
```

---

### Task 10: 文档与 CHANGELOG

**Files:**

- Modify: `README.md`（CLI 命令列表、v6 特性、同步范围、清理命令）
- Modify: `CHANGELOG.md`（顶部写 `## 6.0.0 (2026-10-21)` 段落）
- Modify: `docs/审核和开发/2026.10.21/PathEditor-v6-M4-开发回执.md`（交付时写）

- [ ] **Step 1: README**

在 CLI 命令表补：

```text
patheditor maintenance prune [--dry-run]
patheditor maintenance archive-v5 [--dry-run]
patheditor maintenance cleanup-legacy-backups [--dry-run] [--yes]
patheditor profile import <FILE>
```

同步章节补一句：

> 配置（`profile` / `profile_revision`）现在会随同步包落库；与本地同名的配置会
> 自动改名为「原名 (来自 设备)」，导入提示会显示改名数量。删除的配置以墓碑同步。

- [ ] **Step 2: CHANGELOG**

在 `CHANGELOG.md` 顶部加：

```markdown
## 6.0.0 (2026-10-21)

### 新增

- SQLCipher 加密数据库：历史时间线、配置版本、备份索引、脱机检索。
- 文件式跨设备同步：加密 bundle、字段级合并、冲突 UI；配置与快照均可同步。
- 维护命令：`maintenance prune`、`archive-v5`、`cleanup-legacy-backups`。
- 从 v5 `profiles/*.json` 导入配置（`profile import`）。

### 变更

- v5 文件停写，原件归档为 `.v5.bak`。
- 新 env 备份为 DPAPI 加密的 `.pebak`；遗留 `.json` 转换后清理（需 `--yes`）。
- 配置同名冲突自动改名，不再静默跳过。

### 修复

- 同步导入不再把「未落库的变更」计入成功。
- 墓碑与内容变更使用实体级 LWW，旧内容不会让已删除配置复活。
```

- [ ] **Step 3: 检查文档一致性**

```powershell
npx prettier --check README.md CHANGELOG.md
```

Expected: 通过。

- [ ] **Step 4: 提交**

```powershell
git add README.md CHANGELOG.md
git commit -m "docs(v6): M4 功能与 6.0.0 发布日志"
```

---

### Task 11: 版本号 6.0.0 与验收

**Files:**

- Modify: `package.json`、`package-lock.json`、`Cargo.toml`、`Cargo.lock`、`gui/tauri.conf.json`、`README.md`

- [ ] **Step 1: 升版本号**

```powershell
# package.json 的 version 改为 6.0.0
npm install --package-lock-only
# Cargo.toml 的 [workspace.package] version 改为 6.0.0
cargo update -p path-editor-core -p patheditor -p patheditor-cli
```

`gui/tauri.conf.json` 的 `version` 改为 `6.0.0`；窗口 `title` 若含版本号一并更新。
`README.md` 的版本徽章/安装包说明改为 6.0.0。

- [ ] **Step 2: 确认四处版本一致**

```powershell
node -e "console.log(require('./package.json').version)"
Select-String -Path Cargo.toml -Pattern '^version' | Select-Object -First 3
node -e "console.log(require('./gui/tauri.conf.json').version)"
Select-String -Path README.md -Pattern '5\.1\.7|6\.0\.0'
```

Expected: 全部为 6.0.0（README 中不应再有 5.1.7 的当前版本描述）。

- [ ] **Step 3: 全量质量门**

```powershell
npm run verify:all
```

Expected: exit 0。记录 Rust binaries / passed、前端 files / tests、E2E passed 三个数字。

- [ ] **Step 4: 验收清单核对**

逐条对照 spec §十二，把结果写进开发回执：

- [ ] 配置同步：两库含 profile 的包 → profile + revision 落库、同名改名、墓碑生效、重复导入幂等
- [ ] 保留策略：90/30 天清理只删到期且已导出的记录
- [ ] `.v5.bak` 归档幂等、不覆盖
- [ ] 遗留 `.json` 转换成功才删除
- [ ] `profile import` 可用
- [ ] `verify:all` exit 0，覆盖率 ≥80%
- [ ] README/CHANGELOG 更新
- [ ] `CLAUDE.md` / `AGENTS.md` 字节级一致：**当前 `CLAUDE.md` 自述「不要求逐字节一致」，
      与 spec §十二 冲突**。本轮**不修改任何 `CLAUDE.md`**；在回执中如实记录该冲突，
      由用户裁断（全局规则：未经明确指令不得修改 CLAUDE.md）。

- [ ] **Step 5: 提交**

```powershell
git add package.json package-lock.json Cargo.toml Cargo.lock gui/tauri.conf.json README.md
git commit -m "chore(v6): 版本号升至 6.0.0"
```

---

## Self-Review

**规格覆盖**

| spec 条目                                        | 对应 Task                |
| ------------------------------------------------ | ------------------------ |
| §6.5.1 可落库类型 / 依赖排序                     | Task 1                   |
| §6.5.1 profile content / uid 保留 / 使用统计本地 | Task 1                   |
| §6.5.1 同名自动改名 + 计数                       | Task 2                   |
| §6.5.1 墓碑 + 实体级 LWW                         | Task 3、Task 4           |
| §6.5.1 幂等                                      | Task 5                   |
| §6.5.2 changes 90 天 / conflicts 30 天           | Task 6                   |
| §6.5.2 `.v5.bak` 归档                            | Task 7                   |
| §6.5.2 遗留 `.json` 转 `.pebak` 后删除           | Task 8                   |
| §6.5.3 `import_profile_v5`                       | Task 9                   |
| §十二 v5 导出/导入可用                           | Task 9（导入）+ 既有导出 |
| §十二 文档 / 版本                                | Task 10、11              |
| §十三 M4 里程碑                                  | 全部                     |

**占位符扫描**：无 TBD / TODO / 「适当处理」；每个代码步骤都有完整代码。

**类型一致性**：`ImportReport` 在 Task 2 增加 `renamed_profiles`，Task 3-5 的测试按最终形状断言；`ProfileSyncContent` 只在 Task 1 定义；`CleanupReport` / `ArchiveReport` 各自独立。

**已知风险**

- 保留策略在 GUI 每次启动执行一次 DELETE；`changes` 表无按 `applied_at` 的索引，数据量大时会全表扫描。M4 接受（数据量小），若日志显示耗时增长再评估加索引（需 schema v4）。
- `setup_database` 的自动归档会在升级后的首次启动重命名用户文件；这是 spec §7.4 的明确要求，且只改名不删除。
- 配置同步的整实体 LWW 可能让「不同设备的改名与内容修改」互相覆盖；spec 已登记为已知限制，不做字段级合并。

## Execution Notes

（由开发窗口回填：偏离记录、未覆盖项、残余风险、实际测试数。）
