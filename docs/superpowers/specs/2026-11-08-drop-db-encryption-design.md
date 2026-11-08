# PathEditor 移除数据库加密 — 设计文档

- **日期**：2026-11-08
- **作者**：审核窗口
- **状态**：待开发窗口核对
- **范围**：`src/core`（`db` 层为主）、`src/cli`、`src/gui`、CI、当前态文档
- **目标版本**：v7.0.0（存储格式破坏性变更）

## 一、背景与裁断

### 1.1 事实基础

v6 起数据层使用 `rusqlite` 的 `bundled-sqlcipher-vendored-openssl`（`src/core/Cargo.toml:29`），全库加密 + DPAPI 托管密钥。用户于 2026-11-08 提出：同机对照 `cc-switch` 的 `~/.cc-switch/cc-switch.db`（27 MB，存 API key/供应商配置）文件头为 `SQLite format 3`，**完全明文**；而 PathEditor 的 `~/.patheditor/patheditor.db` 是密文。据此判断本项目的加密没有必要。

本机实测（2026-11-08）：

| 库                            | 大小       | 前 16 字节                                        | 结论           |
| ----------------------------- | ---------- | ------------------------------------------------- | -------------- |
| `~/.cc-switch/cc-switch.db`   | 27,176,960 | `53 51 4c 69 74 65 20 66 6f 72 6d 61 74 20 33 00` | 明文           |
| `~/.patheditor/patheditor.db` | 393,216    | `6a 40 7f 68 51 c0 29 e4 b2 02 00 6a b6 d9 b3 aa` | SQLCipher 密文 |

### 1.2 收益与代价

**收益（可机械验证）**

| 项            | 现状                                                                                | 移除后                                  |
| ------------- | ----------------------------------------------------------------------------------- | --------------------------------------- |
| 依赖链        | `rusqlite → libsqlite3-sys → SQLCipher(C) → OpenSSL(C)`                             | `rusqlite → libsqlite3-sys → SQLite(C)` |
| 本地 GNU 构建 | 需 `OPENSSL_SRC_PERL` + `PERL5LIB` + `PATH` 追加 Git `usr\bin`（`CLAUDE.md:57-63`） | 不需要                                  |
| 首次全量编译  | 文档记载约 24 分钟、`target/` 8–9 GB                                                | 明显下降（实施后实测填数）              |
| CI            | 额外安装 NASM（`.github/workflows/ci.yml:53-58`）                                   | 删除该步                                |
| 测试约束      | 「一文件一测试」因 SQLCipher 栈溢出而强制                                           | 约束消失                                |
| 密钥代码面    | `db_key(32)` 派生、`key_hex`、`PRAGMA key`、密钥错误分支                            | 全部删除                                |

**代价（如实登记）**

- `~/.patheditor/patheditor.db` 变成任何进程可读的普通 SQLite 文件，其中包含 `env_values` 的历史快照值。
- 已存加密库无法被新版本读取，用户必须删除（见 §1.3）。
- 回退到 6.x 不可行：6.x 会对明文库执行 `PRAGMA key` 并报错。

### 1.3 用户裁断：不做迁移

用户明确：**不写任何迁移代码**，旧库由用户自行删除后重装新版本；理由是用的人少。

因此本设计**不含** `sqlcipher_export`、无 legacy 读取模块、无两阶段发布。新版本遇到旧密文库时的行为见 D8：明确报错并给出删除指引，**不静默重建空库**（沿用既有 fail-open 与「不静默重建」约束）。

## 二、决策

