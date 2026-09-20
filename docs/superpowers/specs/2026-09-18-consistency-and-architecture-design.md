# PathEditor 一致性与架构收口 Design

- **状态**：已实现（Wave 2）—— Wave 0/1 已实现并审核通过（2026-09-19，见 `docs/审核和开发/2026.09.19/`）；Wave 2 于 2026-09-20 完成实现与收口（见 `docs/审核和开发/2026.09.20/`）
- **日期**：2026-09-18
- **来源**：`docs/审核和开发/2026.09.18/PathEditor-全项目架构与对抗性复审报告.md`（结论 Changes Requested，0×P0 / 4×P1 / 7×P2）
- **范围**：全部 11 项（P1×4 + P2×7）
- **基线**：`main` @ `2c8117c`

## 1. 背景与目标

PathEditor 已完成 C/IUP → Rust/Tauri/React/CLI 的主体迁移，核心方向正确：Windows 操作集中在 Rust，GUI 与 CLI 共享 `core`，PATH 与通用环境变量分两条业务通路，前端 IPC 已集中到 `backend.ts`。

但对抗性复审确认了 11 项问题，集中在三个面：

1. **数据一致性**：编辑弹窗异步读值可能覆盖外部新值；CLI `--force` 语义与文档不符；CLI 注册表与 `disabled.json` 双写无失败恢复；注册表枚举/读取错误被静默跳过。
2. **可观测性与完整性**：错误仍是自由文本跨四层传播；持久化文件无 schema 版本化与损坏恢复。
3. **架构收口**：`registry.rs` 仍是 1101 行大模块；GUI 与 CLI 对同一持久化模型有两套事务编排；测试依赖真实 HKCU，导致完整 Rust 质量门无法运行；C→Rust 无行为等价基线。

**目标**：把这 11 项收口，使「全部环境变量管理」达到可以作为稳定接口对外承诺的成熟度。

**完成判据**：见 §7 验收标准；须逐条可勾选。

## 2. 术语

| 术语              | 含义                                                        |
| ----------------- | ----------------------------------------------------------- |
| hive              | Windows 注册表配置单元，本项目用 HKLM（系统）/ HKCU（用户） |
| revision          | 变量当前状态的摘要（FNV-1a 散列），用于 CAS 比对            |
| CAS               | compare-and-swap：读 revision → 比对 → 写，不一致则拒写     |
| sidecar           | 与注册表并行持久化的文件，即 `disabled.json` 与 PATH 快照   |
| pending / journal | 注册表写成功但 sidecar 写失败时保留的可重试状态             |
| 端口 / adapter    | 把系统调用抽象成 trait，生产用真实实现、测试用内存实现      |

## 3. 分期与依赖

复审报告给出「第一批（合并前必须处理）」与「第二批（架构收口）」。审核窗口在此之上做一处**顺序调整**，理由是依赖关系：

- **F-09 的注册表端口必须前置。** F-04 要求「增加可注入注册表适配器」来测试枚举中途失败；F-07 的拆分也要以端口为边界。不先做 F-09，F-04 无法写出故障注入测试，且完整 `cargo test --workspace` 仍会写真实 HKCU。

调整后的三个波次：

```text
Wave 0  注册表端口与列表失败语义（同一处切口，一起做）
  P0-F09  EnvHiveStore 端口 + WinregHive + MemoryHive；环境变量测试不再写真实 HKCU
  P1-F04  列表枚举/读取/解码失败不再静默；单 hive 内任一失败即 Err

Wave 1  数据一致性
  P1-F01  编辑弹窗完整值绑定 revision
  P1-F02  core 真正 force API + CLI 接线
  P1-F05  双 hive 快照契约措辞
  P1-F03  CLI 双写失败恢复（Wave 1 只做「失败不丢状态」的最小闭环）

Wave 2  架构收口
  P2-F06  结构化 CoreError
  P2-F07  拆分 registry.rs
  P2-F08  共享应用服务层
  P2-F10  C→Rust golden behavior matrix
  P2-F11  持久化 schema 版本化与损坏恢复
```

