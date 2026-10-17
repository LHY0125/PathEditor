# F-08 收口完成 + 死码清理 — 设计文档

- **日期**：2026-10-17
- **版本目标**：5.1.6（本轮**不升版本号、不提交**）
- **作者**：审核窗口
- **状态**：待开发窗口逐行核对
- **来源**：`PathEditor-架构评审报告.md` + `PathEditor-架构评审核对轮交接.md` + `PathEditor-架构评审裁断.md`
- **波次命名**：本波为 **Wave 3.0「F-08 收口完成」**，子波 **3.0-A / 3.0-B / 3.0-C**。与历史代号对应：Wave 0（注册表端口）/ Wave 1（数据一致性）/ Wave 2（架构收口，`core/src/service.rs` 即「F-08, Wave 2 Task 5」）。**F-1 本质是 Wave 2 的收尾**，故接续为 3.0，避免与 Wave 0/1/2 混号。

## 概述

架构评审发现：`core::service` 是 Wave 2 为「统一 GUI/CLI 的保存 PATH 编排」而建的服务层，但**迁移只做了一半**——GUI 至今仍跑 TS 版编排（`path-session.ts`），Rust 服务层 5 个公开函数里 3 个在生产里**零调用**；且两份编排**已经分叉**（备份 / 乐观并发 / 组合长度三项）。

本版做三件事，对应三个子波：

| 子波      | 内容                                                                                                                          | 授权                        |
| --------- | ----------------------------------------------------------------------------------------------------------------------------- | --------------------------- |
| **3.0-A** | **删除前端 3 个运行时零引用的模块**（`path-manager.ts` / `import-export.ts` / `validation.ts`）及其独占测试                   | 用户已授权删除              |
| **3.0-B** | **F-08 收口完成**：补全 `core::service::save_path_with_sidecar`（写前备份 + 乐观并发），GUI 与 CLI **都切换**到它成为唯一编排 | 用户已授权改代码 + 真机回归 |
| **3.0-C** | **删除 3 个前端零调用的孤儿 IPC 命令**（`scan_conflicts` / `scan_tools` / `get_appdata_dir`）                                 | 用户已授权删除              |

**为什么合并成一版**：3.0-A / 3.0-C 是纯删除（零行为风险），与 3.0-B 触及的文件不重叠（A 删 `src/core/*`，B 改 `path-session.ts` / `service.rs` / CLI，C 改 `gui/src/{lib,commands}`）。合并可省去两次「跑全量质量门」的开销。

**为什么本波值得做**：分叉的三项里**两条是安全相关**（写前备份、乐观并发保护）。维持双份 = 每次修 bug 只改一侧，分叉持续拉大。

## 背景与现状（已核实的代码事实）

