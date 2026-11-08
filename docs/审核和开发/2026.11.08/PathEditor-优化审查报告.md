# PathEditor 优化审查报告

- **日期**：2026-11-08
- **范围**：`src/core`、`src/cli`、`src/gui`、`tests`、CI 与当前态文档
- **方法**：只读静态度量（行数/函数长度/重复模式/依赖图/文档一致性）+ 已有质量门结果；**本轮未改代码**
- **质量门现状**：Rust 336 passed / 2 ignored，clippy 零告警，前端 40 files / 425，ESLint 0 error

## 一、结论概览

项目**不是**「屎山」：`TODO/FIXME` 全文仅 1 处，clippy 零告警，分层契约可机械验证，前后端规则没有双份实现（前端 `core/` 只做展示逻辑，保留名/权限判定全在 Rust）。

真正的风险不是存量乱，而是**新增功能的落点已经出现瓶颈**：

| 落点                               | 现状                                                                                | 风险                                           |
| ---------------------------------- | ----------------------------------------------------------------------------------- | ---------------------------------------------- |
| `app-store.ts`                     | 433 行，同时管 PATH 数据、undo/redo、tab、搜索、选择、权限、状态消息、加载/保存标志 | 每个新交互都会往里加字段，最终无人敢改         |
| `db/sync/import.rs::import_bundle` | 单函数 234 行                                                                       | 解析/校验/冲突/落库/报告纠缠，改动易回归       |
| `services/backend/`                | 1571 行手写 IPC 形状校验，**不在覆盖率门槛内**                                      | 安全边界（拒绝畸形响应）靠自觉，新增命令易漏测 |
| 集成测试                           | 56 个独立测试二进制，每份都重复 `TempDir + Db::open + migrate`                      | 编译/链接成本线性增长，样板掩盖真实断言        |

## 二、度量（2026-11-08 实测）

| 指标      | 数值                                                                                                 |
| --------- | ---------------------------------------------------------------------------------------------------- |
| Rust 源码 | 144 文件 / 27,555 行（含内联测试）                                                                   |
| core 各层 | platform 8,256 / db 6,757 / app 1,734 / domain 678                                                   |
| CLI       | 4,070 行；gui src-tauri 902 行                                                                       |
| 集成测试  | 56 文件 / 4,977 行；61 处 `TempDir`、56 处 `migrate_to_current()`                                    |
| 前端      | 66 文件 / 9,426 行；`services/backend` 1,571 行                                                      |
| 函数长度  | 8 个生产函数超过 80 行（不含同文件测试模块），最长 `import_bundle` 234 行                            |
| 债务标记  | `TODO/FIXME/HACK` 共 1 处                                                                            |
| 静默抑制  | `allow(dead_code)` 3 处、`allow(too_many_arguments)` 4 处、`type_complexity` 1 处                    |
| 依赖重复  | `dirs 5` vs `dirs 6`；`getrandom 0.2` vs `0.3` vs `0.4`；`base64 0.22/0.23`                          |
| 文档重复  | 测试基线出现在 README 徽章 + README 表 + CLAUDE/AGENTS 两处 + testing.md；schema 版本出现在 4 份文档 |

## 三、P1（建议下一轮就做）

### P1-1 覆盖率门槛漏掉安全边界

`vitest.config.ts:17` 的 `coverage.include` 只有 `core/**`、`store/**`、`hooks/**`。`services/backend/`（`parse-core.ts` 528 + `parse-data.ts` 640 + `index.ts` 403）承担「拒绝畸形 IPC 响应」的职责——这是 AGENTS 明确要求的红线，却不在 80% 门槛内。现有 `backend-*.test.ts` 5 个文件只是部分覆盖。

**动作**：把 `src/gui/web/src/services/backend/**` 加进 `include`；补 `parse-core`/`parse-data` 的负例（缺字段、类型错、未知 `code`）。

### P1-2 `import_bundle` 拆成四段

`src/core/src/db/sync/import.rs:130`，234 行、无中间返回类型。它把「包读取 → 头部/限额校验 → 逐实体合并与冲突 → 事务提交 → 报告」压在一层。

**动作**：拆 `parse_bundle` / `validate_bundle` / `apply_changes_tx` / `build_report`；每段单独测试（现有 `db_sync_import*` 只测端到端）。

### P1-3 `app-store` 拆分