**F-03 的定位说明**：报告把它放在第一批，但它要求的「把 PATH 应用流程下沉到 core 的 application service」正是第二批 F-08 的内容。若严格分批，F-03 与 F-08 会互相等待。本 spec 的处理：**F-03 拆成两半**——「CLI 侧 sidecar 失败留下可重试状态」属 Wave 1（最小可用修复，不等 F-08）；「PATH 应用事务整体下沉、多 hive 语义统一」并入 Wave 2 的 F-08。这样 Wave 1 不产生跨批依赖，Wave 2 再做真正的收口。

## 4. 接口规格（逐项）

### F-01 编辑弹窗完整值绑定 revision

**现状**：`EditEnvVarDialog.tsx:43-63` 打开后读完整值，只写入本地 `value`；`env-store.ts:113-117` 的 `fetchFullValue()` 直接返回 `revealEnvVar()` 结果，**不带 revision**；`AppShell.tsx:291-300` 保存时用「当前快照的 `meta.revision`」配「弹窗里可能已过时的 `value`」。

**规格**：

- `reveal_env_var` 的返回契约扩展为携带读取时的 revision。Rust：`reveal_env_var(hive, name) -> Result<RevealedValue, CoreError>`，`RevealedValue { value: String, revision: String }`。
- `env-store.ts` 的 `fetchFullValue` 返回 `{ value, revision }`；`revealEnvVar` 同样。
- `EditEnvVarDialog` 记录「本值读取时的 revision」（`readRevision`）。
- **快照刷新时**：若该变量当前快照 revision ≠ `readRevision`，弹窗必须提示「变量已被外部修改，已重新加载」并刷新输入框，不得静默保留旧值。
- **提交前**：断言 `readRevision === 当前快照 revision`；不等则强制重新读取完整值，再允许保存。
- 冲突（core 返回 `[E_CONFLICT]`）时草稿保留策略与既有裁决一致（草稿保留，避免输入丢失）。

**回归测试**：`fetch` 延迟期间触发快照刷新，旧值返回后不得提交覆盖新 revision。

### F-02 core 真正 force API

**现状**：spec `2026-09-17-cli-env-vars-design.md:62-64` 承诺 `--force` 跳过校验直接覆盖；`cli/src/env_ops.rs:319-348` 的 `Concurrency::Force` 仍调用 `current_revision()` 再交给 `update_env_var`/`delete_env_var`；`core/src/registry.rs:431-450` 恒做 revision 比较。结果：`--force` 实为「重读一个 revision 再 CAS」，读与写之间的窗口仍会以退出码 3 失败。

**用户裁决（2026-09-18）**：**新增真正的 force API**，使实现与 spec 承诺一致。

**规格**：

- core 新增：
  - `pub fn update_env_var_force(hive, name, value) -> Result<(), CoreError>`
  - `pub fn delete_env_var_force(hive, name) -> Result<(), CoreError>`
- **语义**：最后写入者胜；**不做 revision 比对**。但**仍然**执行名称校验（保留名 `Path`、保护名单、`Unsupported` 类型、hive 写权限）——force 只豁免并发校验，不豁免安全校验。
- 文档注释必须写明这是「最后写入者胜」，**不得**称为原子操作。
- CLI `Concurrency::Force` 改调 force API；删除「重读 revision」的模拟实现。
- **退出码契约**：退出码 3 仅在 `--revision` 不匹配时出现；`--force` 永不产生退出码 3。
- 同步更新：spec、`README.md`、`AGENTS.md`/`CLAUDE.md` 的 `--force` 描述、clap 帮助文本。
- **测试**：并发变更测试——在读与 force 写之间修改变量，force 必须成功。

### F-03 CLI 双写失败恢复（Wave 1 部分）

**现状**：GUI `path-session.ts:162-212` 有 `_pendingSys/_pendingUser`；CLI `runtime.rs:56-105` 注册表写入后才调 `save_path_snapshot`，失败直接 `exit_err`；`runtime.rs:125-130` 的 `persist_snapshot` 失败即退出；`profile_ops.rs:45-67` 先写系统 PATH、再写用户 PATH、最后存快照，任一步失败留下半应用状态。

**规格（Wave 1）**：

- CLI 在 sidecar 写失败时，**必须**留下可重试状态（pending/journal 文件），而不是只打印错误后退出。
- pending 文件格式与 GUI 的 pending 语义对齐（同一份持久化模型不应有两种可靠性策略）；具体落盘格式由 Wave 2 的 F-08 服务统一定义时收敛，Wave 1 先实现「失败不丢状态」的最小闭环。
- 错误输出必须包含：注册表已改变、sidecar 未写、重试命令或提示。
- **退出码**：sidecar 失败应使用一个可与一般错误区分的退出码（沿用现有契约，不新增除非必要）。

