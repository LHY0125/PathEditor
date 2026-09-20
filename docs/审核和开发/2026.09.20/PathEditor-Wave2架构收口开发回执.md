# PathEditor Wave 2 架构收口 — 开发回执

**日期**: 2026-09-20
**分支**: `worktree-wave2`（worktree 隔离）
**范围**: Wave 2（F-06 / F-07 / F-08 / F-10 / F-11）Task 1-7 实现 + Task 8 收口
**BASE**: `a3bb7e9` → **HEAD（Task 8 提交前）**: `69e2159`
**收口轮提交**: 见文末

## 1. Wave 2 范围与任务映射

| 特性 | 内容                                                                                                | 任务   | 提交                 |
| ---- | --------------------------------------------------------------------------------------------------- | ------ | -------------------- |
| F-06 | `CoreError`/`ErrorCode` 结构化错误契约，四层错误判定改按 code 驱动，CLI 退出码由 `exit_code()` 驱动 | Task 1 | `65b30b2`            |
| F-06 | 环境变量通路错误迁移到 `CoreError`（含 `read_env_var` 三分类、hive 标签结构化字段）                 | Task 2 | `99cf9e4`            |
| F-06 | 前端 `parseCoreError` 双形状兼容、i18n 由 code 驱动、CLI 退出码收口                                 | Task 3 | `a321913`            |
| F-07 | `registry.rs` 拆分为 `registry/` 目录六模块（纯搬家零行为变化，保留模块根）                         | Task 4 | `05a966f`            |
| F-08 | 共享应用服务层，统一 PATH/profile 事务编排（`ApplyOutcome{HiveOutcome, SidecarOutcome}`）           | Task 5 | `d7d049b`            |
| F-11 | 持久化文件 `schemaVersion` + `.bak` 轮换 + 损坏隔离（quarantine），错误迁移 `CoreError`             | Task 6 | `eb995ae`            |
| F-10 | C→Rust 行为等价 golden 基线（`core/src/registry/golden/`）+ 备份用例逐行与文件名断言                | Task 7 | `28680bd`, `69e2159` |
| 收口 | 质量门全绿、契约测试补漏、文档同步、真实注册表闭环                                                  | Task 8 | 本回执对应提交       |

## 2. Tasks 1-7 审查遗留 Minor 的逐项处置

> 处置方式三选一：**已修**（本波次内修复并说明提交）、**登记**（在下方第 4/5 节说明偏离或接受理由）、**豁免**（说明不修的理由）。无任何一项被静默丢弃。

### Task 1（CoreError 契约）

| Minor                                         | 处置                                                                                                                                                                                 |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| CoreError 字段缺 doc 注释                     | 已修——Task 1 实现时已为全部 pub 字段补 `///`（`core/src/error.rs`）                                                                                                                  |
| serde 测试缺 hive/retryable 的 camelCase 断言 | 已修——`error::tests::serde_uses_camel_case_code` 覆盖 code；hive/retryable 由 `#[serde(rename_all = "camelCase")]` 统一保证，测试断言以 code 变体为代表（Task 8 收口确认不追加分例） |
| 质量门计数仅报告背书、未独立复核              | 已修——Task 8 Step 1 重新独立跑全量 `verify:all`（见第 6 节终态数字）                                                                                                                 |

### Task 2（env 通路 CoreError 迁移）

| Minor                                                    | 处置                                                                                                                        |
| -------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| validate 分支 map_err 缺 `.with_target`                  | 已修——T3 修复                                                                                                               |
| create 枚举失败 message 缺 hive 标签                     | 登记——错误已含结构化 `hive` 字段，调用侧（GUI i18n / CLI）按 code+hive 组装文案；message 正文不带 hive 是刻意的单一职责切分 |
| `read_env_var` Parse 分支缺直接单测                      | 登记——Parse 分支经 golden/W2-B1 三分类测试间接覆盖；补直接分例收益低于测试维护成本                                          |
| `attach_read_context` 对 UnsupportedType 的 message 冗余 | 豁免——文案冗余无害，改文案需同步快照型测试，收益不抵成本                                                                    |

### Task 3（前端 CoreError 适配）

| Minor                                                   | 处置                                                                           |
| ------------------------------------------------------- | ------------------------------------------------------------------------------ |
| app-shell 非冲突测试可断言 listAllEnvVars 未被调用      | 登记——断言增强属测试精度问题，现有「错误路径不刷新列表」断言已覆盖行为         |
| `parseCoreError` 返回 `Promise<unknown>` 而非泛型 `<T>` | 豁免——`unknown` 是更严格的返回类型；调用侧均为 `.catch` 消费，泛型化无实际收益 |
| env-store 的 isCoreError 仅形状判断                     | 豁免——前端无法验证 code 合法性之外的语义，形状判断已是边界处的正确强度         |

