# F-08 收口完成 + 死码清理 Implementation Plan（修订版 v2）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **v2 修订说明（2026-10-17，据开发窗口第 1 轮核对 N1–N10 + 审核窗口第 2 轮裁断）**：Task 1 增「注入缝」；Task 4 从「改 2 入口」重写为「6 入口全收口 + 删 7 处广播 + 删 3 死函数」；补入 N7/N8/N9/N10。
>
> **v3 修订说明（2026-10-17，据开发回执 + 审核窗口第 3 轮复审裁断）**：Task 5 增 Step 9（sidecar 状态提示，裁断 ④）；Task 8 增「文档同步已授权」+ Step 2.5（一次性对齐 `disabled.json` 解阻真机回归，裁断 ③）；真机回归阻塞的**根因更正**见复审裁断 §二（顺序敏感，非陈旧条目）。

**Goal:** 完成 Wave 2 遗留的 F-08 收口——让 `core::service::save_path_with_sidecar` 成为 GUI 与 CLI **唯一**的「保存 PATH + sidecar」编排（补写前备份与乐观并发），并删除前端 3 个死码模块与 3 个孤儿 IPC 命令。

**Architecture:** 改动落在 `core`（补全服务层 + 注入缝）、`gui`（接线 + 退役裸命令）、`cli`（6 入口全收口）、`src/`（`path-session.ts` 薄化 + 删死码）。**核心原则：core 负责写与事务，GUI/CLI 只做参数转换与呈现。**

**Tech Stack:** Rust（core / gui / cli workspace，edition 2021）、winreg 0.52、serde / serde_json、tauri 2.11、clap 4；前端 React 19 + TS strict + Zustand + Vitest + Playwright。

**Spec:** `docs/superpowers/specs/2026-10-17-f08-closeout-and-deadcode-removal-design.md`（含 D13–D16 / K7 增补）

## Global Constraints

- **提交与版本**：本波**不授权提交、不推送、不升版本号**（用户 2026-10-17：仅授权「改代码 + 真机回归」）。完成代码与质量门即可。
- **删除授权范围**：本波被授权的删除 = spec §3.0-A（3 前端模块 + 3 测试文件）+ §3.0-C（3 个孤儿 IPC 命令）+ **本轮新增**：CLI 侧 `verify_and_save` / `persist_snapshot` / `flush_pending_snapshot` 三个死函数。**不得再扩大**——不删任何 core 函数、不删 `src/core/` 其余模块、不删其它命令。
- **架构边界**：`gui` / `cli` 只做参数转换、命令分派、错误呈现；保护名单、`Unsupported` 类型、hive 写权限、名称与值合法性、revision 校验、**乐观并发校验**全部由 core 判定。
- **错误契约**：判定只认 `CoreError.code`（serde camelCase）；**不得**匹配 `message` 文本或 `[E_CONFLICT]` 前缀。
- **冻结分叉（K6）**：Task 1 到 Task 5 完成前，**禁止任何一侧单独修改保存编排**。
- **文档注释**：所有 `pub fn` / `pub(crate) fn` 必须有 `///` 文档注释。
- **代码风格**：UTF-8 / CRLF；TS 2 空格、Rust 与 TOML 4 空格；Prettier 单引号、尾逗号、100 列。无新增 `unsafe`。
- **注释语言**：代码注释一律中文。
- **测试命令**：CLI 单测 `cargo test -p patheditor-cli --bins`；core 单测 `cargo test -p path-editor-core <name>`；前端 `npx vitest run tests/unit/<file>`。
- **质量门**：`npm run verify:all`（Prettier → ESLint → 构建 → 覆盖率 80% → `cargo fmt` → Clippy `-D warnings` → Rust 测试 → Playwright）。
- **真实注册表**：**Task 1-7 全程不写真实注册表**（契约测试经 fake `PathHiveIo`，见 K7）；**仅 Task 8 真机回归**写真实注册表。
- **模块路径写法**：`core/src/registry/env_var.rs` 所在模块是私有的，**不得**写 `crate::registry::env_var::X`（`E0603`）；经 `crate::registry::X`（根 re-export）。
- **`MemoryHive` 位置**：`use crate::reg_store::memory::MemoryHive;`。
- **每任务结束**：`cargo fmt` + `cargo clippy --workspace --all-targets -- -D warnings` 零警告。