| 事实                                                                                                                                 | 位置                                                                                                                        | 影响                            |
| ------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------- | ------------------------------- |
| `core::service` 模块自述目标：「把 GUI（`path-session.ts`）与 CLI（`runtime.rs`）各自编排的…用例编排下沉到此处，消除两端的策略分叉」 | `core/src/service.rs:1-5`                                                                                                   | 目标明确，未达成                |
| `save_path_with_sidecar` / `apply_path_snapshot` / `apply_profile` **生产侧仅被 gui 包装，前端零调用**                               | `core/src/service.rs:173/193/287`、`gui/src/commands/service.rs:13/22/37`、`gui/src/lib.rs:54-57`                           | 三个函数是**半成品迁移的死重**  |
| `commit_sidecar_snapshot` / `retry_pending_path_state` 被 CLI 调用                                                                   | `cli/src/runtime.rs:168/195`                                                                                                | 服务层**部分已接线**（CLI 侧）  |
| GUI 实际保存路径：`app-store.savePaths` → `path-session.ts`（TS 编排）→ 裸命令                                                       | `src/store/app-store.ts:365`、`src/services/path-session.ts:66`                                                             | GUI 走的是**旧编排**            |
| `save_path_with_sidecar` **不做**乐观并发校验                                                                                        | `core/src/service.rs:191-192`（注释明说）                                                                                   | 分叉点 1                        |
| `save_path_with_sidecar` **不做**写前备份；`save_paths` 内也不备份                                                                   | `core/src/service.rs:193`、`core/src/registry/path.rs:48-71`                                                                | 分叉点 2                        |
| GUI 保存**做了**写前备份                                                                                                             | `src/services/path-session.ts:129-134`（`backend.backupRegistry()`，默认 `customDir=null`）                                 | 分叉点 2 的另一侧               |
| 乐观并发（读-比-写）**同时存在于两处**：GUI 命令与 CLI 层                                                                            | `gui/src/commands/registry.rs:13-31`、`cli/src/runtime.rs:53-68`                                                            | 第三处重复                      |
| GUI 保存**做了**组合长度预检 `sys+user ≤ 32767`                                                                                      | `src/services/path-session.ts:114-117`，值在 `src/config/default.json:14-18`                                                | 分叉点 3（**见 K1**）           |
| 逐 hive 32767 上限**已在 core**                                                                                                      | `core/src/registry/path.rs:134`（经 `save_paths` ← `save_system_paths` ← `write_hive_registry` ← `save_path_with_sidecar`） | **非缺口**（见 K1）             |
| GUI 两 hive **并行**写；Rust 服务层**顺序**写                                                                                        | `src/services/path-session.ts:138-151`、`core/src/service.rs:202-209`                                                       | 时序差异（见 K2）               |
| `path_backup_*.txt` 轮换**仅默认目录生效**（`custom_dir.is_none()`），N4 裁断                                                        | `core/src/backup.rs:440`（`rotate_path_backups`）、`backup_registry`（`backup.rs:1228`）                                    | core 侧新备份须沿用（见 K3）    |
| `backup_registry(custom_dir: Option<String>) -> Result<String, String>`（**返回 `String` 错误**）                                    | `core/src/backup.rs:1228`                                                                                                   | F-3 证据之一                    |
| `get_appdata_dir()` 是 core 函数，被 `backup_base_dir` 内部使用                                                                      | `core/src/backup.rs:165`、`:147`                                                                                            | **F-6 删的是 IPC 命令，不是它** |
| `src/core/{path-manager,import-export,validation}.ts` **运行时零引用**，各有 Rust 实现                                               | 见 §3.0-A 清单                                                                                                              | 死码（三重核实）                |
| `registry/env_var.rs` 错误契约**混合**（`validate_env_name`/`validate_env_value` 返回 `String`）                                     | `core/src/registry/env_var.rs:215/232`                                                                                      | F-3 迁移范围含此模块            |
| `gui/src/commands/backup.rs` **单文件内即两契约**                                                                                    | `:6`（`String`）vs `:22/32/48/67`（`CoreError`）                                                                            | F-3 首选证据                    |

**已知未被本版触及的既有事实**（避免开发窗口误以为要一并改）：`RestorePreview.modified` / `RestoreOutcome.skipped` / `EnvBackupInfo.variable_count` 恒为 0、TOCTOU 窗口——均属规格级已知取舍（见 `CLAUDE.md`），**本版不动**。

## 已确认的决策

