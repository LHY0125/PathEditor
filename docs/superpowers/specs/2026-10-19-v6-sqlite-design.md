# PathEditor v6.0.0 SQLite 存储与高级特性 — 设计文档

- **日期**：2026-10-19
- **作者**：审核窗口
- **状态**：待开发窗口核对 → plan
- **前置**：v5.1.7 / Wave 4.1 已合并（`main @ aae0909`）
- **调研来源**：`docs/审核和开发/2026.10.19/PathEditor-v6-SQLite功能调研.md`（含核对轮 G1–G5 裁断）
- **发布边界**：**一个 v6.0.0 发布**；开发按 M0–M4 里程碑推进，每个里程碑独立验证、可回滚

---

## 一、概述

### 1.1 背景

PathEditor 当前把同一类东西——「某一时刻的 PATH / 环境变量快照」——存在四种互不相通的文件里：`disabled.json`（当前快照）、`profiles/*.json`（命名快照）、`env_backup_*.json`（定时快照，含明文敏感值）、`path_backup_*.txt`（定时快照）。这导致：

- 无法按时间/内容/来源查询历史；
- 撤销栈只在内存，关掉即丢；
- 配置没有标签、版本、最近使用；
- 备份列表不解析内容，无法检索/去重；
- GUI 与 CLI 并发写文件无锁；
- 无法跨设备同步。

### 1.2 目标

把 SQLite（SQLCipher 加密）作为 **sidecar / 历史 / 索引**，统一快照模型，并在此之上交付：

- **Tier 1**：历史时间线、跨会话撤销/重做、配置元数据与版本、备份索引；
- **Tier 2**：FTS5 全文检索、统计仪表盘、N 路对比、环境变量引用图、冲突历史；
- **Tier 3**：基于**文件式加密变更包**的跨设备同步。

**注册表仍是唯一真相源**；数据库永远不是第二个真相源。所有对注册表的写入仍走 `save_path_with_sidecar` / `update_env_var` / `delete_env_var` / `restore_env_backup` 既有编排，数据库只记录结果。

### 1.3 完成判定（可机械验证）

```bash
grep -rn "sqlite\|rusqlite" core/Cargo.toml          # 依赖存在且锁定
cargo test -p path-editor-core --lib                 # 新增 DB/加密/同步测试全绿
npm run verify:all                                   # 退出码 0
# 新增验收（示例）
cargo test -p path-editor-core v6_acceptance -- --nocapture
```

---

## 二、已确认的决策

| #   | 决策        | 选择                                                                                                    | 影响                                                                 |
| --- | ----------- | ------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| D1  | 存储引擎    | SQLite（`rusqlite`，core 内）                                                                           | 新增原生依赖；前端不得直连 SQL                                       |
| D2  | 加密        | **SQLCipher 全库加密**                                                                                  | 需密钥管理；构建复杂度上升                                           |
| D3  | 本地密钥    | **DPAPI 保护的密钥文件**（用户作用域）                                                                  | 免输入；绑定 Windows 用户/机器；换机不可解密                         |
| D4  | 范围        | **Tier 1+2+3 全量，单版本 v6.0.0**                                                                      | 按 M0–M4 里程碑推进；每里程碑有验证门                                |
| D5  | 敏感值      | 允许入库（DB 已加密），**历史 UI 默认掩码**；`EnvVarMeta` 仍不含 value，明文只经 `reveal_env_var`       | 历史可回放敏感值，但必须显式 reveal                                  |
| D6  | 事件表      | **禁止存明文值**；只存 `changed_fields` + `before/after_revision` + `snapshot_id`                       | 防止绕过 `reveal_env_var` 的旁路（G2）                               |
| D7  | 备份文件    | **path 备份保留明文文件；env 备份写 DPAPI 加密文件（`.pebak`）**；DB 只做索引                           | env 备份不再新增明文秘密；path 备份仍可手工打开；DB 损坏时备份仍可用 |
| D8  | 配置方案    | **迁入 DB**（标签/版本/最近使用），保留 v5 JSON 导出                                                    | 获得查询能力；导出兜住可移植性                                       |
| D9  | 旧备份清理  | 开发期间保持明文；**开发完成后删除 `env_backup_*.json`**                                                | 有条件删除：导入→校验→用户确认；删除前保留回滚副本                   |
| D10 | 同步形态    | **文件式加密变更包**（用户指定目录）                                                                    | 包用口令派生密钥；本地 DB 仍用 DPAPI；schema 从第一天含同步字段      |
| D11 | 同步密钥    | 用户口令 → **Argon2id** → AEAD（ChaCha20-Poly1305）                                                     | 与本地 DPAPI 密钥分离；口令不落盘                                    |
| D12 | 降级        | v5 已发布，无法识别 DB → **降级不受支持**；v6 提供导出 v5 JSON                                          | 若要拒绝启动，需在 v6 前发 v5.x 守卫（G4）                           |
| D13 | Path 不变式 | `Path` **永不进入 `env_values`**；`Unsupported` 只存元数据                                              | `CHECK (lower(name) <> 'path')` + 采集层排除（G1）                   |
| D14 | 并发        | `BEGIN IMMEDIATE` + `version` CAS；WAL 不代替业务并发控制                                               | lost update 必须由业务层消除（G3）                                   |
| D15 | revision    | 仅作变更/冲突判定；**不是加密**                                                                         | 不宣称「只存哈希 = 零泄漏」（G5）                                    |
| D16 | DB 故障     | **fail-open**：注册表/环境变量编辑照常；历史/同步/统计降级；降级模式写 `disabled.json` 兜底；不静默重建 | sidecar 定位；密钥错误不阻断编辑                                     |
| D17 | 新 env 备份 | **DPAPI 加密文件 `.pebak`**；旧 `.json` 一次性导入/恢复，M4 删除                                        | 不再新增明文敏感文件；跨机不可解密（与 D3 一致）                     |

---

## 三、架构

### 3.1 模块布局

```text
core/src/
├── db/
│   ├── mod.rs          # Db 句柄：open/transaction/PRAGMA
│   ├── schema.rs       # DDL + user_version 迁移
│   ├── crypto.rs       # SQLCipher key、DPAPI、Argon2id+AEAD（bundle）
│   ├── history.rs      # snapshots/events 查询与写入
│   ├── profiles.rs     # 配置元数据/版本（替换/包装现有文件实现）
│   ├── backups.rs      # 备份索引（扫描现有文件）
│   ├── search.rs       # FTS5
│   ├── stats.rs        # 聚合查询
│   ├── refs.rs         # %VAR% 引用图 + 递归 CTE
│   └── sync.rs         # change log / bundle 导出导入 / 合并
├── disabled.rs         # 保留：当前快照的兼容读写（迁移期）
├── persist.rs          # 保留：v5 JSON 读取（迁移期）
└── backup.rs           # 保留：备份文件读写（文件仍是产物）
```

### 3.2 所有权与边界

