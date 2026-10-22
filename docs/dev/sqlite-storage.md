# v6 SQLite 存储

> 本文件是 `CLAUDE.md` 的详细补充。日常开工只需读 `CLAUDE.md` 的速览；
> 涉及 schema、迁移、同步、保留策略或构建成本时再读本文。

## 基本形态

- **sidecar**：`~/.patheditor/patheditor.db`（SQLCipher）；注册表仍是唯一真相源。DB 打不开时 GUI fail-open：编辑照常、历史/同步/检索降级、`disabled.json` 兜底、窗口顶部常驻条幅；**不静默重建空库**。
- **密钥**：`~/.patheditor/keys/db.key.dpapi`，内容 `version(1)+db_key(32)+hash_salt(16)`，整体 DPAPI（**当前 Windows 用户**作用域）；互斥量 `Local\PathEditor.KeyInit` 串行化首次创建，DACL 限当前用户 + SYSTEM。首次创建用 `create_new(true)` 原子写入，禁止 `exists()`→写入的 TOCTOU。
- **schema**：`core/src/db/schema.rs`，`CURRENT_SCHEMA_VERSION = 3`。`PRAGMA user_version` 启动只读，**只在建库/迁移事务内写**；新增迁移时递增并补 `DDL_Vn`。

## 表

| 版本 | 表                                                                                                                  | 性质                                                                |
| ---- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| v1   | `snapshots` / `path_entries` / `env_values` / `events` / `profiles` / `profile_revisions` / `backups` / `app_state` | 真相或业务状态                                                      |
| v2   | `scan_runs` / `var_references` / `daily_stats` / `search_fts`                                                       | `scan_runs` 是真实记录；其余是派生表                                |
| v3   | `changes` / `sync_conflicts`                                                                                        | `changes` 是变更日志（真相，不可重建）；`sync_conflicts` 待人工决定 |

**派生表**：`search_fts` / `daily_stats` / `var_references` 可随时重建（不是真相）；`scan_runs` 是真实记录。GUI 启动时按需重建（FTS 仅空时、daily/refs 每次）；**CLI 不重建**（已知限制，改 PATH 后 GUI 派生数据可能陈旧）。

## 测试规则

**一文件一测试**：任何使用 SQLCipher 的 `.rs` 测试文件**只放一个 `#[test]`**。同一进程内跑第二个 SQLCipher 测试会 `STATUS_STACK_OVERFLOW`（GNU 与 MSVC 均已复现，`--test-threads=1` 无效）。

CI 分两步跑（core/cli 用 `--exclude patheditor`，GUI 用 `--lib` + 清单开关），覆盖整个 workspace——规则靠文件布局保证，不靠 CI 逐文件调用。依据：`docs/审核和开发/2026.10.19/PathEditor-v6-M0-探针结果.md` §5.3。

## v5 迁移与归档

- `migrate_v5` 一次性导入 `disabled.json` / `profiles/*.json` / `backups/*`，幂等（`migration_imports(source_path, content_hash)`）；迁移期间**不改名、不删除**。
- 迁移成功且零失败后，`archive_v5` 在**启动序列末尾**（读路径已切到 DB 之后）把原件改名为 `.v5.bak`；目标已存在则跳过，绝不覆盖。
- 遗留 `env_backup_*.json` 用 `maintenance cleanup-legacy-backups --yes` 先转 `.pebak` 并回读校验，再删除明文。

## 同步

`changes` 是变更日志（真相，不可重建），`sync_conflicts` 是待人工决定。

- **bundle 格式**（spec §5.2 / §6.4）：`PESYNC1\0 + version + header_len + header JSON + salt(16) + nonce(12) + ciphertext + tag`
- **加密**：Argon2id（m=65536 KiB, t=3, p=1）+ ChaCha20-Poly1305；AAD 覆盖 header/salt/nonce。
- **导入**：单事务；冲突写 `sync_conflicts`；失败整体回滚；**不写注册表**。
- **合并**：LWW（`lamport` 高者胜，同 lamport 按 `device_id`）；`snapshot` / `profile` 按**整实体**比较（墓碑与内容变更同一时间线）。
- **配置同步**：`profile` / `profile_revision` 随包落库；远端 uid 保留；同名不同 uid 自动改名 `原名 (来自 设备前 8 位)`；墓碑（`content=null` + `field=deleted_at`）软删除；`profile.name` 过本地同一套 `validate_name`，非法名称 → `SyncFormat` 整包回滚。
- **增量包**：导出只包含 `exported_at IS NULL` 的变更；新设备需从第一份包按顺序应用。

## 保留策略

已导出的 `changes` 保留 90 天、已解决的 `sync_conflicts` 保留 30 天；GUI 每次启动执行一次，CLI `maintenance prune`。**绝不删除 `exported_at IS NULL` 的待导出队列**。

## 旧明文

新 env 备份写 `.pebak`（DPAPI）；遗留 `env_backup_*.json` 仍可读，`maintenance cleanup-legacy-backups` 先转 `.pebak` 并**回读校验**、再删除（需 `--yes`）。备份目录 ACL 未收紧；遗留/回落 `.json` 仍是明文。

## 为什么依赖 OpenSSL（构建成本来源）

依赖链：`rusqlite → libsqlite3-sys → SQLCipher（C）→ OpenSSL（C）`。

- **OpenSSL 的唯一作用**：给 SQLCipher 提供 AES-256、HMAC、PBKDF2 等密码学原语。项目**没有任何 TLS / 网络调用**，与「OpenSSL 漏洞」常见的 Web 攻击面不同。
- `bundled-sqlcipher-vendored-openssl` 把 SQLCipher 与 OpenSSL 的 **C 源码**都打进 crate，构建时用 `cc` 编译。因此需要 `make`、`perl`（MSVC 下还需 NASM），首次全量编译约 45 分钟，二进制 +1–2 MB。
- 选 `vendored` 是为了「不依赖用户机器上是否装了 OpenSSL、版本是否兼容」，代价是构建工具链与时间——这是有意的权衡，不要改成链接系统 OpenSSL。
- 密钥：`~/.patheditor/keys/db.key.dpapi`，内容为 `version(1)+db_key(32)+hash_salt(16)`，整体经 DPAPI（**当前 Windows 用户**作用域）加密；互斥量 `Local\PathEditor.KeyInit` 串行化首次创建，DACL 限当前用户 + SYSTEM。详见 spec §5.1 与 M1 计划 Task 3。

## 参考

- 设计：`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`
- 计划：`docs/superpowers/plans/`（M1–M4）
- M0 探针：`docs/审核和开发/2026.10.19/PathEditor-v6-M0-探针结果.md`