| #       | 决策                  | 选择                                                                                             | 理由                                                            |
| ------- | --------------------- | ------------------------------------------------------------------------------------------------ | --------------------------------------------------------------- |
| **D1**  | F-1 方向              | **A′：分步收口**（不选 B 冻结）                                                                  | 分叉含两条安全项；B 等于用测试固化错误状态                      |
| **D2**  | 长度预检归属          | 逐 hive 上限**已在 core，不动**；组合约束**留 GUI**；删 `default.json` 冗余两键                  | 见 §关键设计决策 K1                                             |
| **D3**  | 两 hive 写入模型      | **统一为顺序**（system→user）                                                                    | 确定性优于并行；契约测试好写                                    |
| **D4**  | 写前备份              | **core 内补**，调 `backup_registry(None)`（默认目录 → 轮换生效）                                 | 沿用 N4 裁断；见 K3                                             |
| **D5**  | 乐观并发              | **下沉 core**，`save_path_with_sidecar` 签名变更（接受期望值）                                   | 消除 gui/cli 两处重复                                           |
| **D6**  | pending 机制          | **统一磁盘文件**（`disabled::save_pending_path_snapshot` + `retry_pending_path_state`）          | 磁盘 pending 跨重启存活；GUI 不再在 store 携带                  |
| **D7**  | GUI 终态边界          | GUI **保留**：权限预检（`canWriteTarget`）、组合长度软提醒（`force` 放行）、i18n 文案映射        | `ApplyOutcome` 表达不了这三样；见 K4                            |
| **D8**  | CLI 对齐              | CLI **切换到** `save_path_with_sidecar`（`verify_and_save` + `persist_snapshot` 退化为单次调用） | **不切换则 GUI≠CLI，F-08 未达成**                               |
| **D9**  | F-2 删除              | 删 3 模块 + 3 独占测试文件                                                                       | 用户授权；见 §3.0-A                                             |
| **D10** | F-6 删除              | 删 3 个孤儿**IPC 命令**（gui 侧注册与包装）                                                      | 用户授权；core 函数不动                                         |
| **D11** | 波次命名              | Wave 3.0 / 3.0-A / 3.0-B / 3.0-C                                                                 | 接续历史 Wave 0/1/2                                             |
| **D12** | 本波提交              | **不提交、不推送、不升版本号**                                                                   | 用户本轮授权仅「改代码 + 真机回归」                             |
| **D13** | 注入缝（第 2 轮裁断） | `service.rs` 加 `PathHiveIo` + `save_path_with_sidecar_io`；**`registry/path.rs` 不动**          | 见 K7；契约测试需真机外可验，缝仿 `EnvHiveStore`                |
| **D14** | `clean` 修复          | `clean_one` 收口顺带修「clean 后 sidecar 陈旧」（既有 bug）                                      | 收口由构造修掉；**行为变化**，记 CHANGELOG `### 修复`           |
| **D15** | CLI 备份声明          | CLI 每次成功写 PATH 产 `path_backup_*.txt`（默认目录、保留 20）                                  | 与 env 侧对称；**非等价迁移**，记 CHANGELOG `### 变更` + README |
| **D16** | 删除授权扩展          | 删 CLI `verify_and_save` / `persist_snapshot` / `flush_pending_snapshot`                         | 6 入口收口后成死码；用户已扩权授权                              |

## 关键设计决策

### K1 —— 长度预检：不是缺口，是两件事分家

评审报告与核对轮交接都把「长度预检」记成「TS ✅ / Rust ❌」。追完调用链后**双方措辞都偏宽**：

- **逐 hive UTF-16 ≤ 32767** 是 **Windows 平台事实**，**core 已在执行**（`path.rs:134`，经 `save_path_with_sidecar` → `write_hive_registry` → `save_system_paths` → `save_paths` → `validate_and_join_paths` 到达）。**本轮零改动。**
- **`sys + user ≤ 32767` 组合约束**是 TS 自加的、**比 Windows 更严**的应用层约束（`path-session.ts:114-117`），**不是平台规则**。它属 GUI 的「有效 PATH 上限」**软提醒**（`force` 可继续，`path-session.ts:118`）。**不搬进 core**——搬它 = 引入 core 从未有过的新约束 = 行为变化，与「收口 = 行为等价迁移」相悖。

**附带清理**：`src/config/default.json` 的 `maxSystemLength` / `maxUserLength`（`:15-16`）与 core 常量重复，**删除**；`maxCombinedLength`（`:17`）**保留**。

→ **W-2 的「长度预检」步骤塌缩为空操作。** 开发窗口 A′ 的 4 步实质变为 3 步。

### K2 —— 顺序写

统一为 `system → user` 顺序写。并行只省一次注册表写的耗时（微秒级），却让「一个 hive 落地、另一个失败」的窗口变成竞态；顺序写让该窗口**显式可推理**，也便于契约测试断言中间态。

### K3 —— 备份必须沿用默认目录轮换（N4 裁断）

core 侧新增的写前备份**必须**调 `backup_registry(None)`（`backup.rs:1228`，`custom_dir = None`），从而走 `rotate_path_backups` 的**默认目录限定**（`backup.rs:440`）。**若传了自定义目录，该目录不参与轮换**（可能混有用户自己的文件）——这是 5.1.5 刚立的 N4 裁断，不得在收口时破坏。