**测试（故障注入）**：磁盘写失败、系统 PATH 成功/用户 PATH 失败、快照成功/注册表失败。

### F-04 列表失败不再静默

**现状**：`registry.rs:311` `key.enum_values().flatten()` 丢弃枚举错误；`registry.rs:317-322` 单项读取失败只 warning + `continue`；`registry.rs:327-334` 解码失败同样只 warning + `continue`。

**用户裁决（2026-09-18）**：**整个 hive 报错**。

**规格**：

- 移除 `Iterator::flatten()`，枚举错误不得被吞掉。
- 单个 hive 内，枚举 / 读取 / 解码任一失败 → **该 hive 返回 `Err`**；调用方明确知道结果不可用。
- `list_all_env_vars()` 的签名本来就是 `Result<EnvVarSnapshot, String>`；要改的是**语义**：不再是「返回部分结果的 `Ok`」，而是「同一 hive 内任一枚举/读取/解码失败即 `Err`」。错误携带 hive 与失败原因（F-06 落地后为 `CoreError`）。
- 日志 warning 保留，但**不再是唯一反馈**。
- **测试**（依赖 F-09 的内存 adapter）：枚举中途失败、单值读取失败、不支持类型，三种都必须让 hive 返回错误。

### F-05 双 hive 快照契约措辞

**现状**：`registry.rs:361-369` 先读 HKLM 再读 HKCU，无跨键事务，注释却写「保证快照一致」。

**规格**：

- 契约与注释改为「单次请求返回两个**接近时刻**的快照（best-effort）」，**删除**「一致快照 / 原子」措辞。
- 快照增加 `capturedAt`（或每个 hive 的 generation/revision），让调用方知道时刻。
- 若业务确需强一致，走**单 hive revision 的提交检查**，而不是在列表层声称跨 hive 一致。
- 本项为文档 + 契约字段的小改，不含事务实现。

### F-06 结构化 CoreError

**现状**：`gui/src/commands/*.rs` 全部 `Result<T, String>`；前端靠字符串包含 `[E_CONFLICT]` 分支；CLI 靠文本前缀映射退出码。

**规格**：

- core 定义 `CoreError`：

  ```rust
  pub struct CoreError {
      pub code: ErrorCode,        // 稳定枚举，机器可读
      pub operation: String,      // 出错的操作名
      pub hive: Option<EnvHive>,
      pub name: Option<String>,
      pub retryable: bool,        // 冲突可重试，权限拒绝不可
      pub message: String,        // 安全展示文案（中文，人工阅读）
  }

  pub enum ErrorCode {
      Conflict,
      ReservedName,
      Protected,
      UnsupportedType,
      PermissionDenied,
      NotFound,
      NameExists,
      InvalidName,
      InvalidValue,
      Io,
      Parse,
      Internal,
  }
  ```

- **Tauri**：命令返回可序列化错误对象（结构体），前端不再匹配中文文案。
- **CLI**：`code` → 退出码映射（`Conflict` → 3，其余 → 1，clap 参数错误仍由 clap 负责退出码 2）。
- **前端**：`code` → i18n key 映射（`src/i18n/locales/{zh-CN,en}.json`）。
- `String` 仅保留在日志与最终 fallback 展示。
- **兼容**：`[E_CONFLICT]` 前缀在过渡期可保留在 `message` 中，但判定必须改为看 `code`。

### F-07 拆分 registry.rs

**现状**：`core/src/registry.rs` 约 1101 行，同时承载 PATH 读写、PATH 清理、注册表类型、通用环境变量读写、revision、错误、测试隔离键。

**规格**：拆为目录模块

```text
core/src/registry/
  mod.rs          # 统一入口与类型导出
  path.rs         # PATH value 读写、长度、分割、清理
  env_var.rs      # 通用环境变量 CRUD、revision
  access.rs       # HKLM/HKCU 权限与 hive 定位
  conflict.rs     # 注册表侧冲突常量与构造（区别于 crate 根的 error.rs）
  test_adapter.rs # 测试用注册表端口（F-09）
```