---

## 文件结构

| 文件                                                                       | 职责                                                                                                            |
| -------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `core/src/service.rs`                                                      | **Task 1** 加 `PathHiveIo` 缝 + `save_path_with_sidecar_io`；**Task 2** 备份；**Task 3** 并发 + `PathHiveWrite` |
| `gui/src/commands/service.rs`                                              | Task 5：`save_path_with_sidecar` 命令改新签名                                                                   |
| `gui/src/commands/{registry,scanner,backup}.rs`                            | Task 7：删孤儿命令包装                                                                                          |
| `gui/src/lib.rs`                                                           | Task 7：注册表增删命令                                                                                          |
| `cli/src/{runtime,main,import_export,profile_ops}.rs`                      | Task 4：6 入口收口 + 删 7 处广播 + 删 3 死函数                                                                  |
| `src/services/{backend,path-session}.ts`                                   | Task 5：新 IPC 封装 + 薄化                                                                                      |
| `src/config/default.json`                                                  | Task 5：删 `maxSystemLength` / `maxUserLength`                                                                  |
| `src/core/{path-manager,import-export,validation}.ts`                      | **删除**（Task 6）                                                                                              |
| `tests/unit/{path-manager,import-export,import-parity,validation}.test.ts` | **删除**（Task 6）                                                                                              |
| `tests/unit/app-store.test.ts`                                             | Task 5：修 `mockResolvedValueOnce` 队列错位（N7）                                                               |
| `core/src/registry/golden/README.md`                                       | Task 3：复核 `:62` 行号引用（N10）                                                                              |
| `CLAUDE.md` / `AGENTS.md` / `README.md` / `CHANGELOG.md`                   | Task 8                                                                                                          |

---

### Task 1: 加 `PathHiveIo` 注入缝（纯重构）+ 契约测试

**Files:**

- Modify: `core/src/service.rs`
- Test: `core/src/service.rs`（同文件 `#[cfg(test)] mod tests`）

**Interfaces:**

- Produces:
  - `pub(crate) trait PathHiveIo { fn read_enabled(&self, hive: EnvHive) -> Result<Vec<String>, CoreError>; fn write_enabled(&self, hive: EnvHive, paths: &[String]) -> Result<(), CoreError>; fn backup(&self) -> Result<(), CoreError>; }`
  - `pub(crate) fn save_path_with_sidecar_io(io: &dyn PathHiveIo, system: Option<Vec<PathEntry>>, user: Option<Vec<PathEntry>>) -> Result<ApplyOutcome, CoreError>`
  - 真实现 `struct RealPathHiveIo`（转调 `registry::load_*_paths` / `save_*_paths` / `backup::backup_registry(None)`）
  - `pub fn save_path_with_sidecar(...)` 转调 `save_path_with_sidecar_io(&RealPathHiveIo, ...)`（**签名暂不变**）

- [ ] **Step 1:** 定义 `PathHiveIo` trait + `RealPathHiveIo`；把 `save_path_with_sidecar` 的编排体抽到 `save_path_with_sidecar_io`（**纯重构，零行为变化**）。
- [ ] **Step 2:** 运行 `cargo test -p path-editor-core service`，确认既有测试仍全绿（重构无回归）。
- [ ] **Step 3:** 写 fake `PathHiveIo`（记录调用序列）+ 三条契约测试（**此时 backup/并发两条 RED**）：
  - `backup_called_once_when_write_happens`
  - `concurrent_mismatch_writes_nothing_and_no_backup`（**依赖 Task 3 的 `expected`，先写为 `#[ignore]` 或注释说明**）
  - `writes_are_sequential_system_then_user`
- [ ] **Step 4:** `cargo fmt` + `clippy -D warnings` 零警告。

> **K7**：契约测试经 fake `PathHiveIo`，**全程不碰真实注册表**。`registry/path.rs` **零改动**（缝定义在 `service.rs`）。

---

### Task 2: `core::service` 补写前备份（经缝）