- `Db` 由 **GUI / CLI 启动时创建**，显式传入服务层；不使用全局单例。
- `Db::open(path, key)` 在 core；密钥由 core 的 `crypto` 模块从 DPAPI 取得。
- 前端**只能**经 `src/services/backend.ts` 新增的 IPC 命令访问；**禁止** `tauri-plugin-sql` / 前端 SQL。
- `gui` / `cli` 只做参数转换与错误呈现；历史/同步/统计规则全在 core。
- 所有写事务：`BEGIN IMMEDIATE`；读-改-写带 `version` CAS；冲突返回 `CoreError.code = conflict`。
- **服务层接缝（H1）**：定义 `HistorySink` trait（`record(&self, event: HistoryEvent) -> Result<(), CoreError>`）。测试/无 DB 场景用 `NoopHistory`，生产用 `DbHistory`；`save_path_with_sidecar` 等调用点增加一个 `history: &dyn HistorySink` 参数，既有测试传 `&NoopHistory`，行为不变。**禁止全局单例**。

### 3.2.1 IPC 边界（前端如何访问 DB）

**选择：细粒度 IPC（方案 A）**。每个查询/写入是一个独立 `#[tauri::command]`，前端经 `backend.ts` 的同名方法调用；**不做**「一次返回整块面板数据」的聚合命令，也**不**让前端拼 SQL。

理由：

- 与现有 20+ 个 IPC 命令的风格一致（`list_profiles` / `scan_paths` / `reveal_env_var` …）；
- 每个命令可独立做**运行时形状校验**与单元测试，符合仓库「不靠 TS 断言」的既有约束；
- 聚合视图是**前端组合**，不是后端职责；后端只保证每个原语正确。

**命名与形状约定**：

| 规则   | 说明                                                                                                                      |
| ------ | ------------------------------------------------------------------------------------------------------------------------- |
| 命令名 | `snake_case`；历史相关一律以 `history_` 前缀（如 `history_list_events`）                                                  |
| 参数   | 单个结构体参数（`{ filter: ... }`），避免位置参数在 Tauri 下的序列化歧义                                                  |
| 返回   | 结构体（`serde(rename_all = "camelCase")`），与 `EnvVarMeta` 等既有契约一致                                               |
| 错误   | `Result<T, CoreError>`；前端 `parseCoreError` 统一处理，`code` 是唯一判定依据                                             |
| 校验   | `backend.ts` 每个方法返回前调用 `parseXxx(await invoke<unknown>(...))`，**禁止**直接 `invoke<T>` 后当 `T` 用              |
| 敏感值 | **任何历史命令默认不返回明文**；需要明文时走独立的 `history_reveal_env_value(uid, hive, name)`，语义对齐 `reveal_env_var` |

**M1 命令清单**（最小集；M2/M3 另加）：

```text
history_list_events(filter)        -> EventPage          # 分页、时间倒序
history_get_snapshot(uid)          -> SnapshotView       # 敏感值掩码
history_restore_snapshot(uid)      -> RestoreOutcome
history_list_snapshots(filter)     -> SnapshotPage
history_get_current()              -> SnapshotView | null
profile_list(filter)               -> ProfileView[]      # 含 tags/lastAppliedAt/applyCount
profile_save(request)              -> ProfileView
profile_apply(uid)                 -> ApplyOutcome
profile_delete(uid)                -> void
profile_export_v5(uid, path)       -> void
backup_list(filter)                -> BackupView[]       # 含 encrypted / variableCount
```

**明细规则**：

1. **分页**：所有列表命令接受 `{ limit, offset }`，默认 50 / 0，上限 500；`EventPage` 含 `total`。
2. **排序**：服务端固定（事件按 `created_at DESC, id DESC`；快照按同口径），**不接受前端传排序字段**——避免 SQL 注入面与语义漂移。
3. **掩码**：`SnapshotView` 中敏感变量 `value` 恒为 `null` 且带 `sensitive: true`；只有 `history_reveal_env_value` 返回明文，且必须在 GUI 里显式点击。
4. **恢复**：`history_restore_snapshot` 不接收「部分字段」；整快照交给既有注册表编排，成功后追加新快照与事件（append-only）。
5. **DB 不可用（B7）**：所有 `history_*` / `profile_*` / `backup_*` 命令在降级模式下返回 `CoreError { code: database | crypto }`；**注册表编辑命令不受影响**。GUI 据此显示降级条幅。
6. **禁止**：`sqlite_*` / `db_*` 之类暴露底层语义的命令名；前端不得出现 SQL 字符串。

**前端调用示例**（`backend.ts` 风格）：

```ts
historyListEvents: async (filter: EventFilter) =>
  parseEventPage(await parseCoreError(invoke<unknown>('history_list_events', { filter }))),
```

**测试要求**：

- 每个 `parseXxx` 有对应的形状测试（缺字段/类型错/未知 code）；
- 每个命令的 core 层有独立测试（一文件一测试，见 M0 §5.3）；
- E2E 只覆盖 3–4 条主路径（历史面板、恢复、配置应用），其余靠单元测试。

### 3.3 数据库位置与 PRAGMA

```text
~/.patheditor/patheditor.db
~/.patheditor/keys/db.key.dpapi     # DPAPI 保护的 32 字节密钥
~/.patheditor/sync/                 # 默认变更包目录（用户可改）
```

```sql
PRAGMA key = "x'<hex>'";
PRAGMA cipher_memory_security = OFF;  -- M0 实测：ON 在受限进程里 VirtualLock 失败；默认 OFF（见下）
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
-- user_version 不在启动序列中写：启动只读；建库/迁移事务内写（B2）
```

**版本流程（B2）**：启动 `PRAGMA user_version` 只读；0 = 新库（建库事务写 1）；小于当前 = 迁移事务内逐级升级；大于当前 = 拒绝启动（文件由更新版本写入）。**启动序列中不得出现写 `user_version` 的语句**。

**`cipher_memory_security`（M0 实测修正）**：M0 在测试进程里观察到 `VirtualLock() ... LastError=1453`（`ERROR_NOT_ENOUGH_MEMORY`），且 ON/OFF 性能无显著差异（GNU 902 µs vs 931 µs；MSVC 1.1715 ms vs 1.1498 ms）。**生产默认值由 M1 决定**：先按 `OFF` 实现并观察 GUI/CLI 运行期是否出现锁内存失败；若稳定则保持 OFF（更快、无锁定失败风险），若安全评审要求 ON 则显式开启并记录。**spec 不把 ON 写成硬性要求。**

启动验收：`PRAGMA cipher_version` 非空；`SELECT count(*) FROM sqlite_master` 可执行；错误密钥必须失败且不产生半开库。

### 3.4 新增依赖（版本以 Cargo 实际锁定为准）