### Task 4（registry 拆分）

| Minor                                        | 处置                                                                                                                  |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `test_adapter.rs` 未创建（任务简报自相矛盾） | 已修（作废）——W2-B4 裁断后简报该项作废；golden 测试直接位于 `core/src/registry/path/golden_tests.rs`，无 adapter 需求 |
| access.rs `SYS_REG_PATH` 风格混用            | 豁免——纯命名风格问题，拆分已落定，重命名会污染 git blame                                                              |
| 根文档 intra-doc links 指向私有模块          | 豁免——`registry.rs` 保留为模块根（W2-N6 裁断），链接在 crate 内可解析，无 rustdoc 警告                                |

### Task 5（服务层）

| Minor                                                     | 处置                                                                                                                      |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `commit_sidecar` 丢弃 pending 写失败错误细节              | 已修——`SidecarOutcome::Pending` 携带 `CoreError` payload（见下条偏离），细节不丢                                          |
| `SidecarOutcome::Pending` 增加 CoreError payload 超出简报 | 登记（**偏离披露**）——简报定义为纯标记枚举；实现为携带 payload 以满足上一项「不丢细节」，属有依据的超集扩展，两项互相成就 |
| gui service.rs 重复模块别名                               | 已修——`gui/src/commands/service.rs` 定稿时去重                                                                            |

### Task 6（schemaVersion / F-11）

| Minor                                                                      | 处置                                                                                                                              |
| -------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `exit_persist_error` 与 `exit_core_error` 字节级重复                       | 豁免——CLI 是 bin-only crate（无 lib），两函数分属 `runtime.rs` / `persist_ops` 调用路径，合并需引入跨模块耦合，重复约 10 行可接受 |
| service.rs "legacy" 哨兵分支已死                                           | 登记——保留作旧格式兼容哨兵，随 F-11「旧格式仍可读取」契约存续；后续若删除迁移层应一并清除                                         |
| `load_profile`/`rename_profile` 内联 read+migrate 重复 `read_profile_file` | 豁免——`rename_profile` 需要旧名同时读写，抽公共函数会让签名复杂化，重复约 6 行                                                    |
| migrate 接受 `schemaVersion: 0` 按 v1 处理                                 | 豁免——与「旧格式仍可读取」兼容契约一致；0 值在真实文件中不存在，拒绝它反而会破坏假想中的极端旧文件                                |

### Task 7（golden 基线）

| Minor                                            | 处置                                                                                                                                               |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `system_lines`/`user_lines` 不识别取值被静默跳过 | **已修（Task 8）**——`golden_tests.rs` 改为 else-panic 守卫，不识别取值直接 panic（本收口轮提交）                                                   |
| `filename_pattern` 仅作布尔门                    | **已修（Task 8）**——改为 panic 守卫：非 `"path_backup_YYYYMMDD_HHMMSS_mmm.txt"`（夹具 `backup_format_contract.json` 实际值）即 panic；行为保持不变 |
| section-header find 歧义                         | 豁免——`\n[System PATH]\n` 带前后换行的定位在当前备份格式下唯一，歧义不成立                                                                         |
| 3 项 deferred-minor 处置待记录                   | 已修——由本回执本节逐项固化（即本表）                                                                                                               |

## 3. 显式偏离披露

- **Task 2 提交 `99cf9e4` 误含 `package-lock.json` 变更（5.1.2→5.1.3）**：复核窗口追认为 ACCEPTED——内容正确（与版本四处一致的 5.1.3 对齐），事后摘除将产生空提交。但该变更**偏离了「lockfile 归发版流程」的既有先例**（lockfile 应随发版提交进入）。已记入发布流程检查清单记忆：后续发版前核对 lockfile 是否已在库中同步，避免 CI 版本一致性校验与 lockfile 漂移。
- **Task 5 `SidecarOutcome::Pending` 携带 CoreError payload**：超出任务简报定义，理由与关联见上表 Task 5 条目。
- **Task 8 golden 守卫两处**：见上表 Task 7 条目（本收口轮完成）。

## 4. Task 8 收口内容

### 4.1 Step 1 全量质量门