**Files:** Modify `core/src/service.rs`；Test 同文件。

- [ ] **Step 1:** 在**确实将发生写入**（至少一 hive 有变更）时，经 `io.backup()` 备份（真实现 → `backup_registry(None)`，默认目录 → 走 `rotate_path_backups` 轮换，K3）。
- [ ] **Step 2:** 备份失败**不阻断**写入——`log::warn!` 并继续。
- [ ] **Step 3:** `cargo test -p path-editor-core service`，`backup_called_once_when_write_happens` 转 GREEN。
- [ ] **Step 4:** 变异测试——去掉 `io.backup()` 调用，确认该测试转红；恢复。
- [ ] **Step 5:** `cargo fmt` + `clippy -D warnings` 零警告。

---

### Task 3: `core::service` 补乐观并发（经缝 + 签名变更）

**Files:** Modify `core/src/service.rs`；Test 同文件；复核 `core/src/registry/golden/README.md:62`（N10）。

**Interfaces:**

- Produces:
  - `pub struct PathHiveWrite { pub entries: Vec<PathEntry>, pub expected: Option<Vec<String>> }`（serde camelCase）
  - `pub fn save_path_with_sidecar(system: Option<PathHiveWrite>, user: Option<PathHiveWrite>) -> Result<ApplyOutcome, CoreError>`
  - `save_path_with_sidecar_io(io, system: Option<PathHiveWrite>, user: Option<PathHiveWrite>)`
  - `apply_path_snapshot` / `apply_profile` 转调（`expected: None`）

- [ ] **Step 1:** 定义 `PathHiveWrite`；改 `save_path_with_sidecar` / `_io` 签名。
- [ ] **Step 2:** 写前经 `io.read_enabled(hive)` 与 `expected` 比对；不符 → `HiveOutcome::Failed(CoreError{ code: Conflict, .. })`，**不写、不备份、不产 pending**。
- [ ] **Step 3:** 更新 `apply_path_snapshot` / `apply_profile` 转调（`expected: None`）。
- [ ] **Step 4:** `cargo test -p path-editor-core service`，`concurrent_mismatch_writes_nothing_and_no_backup` 转 GREEN。
- [ ] **Step 5:** 变异测试——去掉并发校验，确认该测试转红；恢复。
- [ ] **Step 6:** 复核并校正 `core/src/registry/golden/README.md:62` 的行号引用（N10）。
- [ ] **Step 7:** `cargo fmt` + `clippy -D warnings` 零警告。

---

### Task 4: CLI 6 入口全收口 + 删广播 + 删死函数（N2/N3/N6/E）

**Files:**

- Modify: `cli/src/runtime.rs`（`load_and_save` `:70`、`load_operate_save` `:92`；**删** `verify_and_save` `:53`、`persist_snapshot` `:164`、`flush_pending_snapshot` `:194`）
- Modify: `cli/src/main.rs`（`clean_one` `:430`、`cmd_toggle` `:473`；删广播 `:368/407/426/507`）
- Modify: `cli/src/import_export.rs`（`cmd_import`；删广播 `:40`）
- Modify: `cli/src/profile_ops.rs`（`profile_apply`；删广播 `:67`）
- Test: `cli/src/*.rs`（`--bins`）

**Interfaces:**

- Consumes: `core::service::{save_path_with_sidecar, PathHiveWrite, HiveOutcome, SidecarOutcome}`

- [ ] **Step 1:** **6 个入口逐个**改为一次 `save_path_with_sidecar(Some/None PathHiveWrite)` 调用（带 `expected`）：
  - `load_and_save`（`runtime.rs:70`）、`load_operate_save`（`:92`）
  - `clean_one`（`main.rs:430`）、`cmd_toggle`（`:473`）
  - `cmd_import`（`import_export.rs`）、`profile_apply`（`profile_ops.rs`）
  - **N9**：构造 `PathHiveWrite { entries }` 时 `enabled: true` 的等价性**写死注释**（输入本就是 `enabled_paths` 启用集）。