| crate              | 用途                  | 备注                                                                                                                                                                                       |
| ------------------ | --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `rusqlite`         | SQLite / SQLCipher    | `features = ["bundled-sqlcipher-vendored-openssl"]`；M0 验证 Windows MSVC 构建                                                                                                             |
| `argon2`           | bundle KDF            | Argon2id（m=64 MiB, t=3, p=1）                                                                                                                                                             |
| `chacha20poly1305` | bundle AEAD           | 32 B key / 12 B nonce                                                                                                                                                                      |
| `uuid`             | UUIDv7                | `features = ["v7"]`                                                                                                                                                                        |
| `rand`             | 密钥 / nonce          | OS RNG                                                                                                                                                                                     |
| `blake3`           | `content_hash`        | 去重/判等；**非安全边界**（H5）                                                                                                                                                            |
| `windows-sys`      | DPAPI / DACL / 互斥量 | 新增 `Win32_Security_Cryptography` / `Win32_Security` / `Win32_Security_Authorization` / `Win32_Storage_FileSystem` / `Win32_System_Memory` / `Win32_Foundation`，M0 逐一编译验证（H4/M7） |

> **NASM（B1 最终收敛）**：本地 M0 **先做 no-asm 实测**（`openssl-src` 找不到 NASM 会自动 `no-asm`）；构建成功且加密性能可接受则不安装 NASM。若构建失败或性能不可接受，再由用户执行 `scoop install nasm`。**CI 显式安装 NASM**，保证 release 构建可重复且性能一致；本地 no-asm 用于验证降级路径。
>
> **GNU 本机构建（B1b）**：`openssl-src` 在 GNU 路径**硬编码 `Command::new("make")`**（不读 `MAKE` 环境变量）；本机只有 `mingw32-make.exe`，因此 M0 前需用户执行 **`scoop install make`**（main 4.4.1）。CI 是 MSVC + `nmake`，不需要 `make`。
>
> SQLCipher 的 OpenSSL 构建仍是 M0 的硬门；若 `bundled-sqlcipher-vendored-openssl` 在 MSVC 上不可行，回退为 `bundled-sqlcipher` + 系统 OpenSSL（vcpkg），并在 spec/plan 记录实际选择。

---

## 四、数据模型

### 4.1 表

```sql
CREATE TABLE snapshots (
  id           INTEGER PRIMARY KEY,
  uid          TEXT NOT NULL UNIQUE,          -- UUIDv7（同步身份）
  created_at   INTEGER NOT NULL,
  source       TEXT NOT NULL,                 -- gui / cli / import / restore / migration / sync
  kind         TEXT NOT NULL,                 -- current / profile / backup / history
  reason       TEXT,
  content_hash TEXT NOT NULL,
  parent_id    INTEGER REFERENCES snapshots(id),
  note         TEXT,
  updated_at   INTEGER NOT NULL,
  deleted_at   INTEGER                        -- 墓碑（同步）
);
CREATE INDEX idx_snapshots_time ON snapshots(created_at);
CREATE INDEX idx_snapshots_hash ON snapshots(content_hash);

CREATE TABLE app_state (
  id                  INTEGER PRIMARY KEY CHECK (id = 1),
  current_snapshot_id INTEGER REFERENCES snapshots(id),
  version             INTEGER NOT NULL DEFAULT 1,
  migrated_v5         INTEGER NOT NULL DEFAULT 0,
  device_id           TEXT NOT NULL,          -- UUIDv7，首次启动生成
  lamport             INTEGER NOT NULL DEFAULT 0
);

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
  value       TEXT,                           -- DB 已加密；Unsupported 为 NULL
  revision    TEXT NOT NULL,
  sensitive   INTEGER NOT NULL DEFAULT 0,
  restorable  INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (snapshot_id, hive, name),
  CHECK (lower(name) <> 'path')               -- D13
);

CREATE TABLE events (
  id              INTEGER PRIMARY KEY,
  uid             TEXT NOT NULL UNIQUE,       -- UUIDv7
  created_at      INTEGER NOT NULL,
  source          TEXT NOT NULL,              -- gui / cli / sync
  action          TEXT NOT NULL,              -- save/apply/restore/import/env_set/undo/redo/sync
  hive            TEXT,
  target          TEXT,
  changed_fields  TEXT,                       -- JSON 数组；禁止明文值（D6）
  before_revision TEXT,
  after_revision  TEXT,
  result          TEXT NOT NULL,              -- success/partial/failure
  error_code      TEXT,
  snapshot_id     INTEGER REFERENCES snapshots(id),
  device_id       TEXT NOT NULL
);
CREATE INDEX idx_events_time   ON events(created_at);
CREATE INDEX idx_events_target ON events(target, created_at);

CREATE TABLE profiles (
  id              INTEGER PRIMARY KEY,
  uid             TEXT NOT NULL UNIQUE,       -- UUIDv7
  name            TEXT NOT NULL,
  description     TEXT,
  tags            TEXT,                       -- JSON 数组
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  last_applied_at INTEGER,
  apply_count     INTEGER NOT NULL DEFAULT 0,
  version         INTEGER NOT NULL DEFAULT 1, -- CAS
  deleted_at      INTEGER                     -- 墓碑
);
-- B5：软删除后允许同名重建；墓碑行不占名
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
  kind         TEXT NOT NULL,                 -- env / path
  created_at   INTEGER NOT NULL,
  content_hash TEXT NOT NULL,
  size_bytes   INTEGER NOT NULL,
  file_path    TEXT NOT NULL,
  snapshot_id  INTEGER REFERENCES snapshots(id),
  deleted_at   INTEGER
);
CREATE INDEX idx_backups_kind_time ON backups(kind, created_at);

-- B3：迁移幂等键（source_path + content_hash），与 snapshots 解耦
CREATE TABLE migration_imports (
  source_path  TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  imported_at  INTEGER NOT NULL,
  PRIMARY KEY (source_path, content_hash)
);

CREATE TABLE scan_runs (
  id             INTEGER PRIMARY KEY,
  uid            TEXT NOT NULL UNIQUE,
  created_at     INTEGER NOT NULL,
  conflicts_json TEXT NOT NULL,
  tools_json     TEXT NOT NULL
);

CREATE TABLE var_references (
  snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
  from_hive   TEXT NOT NULL,
  from_name   TEXT NOT NULL,
  to_name     TEXT NOT NULL,                  -- 统一大写比较键
  PRIMARY KEY (snapshot_id, from_hive, from_name, to_name)
);

-- ── 同步 ──────────────────────────────────────────────
CREATE TABLE changes (
  id           INTEGER PRIMARY KEY,
  uid          TEXT NOT NULL UNIQUE,          -- 变更 UUID
  device_id    TEXT NOT NULL,
  seq          INTEGER NOT NULL,              -- 本设备单调序号
  lamport      INTEGER NOT NULL,              -- 逻辑时间，参与 LWW 比较
  origin       TEXT NOT NULL CHECK (origin IN ('local','remote')),
  entity_type  TEXT NOT NULL,                 -- snapshot/profile/event/...
  entity_uid   TEXT NOT NULL,
  field        TEXT,                          -- NULL = 整实体
  old_value    TEXT,                          -- JSON；敏感值只在加密 bundle 内
  new_value    TEXT,
  created_at   INTEGER NOT NULL,
  applied_at   INTEGER NOT NULL,              -- 本地落地时间（本地=created_at；远端=导入时间）
  exported_at  INTEGER                        -- 仅 local：NULL=待导出；导出后写时间戳
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
  resolution    TEXT                          -- local / remote
);

-- ── 派生/检索（可重建） ────────────────────────────────
CREATE VIRTUAL TABLE search_fts USING fts5(
  content, kind, ref_uid, tokenize='unicode61'
);

CREATE TABLE daily_stats (
  day        TEXT PRIMARY KEY,                -- YYYY-MM-DD
  path_items INTEGER NOT NULL,
  env_items  INTEGER NOT NULL,
  events     INTEGER NOT NULL,
  conflicts  INTEGER NOT NULL
);
```