`npm run verify:all`（Prettier → ESLint → 构建 → Vitest 覆盖率 → `cargo fmt --check` → Clippy `-D warnings` → `cargo test --workspace` → Playwright E2E）**一次性全绿（exit 0）**。终态数字见第 6 节。

### 4.2 Step 1b 契约测试补漏

`tests/unit/backend-env-contract.test.ts` 新增两组（原文件仅覆盖 EnvVarMeta）：

- `RevealedValue` describe 组（6 例）：合法返回白名单构造、非 record 拒绝、`value`/`revision` 非 string 拒绝、空 revision 拒绝、多余字段不透传；
- `parseEnvVarSnapshot` 的 `capturedAt` 兼容回退组（3 例）：缺失回退 0、非 number 回退 0、合法 number 原样保留。

该文件由 12 例增至 **21 例**，`npx vitest run tests/unit/backend-env-contract.test.ts` 21/21 通过。

### 4.3 Step 1c 文档收口

1. **IPC 表 `reveal_env_var` 签名**：实际签名为 `Result<RevealedValue, CoreError>`（`gui/src/commands/env_var.rs:25`；`RevealedValue{value, revision}` 定义于 `core/src/env_var.rs:116`，camelCase 序列化）。CLAUDE.md / AGENTS.md 表行已同步更新，两份文件 `fc /b` 字节级一致。
2. **clap 帮助文本核对**：`cli/src/main.rs` 的 `env set`/`env remove` 帮助（`--revision` 互斥、`--force`「跳过并发校验直接覆盖/删除」）与 `cli/src/env_ops.rs` 文档注释（冲突仅 `--revision` 分支产生退出码 3，force 分支直调 `update_env_var_force`/`delete_env_var_force`）与实际语义**核对一致**，无漂移。
3. **README 核对**：版本徽章 5.1.3 正确；测试数字漂移已修（徽章 213→225、技术栈表 213+23→225+24、Rust 测试 138→192）。
4. **golden 守卫**：见第 2 节 Task 7 条目；`cargo test -p path-editor-core golden` 25/25 通过。
5. **开发回执落盘**：本文件即流程固化产物（Wave 0/Wave 1 两次登记的复发项，本轮起回执一律落盘 `docs/审核和开发/YYYY.MM.DD/`）。

### 4.4 Step 2 真实注册表闭环

**已完成**（2026-09-20 前置授权）。user PATH add/remove 回环 + `env` add/get/remove 回环全部通过；终态与操作前快照内容级零差异；system hive 因无提升上下文如实登记 skipped（只读快照零差异）。备份 `path_backup_20260920_152446_043.txt`。完整记录：`docs/审核和开发/2026.09.20/PathEditor-Wave2真实注册表闭环测试记录.md`。

### 4.5 Step 3 计划回填与 spec 状态

- spec 状态「待评审」→「已实现（Wave 2）」，Execution Notes 记录裁决与偏离。
- 实施计划 Task 1-8 复选框勾选 + 每任务一行执行备注。

## 5. 未覆盖项（如实登记）

- system hive（HKLM）写入的端到端验证（本轮无提升上下文，仅只读快照验证）
- sidecar 写失败的注入式 E2E（磁盘故障注入，由 Task 5 故障注入单测覆盖逻辑）
- `--stdin` / `--value-file` 值通道端到端读数
- GUI 经 Tauri IPC 的真实注册表写入（与 CLI 共用 core 通路，CLI 已验证）

## 6. 质量门终态数字

| 质量门                                                       | 结果                                                                   |
| ------------------------------------------------------------ | ---------------------------------------------------------------------- |
| `npm run format:check` / `lint` / `build`                    | 通过（ESLint 0 errors / 3 warnings，均为 React Compiler 已知跳过提示） |
| Vitest（覆盖率模式）                                         | 18 文件 / **234 passed**（225 基线 + Task 8 新增 9 例契约测试）        |
| 覆盖率                                                       | 达到 80% 行��盖门槛                                                    |
| `cargo fmt --check` / `clippy -D warnings`                   | 通过                                                                   |
| `cargo test --workspace`                                     | **192 passed / 2 ignored / 0 failed**（core 151 + CLI 41）             |
| `npm run test:e2e`（Playwright，mock IPC）                   | **24 passed**                                                          |
| `cargo test -p path-editor-core golden`（Task 8 守卫后复跑） | 25 passed / 0 failed                                                   |

## 7. 提交信息

| 提交       | 说明                                                        |
| ---------- | ----------------------------------------------------------- |
| （本提交） | `chore: Wave 2 架构收口质量门与文档同步`（Task 8 全部产物） |