### K4 —— 终态边界：GUI 不归零

方案 A 的终态**不是**「GUI 零逻辑」。GUI 保留三类 core 表达不了的东西：

1. **权限预检** `kind: 'blocked'`——`canWriteTarget(isAdmin, capabilities, target)`（`src/core/path-capabilities.ts:31`，报告 §五明确**要保留**的模块）；
2. **组合长度软提醒** `kind: 'warning'`（K1）；
3. **i18n 文案映射**（`ApplyOutcome` → `statusMessage`）。

即终态是「**core 负责写与事务；GUI 保留权限预检、组合长度软提醒与文案映射**」。

### K5 —— 契约测试先行

Wave 3.0-B 的 **Task 1 是无行为变化的契约测试**，先钉住「保存 PATH + sidecar」的目标语义（注册表终态 / 备份产生 / 并发冲突不写 / 顺序），后续 Task 2-5 逐步使其 GREEN。**没有它，后续每一步都无从验证等价性。**

### K6 —— 收口期间冻结分叉

从本波开工到 GUI + CLI 双双切换完成，**禁止任何一侧单独修改保存编排**（否则分叉在收口期间继续拉大）。经本辨析，`core::service` 的 3 个「死函数」在本波**将被接线**，不再是死码。

### K7 —— 契约测试经注入缝（第 2 轮裁断，消解 K5 与 Global Constraint 的冲突）

**问题（开发窗口 N1）**：K5 要求契约测试断言「备份产生 / 并发拒绝 / 顺序写」，Global Constraint 要求「Task 1-7 不写真实注册表」——**二者同时满足不了**：`save_path_with_sidecar` 无注入参数，一路硬编码到 HKLM（`service.rs:83` → `registry/path.rs:95/48`）；仓库既有的注册表测试惯例（`path.rs:327` `save_paths_keeps_expand_sz_in_isolated_key`，且为 `#[ignore]`）只存在于**参数化**的 `save_paths(root, sub_path, ..)` 层，**延伸不到** `save_path_with_sidecar`。

**裁断：加注入缝（不选「抽纯函数 + 端到端移真机」）**。理由：后者把**成功路径**推给手动真机回归——正是 5.1.5 用 SP-1 刚关掉的「关键写路径只有手工验证」缺口类型，不得复制到 PATH。

**缝的形态**（`pub(crate) trait PathHiveIo`，定义在 `service.rs`）：

```rust
pub(crate) trait PathHiveIo {
    fn read_enabled(&self, hive: EnvHive) -> Result<Vec<String>, CoreError>;
    fn write_enabled(&self, hive: EnvHive, paths: &[String]) -> Result<(), CoreError>;
    fn backup(&self) -> Result<(), CoreError>;
}
// 真实现 RealPathHiveIo → registry::load_*_paths / save_*_paths / backup::backup_registry(None)
// 公开入口 save_path_with_sidecar 转调 save_path_with_sidecar_io(&RealPathHiveIo, ..)
// 契约测试经 fake impl，零真实注册表
```

**三条断言（真机外可验）**：`backup_called_once_when_write_happens` / `concurrent_mismatch_writes_nothing_and_no_backup` / `writes_are_sequential_system_then_user`。

**不是过度设计**：真注册表 + 内存 store = 两个适配器 = 真实缝（`codebase-design` 判据）；且是仓库 `registry/env_var.rs` 的 `_with_store` 模式的直接复用。**`registry/path.rs` 零改动**，不触及 F-3 领地。

## 接口规格

### 3.0-B-1（core）：`save_path_with_sidecar` 补全

**签名（spec 级；开发窗口有实施自由度，唯一不变量见下）**：