### 4.2 不可违反的约束

1. `env_values` 不含 `Path`；`Unsupported` 的 `value=NULL`、`restorable=0`。
2. `events` 不含 env 明文值；只存摘要与 revision。
3. 所有读-改-写用 `BEGIN IMMEDIATE` + `version` CAS；冲突 `conflict`。
4. `revision` 是弱摘要，不是加密。
5. 派生表（`search_fts`/`daily_stats`/`var_references`）可随时重建，不是真相。
6. 注册表写入必须走既有服务编排；DB 不直接写注册表。
7. `changes.old_value` / `new_value` 仅用于同步，位于 SQLCipher 加密库与加密 bundle 内；UI 展示敏感值时仍须掩码，且不得写入 `events`。
8. `app_state.version` 保护单行状态（`current_snapshot_id` 等）：`UPDATE app_state SET …, version = version + 1 WHERE id = 1 AND version = ?`；`profiles.version` 保护配置行（M6）。
9. `sync_conflicts.local_value` / `remote_value` 展示时按对应变量的 `sensitive` 掩码（M5）。
10. 保留策略：已应用的 `changes` 保留 90 天；已解决的 `sync_conflicts` 保留 30 天；清理在维护任务中执行（M4）。

---

## 五、安全模型

### 5.1 本地数据库

- **密钥文件**（`keys/db.key.dpapi`）内容：`version(1) + db_key(32) + hash_salt(16)`，整体 `CryptProtectData`（用户作用域，`CRYPTPROTECT_UI_FORBIDDEN`）后落盘；`hash_salt` 用于 `.pebak` 的 `payload_hash`。
- **首次创建（H4）**：用 `Local\PathEditor.KeyInit` 命名互斥量串行化；密钥文件以 `create_new(true)` 原子创建；已存在时读取并校验 DPAPI blob，失败有界重试。**禁止「先 exists() 再写」的 TOCTOU**。
- 密钥文件 DACL：仅当前用户 + SYSTEM；启动时校验，权限过宽则告警（不自动修改）。
- 启动：`CryptUnprotectData` → `PRAGMA key` → `cipher_version` 校验。
- `cipher_memory_security`（M3）：**已由 M0 实测并决定默认 OFF**（ON/OFF 性能无显著差异，且 ON 在受限进程里 `VirtualLock` 失败）。若 M1 运行期出现安全问题再评估开启。
- 失败：密钥丢失/DPAPI 不可用 → **明确报错，不自动重建空库**（否则会静默丢失历史）。
- 密钥轮换：v6.0.0 不做（Tier 3 之后）；文档登记。
- 备份：`sqlcipher_export`（ATTACH + KEY）生成加密副本；**禁止直接复制 `.db`**（WAL 未 checkpoint 会丢事务）。
- `VACUUM INTO` 是 **M0 诊断项**（记录产物 header 与 `cipher_page_size` / KDF 参数），**不作为 CI 门**，也不作为备份路径；备份只用 `sqlcipher_export`。

### 5.2 同步包

- 口令 → Argon2id（m=64 MiB, t=3, p=1，salt 16 B）→ 32 字节密钥。
- AEAD：ChaCha20-Poly1305；nonce 12 B 随机；AAD 包含 header（版本/设备/序号）。
- 包格式：`magic + version + kdf params（含 salt 16 B）+ nonce + ciphertext + tag`；AAD 覆盖 header；payload 为 JSON（manifest + changes + tombstones，见 §6.4）。
- 口令**不落盘**；导入时输入；导出时二次确认。
- 防重放：`changes.uid` + `applied_at`；同一变更重复导入幂等。
- 防篡改：AEAD 验证失败即拒绝，不产生部分导入。

### 5.3 备份文件与旧明文

**新 env 备份（D17）**：`~/.patheditor/backups/env_backup_<ts>.pebak`

- 文件头（明文，不含秘密）：`magic "PEBK1" + version + created_at + kind + variable_count + plaintext_len + payload_hash`。
- `payload_hash = BLAKE3(hash_salt || canonical_json(payload))`；`hash_salt` 为每安装 16 字节随机，存于 DPAPI 密钥文件（**不写进 `.pebak`**）。用于去重/索引，不是加密；无 salt 的外部观察者不能直接验证候选值。
- 正文：`CryptProtectData`（用户作用域，`CRYPTPROTECT_UI_FORBIDDEN`）加密的 `EnvBackupPayload` JSON。
- 写入：先写临时文件再 rename；备份失败**不阻断**写入（与现有语义一致）。
- 读取：`env restore <FILE>` 识别 `.pebak` 并 DPAPI 解密；DPAPI 失败（换机/换用户）→ `Crypto` 错误（exit 5）。
- 列表：`env backups` 同时列出 `.pebak` 与遗留 `.json`，带 `encrypted` 标志；`.pebak` 的变量数从文件头读取，**不解密**。
- 轮换：只删除本功能生成的 `.pebak` / `.json`；`config.ini` 的 `env_backup_keep` 对两者都生效。

**旧明文（D9）**：`env_backup_*.json` 开发期间保留；M4 导入/校验后删除。
**Path 备份**：`path_backup_*.txt` 不含敏感值，保持明文可读。
**disabled.json / profiles/\*.json**：迁移后归档 `.v5.bak`；不自动删除。
**删除纪律**：不可逆；用户书面确认 + 回滚副本。

---

## 六、功能规格

### 6.1 Tier 1

#### 历史时间线

- `list_events(filter)`：按时间倒序，支持 `source` / `action` / `hive` / `target` / `result` / 时间范围 / 分页。
- `get_snapshot(uid)`：返回 PATH 条目与环境变量（敏感值默认掩码；`reveal` 需显式调用）。
- `restore_snapshot(uid)`：把该快照交给既有注册表写入编排；成功后追加新快照与事件（**append-only**，不回滚 DB）。

#### 跨会话撤销/重做

- 撤销/重做以「新快照 + 事件」表示，不删除历史。
- UI 的撤销栈从事件流重建；进程重启后仍可用。
- 与现有 `UndoRedoManager` 的 9–10 种操作类型兼容；v6.0.0 保留内存栈作为即时 UI，持久化事件作为跨会话恢复来源。