- 拆分是**纯搬家**，不改行为；每个 `pub` 项的外部路径保持兼容（`mod.rs` 重导出）。
- 拆分后跑全量质量门，确认零行为变化。

### F-08 共享应用服务层

**现状**：GUI 在 `path-session.ts` 编排保存/pending/部分成功；CLI 在 `runtime.rs` 与 `profile_ops.rs` 重新编排读、比、写、快照、广播。共享底层函数，不共享用例服务 → 产生 F-03 的策略分叉。

**规格**：core 暴露以用例为单位的服务：

- `apply_path_snapshot(...)`
- `apply_profile(...)`
- `save_path_with_sidecar(...)`
- `retry_pending_path_state(...)`

- 每个服务返回**结构化结果**（每个 hive、注册表、sidecar 的状态）。
- 多 hive apply 必须明确 **atomic / best-effort / partial** 三种语义之一，并在输出与退出码中表达。
- GUI/CLI 只负责输入转换、权限展示、输出渲染；任何改变注册表与 sidecar 顺序/补偿/广播的逻辑都在同一处 Rust service。

### F-09 注册表端口（Wave 0，前置）

**现状**：`registry.rs:737-775` 测试通过 `HKEY_CURRENT_USER\Software\PathEditor\Tests\...` 建隔离键，Drop 时删除。不碰真实环境变量键，但仍写当前用户真实注册表；完整 `cargo test --workspace` 因此不能在审查/受限环境执行。

**规格**：

- 定义端口 trait：

  ```rust
  pub trait EnvHiveStore {
      fn writable(&self) -> bool;
      fn enum_names(&self) -> Result<Vec<String>, String>;
      fn get_raw(&self, name: &str) -> Result<RegValue, String>;
      fn set_raw(&self, name: &str, value: &RegValue) -> Result<(), String>;
      fn delete_value(&self, name: &str) -> Result<(), String>;
  }
  ```

- 生产：`WinregHive` adapter（封装现有 Winreg 调用）。
- 测试：`memory::MemoryHive` adapter（内存实现，可注入失败，位于 `#[cfg(test)]`）。

  > 命名勘误（2026-09-19，评审裁断 P-4）：本节草稿曾用 `RegistryStore`/`WinregStore`/`MemoryStore`，与 §3 波次表冲突。以实现采用的 `EnvHiveStore`/`WinregHive`/`MemoryHive` 为准。

- **所有测试不再写真实 HKCU**；完整 `cargo test --workspace` 可在任意环境运行。
- 该端口同时是 F-04 故障注入测试与 F-07 拆分的地基。

### F-10 C→Rust golden behavior matrix

**现状**：历史首版是 C/IUP（`src/registry.c`、`src/callbacks.c`、`src/main.c`，现已不在工作区）。当前测试只证明「Rust 符合当前测试」，没有可执行的迁移兼容矩阵。

**规格**：建立 golden tests：输入注册表快照 + 操作 → 期望注册表/文件/广播结果。覆盖：

- PATH 分割、空项、空白、重复项处理
- REG_SZ / REG_EXPAND_SZ 写回类型
- 用户/系统 PATH 权限失败时的 UI 行为
- 备份格式与恢复可用性
- profile、导入导出、禁用项在升级后的兼容性
- `WM_SETTINGCHANGE` 广播时机

旧 C 行为不必全部保留，但**每一处改变都要在迁移记录中标注原因**。

### F-11 持久化 schema 版本化与损坏恢复

**现状**：`disabled.json` 靠字段默认值兼容旧格式，无 `schemaVersion`、校验和、备份恢复、损坏隔离；`profiles/*.json` 直接反序列化为当前结构。

**规格**：

- 文件顶层增加 `schemaVersion`。
- 写入时保留上一份 `.bak`，或用可轮换 journal。
- 读取失败：移动到 quarantine 文件并给出恢复提示，**不要**只返回通用 JSON 解析错误。
- profile / disabled 快照做 schema migration 测试。

## 5. 关键设计决策