| 编号 | 决策                                                                                                                                  | 理由                                                                                                                         | 代价                                           |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| D1   | `rusqlite` 改为 `features = ["bundled"]`                                                                                              | 只编译 SQLite 自身，自带 FTS5/JSON1，无需 OpenSSL 与 NASM                                                                    | 失去全库加密                                   |
| D2   | 不实现数据迁移                                                                                                                        | 用户裁断；用户规模小                                                                                                         | 旧库历史丢弃，须手动删文件                     |
| D3   | `KeyMaterial` 只保留 `hash_salt`，删除 `db_key` 与 `key_hex()`                                                                        | `.pebak` 的 `payload_hash = BLAKE3(hash_salt ‖ json)` 与备份索引依赖该盐；`db_key` 仅 SQLCipher 用                           | 密钥文件里 32 字节成为遗留填充                 |
| D4   | 密钥文件**路径、文件名、长度、格式版本全部不变**（`~/.patheditor/keys/db.key.dpapi`，49 字节，`version(1)+reserved(32)+salt(16)`）    | 盐若变化，磁盘上既有 `.pebak` 的 `payload_hash` 全部失配 → 备份报「内容哈希与头部不一致」。保持不变 = 零迁移且旧备份继续可读 | 文件名 `db.key.dpapi` 名不副实，登记为已知遗留 |
| D5   | `.pebak`（DPAPI）与同步 bundle（Argon2id + ChaCha20-Poly1305）**保留**                                                                | 威胁模型不同：前者是静态文件可被拷走，后者跨设备传输、可能经网盘/邮件                                                        | 无                                             |
| D6   | 密钥文件 DACL 校验从 `db::open_default` 移到 `crypto::load_or_create_key`                                                             | DB 不再需要密钥，打开库不应为此创建文件；但盐被他人替换会导致备份不可读，守护仍需要                                          | `load_or_create_key` 多一次 ACL 读取（可忽略） |
| D7   | 版本号升 **7.0.0**，四处同步（`package.json` / `Cargo.toml [workspace.package]` / `src/gui/src-tauri/tauri.conf.json` / README 徽章） | 存储格式破坏性变更                                                                                                           | 需完整 CHANGELOG Breaking 段                   |
| D8   | 旧库检测：打开失败且文件头不是 `SQLite format 3` 时返回 `ErrorCode::Database`，消息含旧库路径与「删除/重命名后重启」指引              | 用户唯一需要的「迁移」就是删文件；GUI 已有 fail-open 降级通道，CLI 已有非零退出                                              | 新增一处错误分支与一条测试                     |
| D9   | `CURRENT_SCHEMA_VERSION` 保持 **4**                                                                                                   | 表结构与迁移语义未变，只有页级加密消失；升版本号会让 `migrate_to_current` 误判                                               | 无                                             |
| D10  | `probes/v6-sqlcipher/` 与 `.github/workflows/v6-sqlcipher-probe.yml` **保留不删**                                                     | 仓库硬约束：未经书面同意不得删除文件；且是 v6 的历史证据                                                                     | 文档需标注为历史                               |

## 三、影响面（实测）

### 3.1 生产代码