#### 配置元数据

- `list_profiles(filter)`：标签、描述、最近使用、应用次数。
- `save_profile(name, description, tags)`：写 `profiles` + `profile_revisions` + 快照，单事务。
- `apply_profile(uid)`：走 `save_path_with_sidecar`；成功事件 + `apply_count+1`。
- `export_profile_v5(uid)`：导出旧 JSON；`import_profile_v5(file)` 兼容导入。

#### 备份索引

- 扫描 `backups/`：kind/time/hash/size/path；`.pebak` 从文件头读 `variable_count`（**不解密**）；遗留 `.json` 解析内容（迁移期）。
- 搜索/筛选/去重视图；文件仍是产物，DB 只索引。
- `encrypted` 标志；`env restore` 同时支持 `.pebak` 与遗留 `.json`（后者给明文警告）。
- 保留策略：继续按 `config.ini` 的数量；v6.1 可扩展时间/大小策略。

### 6.2 Tier 2

- **FTS5**：索引路径串、变量名、配置描述/标签；`search(query)` 返回排名与高亮。
- **统计**：PATH 条目数/长度趋势、最常改变量、配置应用次数、冲突频率；`daily_stats` 每日聚合。
- **N 路对比**：任选两个快照/配置，返回新增/删除/修改/顺序变化；敏感值掩码。
- **引用图**：解析 `%VAR%` 边；递归 CTE 查传递依赖/循环；删除/修改变量前显示影响范围。
- **冲突历史**：`scan_runs` 持久化；两次扫描 diff；「首次出现/最近出现」。

### 6.3 Tier 3：同步

- **导出**：收集 `origin='local' AND exported_at IS NULL` 的记录（含墓碑）→ JSON → Argon2id/AEAD → 文件；成功后在同一事务内 `UPDATE changes SET exported_at = ? WHERE id IN (...)`。
- **导入**：解密/验签 → 在单事务内逐条应用 → 远端变更以 `origin='remote'`、`applied_at=now`、`exported_at=now` 写入（不再参与导出）→ 冲突写 `sync_conflicts` → 更新 `app_state.lamport`。
- **合并**：字段级 LWW（`lamport` + `device_id` 打破平局）；同字段并发修改记录冲突；UI 选择 local/remote。
- **范围**：同步 `profiles`/`profile_revisions`/`snapshots`/`path_entries`/`env_values`；不同步 `backups` 索引（本地文件）、派生表（本地重建）。
  - **`events` 不同步**（M3 核对轮裁断，2026-10-21 修订）。原设计列入了 `events`，但它是**本机操作日志**而非用户数据：同步过去会让设备 B 的历史面板出现「不是我做的」操作，而 UI 目前不渲染 `device_id`；且 bundle 体积会随操作次数无界增长，同步过去没有应用场景。`events.device_id` 保留在本地库中，供将来需要时按来源筛选。
- **UI**：同步中心（导出/导入/冲突列表/最近同步时间）；口令输入用密码框，不记忆。
- **限制**：LWW 可能丢失并发编辑；冲突日志保留败方值；不宣称 CRDT。

### 6.4 Bundle Schema（H2）

**Manifest**

```json
{
  "magic": "PESYNC1",
  "schemaVersion": 3,
  "deviceId": "<uuid>",
  "exportedAt": 0,
  "baseLamport": 0,
  "changeCount": 0,
  "kdf": { "alg": "argon2id", "m": 65536, "t": 3, "p": 1, "salt": "<base64>" },
  "nonce": "<base64>"
}
```

> `schemaVersion` 是**导出方的库结构版本**（M3 为 3），不是 bundle 布局版本。
> 布局版本在二进制头（`FORMAT_VERSION = 1`），由 `open()` 校验；`schemaVersion`
> 当前只记录、不拒绝，M4 需明确其语义或改为校验。

**Change（payload 内）**

```json
{
  "uid": "<uuid>",
  "deviceId": "<uuid>",
  "seq": 0,
  "lamport": 0,
  "entityType": "profile|snapshot|profile_revision|path_entry|env_value",
  "entityUid": "<uuid>",
  "field": "name|tags|...|null",
  "oldValue": "...",
  "newValue": "...",
  "createdAt": 0,
  "content": null
}
```

- **`content`（M3 补充）**：实体在导出瞬间的当前内容，接收端据此重建实体。
  `oldValue` / `newValue` 只是变更记录，不足以重建实体（例如只知道「快照被创建」，
  不知道它包含哪些 PATH 条目）。`content = null` 表示实体已不存在（墓碑语义，
  取代早期的 `deletedAt` 字段）。
- **`seq`**：导出方的本设备单调序号，导入时随 `changes` 落库，用于幂等与去重。
- **引用映射**：bundle 内所有引用使用 **uid**（`parentUid` / `snapshotUid` / `profileUid`），导入时按 uid 解析本地 `id`；找不到父实体时整体失败回滚。**禁止在 bundle 中出现本地整数主键**。
- **导入顺序**：单事务，按依赖序 `snapshots → path_entries/env_values → profiles → profile_revisions`；`events` 不在同步范围（见 §6.3）。任一失败整体回滚。
- **幂等**：`changes.uid` 已存在则跳过。
- **可落库类型（M3）**：`snapshot`。格式合法但当前版本不落库的类型（`profile` / `profile_revision` / `path_entry` / `env_value`）**不应用、不写 `changes`**，单独计入 `skippedUnsupported` 并在 UI/CLI 明示——写 `changes` 会占住 uid，使将来的重放被永久跳过。M4 实现配置同步后移除该限制。
- **错误**：未知 bundle 布局版本 / 未知 `entityType` / 悬空 uid → `SyncFormat`（退出码 1）。

### 6.5 M4：配置同步落库、墓碑与清理（2026-10-21 增补）

M3 的同步只落 `snapshot`；`profile` / `profile_revision` 等被跳过并计入 `skippedUnsupported`。
M4 补齐配置同步，并完成 v6.0.0 的清理与发布项。

#### 6.5.1 配置同步

- **可落库类型**：`snapshot` / `profile` / `profile_revision` / `path_entry` / `env_value`。
  后三者的 `content` 为 `null`（内容随父实体），导入时只记 `changes`，不单独写实体。
- **依赖序**：同一包内按 `snapshot → profile → profile_revision → path_entry → env_value`
  **稳定排序**后应用；同类型内部保持包内原有顺序。
- **profile content**（`ProfileSyncContent`）：`uid` / `name` / `description` / `tags` /
  `createdAt` / `updatedAt` / `version` / `snapshotUid`。导入端**忽略** `applyCount` /
  `lastAppliedAt`。
- **uid 保留**：远端 uid 直接写入本地 `profiles.uid`，引用不重新生成。
  `profile_revisions.uid` 在本地生成（bundle 不引用 revision uid）；
  `profile_revisions(profile_id, version)` 用 upsert 更新 `snapshot_id`。
- **使用统计不同步**：`apply_count` / `last_applied_at` 是本机使用数据，保留本地值；
  新建行取 `0` / `NULL`。