| #    | 决策                                         | 理由                                                                             | 放弃的选项                                |
| ---- | -------------------------------------------- | -------------------------------------------------------------------------------- | ----------------------------------------- |
| D-01 | `--force` 新增真正的 core force API          | 用户裁决；使实现与 spec 既有承诺一致                                             | 改文档承认 CAS；改名 `--refresh-revision` |
| D-02 | 列表枚举/读取失败时整个 hive 报错            | 用户裁决；「成功但不完整」是错误语义                                             | `complete=false` + 诊断的结构化快照       |
| D-03 | F-09 注册表端口前置到 Wave 0                 | F-04 的故障注入测试、F-07 的拆分边界、可运行的 `cargo test --workspace` 都依赖它 | 严格按报告分批，F-09 留第二批             |
| D-04 | `CoreError` 用 `code` 枚举做机器可读判定     | 终结「文案一变、分支静默失效」                                                   | 继续用自由文本 + 前缀匹配                 |
| D-05 | F-03 拆为 Wave 1 最小闭环 + Wave 2 并入 F-08 | 避免 Wave 1 依赖 Wave 2 的服务层，消除跨批等待                                   | 整体留到第二批                            |
| D-06 | `registry.rs` 拆分是纯搬家，不改行为         | 降低回归风险，拆分本身不夹带语义变更                                             | 拆分同时重构语义                          |

## 6. 安全边界

以下判定**只能**在 `core` 实现，GUI 与 CLI 一律透传，**不得**复制：

- 保留名（`Path`）、保护名单、`Unsupported` 类型、hive 写权限、revision 校验
- `--force` 只豁免 revision 校验，**不豁免**上述任何安全判定
- `Path` 仍走专用 PATH 通路，通用 EnvVar 通路不展示、不允许写入

## 7. 验收标准

Wave 0：

- [ ] `EnvHiveStore` trait + `WinregHive` + `MemoryHive` 就位。
- [ ] 测试不再写真实 HKCU；`cargo test --workspace` 在无注册表写入的环境下全绿。

Wave 1：

- [ ] F-01：能复现并证明「旧请求不会覆盖新 revision」；有对应回归测试。
- [ ] F-02：并发修改场景下 `--force` 行为与 spec/README/帮助完全一致；`--force` 不产生退出码 3。
- [ ] F-03：sidecar 写失败后有可观测、可重试、可恢复状态。
- [ ] F-04：注册表枚举/读取异常时，hive 返回错误，调用方知道结果不可用。
- [ ] F-05：契约与注释不再声称跨 hive 一致快照；快照含 `capturedAt`。

Wave 2：

- [ ] F-06：`CoreError` 贯通 core/Tauri/CLI/前端；退出码与 i18n 由 `code` 驱动。
- [ ] F-07：`registry.rs` 拆为目录模块，外部路径兼容，质量门全绿。
- [ ] F-08：GUI/CLI 共用 application service；多 hive 语义明确。
- [ ] F-10：golden behavior matrix 可执行；每处行为差异有记录。
- [ ] F-11：`disabled.json` / profile 有 `schemaVersion`、`.bak`、quarantine；有 migration 测试。

## 8. 不在本版范围

- **真实 Tauri / 注册表集成闭环测试**：需要专用测试账户与授权环境，本轮不执行；以 §9 风险登记。
- **敏感变量识别升级为真正的 secret 检测**：本轮保持名称启发式，发布说明必须写明「这是降低误暴露概率，不是秘密检测保证」。
- **Windows 注册表真正 CAS（命名互斥量/写后校验）**：本轮不引入；已知竞态窗口如实保留并措辞。
- **C 代码删除或回填**：C 源码仅存于 git 历史，本轮不做删除。

## 9. 风险

| #    | 风险                                            | 处置                                                 |
| ---- | ----------------------------------------------- | ---------------------------------------------------- |
| R-01 | F-06 结构化错误是跨四层契约变更，回归面大       | 分批推进；过渡期保留 `[E_CONFLICT]` 文案在 `message` |
| R-02 | F-07/F-08/F-09 是架构重构，可能引入 PATH 回归   | 拆分纯搬家；golden tests（F-10）先行或并行           |
| R-03 | F-02 的 force API 打破「core 并发校验契约唯一」 | 文档明确 force 是显式豁免；退出码契约区分            |
| R-04 | 本轮仍无法在普通开发机完成真实集成闭环          | 如实登记为未覆盖项；发布说明标注                     |
| R-05 | `disabled.json` 无 CAS，读-比-写仍有竞态        | 延续既有如实措辞，不宣称原子                         |

## 10. 已核实的代码事实（2026-09-18 基线）