- [ ] **Step 2:** **删除 N6 的 7 处调用方广播**（`main.rs:368/407/426/507`、`import_export.rs:40`、`profile_ops.rs:67`、`runtime.rs:119`）——广播已下沉 core。
- [ ] **Step 3:** **删除 3 个死函数**（E，已授权）：`verify_and_save` / `persist_snapshot` / `flush_pending_snapshot`；清理其 `use` 与 `SaveFn` 类型别名。
- [ ] **Step 4:** **退出码不变**——并发冲突仍 `exit_err`（退出码 1），**不**改成 3。
- [ ] **Step 5:** `cargo test -p patheditor-cli --bins` 全绿（含既有 CLI 保存测试）。
- [ ] **Step 6:** `cargo fmt` + `clippy --workspace --all-targets -- -D warnings` 零警告（**确认无 `dead_code`**）。

---

### Task 5: GUI 切换（`path-session.ts` 薄化）

**Files:**

- Modify: `gui/src/commands/service.rs`（`save_path_with_sidecar` 命令改新签名）
- Modify: `src/services/backend.ts`（新增 `savePathWithSidecar` + 形状校验）
- Modify: `src/services/path-session.ts`（薄化）
- Modify: `src/config/default.json`（删 `maxSystemLength` / `maxUserLength`）
- Modify: `tests/unit/app-store.test.ts`（**N7**：修 `:265-266/:281-282` 的 `mockResolvedValueOnce` 队列错位）
- Test: `tests/unit/`

**Interfaces:**

- Consumes: `backend.savePathWithSidecar({system, user, expectedSystem, expectedUser})`
- Produces: `savePathSnapshot` **签名与返回类型不变**（`app-store.ts:365` 不必改）

- [ ] **Step 1:** `gui/src/commands/service.rs::save_path_with_sidecar` 改新签名（接受 `PathHiveWrite`）。
- [ ] **Step 2:** `backend.ts` 加 `savePathWithSidecar`（含 `ApplyOutcome` 形状校验）。
- [ ] **Step 3:** `path-session.ts::savePathSnapshot` 改为：GUI 预检（权限 `blocked` + 组合长度 `warning`，**保留现逻辑**）→ 调 `backend.savePathWithSidecar` → 映射 `ApplyOutcome` → `SaveSnapshotOutcome`。
- [ ] **Step 4:** **删除** `path-session.ts` 内自理备份、并行写、store 内 pending 三块。
- [ ] **Step 5:** 删 `default.json` 的 `maxSystemLength` / `maxUserLength`（保留 `maxCombinedLength`）。
- [ ] **Step 6:** 修 `tests/unit/app-store.test.ts` 的 mock 队列（N7）。
- [ ] **Step 7:** `npx vitest run tests/unit/` 全绿 + `npm run test:coverage`（≥80%）。
- [ ] **Step 8:** `npm run build`（TS 类型检查）通过。
- [ ] **Step 9（复审裁断 ④）:** `path-session.ts` 把 `ApplyOutcome.sidecar` 的 `Pending`/`Failed` 映射为**状态栏非阻断提示**（避免静默持久化失败——`isModified` 收窄后不得连提示也丢）；补对应单测。

---

### Task 6: F-2 删除三个死码模块（3.0-A）

**Files:** Delete `src/core/{path-manager,import-export,validation}.ts` + `tests/unit/{path-manager,import-export,import-parity,validation}.test.ts`

- [ ] **Step 1:** 删前复核——`grep -rn "path-manager\|import-export\|core/validation" src tests e2e`，确认除待删测试外零引用（含 `validation.ts::join_path`/`split_path`）。
- [ ] **Step 2:** 删除 3 模块 + 4 测试文件。
- [ ] **Step 3:** `npx vitest run` 全绿；**`npm run test:coverage` 仍过 80%**。
- [ ] **Step 4:** `npm run build` 通过。

---

### Task 7: F-6 删除三个孤儿 IPC 命令（3.0-C）

**Files:**

- Modify: `gui/src/lib.rs`（删注册）
- Modify: `gui/src/commands/scanner.rs`（删 `scan_conflicts` `:4` / `scan_tools` `:8`）
- Modify: `gui/src/commands/backup.rs`（删 `get_appdata_dir` 包装）
- Modify: `gui/src/commands/registry.rs`（**若** Task 5 后确认 `save_system_paths`/`save_user_paths` 无调用方，删其包装）