- **同名不同 uid**：自动改名为 `"{name} (来自 {deviceId 前 8 位})"`；若仍冲突，
  依次追加 ` 2`、` 3`…。改名只影响本地行，不改变包内 uid。
  `ImportReport` 新增 `renamed_profiles` 计数，UI/CLI 明示。
- **墓碑**：`content = null` **且** `field = "deleted_at"` → 软删除对应实体
  （`profiles.deleted_at` / `snapshots.deleted_at`）。其它 `content = null` 不触发删除。
- **实体级 LWW**：`snapshot` / `profile` 的本地比较取该实体**最新的一条变更（不限 field）**。
  否则墓碑（`field="deleted_at"`）与内容变更（`field=null`）落在两条互不可见的时间线上，
  旧内容会在删除之后「复活」。`path_entry` / `env_value` 仍按 field 比较。
- **悬空 `snapshotUid`**：`SyncFormat` 错误，整包回滚。
- `skipped_unsupported` 字段保留；M4 起对已知类型恒为 0。

#### 6.5.2 保留策略与清理

- `changes`：删除 `exported_at IS NOT NULL AND applied_at > 0 AND applied_at < now - 90d`。
  **绝不删除 `exported_at IS NULL`**（待导出队列）。
- `sync_conflicts`：删除 `resolved_at IS NOT NULL AND resolved_at < now - 30d`。
- 执行：GUI 启动序列调用一次；CLI 提供 `maintenance prune`。失败只告警，不阻断启动。
- `.v5.bak`：迁移成功后把 `disabled.json` 与 `profiles/*.json` 重命名为
  `<原名>.v5.bak`；目标已存在则跳过并记录；**不删除**。
- 遗留 `env_backup_*.json`：先转换为 `.pebak`（DPAPI 加密）并回读校验，
  成功后才删除 `.json`；任一失败保留原文件。CLI 必须 `--yes`。

#### 6.5.3 接口

| 层   | 接口                                                                               |
| ---- | ---------------------------------------------------------------------------------- |
| core | `maintenance::prune(db, now_ms) -> PruneReport`                                    |
| core | `archive_v5::archive_v5_sources(home, dry_run) -> ArchiveReport`                   |
| core | `maintenance::cleanup_legacy_env_backups(dir, material, dry_run) -> CleanupReport` |
| core | `v5_import::import_profile_v5(db, path) -> String`（返回新配置 uid）               |
| CLI  | `patheditor maintenance prune [--dry-run]`                                         |
| CLI  | `patheditor maintenance archive-v5 [--dry-run]`                                    |
| CLI  | `patheditor maintenance cleanup-legacy-backups [--dry-run] [--yes]`                |
| CLI  | `patheditor profile import <FILE>`                                                 |

退出码沿用：0 成功、1 一般错误、2 clap。清理类命令在未给 `--yes` 时打印计划并以 1
退出，不执行删除。

#### 6.5.4 安全边界

- 不写注册表；所有路径由调用方显式传入，测试用 `TempDir`。
- 删除类操作必须显式确认；转换失败不删除原文件。
- 导入仍是单事务，任一错误整体回滚。
- 保留策略只动 `exported_at IS NOT NULL` 的行，待导出队列不受影响。

#### 6.5.5 不在 M4 范围

- 使用统计（`applyCount` / `lastAppliedAt`）跨设备同步
- 配置字段级冲突（仍为整实体 LWW）
- GUI 的 v5 导入入口（仅 CLI）
- `schemaVersion` 校验、`sync-store` 中文兜底

---

## 七、迁移与兼容

1. **首次启动（含 H3）**：先调用 `retry_pending_path_state`；若仍有 pending，把 pending 作为 `kind=current` 的最高优先级来源导入，再归档 pending 文件。然后检测 `disabled.json` / `profiles/*.json` / `backups/*`；创建加密 DB；单事务导入；`app_state.migrated_v5=1`。
2. **导入规则**：
   - `disabled.json` → `snapshots(kind=current)` + `path_entries`；
   - `profiles/*.json` → `profiles` + `profile_revisions` + `snapshots`；
   - `env_backup_*.json` → `snapshots(kind=backup)` + `env_values`（排除 Path；Unsupported 只存元数据）；敏感值入库（DB 已加密）；
   - `path_backup_*.txt` → `snapshots(kind=backup)` + `path_entries`；
   - `backups` 索引行；文件保留。
3. **幂等（B3）**：以 `migration_imports(source_path, content_hash)` 判重；迁移可重跑。
4. **原件**：保留为 `*.v5.bak`；**不自动删除**。
5. **降级**：v5 无法识别 DB；v6 提供「导出 v5 JSON」；若需拒绝启动，需在 v6 前发 v5.x 守卫（G4）。
6. **清理（M4）**：按 §5.3 执行；用户确认后才删除 `env_backup_*.json`。
7. **测试夹具**：无 `schemaVersion` 的旧 JSON、损坏 JSON、空备份、含 `Path`/Unsupported 的备份、含敏感值备份。
8. **v5 文件停写（H7）**：迁移成功后，正常模式**不再写 `disabled.json`**；`disabled.json` / `profiles/*.json` 只读归档。**降级模式**（DB 不可用）写 `disabled.json` 作为 PATH 禁用状态的兜底，并在下次 DB 恢复时对账导入；`pending_path_snapshot.json` 保留为「注册表已写、DB 未提交」的 gap 文件（只含路径，无敏感值）。
9. **CLI 备份语义（H6，已定）**：
   - `env backup` 写 `env_backup_<ts>.pebak`（DPAPI），失败不阻断；
   - `env backups` 列出 `.pebak` + 遗留 `.json`，带 `encrypted` 标志与变量数；
   - `env restore <FILE>` 支持 `.pebak`（解密）与遗留 `.json`（明文，警告）；
   - `--json` 输出新增 `encrypted` / `format` 字段；旧字段保持兼容；
   - DPAPI 失败（换机/换用户）→ `Crypto` 错误，exit 5。
10. **DB 不可用（B7）**：**fail-open** —— 注册表/环境变量编辑与保存照常；历史/同步/统计面板报错；常驻错误条幅；降级操作不写历史（状态栏明示）；**不得静默重建空库**。恢复后可提供「用 `disabled.json` 兜底对账」。

---

## 八、错误处理与退出码

- 新增 `ErrorCode`：`Database`（打开/查询/事务）、`Crypto`（密钥/解密）、`Sync`（**合并冲突**）、`SyncFormat`（**包格式/口令/篡改**）、`Migration`（导入/版本）。
- CLI 退出码：0 成功；1 一般错误；2 clap；3 revision 冲突（既有）；**4 同步/合并冲突**（新增）；**5 加密/密钥错误**（新增）。
  - **`Sync` → 4、`SyncFormat` → 1**（M3 核对轮裁断，2026-10-21 修订）。原设计把两者都归为 `Sync`，导致脚本无法区分「需要人工解决冲突」与「文件坏了/口令错了」——前者应提示用户介入，后者重试无用。拆成两个码后：`Sync`（合并冲突，可解决）→ exit 4；`SyncFormat`（magic 不符、版本未知、AEAD 验证失败、`entityType` 未知）→ exit 1。