> 「直读」= 本轮逐行读过源码；「侦察」= 由子代理汇报，未逐行复核。

### 10.1 错误类型（F-06 现状）

- `core/` 全库 **47 处** `Result<T, String>`；唯一例外 `core/src/fs.rs:19` 的 `atomic_write() -> std::io::Result<()>`（侦察）。
- 全库无 `thiserror` / `anyhow` / `CoreError`（侦察）。
- `ERR_CONFLICT` 是**唯一**结构化错误信号（直读 `registry.rs:18`）：

  ```rust
  pub(crate) const ERR_CONFLICT: &str = "[E_CONFLICT] 变量已被其他进程修改，请重新加载";
  ```

- `gui/src/commands/*.rs` 全部 `Result<T, String>`（直读 grep）。

### 10.2 `core/src/registry.rs`（1101 行，直读）

公开 API：

| 函数                 | 行  | 签名                                                        |
| -------------------- | --- | ----------------------------------------------------------- |
| `load_system_paths`  | 90  | `() -> Result<Vec<String>, String>`                         |
| `load_user_paths`    | 99  | `() -> Result<Vec<String>, String>`                         |
| `save_system_paths`  | 108 | `(Vec<String>) -> Result<(), String>`                       |
| `save_user_paths`    | 117 | `(Vec<String>) -> Result<(), String>`                       |
| `can_write_user`     | 122 | `() -> bool`                                                |
| `clean_path_entries` | 167 | `(Vec<PathEntry>) -> (Vec<PathEntry>, Vec<PathEntry>)`      |
| `clean_paths`        | 199 | `(Vec<String>) -> (Vec<String>, Vec<String>)`               |
| `validate_env_name`  | 262 | `(&str) -> Result<(), String>`                              |
| `validate_env_value` | 279 | `(&str, &str) -> Result<(), String>`                        |
| `list_all_env_vars`  | 365 | `() -> Result<EnvVarSnapshot, String>`                      |
| `reveal_env_var`     | 375 | `(EnvHive, &str) -> Result<String, String>`                 |
| `update_env_var`     | 399 | `(EnvHive, &str, &str, &str) -> Result<(), String>`         |
| `create_env_var`     | 460 | `(EnvHive, &str, &str, EnvValueKind) -> Result<(), String>` |
| `delete_env_var`     | 518 | `(EnvHive, &str, &str) -> Result<(), String>`               |
| `conflict_message`   | 21  | `() -> String`                                              |

内部项（`&RegKey` 或位置注入，专供测试用）：

| 函数                             | 行        | 说明                                    |
| -------------------------------- | --------- | --------------------------------------- |
| `load_paths` / `save_paths`      | 25 / 61   | PATH 读写，`(root, sub_path, label, …)` |
| `env_key`                        | 216       | 打开 hive 环境变量键                    |
| `hive_location`                  | 227       | `EnvHive -> (HKEY, sub_path, label)`    |
| `read_env_var` / `write_env_var` | 241 / 254 | 单值读写，收 `&RegKey`                  |
| `list_hive_env_vars`             | 294       | 单 hive 列举入口                        |
| `list_env_vars_in_key`           | 301       | 列举核心，收 `&RegKey`                  |
| `reveal_env_var_in_key`          | 382       | reveal 核心，收 `&RegKey`               |
| `update_env_var_in_key`          | 410       | 写入核心，位置注入                      |
| `create_env_var_in_key`          | 471       | 新建核心，位置注入                      |
| `delete_env_var_in_key`          | 524       | 删除核心，位置注入                      |

F-04 的静默点（直读）：

- `key.enum_values().flatten()` —— `registry.rs:311`
- 读失败 `log::warn!` + `continue` —— `319-322`
- 解码失败 `log::warn!` + `continue` —— `331-334`
- `Unsupported` 类型走空串 —— `328`

F-05：`list_all_env_vars` 的文档注释自称「保证快照一致」（`363-364`），实现只是 HKLM → HKCU 两次独立读取。

### 10.3 测试隔离现状（直读）

- `issue26_tests`（627-728）：`TempRegistryKey` + 一个 `#[ignore]` 的真实写测试。
- `env_var_tests`（730-1101）：`TempRegistryKey::new` 在 HKCU `Software\PathEditor\EnvVarTests` 下建唯一子键，`Drop` 递归删除（738-777）；`TEST_PATH_SUBKEY` 常量在 `899`。
- 结论：环境变量测试**写真实 HKCU**（不碰真实环境变量键），故 `cargo test --workspace` 无法在受限环境执行。

