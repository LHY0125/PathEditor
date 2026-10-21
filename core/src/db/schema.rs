//! Schema DDL 与版本迁移。
//!
//! **启动只读 `user_version`；只在建库/迁移事务内写**（spec §3.3 B2）。
//! 建表顺序遵循外键依赖：`snapshots` 必须早于引用它的表。

/// 当前 schema 版本。新增迁移时递增并补一个 `DDL_Vn` 分支。
pub const CURRENT_SCHEMA_VERSION: u32 = 3;

/// v1 全量 DDL（与 spec §4.1 一致）。
///
/// 注意 `app_state` 引用 `snapshots(id)`，因此 `snapshots` 必须先建。
pub const DDL_V1: &str = r#"
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

CREATE TABLE app_state (
  id                  INTEGER PRIMARY KEY CHECK (id = 1),
  current_snapshot_id INTEGER REFERENCES snapshots(id),
  version             INTEGER NOT NULL DEFAULT 1,
  migrated_v5         INTEGER NOT NULL DEFAULT 0,
  device_id           TEXT NOT NULL,
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

/// v2 增量 DDL：M2 的派生表（可随时重建，不是真相源）。
///
/// **逐字照抄 spec §4.1**（`2026-10-19-v6-sqlite-design.md`）。这是唯一一处
/// 「照抄权威文档」的地方，不要发挥——列名与类型必须与 spec 完全一致。
///
/// 四张表的性质（spec §4.2 第 5 条）：
/// - `scan_runs`：冲突扫描历史，**不是派生表**（是真实记录，无法从别处重建）；
/// - `var_references` / `daily_stats` / `search_fts`：派生表，必须能从
///   `snapshots` / `events` / `path_entries` 完整重建。
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

/// v3 增量 DDL：M3 的同步表（spec §4.1）。
///
/// 两张表都是**本地状态**，不参与「派生表可从真相重建」的口径——
/// `changes` 是变更日志本身（真相），`sync_conflicts` 是待人工解决的决定。
///
/// **`changes` 的索引是 LWW 与同步的核心**：
/// - `(device_id, seq)` 支撑「本设备单调序号」及导入时的去重；
/// - `(entity_type, entity_uid)` 支撑按实体找回当前值；
/// - `(origin, exported_at)` 支撑 `pending_export()` 的部分索引扫描。
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

/// 建库时写入 `app_state` 单行（J-7）。
///
/// **为什么必须在建库时写**：`app_state` 若为空，
/// - `app_state_version()` 无行可读 → 返回错误；
/// - `set_current_snapshot()` 的 CAS `WHERE id = 1 AND version = ?` 匹配 0 行
///   → **恒返回 `false`**，spec §十二 的「CAS 冲突返回 conflict」永远不可达；
/// - `DbHistory::record` 只能用 `COALESCE(..., 'local')` 兜底 `device_id`。
///
/// `device_id` 在 Rust 侧生成一次（UUIDv7），此后不再变更 —— M3 的同步以它区分设备。
/// 用 `INSERT OR IGNORE` 保证重复调用幂等（已有行时不覆盖真实 device_id）。
pub const SEED_APP_STATE_SQL: &str = "INSERT OR IGNORE INTO app_state
       (id, current_snapshot_id, version, migrated_v5, device_id, lamport)
     VALUES (1, NULL, 1, 0, ?1, 0)";