- **DB 打不开（B7）**：GUI 不退出，进入降级模式；CLI 对**需要 DB 的命令**返回 exit 5（Crypto）或 1（Database），不需要 DB 的命令照常。
- 降级模式写入 `disabled.json` 兜底；状态栏/日志明示「历史不可用，正在使用兜底」。
- 错误文案不得包含密钥、口令、明文敏感值；日志同样。
- 导入/同步失败必须整体回滚（单事务），不产生半应用状态。

---

## 九、测试策略

| 层   | 内容                                                                                     |
| ---- | ---------------------------------------------------------------------------------------- |
| 单元 | schema/迁移、CAS、查询、FTS、引用图、Argon2id/AEAD roundtrip、DPAPI roundtrip（Windows） |
| 集成 | 旧 JSON 迁移夹具、历史/恢复、配置版本、备份索引、同步两库合并、冲突、幂等、篡改/错误口令 |
| 并发 | 两个进程（GUI mock + CLI）同时写；CAS 冲突返回 `conflict`；WAL busy_timeout              |
| 安全 | 错误密钥失败；密钥文件 ACL；`strings`/hex 不出现敏感值；事件表无明文；bundle 篡改失败    |
| 性能 | 冷启动、1 万快照、FTS 查询、迁移 10 年备份                                               |
| E2E  | 历史面板、同步中心、冲突 UI（mock IPC）                                                  |
| 回归 | `npm run verify:all`；现有 320 单测 + 41 E2E 全绿                                        |

**验收口径**：`SELECT sqlite_compileoption_used('ENABLE_FTS5')` 非 0；`SELECT json_valid('{"a":1}')` 为 1；`PRAGMA cipher_version` 非空。

---

## 十、文档同步

- `CLAUDE.md` / `AGENTS.md`：新增「DB 是 sidecar、注册表是真相源」「密钥/备份/同步」章节；两份字节级一致。
- `README.md`：v6 新特性、密钥丢失后果、同步包使用、旧备份清理说明。
- `CHANGELOG.md`：v6.0.0 段落。
- 新增 `docs/` 运维说明：密钥备份/恢复、同步包交换、故障排查。
- 所有新 `pub fn` / `pub(crate)` 中文 `///` 文档注释。

---

## 十一、范围外（本版不做）

- 注册表写入的原子/CAS（OS 契约）；TOCTOU 仍在。
- 服务器式同步（NAS/云端 API）——本版只做文件式变更包。
- 自动删除旧明文备份（必须用户确认）。
- 密钥恢复/托管（DPAPI 丢失即不可解密；文档写明）。
- 网络家目录的可靠 WAL（检测到网络路径时降级/提示）。
- 跨平台（v6 仍 Windows-only）。
- SQLCipher 密钥轮换。
- 前端直连 SQL。

---

## 十二、验收标准

- [ ] SQLCipher DB 只有正确密钥可打开；`cipher_version` 非空；错误密钥失败且不产生空库。
- [ ] 密钥文件 DACL 仅当前用户 + SYSTEM；启动校验失败有明确告警。
- [ ] v5 四种文件全部可迁移；计数/hash 对得上；重复迁移幂等；原件保留 `.v5.bak`。
- [ ] `env_values` 不含 `Path`；`Unsupported` `value=NULL`、`restorable=0`。
- [ ] `events` 无 env 明文值（守卫测试扫描写入路径）。
- [ ] 历史时间线可筛选/预览/恢复；恢复走既有注册表编排；跨会话撤销/重做可用。
- [ ] 配置标签/描述/版本/最近使用/应用次数可用；v5 JSON 导出/导入可用。
- [ ] 备份索引显示真实变量数；`.pebak` 从文件头读取（不解密）；path 备份可手工打开，env 备份需经 PathEditor/CLI 解密。
- [ ] 新 env 备份为 `.pebak`（DPAPI），磁盘上不再新增明文敏感文件；遗留 `.json` 可一次性恢复（带警告）。
- [ ] `.pebak` 头部含 `payload_hash`（salted BLAKE3）；索引可在不解密的情况下按 hash 去重；`hash_salt` 不在文件头。
- [ ] DB 打不开时 GUI fail-open：编辑照常、历史/同步降级、`disabled.json` 兜底、常驻条幅；恢复后对账；不静默重建。
- [ ] FTS5 搜索、统计、N 路对比、引用图、冲突历史可用。
- [ ] 文件式同步：两库导出/导入、并发合并、冲突 UI、幂等、错误口令/篡改拒绝；
      配置（`profile` / `profile_revision`）落库、同名自动改名、墓碑软删除（§6.5.1）。
- [ ] 并发写：两进程同时写，CAS 冲突返回 `conflict`，无 lost update。
- [ ] M4 清理：`env_backup_*.json` 在用户确认后删除；无新的明文敏感文件生成。
- [ ] `npm run verify:all` 退出码 0；覆盖率 ≥80%；现有测试全绿。
- [ ] `CLAUDE.md` / `AGENTS.md` 字节级一致；README/CHANGELOG 更新。

---

## 十三、里程碑（开发顺序，不是多个发布）

| 里程碑             | 内容                                                                                                                                                                | 验证门                                                      |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| **M0 风险探针**    | `rusqlite` + SQLCipher 在 Windows MSVC 构建（**本地 no-asm 先测；CI 装 NASM**，两者对照）；DPAPI 密钥；FTS5/JSON1 编译选项；`sqlcipher_export` / `VACUUM INTO` 行为 | 构建成功 + 最小 open/encrypt/query 测试；失败则回退方案拍板 |
| **M1 存储 + 历史** | schema/迁移/加密/事件/快照/配置/备份索引；`.pebak` 备份；`HistorySink` 接缝；fail-open 降级；Tier 1 UI                                                              | 迁移夹具 + 历史/恢复 + 并发 CAS + 降级 + `verify:all`       |
| **M2 检索与统计**  | FTS5、统计、N 路对比、引用图、冲突历史                                                                                                                              | 查询/图/统计测试 + UI E2E                                   |
| **M3 同步**        | changes/墓碑/bundle/合并/冲突 UI                                                                                                                                    | 两库同步 + 并发/篡改/幂等测试                               |
| **M4 清理与发布**  | 配置同步落库（含同名自动改名/墓碑）、保留策略清理、`.v5.bak` 归档、遗留明文 `.json` 转换后清理（用户确认）、v5 导入、文档、版本 6.0.0、发布检查                     | 验收清单全绿 + 用户确认删除                                 |

**每个里程碑必须独立通过 `verify:all` 并可回滚**；M0 的结论（构建/加密可行性）是 v6.0.0 能否按此设计推进的硬门。

---

## 十四、风险与未决项