### 10.4 持久化文件（侦察，未逐行复核）

- `~/.patheditor/disabled.json`：`DisabledState { system, user, path_snapshot }`，**无** `schemaVersion`、**无** `.bak`；文件缺失/空 → 默认值，JSON 解析失败 → `Err`；写入走 `atomic_write`（`<path>.tmp` + rename）。
- `~/.patheditor/profiles/<name>.json`：`ProfileData { name, sys, user, created, modified }`，**无** `schemaVersion`；`list_profiles` 静默跳过不可读文件。
- `~/.patheditor/backups/path_backup_<timestamp>.txt`：纯文本，无版本头。

### 10.5 CLI `--force` 现状（侦察）

- `cli/src/env_ops.rs`：`Concurrency::Force` 分支仍调用 `current_revision()`（≈333-341）重新读取 revision 并传给 core（≈366-368、397）；`apply_concurrency`（≈116-122）按 `is_conflict` 决定是否 `exit_conflict`。
- 后果：`--force` 下读与写之间发生 TOCTOU 时，**仍会以退出码 3 失败**。
- 与 `docs/superpowers/specs/2026-09-17-cli-env-vars-design.md:107`（自称「`--force` 模式下不会产生退出码 3」）直接矛盾；`README.md:202`、`AGENTS.md:144`、`CLAUDE.md` 同文案。

## Execution Notes（Wave 2 回填，2026-09-20）

Wave 2（Task 1-7 + Task 8 收口）按计划实现完毕，提交链 `65b30b2..69e2159`（Task 8 收口提交见开发回执）。逐项裁决与偏离：

**审核窗口裁决（核对轮 W2-B/N）执行情况**：

- W2-B1（read_env_var 三分类）：采纳，`read_env_var` 内部返回 `CoreError` 三分类（Task 2）。
- W2-B2（故障注入方式）：采纳，输入校验（null/超长）强制注册表阶段失败；retry 保持 sidecar-only（Task 5 Step 4b）。
- W2-B3（Task 6 牵连文件 + Outcome serde）：采纳，全面迁移并列牵连文件；Outcome 补 `Serialize/Deserialize`（Task 6）。
- W2-B4（golden 目录）：采纳，统一 `core/src/registry/golden/`（Task 7）。
- W2-N1（flush 语义）：采纳分层方案——服务层诚实报错、CLI 策略层保持 best-effort 警告（Task 5）。
- W2-N2（PermissionDenied 分类）：采纳，`WinregHive::open` 固有方法单独迁移，`ErrorKind` 诚实分类（Task 2）。
- W2-N3（行号漂移）：采纳，按符号定位（Task 4）。
- W2-N4（双错误形状过渡 + exit_err 调用点）：采纳（Task 3）。
- W2-N5（read_env_var hive 参数）：采纳，结构化 `hive` 字段 + 调用侧文本包装（Task 2）。
- W2-N6（registry.rs 保留模块根）：采纳，`registry.rs` 保留作模块根（Task 4/5）。

**golden README 诚实标注（第 4/6 类）**：与旧 C 行为有意不同的 golden 用例，在 `core/src/registry/golden/README.md` 逐条标注「旧行为 / 新行为 / 改变原因」，不做静默替换。

**第 6 类（broadcast）推迟**：广播行为的 golden 化需要注入 WM_SETTINGCHANGE 观测点，成本高于收益；broadcast 语义未变，推迟到后续波次（Task 7 收口时确认，理由：广播是进程外副作用，无法在进程内 golden 断言中诚实验证）。

**lockfile 偏离**：Task 2 提交 `99cf9e4` 误含 `package-lock.json` 5.1.2→5.1.3 变更，复核窗口追认 ACCEPTED（内容正确、摘除即空提交），但偏离「lockfile 归发版流程」先例，已记入发布流程检查清单（详见开发回执第 3 节）。

**完整处置清单**：Tasks 1-7 全部 deferred-minor 的逐项处置见 `docs/审核和开发/2026.09.20/PathEditor-Wave2架构收口开发回执.md` 第 2 节。