`src/gui/web/src/store/app-store.ts` 433 行；字段已横跨「领域数据」（`sysPaths`/`userPaths`/`undoRedo`）、「视图状态」（`activeTab`/`searchQuery`/`selectedIndices`/`scrollToIndex`）、「运行时标志」（`isAdmin`/`pathCapabilities`/`isLoading`/`isSaving`/`statusMessage`）。`use-app-actions.ts`（257 行）已在承担编排，说明边界已经存在，只是没被 store 承认。

**动作**：拆 `path-store`（数据 + 保存/撤销）与 `ui-store`（tab/搜索/选择/状态条）；`app-store` 保留组合。**先补 store 现状测试再拆**（已有 `app-store.test.ts`、`app-store-save-race.test.ts` 可作护栏）。

### P1-4 测试样板与 56 个二进制

SQLCipher 退场后「一文件一测试」已无必要。现状 56 个 `tests/*.rs`，每个都是独立二进制，各自重复：

```rust
let dir = TempDir::new().unwrap();
let mut db = Db::open(&dir.path().join("x.db")).unwrap();
db.migrate_to_current().unwrap();
```

**动作**：新增 `tests/common/mod.rs`（`fn new_db(name) -> (TempDir, Db)`）；按领域合并二进制（`db_history`、`db_search`、`db_sync`、`db_schema`、`db_backup`…），目标 ≤ 10 个。只搬函数，不改断言。

### P1-5 跨语言契约加机器校验

`ErrorCode` 在 Rust 16 个变体、TS `ERROR_CODES` 16 项，当前一致，但靠人肉镜像；仓库已有 `tests/fixtures/path-capabilities.json` 的先例。

**动作**：Rust 侧单测输出 `tests/fixtures/error-codes.json`（或直接手写并由 Rust 测断言枚举一致），TS 侧断言 `ERROR_CODES` 与 fixture 完全相同。同时把「schema 版本 / 退出码」纳入同一机制。

## 四、P2（顺手做）

1. **死代码 3 处**：`db/backup_index.rs:42` 的 `KIND_PATH`、`:487` 的 `flatten_vars`（均未使用）；`tests/db_sync_export.rs:122` 的 `_assert_header_type` 是为保 import 而写的占位，直接删 import 更干净。
2. **`too_many_arguments` 4 处**：`cli/commands/env/cmd.rs:54,109`、`env/render.rs:43`、`db/sync/change_log.rs:233` → 各自的参数结构体；同时消掉 `bundle.rs:144` 的 `type_complexity`（返回结构体替代嵌套元组）。
3. **依赖去重**：`dirs 5 → 6`（API 相同，唯一能直接消掉的重复）；`getrandom 0.2 → 0.3/0.4` 需改一处调用（`getrandom::fill`）。`base64` 重复来自传递依赖，不动。
4. **SQLite 参数**：现在只设 `journal_mode=WAL` / `foreign_keys` / `busy_timeout`。WAL 下常规做法是 `synchronous=NORMAL`（崩溃只丢最近提交、不损坏）；保留策略删行后文件不缩，可在 `maintenance prune` 末尾按阈值 `wal_checkpoint(TRUNCATE)`。**属于取舍，不是缺陷**，写进文档再改。
5. **`platform/fs.rs` 拆包**：784 行里生产部分 266 行，同时做文件 IO 与 JSON/CSV/TXT 三种解析。拆 `fs/io.rs` 与 `fs/formats.rs`，导入导出测试天然按格式分开。
6. **文档数字单源**：基线数字现在有 5 个落点，schema 版本 4 个。建议 `scripts/check-consistency.mjs`（Node，无新依赖）校验：版本号 4 处一致、`CURRENT_SCHEMA_VERSION` 与文档一致、基线数字与 `docs/dev/testing.md` 一致。可挂进 CI，成本一次。

## 五、P3（不建议现在做）

- **不要引入仓储/ORM 抽象**：`db/` 的手写 SQL + CAS 是当前可读性的来源，抽象化会把并发语义藏起来。
- **不要改名 `db.key.dpapi`**：它现在只存盐，但磁盘上既有备份索引依赖同一盐；改名需要迁移，收益为零。
- **不要重排存量测试断言**：只合并 target / 抽 fixture，不改期望值。
- **`env_edit.rs`（生产 808 行）可拆但不必急**：职责是校验 + 注册表读写 + 广播 + 敏感规则，拆分收益中等，先做 P1 更划算。