| #   | 风险                                         | 缓解                                                                           |
| --- | -------------------------------------------- | ------------------------------------------------------------------------------ |
| R1  | SQLCipher + OpenSSL 在 Windows MSVC 构建复杂 | M0 先探针；本地 no-asm 先测、CI 装 NASM；失败则评估系统 OpenSSL/vcpkg；CI 固化 |
| R2  | DPAPI 密钥丢失 → 历史不可读                  | 文档明确；提供「导出全部历史」；不自动重建空库                                 |
| R3  | 单版本全量范围过大                           | M0–M4 里程碑 + 每门可回滚；M0 失败可缩减范围并重新拍板                         |
| R4  | 同步 LWW 丢并发编辑                          | 冲突日志保留败方值；UI 手动选择；文档写明限制                                  |
| R5  | 旧明文备份在开发期仍是暴露面                 | 迁移后尽快校验；M4 用户确认删除；新备份写 `.pebak`，不再新增明文敏感文件       |
| R6  | 网络家目录 WAL 不可靠                        | 检测网络路径 → 降级/提示；文档建议本地磁盘                                     |
| R7  | DB 损坏                                      | `integrity_check`、`sqlcipher_export` 备份、`.corrupt` 隔离                    |
| R8  | 性能/体积                                    | content_hash 去重、保留策略、分页、FTS 按需重建                                |
| R9  | 迁移/同步测试覆盖不足                        | 旧格式夹具、双进程、kill 中断、篡改/错误口令用例                               |
| R10 | 范围外承诺（密钥恢复/服务器同步）            | 文档明确「本版不做」                                                           |
| R11 | `.pebak` 绑定 DPAPI，换机/换用户不可解密     | 文档写明；跨机备份走同步包或 DB 导出；不承诺可移植                             |
| R12 | 降级模式不记录历史                           | 状态栏明示；恢复后对账；不把降级当正常                                         |
| R13 | 遗留明文 `.json` 清理不可逆                  | M4 用户确认 + 回滚副本；先导入/校验再删                                        |

## 十五、spec 核对轮裁断（2026-10-19）

| 项                       | 裁断                                                                                                                                               | 落点                   |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------- |
| **B1** NASM              | **部分驳回**：缺 NASM 是事实，但 `openssl-src` 在 MSVC 上自动 `no-asm`，未必失败。**最终**：本地 M0 先 no-asm；CI 装 NASM；失败/性能不足再本地安装 | §3.4、§十三 M0         |
| **B2** `user_version`    | 采纳：启动只读，建库/迁移事务内写                                                                                                                  | §3.3                   |
| **B3** 幂等键            | 采纳：新增 `migration_imports(source_path, content_hash)`                                                                                          | §4.1、§7.3             |
| **B4** `changes.lamport` | 采纳：新增列                                                                                                                                       | §4.1                   |
| **B5** 配置名唯一        | 采纳：改为 `WHERE deleted_at IS NULL` 的部分唯一索引                                                                                               | §4.1                   |
| **B6** 备份格式          | **采纳 DPAPI 加密文件 `.pebak`**：新 env 备份加密；遗留 `.json` 一次性恢复；M4 删除                                                                | §5.3、§6.1、§7.9       |
| **B7** DB 不可用         | 采纳 **fail-open + 降级 `disabled.json` 兜底**；不静默重建                                                                                         | §7.10、§8              |
| **H1** 服务接缝          | 采纳 `HistorySink` + `NoopHistory`                                                                                                                 | §3.2                   |
| **H2** bundle schema     | 采纳：新增 §6.4，引用一律用 uid                                                                                                                    | §6.4                   |
| **H3** pending 文件      | 采纳：迁移前先 `retry_pending_path_state`，pending 作为最高优先级 current                                                                          | §7.1                   |
| **H4** 密钥竞争          | 采纳：命名互斥量 + `create_new(true)` + 校验/重试                                                                                                  | §5.1                   |
| **H5** content_hash      | 采纳：`blake3`，仅用于去重/判等                                                                                                                    | §3.4                   |
| **H6** CLI 备份语义      | 已定：`.pebak` + 遗留 `.json` 兼容；`encrypted`/`format` 字段                                                                                      | §7.9                   |
| **H7** v5 文件停写       | 采纳：正常模式停写；降级模式写 `disabled.json` 兜底；pending 保留                                                                                  | §7.8                   |
| **M1–M7**                | 全部采纳（salt、VACUUM INTO 验证、memory_security、保留策略、冲突掩码、CAS 对象、feature 核对）                                                    | §3.4、§4.2、§5.1、§5.2 |

> **收口状态（2026-10-19）**：B1（本地 no-asm 先测 / CI 装 NASM）、B6（`.pebak` DPAPI 加密）、B7（fail-open + 兜底）均已拍板并写入上文。**spec 已收口，下一步写实施计划。**

## 十六、M0 计划核对轮补充裁断（2026-10-19）

| 项                             | 裁断                                                                                                                               | 落点             |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------- | ---------------- | ---------------------------------------- | --------------- |
| **B1b** 本机缺 `make`          | 确认：`openssl-src` GNU 路径硬编码 `make`，本机只有 `mingw32-make`；**用户执行 `scoop install make`**（main 4.4.1）后才能跑本机 M0 | §3.4、M0 计划    |
| **B2** 错误密钥测试 panic      | 采纳：探针拆 `open_with_key`（只设 key）与 `open_encrypted`（先校验再 PRAGMA）；测试额外验证原库仍可用                             | M0 计划 Task 1/2 |
| **B3** 同步三态                | 采纳：`changes` 增加 `origin` / `applied_at NOT NULL` / `exported_at`；导出条件改为 `origin='local' AND exported_at IS NULL`       | §4.1、§6.3       |
| **B4** CI 触发                 | 采纳：workflow 用 `push` + `pull_request`，不用 `workflow_dispatch`（分支上会 404）                                                | M0 计划 Task 6   |
| **H** `.pebak` content_hash    | 采纳：头部加 `payload_hash = BLAKE3(hash_salt                                                                                      |                  | payload)`；`hash_salt` 存 DPAPI 密钥文件 | §5.1、§5.3、§12 |
| **H** `migration_imports` 主键 | 采纳：`PRIMARY KEY (source_path, content_hash)`                                                                                    | §4.1             |
| **H** `VACUUM INTO`            | 采纳：M0 诊断项（记录 header/参数），不是 CI 门；备份只用 `sqlcipher_export`                                                       | §5.1             |
| **H** `cipher_memory_security` | 采纳：M0 基准诊断，不作硬门                                                                                                        | M0 计划          |

> **精度更正**：开发窗口说「NASM 分支只对 MSVC 生效、GNU 不查 NASM」不完全准确——`openssl-src` 的 `is_nasm_ready()` 是 `#[cfg(windows)]`，对 Windows GNU 也会检测；MSVC 专属的是 `nmake` 分支。实用结论不变：GNU 需要 `make`，NASM 可选。

---

**审核窗口**：2026-10-19