| 位置                                                                                   | 改动                                                                                                                                           |
| -------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/core/Cargo.toml:29`                                                               | `bundled-sqlcipher-vendored-openssl` → `bundled`                                                                                               |
| `src/core/src/db/mod.rs:44-82`                                                         | `Db::open_encrypted(path, key_hex)` → `Db::open(path)`；删除密钥长度校验、`PRAGMA key`、密钥校验查询、`cipher_memory_security`                 |
| `src/core/src/db/mod.rs:242-266`                                                       | `open_default()` 不再加载密钥、不再校验 DACL；只做「建/开普通库」+ D8 错误映射                                                                 |
| `src/core/src/db/crypto.rs`                                                            | 删除 `db_key` 字段与 `key_hex()`；`parse_material` 跳过 `[1..33]`；`create_key_locked` 仍写 49 字节；`load_or_create_key` 内挂 DACL 校验（D6） |
| `src/core/src/db/mod.rs:1-11`                                                          | 模块文档「SQLCipher 数据库句柄」→「SQLite 数据库句柄」；安全边界清单更新                                                                       |
| `src/core/src/db/search.rs:5`、`sync/merge.rs:3-4`、`maintenance.rs:267`、`dpapi.rs:3` | 注释中的 SQLCipher 表述修正；`dpapi.rs:3` 的「移植自探针」可保留（历史事实）                                                                   |
| `src/cli/src/commands/maintenance.rs:40,66`                                            | 注释里的密钥语义更新（`load_or_create_key` 现只提供盐）                                                                                        |
| `src/gui/src-tauri/src/lib.rs:71-88`、`src/cli/src/runtime.rs:51,59`                   | **无需改代码**：`open_default` / `open_existing` / `open_and_migrate` 签名不变                                                                 |

`KeyMaterial` 的其余消费方（`db/backup_index.rs`、`db/maintenance.rs`、`platform/backup/env.rs`、`platform/backup/restore.rs`、`platform/backup/tests.rs`、`src/cli/src/commands/env/tests.rs`）只使用 `material` 与 `hash_salt`，**签名不变，零改动**。

### 3.2 测试

- **51 个** `src/core/tests/*.rs`、共 **56 处**引用 `db::crypto::{key_hex, load_or_create_key}` 或 `Db::open_encrypted`（`grep -rl open_encrypted src/core/tests | wc -l` → 51；`grep -rn ... | wc -l` → 56）。
- 机械改动（可脚本化）：删 `key_hex` 导入；删不再使用的 `let key = load_or_create_key(...)`；`Db::open_encrypted(&p, &key_hex(&k))` → `Db::open(&p)`。
- **语义需重写**（4 个）：

| 文件                         | 现断言                            | 改为                                                                          |
| ---------------------------- | --------------------------------- | ----------------------------------------------------------------------------- |
| `db_key_lifecycle.rs`        | 密钥稳定 / 错误密钥打开失败       | 盐稳定、非全零、不同目录不同；删除「错误密钥」断言                            |
| `db_schema_open.rs`          | `cipher_version` 非空、错密钥失败 | 建库可重开；**写入非 SQLite 头后 `Db::open` 报错且消息含删库指引**（覆盖 D8） |
| `db_schema_v1_to_v2.rs`      | 手写 `PRAGMA key` 造 v1 加密库    | 直接建普通库写 `DDL_V1`                                                       |
| `db_schema_v2_to_current.rs` | 同上                              | 同上                                                                          |

`db_backups_index.rs` 的 `other_key` 断言**保持原样**：它验证的是「不同目录的盐 → `payload_hash` 不一致」，与 DB 是否加密无关，只需改打开方式。

- `db_backup_production.rs`、`db_search_self_heal.rs`、`db_current_snapshot_read.rs`、`db_path_save_state.rs`、`db_profile_apply*.rs`、`db_restore_snapshot.rs`、`bootstrap_initialize.rs`：只改打开方式。
- 「一文件一测试」规则（`CLAUDE.md:159`、`docs/dev/testing.md:17-22`）**作废**：栈溢出根因是 SQLCipher 全局状态。存量不合并（避免纯 churn），新测试可同文件多测试。
- `dacl_principal_check.rs` 不变（DACL 校验保留，只是调用点迁移）。

### 3.3 CI

- `.github/workflows/ci.yml:53-58` 删除 NASM 安装步。`release.yml` 未含 NASM/OpenSSL 步骤，无需改。
- `.github/workflows/v6-sqlcipher-probe.yml` 保留（D10）。
- `.cargo/audit.toml` 无 OpenSSL 条目，无需改白名单；移除依赖后应复核 `cargo audit` 仍为 0。

### 3.4 文档

| 文档                            | 需改位置（2026-11-08 行号）                                                                                                                           |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CLAUDE.md` / `AGENTS.md`       | 15（v6 数据层描述）、57-63（GNU 三行环境变量）、78-80（目录注释）、92（probes 说明）、126-129（sidecar/密钥/一文件一测试）、158-159（基线、测试约束） |
| `README.md`                     | 114、149-152、396、570、596-597、651-652                                                                                                              |
| `docs/dev/sqlite-storage.md`    | 8-9（sidecar/密钥）、25（一文件一测试）、55-60（OpenSSL 构建成本整节）                                                                                |
| `docs/dev/testing.md`           | 17-22（一文件一测试）、94-101（GNU 三行）、质量门基线数字                                                                                             |
| `docs/dev/architecture.md`      | 25-28；**顺带修正滞后事实**：现写 `CURRENT_SCHEMA_VERSION = 3`，实际为 4                                                                              |
| `docs/dev/data-and-security.md` | 11、15（备份与暴露面表述）；新增「DB 不加密」边界声明                                                                                                 |
| `docs/dev/interfaces.md`        | 76、93（退出码 5 原因收窄为 `.pebak`/bundle/口令）                                                                                                    |
| `SECURITY.md`                   | 37 现称「备份文件格式为纯文本，不加密」，与 `.pebak` 事实不符，一并更正                                                                               |
| `ROADMAP.md`                    | v6.2 段后追加 7.0.0 条目（交付后）                                                                                                                    |
| `CHANGELOG.md`                  | 新增 7.0.0 段，`### Breaking` 写明旧库必须删除、不可回退                                                                                              |

`CLAUDE.md` 与 `AGENTS.md` 同步后按仓库既有做法做**字节级**比较（`git hash-object`，不用 `cmp`）。

## 四、安全影响评估（诚实结论）

1. **威胁模型变化**：v6 的加密只覆盖「攻击者拿到 `~/.patheditor/patheditor.db` 但拿不到 DPAPI 密钥」。而 PATH 的真相源在注册表：`HKCU\Environment` 与 `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment` 对同用户/本机管理员本就可读。DB 只是副本，因此加密的边际保护面很窄。
2. **唯一实质损失**：`env_values` 中存的历史环境变量值（可能包含用户自设的 token）。这些值同时存在于注册表明文与 `.pebak`（DPAPI）中。v7 起 DB 副本也为明文，属于可接受降级；文档须写清。
3. **保留的加密**：`.pebak` 备份仍经 DPAPI 当前用户作用域保护；同步 bundle 仍为 Argon2id（m=64 MiB, t=3, p=1）+ ChaCha20-Poly1305，跨设备传输不因本变更降级。
4. **不宣称**：不得在 README/CHANGELOG 写「安全存储」「加密数据库」；也不得声称「等效于 v6 安全性」。

## 五、验收判据（机械可验证）

```bash
# 1. 依赖链中不再有 SQLCipher / OpenSSL（probes 除外）
grep -rn "bundled-sqlcipher" --include=Cargo.toml .            # 仅 probes/v6-sqlcipher/Cargo.toml
cargo tree -e normal,build -i openssl-src                      # nothing to print

# 2. 源码不再出现旧入口
grep -rn "open_encrypted\|key_hex\|PRAGMA key\|cipher_memory_security" src/
# 期望：无输出

# 3. 密钥材料只剩盐
grep -rn "db_key" src/ | grep -v "^src/core/src/db/crypto.rs:.*遗留"
# 期望：无输出（crypto.rs 中说明性注释除外）

# 4. 新库是明文 SQLite（集成测试内用 TempDir 断言）
#    读新建文件前 16 字节 == b"SQLite format 3\0"

# 5. 旧密文库行为：写入 16 字节随机头后 Db::open 报错，消息含 "旧版加密数据库"

# 6. 质量门
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run verify          # 前端与 Rust 全量
# E2E 不属于本次改动面，可沿用既有证据
```

## 六、风险与缓解

| 编号 | 风险                                        | 缓解                                                                          |
| ---- | ------------------------------------------- | ----------------------------------------------------------------------------- |
| R1   | 用户没删旧库就升级，看到「历史不可用」      | D8 的错误消息给出精确路径与删除步骤；GUI 已有顶部条幅通道                     |
| R2   | 删除依赖后 FTS5 未编进 bundled SQLite       | `rusqlite` 的 `bundled` 默认启用 FTS5；由 `db_search_fts` 等 6 个检索测试兜底 |
| R3   | 盐文件被误改 → 旧 `.pebak` 全部报哈希不一致 | D4 保持格式不变 + D6 保留 DACL 校验；`db_backup_production` 测试覆盖读回      |
| R4   | 用户回退到 6.x                              | CHANGELOG 明确「不可回退」；发布说明同步                                      |
| R5   | 基线数字失真                                | 实施后实测并只更新 `docs/dev/testing.md` 一处，其他文档引用                   |
| R6   | 批量改 51 个测试文件引入误改                | 用带 `--binary` 的 `sed`，改完跑 `git ls-files --eol src` 核对行尾与全量测试  |

## 七、未覆盖项（如实列出）

- 不提供任何历史导出/迁移工具（用户裁断）。
- 不改 `probes/v6-sqlcipher/` 与对应 workflow（需书面同意才能删）。
- 不重命名 `db.key.dpapi`、不引入新盐文件名（为了既不迁移又保持 `.pebak` 可读）。
- 不合并现有单测试文件。
- 不评估 GUI 是否需要「数据库未加密」的新 UI 提示——本次只依赖既有降级条幅；若要显式告知，另开特性。