## 六、建议执行顺序

| 批次    | 内容                                                | 预计影响面             |
| ------- | --------------------------------------------------- | ---------------------- |
| 第 1 批 | P1-5 契约 fixture、P1-1 覆盖率 include、P2-1 死代码 | 低风险，纯增益         |
| 第 2 批 | P1-4 测试合并 + `tests/common`                      | 只动测试布局，不改行为 |
| 第 3 批 | P1-2 `import_bundle` 拆分、P2-2 参数结构体          | 需要 `db_sync_*` 回归  |
| 第 4 批 | P1-3 `app-store` 拆分                               | 前端，需 E2E/单测护栏  |

## 七、整改状态（2026-11-08 第一批）

| 项                                 | 状态                                                                                                                                                                                                                                                                                                                        |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P1-1 覆盖率纳入 `services/backend` | **完成**：`vitest.config.ts` 的 `include` 已加入；实测行覆盖 81.33%，门槛 80 通过                                                                                                                                                                                                                                           |
| P1-2 `import_bundle` 拆分          | **部分**：拆出 `read_bundle_file` / `open_bundle`（234 → 约 165 行）；事务内逐条应用待下一批                                                                                                                                                                                                                                |
| P1-5 ErrorCode 契约 fixture        | **完成**：`tests/fixtures/error-codes.json`，Rust `error_codes_match_shared_fixture` 与 TS `ipc-contract.test.ts` 双端断言                                                                                                                                                                                                  |
| P2-1 死代码                        | **完成**：删 `flatten_vars`、`_assert_header_type`；`KIND_PATH` 实为在用（原 `allow(dead_code)` 已过期），去掉 allow 保留常量                                                                                                                                                                                               |
| P2-3 依赖去重                      | **部分**：`dirs 5 → 6` 已消重；`getrandom` 本仓升 0.3，0.2 仍由 `chacha20poly1305`/`argon2` 传递引入，需大版本升级才能消                                                                                                                                                                                                    |
| P2-6 文档数字单源                  | **完成**：`scripts/check-consistency.mjs` + `npm run check:consistency`，已接入 `verify` 与 CI                                                                                                                                                                                                                              |
| P1-3 `app-store` 拆分              | **完成**：改为 zustand 切片——`store/slices/path-slice.ts`（数据/撤销/存取）、`store/slices/ui-slice.ts`（tab/搜索/选择/权限/状态栏）、`store/app-store.types.ts`；`app-store.ts` 仅负责组合与 `initialize`。对外仍只有一个 `useAppStore`，65 处调用点零改动                                                                 |
| P1-4 测试合并 / `tests/common`     | **完成**：56 个独立测试二进制合并为 6 个聚合 root（`history` / `search` / `schema` / `backup` / `sync` / `contracts`），用例文件移至 `tests/suites/<group>/`；新增 `tests/common/mod.rs::new_db`。断言零改动，测试总数仍为 337 passed / 2 ignored；`cargo test --workspace` 的 target 数 62 → 12，本机全量耗时约 135s → 50s |
| P2-2 参数结构体（4 处）            | **完成**：`env set` / `env add` → `SetArgs` / `AddArgs`；`build_change_preview` → `ChangePreviewInput`；`record_remote_change_tx` → `RemoteChange`；顺带把 `bundle::split` 的五元组返回改为 `SplitBundle`，4 个 `too_many_arguments` 与 1 个 `type_complexity` 全部移除                                                     |
| P2-5 `fs.rs` 拆包                  | **完成**：`platform/fs/{mod,io,formats}.rs`——`io.rs` 负责文本读取/原子写入/目录黑名单/写文件，`formats.rs` 负责 JSON/CSV/TXT 解析与序列化，`mod.rs` 保留 `import_paths` / `export_path_entries` 与测试；对外路径仍为 `platform::fs::*`                                                                                      |

## 八、未覆盖与局限

- 本轮未做运行时剖析（内存/启动耗时/DB 查询计划），性能项只基于静态观察。
- 未检查 `pub` API 的外部消费面（无 `cargo public-api`），死代码判断以 `allow(dead_code)` 与实际引用为准。
- 未评估前端渲染性能（虚拟列表已存在），`AppShell.tsx` 478 行未逐段审阅。
- 测试合并会改变 CI 的 target 列表，需同步 `docs/dev/testing.md` 的 CI 步骤描述。
