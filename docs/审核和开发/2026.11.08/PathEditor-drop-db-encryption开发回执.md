# PathEditor 移除数据库加密 — 开发回执

- **日期**：2026-11-08
- **对应 spec**：`docs/superpowers/specs/2026-11-08-drop-db-encryption-design.md`
- **对应计划**：`docs/superpowers/plans/2026-11-08-drop-db-encryption-implementation.md`
- **结论**：**实施完成，质量门全绿**。数据库改为标准 SQLite；`.pebak`（DPAPI）与同步 bundle（Argon2id + ChaCha20-Poly1305）保持不变；不包含任何数据迁移。

## 一、代码事实（供下一轮核对）

| 位置                                        | 现在的事实                                                                                                                                                                                     |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/core/Cargo.toml:28-29`                 | `rusqlite = { version = "0.40", features = ["bundled"] }`；未改版本号                                                                                                                          |
| `src/core/src/db/mod.rs:46-72`              | `Db::open(path)`：先 `has_sqlite_header`，非 SQLite 头直接 `ErrorCode::Database` 报错（消息含 `旧版加密数据库` 与文件名）；随后设 `journal_mode=WAL` / `foreign_keys=ON` / `busy_timeout=5000` |
| `src/core/src/db/mod.rs:227-254`            | `has_sqlite_header`：文件不存在或 0 字节视为可新建；读到前 16 字节与 `b"SQLite format 3\0"` 比较                                                                                               |
| `src/core/src/db/mod.rs:256-276`            | `open_default` = `crypto::data_home().join("patheditor.db")` + `Db::open`；不再加载密钥、不再校验 DACL；`open_existing` 同源路径                                                               |
| `src/core/src/db/crypto.rs:26-31`           | `KeyMaterial { hash_salt: [u8; 16] }`；`db_key` 与 `key_hex()` 已删除                                                                                                                          |
| `src/core/src/db/crypto.rs:196-221`         | `parse_material`：长度必须 49、版本必须 1，盐取自 `[33..49]`；前 32 字节被跳过                                                                                                                 |
| `src/core/src/db/crypto.rs:60-68`           | 新增 `data_home()`：`PATHEDITOR_DATA_DIR` 优先，否则 `config_dir()`                                                                                                                            |
| `src/core/src/db/crypto.rs:83-112`          | `load_or_create_key` 内新增 DACL 校验（仅 Windows，过宽只告警）                                                                                                                                |
| `src/core/src/db/crypto.rs:138-149`         | 创建时仍写 `version + 32 字节随机保留 + 16 字节盐`，**49 字节格式不变**                                                                                                                        |
| `src/cli/src/commands/env/tests.rs:555-579` | 新增 `data_home()`（`LazyLock`）并在 `cli_cmd()` 下发 `PATHEDITOR_DATA_DIR`                                                                                                                    |

## 二、偏离与补充（如实列出）

1. **删除过时测试头部注释**（计划外的小扩展）：49 个 `src/core/tests/*.rs` 开头断言「一文件一测试（SQLCipher）…第二个测试会 `STATUS_STACK_OVERFLOW`」，在 SQLCipher 退场后不再成立，逐条删除；`db_sync_merge.rs` 被删行截断的句子已补完整。存量测试**未合并**。
2. **`PATHEDITOR_DATA_DIR` 为实施中发现的必需项**：v6 的 `open_default` 借 `key_home()`（`PATHEDITOR_KEY_DIR`）顺带把库重定向到临时目录；数据库不再需要密钥后若不补一个数据目录开关，CLI 子进程测试会写用户真实库。已按 spec D6 的同源思路补齐并在 `docs/dev/testing.md` 登记。
3. `probes/v6-sqlcipher/` 与 `.github/workflows/v6-sqlcipher-probe.yml` 按计划保留未动。
4. **代码注释与用户可见文案不再带发布版本号**：把「v7 起 / v6 及更早 / 自 v7」一类表述改为「不加密 / 旧版 / 当前版本」，避免注释随版本漂移；`uuid` 的 `v7` 特性与 `comctl32 v6` 清单属无关版本号，保留。文档中的 CHANGELOG、ROADMAP、SECURITY 支持表与按日期归档的设计/计划文档仍按历史记录保留版本号。

## 三、质量门实测（2026-11-08，本机）

| 检查           | 命令                                                                          | 结果                                                                                              |
| -------------- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Rust 全量      | `cargo test --workspace`                                                      | **336 passed / 0 failed / 2 ignored**（62 suites）                                                |
| CLI 单测       | `cargo test -p patheditor-cli --bins`                                         | **72 passed**                                                                                     |
| Clippy         | `cargo clippy --workspace --all-targets -- -D warnings`                       | exit 0，无告警                                                                                    |
| 格式           | `cargo fmt --all -- --check`                                                  | exit 0                                                                                            |
| 前端           | `npm run format:check && npm run lint && npm run build`                       | Prettier 全过；ESLint 3 条既有 warning（`useVirtualizer`，无 error）；`tsc -b && vite build` 成功 |
| 前端单测       | `npx vitest run`                                                              | **40 files / 425 passed**                                                                         |
| 依赖退场       | `cargo tree -e normal,build -i openssl-src`                                   | `did not match any packages`（依赖图已无 OpenSSL）                                                |
| 旧入口清零     | `grep -rn "open_encrypted\|key_hex\|PRAGMA key\|cipher_memory_security" src/` | 无输出                                                                                            |
| SQLCipher 残留 | `grep -rn "bundled-sqlcipher" --include=Cargo.toml .`                         | 仅 `probes/v6-sqlcipher/Cargo.toml`                                                               |
| 新库明文       | `db_schema_open` 用例内断言文件头                                             | `b"SQLite format 3\0"` 通过                                                                       |
| 旧库拒绝       | 同上用例写入 v6 真实文件头样本                                                | 报错含「旧版加密数据库」+ 文件名；文件未被改动                                                    |

## 四、未覆盖项（如实列出）

- **E2E 未跑**：本次未改前端代码；沿用 2026-10-24 的 54 passed。
- **`cargo audit` 未取得结论**：本机 fetch `RustSec/advisory-db` 失败（`git-upload-pack` IO error），与代码无关；CI 的 `audit` job 仍需在推送后复验。
- **真实目录未触碰**：`~/.patheditor/patheditor.db` 仍是 v6 密文库，按用户裁断由用户手动删除；本轮回执中的所有验证都在 `TempDir` 内完成。
- **回退不可行**：6.x 打开明文库会在 `PRAGMA key` 处失败，已在 CHANGELOG 的 Breaking 段写明。

## 五、待用户执行

```text
1) 关闭 PathEditor
2) 删除 %USERPROFILE%\.patheditor\patheditor.db、patheditor.db-wal、patheditor.db-shm
3) 启动 v7 → 历史为空、PATH 编辑正常
4) 可选核对文件头：od -An -tx1 -N16 %USERPROFILE%\.patheditor\patheditor.db
   期望 53 51 4c 69 74 65 20 66 6f 72 6d 61 74 20 33 00
```

## 六、交付状态

- 未提交、未推送、未打 tag（仓库约束）。
- 工作区改动：81 个跟踪文件 + 2 个新增文档；全部文本文件为 CRLF（`Cargo.lock` 由 `.gitattributes` 固定 LF）。
- `CLAUDE.md` 与 `AGENTS.md` 已同步，`git hash-object` 两者一致（`a06a8349…`）。