- [ ] **Step 1:** 复核——`grep -rn "scan_conflicts\|scan_tools\|get_appdata_dir\|save_system_paths\|save_user_paths" src tests e2e`（**N8：范围含 `tests/unit/`**），确认前端零调用（`get_appdata_dir` 仅 e2e mock 除外）。
- [ ] **Step 2:** 删 gui 侧注册与包装。**不删** core 的 `backup::get_appdata_dir` / `scanner::{scan_conflicts,scan_tools}`。
- [ ] **Step 3:** `cargo build -p patheditor` + `cargo clippy --workspace --all-targets -- -D warnings` 零警告。
- [ ] **Step 4:** `npm run test:e2e` 全绿（e2e mock 若引用被删命令，同步清理）。

---

### Task 8: 文档同步 + 真机回归

**Files:** `CLAUDE.md` / `AGENTS.md` / `README.md` / `CHANGELOG.md`；真机回归。

- [ ] **Step 1:** 同步 `CLAUDE.md` / `AGENTS.md`：保存路径经 `save_path_with_sidecar`；删已退役命令；`cmp CLAUDE.md AGENTS.md` 无输出。**（用户 2026-10-17 已授权修改 `CLAUDE.md`/`AGENTS.md`——复审裁断 §三.①）**
- [ ] **Step 2:** `CHANGELOG.md`：**`### 变更`**（CLI 每次写 PATH 产备份，D15）、**`### 修复`**（clean 后 sidecar 陈旧，D14）；**不升版本号**，标「未发布」。`README.md` 同步 CLI 备份说明。
- [ ] **Step 2.5（复审裁断 ③，用户已授权）:** **一次性对齐 `~/.patheditor/disabled.json`**——把 `userSnapshot` 顺序对齐到注册表当前顺序，并移除已消失的 `Scoop\apps\git\current\cmd`。该文件当前全为启用项、无禁用状态（`system`/`user` 遗留列表为空），**不丢语义**。**操作前备份**（已有 `disabled.json.bak`）。**这是本波唯一允许触碰的用户数据，范围仅限此文件。** 目的：解阻 Step 3 真机回归（根因是顺序敏感，见复审裁断 §二；代码根治见新 spec `2026-10-17-path-conflict-order-sensitivity-design.md`）。
- [ ] **Step 3: 真机回归**（**唯一写真实注册表处**，须记录备份/前后快照/重启/回滚）：
  - GUI 保存 → 注册表生效 + `path_backup_*.txt` 产生；
  - CLI 保存（`add`/`clean`/`toggle`/`import`/`profile apply` 各一）→ 同上；
  - 并发冲突：外部改注册表后保存 → 拒绝且不写；
  - pending 补写：制造 sidecar 失败 → 下次命令补写。
- [ ] **Step 4:** `npm run verify:all` 退出码 0。
- [ ] **Step 5:** 落**开发回执**（`docs/审核和开发/2026.10.17/PathEditor-F08收口-开发回执.md`）。

---

## 验收（对应 spec §验收标准）

1. `cargo test --workspace` + `cargo test -p patheditor-cli --bins` 全绿。
2. `npm run verify:all` 退出码 0（覆盖率 ≥80%、E2E 30+ passed）。
3. `clippy -D warnings` 零警告；`fmt --check` 干净；`cmp CLAUDE.md AGENTS.md` 无输出。
4. Task 1 契约测试经 fake `PathHiveIo`，三条断言真机外可验；变异测试证明可证伪。
5. **CLI 6 入口 + GUI 都经 `save_path_with_sidecar`**；全仓无其它「读-比-写 + 备份 + sidecar」编排副本。
6. 删除后覆盖率仍过 80%；无新增 `unsafe`；新增 `pub fn` 均有 `///`。
7. 真机回归四项记录齐全。

## 范围外（本波不做）

F-3（IPC 错误契约统一）、F-4（`backup.rs` 拆分）、F-5（三层链）、F-7（双通路守卫）；组合长度约束的语义升级；`CLAUDE.md` 已登记的规格级已知问题；TOCTOU 窗口。
