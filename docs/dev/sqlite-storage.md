# SQLite 存储（不加密）

> 本文件是 `CLAUDE.md` 的详细补充。日常开工只需读 `CLAUDE.md` 的速览；
> 涉及 schema、迁移、同步或保留策略时再读本文。

## 基本形态

- **sidecar**：`~/.patheditor/patheditor.db`（**标准 SQLite，明文**）；注册表仍是唯一真相源。DB 打不开时 GUI fail-open：编辑照常、历史/同步/检索降级、禁用状态不可恢复、窗口顶部常驻条幅；**不静默重建空库**。
- **遗留加密库**：旧版加密库的文件头不是 `SQLite format 3`，`Db::open` **直接拒绝**并给出删除指引；不做任何自动转换。
- **本机盐**：`~/.patheditor/keys/db.key.dpapi`，内容 `version(1)+reserved(32)+hash_salt(16)`，整体 DPAPI（**当前 Windows 用户**作用域）；互斥量 `Local\PathEditor.KeyInit` 串行化首次创建，DACL 限当前用户 + SYSTEM。首次创建用 `create_new(true)` 原子写入，禁止 `exists()`→写入的 TOCTOU。前 32 字节是旧版 `db_key` 的遗留位置，现只作保留位；**保持 49 字节与文件名不变**，否则磁盘上既有 `.pebak` 的 `payload_hash` 会全部失配。
- **数据目录**：`PATHEDITOR_DATA_DIR` 仅供 CLI 子进程测试把库重定向到临时目录；生产不设置。
- **schema**：`src/core/src/db/schema.rs`，`CURRENT_SCHEMA_VERSION = 5`。`PRAGMA user_version` 启动只读，**只在建库/迁移事务内写**；新增迁移时递增并补 `DDL_Vn`。

## 表

| 版本 | 表                                                                                                                  | 性质                                                                |
| ---- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| v1   | `snapshots` / `path_entries` / `env_values` / `events` / `profiles` / `profile_revisions` / `backups` / `app_state` | 真相或业务状态                                                      |
| v2   | `scan_runs` / `var_references` / `daily_stats` / `search_fts`                                                       | `scan_runs` 是真实记录；其余是派生表                                |
| v3   | `changes` / `sync_conflicts`                                                                                        | `changes` 是变更日志（真相，不可重建）；`sync_conflicts` 待人工决定 |
| v4   | `sync_conflicts.remote_content` + 三个查询索引                                                                      | 冲突需保留远端实体内容，「采纳远端」才能真正落地                    |

**派生表**：`search_fts` / `daily_stats` / `var_references` 可随时重建（不是真相）；`scan_runs` 是真实记录。GUI 启动时按需重建（FTS 仅空时、daily/refs 每次）；**CLI 不重建**（已知限制，改 PATH 后 GUI 派生数据可能陈旧）。

## 测试规则

数据库是普通 SQLite，**不再有「一文件一测试」约束**（该约束的根因是 SQLCipher 全局状态导致同进程第二个测试 `STATUS_STACK_OVERFLOW`）。存量单测试文件保留原样，不为了合并而重排；新测试可以在同一文件放多个 `#[test]`。

CI 分两步跑（core/cli 用 `--exclude patheditor`，GUI 用 `--lib` + 清单开关），覆盖整个 workspace。

## 初始化

- 启动时 `migrate_to_current` 只做 schema 迁移；首次启动若没有 `current_snapshot_id`，用注册表当前状态建一条基线。
- 遗留 `env_backup_*.json` 用 `maintenance cleanup-legacy-backups --yes` 先转 `.pebak` 并回读校验，再删除明文。

## 同步

`changes` 是变更日志（真相，不可重建），`sync_conflicts` 是待人工决定。