```rust
/// 单个 hive 的写入请求：待写条目 + 乐观并发的期望当前值。
pub struct PathHiveWrite {
    pub entries: Vec<PathEntry>,
    /// 期望的注册表现值（**启用**路径）。None = 跳过并发校验。
    pub expected: Option<Vec<String>>,
}

/// 同时写注册表与 sidecar（顺序 system→user），含写前备份与乐观并发。
pub fn save_path_with_sidecar(
    system: Option<PathHiveWrite>,
    user: Option<PathHiveWrite>,
) -> Result<ApplyOutcome, CoreError>
```

**硬性不变量**：

1. **写前备份**：仅当确实将发生写入时，先调 `backup_registry(None)`；备份失败**不阻断**写入（与 env 侧 `WriteOutcome.backup` 同语义）。
2. **乐观并发**：`expected = Some(orig)` 时，读当前启用路径与之比对；**不匹配 → 该 hive `HiveOutcome::Failed(CoreError{ code: Conflict, .. })`，不写该 hive、不产生 pending**（与 `service.rs:82` W2-B2 注入机制一致）。
3. **顺序**：system 写完再写 user。
4. **`Err` 仅保留给预置失败**：单 hive 的读写失败一律以 `HiveOutcome::Failed` 在 `Ok(ApplyOutcome)` 中诚实表达（既有语义，`service.rs:64-71`）。
5. **sidecar/pending**：仅对写成功的 hive 提交快照；失败落 pending（磁盘）——**既有逻辑不动**。

**兼容**：`apply_path_snapshot` / `apply_profile` 转调新签名（`expected: None`）；`commit_sidecar_snapshot` / `retry_pending_path_state` 不变。

### 3.0-B-2（gui）：接线 + 退役裸命令

- `gui/src/commands/service.rs::save_path_with_sidecar` **改为新签名**（接受 `PathHiveWrite`），暴露给前端。
- GUI 保存路径改为调这一个命令（见 3.0-B-4）。
- ⚠️ **`save_system_paths` / `save_user_paths`（`gui/src/commands/registry.rs:13/23`）在 GUI 切换后成为孤儿**——它们的唯一 GUI 调用方是 `path-session.ts`。**处置随 3.0-C 的 F-6 清理**（先确认无其它调用方）。

### 3.0-B-3（cli）：`verify_and_save` + `persist_snapshot` 收口

- `cli/src/runtime.rs::verify_and_save`（`:53-68`）与 `persist_snapshot`（`:164`）**退化为对 `save_path_with_sidecar` 的单次调用**（带 `expected`）。
- `load_and_save`（`:70`）/ `load_operate_save`（`:92`）内的 `verify_and_save(...) + persist_snapshot(...)` 两步合并为一次 `save_path_with_sidecar`。
- `flush_pending_snapshot()`（`:194`）成为冗余（新函数内部已 `retry_pending_path_state`）——保留函数签名、体内转调，避免大改调用点。
- **CLI 退出码不变**：并发冲突仍是退出码 1（既有 `verify_and_save` 的 `exit_err` 语义），**不**改成 3（3 专属 `env --revision`）。

### 3.0-B-4（前端）：`path-session.ts` 薄化

`savePathSnapshot`（`src/services/path-session.ts:66`）保留**签名与返回类型不变**（`app-store.ts:365` 不必改），但内部改为：

1. GUI 预检：`canWriteTarget` 权限（`kind:'blocked'`）、组合长度软提醒（`kind:'warning'`，K1）——**保留现逻辑**；
2. 调 `backend.savePathWithSidecar({system, user, expectedSystem, expectedUser})`（新 IPC 封装）；
3. 把 `ApplyOutcome` 映射为 `SaveSnapshotOutcome`：`HiveOutcome::Failed(Conflict)` → 冲突文案；`SidecarOutcome::{Pending,Failed}` → 状态栏；`isModified` 由「保存后是否仍与草稿不等」派生。

**删除**：`path-session.ts` 内**自理备份、并行写、store 内 pending** 三块（改由 core 承担）。

### 3.0-A：F-2 删除清单

| 删除                        | 行数 | 独占测试（一并删）                                                     |
| --------------------------- | ---- | ---------------------------------------------------------------------- |
| `src/core/path-manager.ts`  | 54   | `tests/unit/path-manager.test.ts`                                      |
| `src/core/import-export.ts` | 237  | `tests/unit/import-export.test.ts`、`tests/unit/import-parity.test.ts` |
| `src/core/validation.ts`    | 35   | `tests/unit/validation.test.ts`                                        |