- **bundle 格式**：`PESYNC1\0 + version + header_len + header JSON + salt(16) + nonce(12) + ciphertext + tag`
- **加密**：Argon2id（m=65536 KiB, t=3, p=1）+ ChaCha20-Poly1305；AAD 覆盖 header/salt/nonce。
- **导入**：单事务；冲突写 `sync_conflicts`；失败整体回滚；**不写注册表**。
- **合并**：LWW（`lamport` 高者胜，同 lamport 按 `device_id`）；`snapshot` / `profile` 按**整实体**比较（墓碑与内容变更同一时间线）。
- **配置同步**：`profile` / `profile_revision` 随包落库；远端 uid 保留；同名不同 uid 自动改名 `原名 (来自 设备前 8 位)`；墓碑（`content=null` + `field=deleted_at`）软删除；`profile.name` 过本地同一套 `validate_name`，非法名称 → `SyncFormat` 整包回滚。
- **增量包**：导出只包含 `exported_at IS NULL` 的变更；新设备需从第一份包按顺序应用。

## 保留策略

已导出的 `changes` 保留 90 天、已解决的 `sync_conflicts` 保留 30 天；GUI 每次启动执行一次，CLI `maintenance prune`。**绝不删除 `exported_at IS NULL` 的待导出队列**。

快照按 `snapshot_keep_days`（默认 365 天）与 `snapshot_keep_max`（默认 500 条）清理，事件按 `event_keep_days`（默认 730 天）清理，键写在 `~/.patheditor/config.ini`。以下快照**永不删除**：`pinned = 1`、`app_state.current_snapshot_id` 指向的、被 `profile_revisions` 引用的、被 `backups` 引用的。被删快照只把 `events.snapshot_id` 置 NULL，审计事件保留。`maintenance vacuum` 先清理再 `wal_checkpoint(TRUNCATE)` + `VACUUM`。

## 迁移前备份与健康检查

- `migrate_to_current` 在真正迁移前用 SQLite Online Backup API 复制一份
  `<数据库文件名>.pre-v{旧版本}.bak`（同目录，最多保留 3 份）；备份失败则中止迁移。
- `db_health` / `maintenance health` 输出 schema、`integrity_check(1)`、9 张关键表行数、
  主库/WAL/SHM 体积与最近清理时间。
- 最近清理时间写在 `<数据目录>/maintenance.json` 的 `lastPruneAt`，不占用 schema 列。

## 旧明文

新 env 备份写 `.pebak`（DPAPI）；遗留 `env_backup_*.json` 仍可读，`maintenance cleanup-legacy-backups` 先转 `.pebak` 并**回读校验**、再删除（需 `--yes`）。备份目录 ACL 未收紧；遗留/回落 `.json` 仍是明文。

## 构建（不需要 OpenSSL）

依赖链：`rusqlite → libsqlite3-sys → SQLite（C）`。

- `bundled` 把 SQLite 的 C 源码打进 crate，由 `cc` 编译；FTS5 与 JSON1 默认开启（`libsqlite3-sys` 的 `build.rs` 显式 `-DSQLITE_ENABLE_FTS5` / `-DSQLITE_ENABLE_JSON1`）。
- 不再需要 `make` / `perl` / NASM，也不需要 `OPENSSL_SRC_PERL` / `PERL5LIB` 这类会话变量。
- `.pebak` 备份与同步 bundle 的密码学全部来自纯 Rust crate（`argon2` / `chacha20poly1305`）与 Windows DPAPI，与构建链无关。

## 参考

- v7 设计：`docs/superpowers/specs/2026-11-08-drop-db-encryption-design.md`
- v7 计划：`docs/superpowers/plans/2026-11-08-drop-db-encryption-implementation.md`
- v6 设计：`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`
- v6 计划：`docs/superpowers/plans/`（M1–M4）
- M0 探针：`docs/审核和开发/2026.10.19/PathEditor-v6-M0-探针结果.md`

- **当前状态**：`app_state.current_snapshot_id` 由 `core::service::save_path_with_db` 维护（GUI 与 CLI 共用）；每次成功写 PATH 后写入 `kind=current` 快照并增量索引。DB 不可用时只读注册表。