**删除前确认**：`validation.ts::join_path` / `split_path` 是否有测试外依赖（本次核实：`is_valid_path_format` 在 `src/` 零命中）。

### 3.0-C：F-6 删除清单

| 删除                                                                     | 位置                                                           |
| ------------------------------------------------------------------------ | -------------------------------------------------------------- |
| `scan_conflicts` / `scan_tools`                                          | `gui/src/lib.rs` 注册 + `gui/src/commands/scanner.rs:4/8` 包装 |
| `get_appdata_dir`                                                        | `gui/src/lib.rs` 注册 + `gui/src/commands/backup.rs:10` 包装   |
| （**若** GUI 切换后确认无调用方）`save_system_paths` / `save_user_paths` | `gui/src/lib.rs` 注册 + `gui/src/commands/registry.rs:13/23`   |

**不删**：`core/src/backup.rs:165 get_appdata_dir()`（被 `backup_base_dir` 内部使用）、`core/src/scanner.rs::{scan_conflicts,scan_tools}`（core 函数，保留）。**只删 gui 侧的注册与包装。**

## 安全边界

- **S1（真机回归授权）**：用户已授权真机注册表回归。**每次真机操作必须**：操作前备份、记录操作前后快照、记录重启结果、记录回滚结果（样本见 `docs/审核和开发/2026.09.18/`）。E2E 仍走 mock IPC，不写真实注册表。
- **S2（删除授权范围）**：本波**唯一被授权的删除**是 §3.0-A / §3.0-C 列出的文件与命令。**不扩大**：不删任何 core 函数、不删 `src/core/` 其余模块、不删其它 IPC 命令。
- **S3（备份轮换不越界）**：core 侧新备份只走默认目录轮换（K3），自定义目录不参与——沿用 N4。
- **S4（不提交）**：本波**不提交、不推送、不升版本号**。开发窗口完成代码 + 质量门即可。

## 验收标准

1. `cargo test --workspace` 全绿；`cargo test -p patheditor-cli --bins` 全绿。
2. `npm run verify:all` 退出码 0（含覆盖率 80% 行门槛、Playwright E2E）。
3. `cargo clippy --workspace --all-targets -- -D warnings` 零警告；`cargo fmt --check` 干净。
4. `cmp CLAUDE.md AGENTS.md` 字节级一致。
5. **契约测试（K5）**存在且真实执行：断言「写前备份产生 / 并发冲突不写且不产 pending / 顺序写」。
6. **GUI 与 CLI 都经 `save_path_with_sidecar`**：全仓无其它「读-比-写 + 备份 + sidecar」编排副本（`save_system_paths`/`save_user_paths` 命令若保留，仅作为**非保存路径**的兼容命令，且无并发校验职责）。
7. **可证伪性**：删掉 K5 契约测试或注释掉备份调用，测试必须转红（变异测试验证，参照 5.1.5 的 SP-1 手法）。
8. **删除后覆盖率**：`npm run test:coverage` 仍过 80%（删除改变分母，必测）。
9. **无新增 `unsafe`**；新增 `pub fn` / `pub(crate) fn` 均有 `///` 文档注释。
10. **真机回归**：至少一次端到端——GUI 保存（含备份产生）、CLI 保存、并发冲突拒绝、pending 补写——记录为回执 §真机回归。

## 范围外（本波不做）

- **F-3**（IPC 错误契约统一 `CoreError`）——报延后；本波只**记录**其首选证据（`gui/src/commands/backup.rs` 单文件内两契约）与范围（含 `registry/env_var.rs` 混合契约）。
- **F-4**（`backup.rs` 纯搬家拆分）、**F-5**（`registry/env_var.rs` 三层链）、**F-7**（双通路守卫测试）——延后波次。
- **组合长度约束的语义澄清**（是否升级为 core 硬约束）——登记为 open question，本波保留现状。
- 不碰 `CLAUDE.md` 已登记的规格级已知问题；不修 TOCTOU 窗口。
