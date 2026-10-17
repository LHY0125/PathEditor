# PathEditor 架构评审报告

- **评审日期**：2026-10-17
- **评审角色**：审核窗口
- **基准点**：`main` @ `9f9c0d8`（v5.1.5 发布、issue 回填之后）
- **评审方法**：`improve-codebase-architecture` 流程（范围界定 → 探索 → 候选 → 建议），证据全部来自对当前工作区源码的检索与阅读（非采信文档陈述）
- **产出目的**：交给**开发窗口**，作为下一波重构/优化的输入

> **本报告的边界**：只给**方向、证据与优先级**，**不给具体接口设计**。每个候选的接口形状留待开发窗口选定方向后的「grilling / 设计两遍」环节。文中的 `file:line` 均为本次实测。

---

## 一、评审方法与词汇（先对齐语言）

本报告用一套固定词汇描述「模块」，避免「组件 / 服务 / API / 边界」这类含糊说法。请开发窗口沿用同一套词：

| 术语                   | 含义                                                                                                                |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------- |
| **模块**               | 任何有「接口」和「实现」的东西——函数、结构、crate、跨层切片都算                                                     |
| **接口**               | 调用方为了正确使用它**必须知道的一切**：签名，还包括不变式、顺序约束、错误模式、必需配置、性能特征                  |
| **深度**               | 接口处的**杠杆**：调用方（或测试）每学一个单位的接口，能驱动多少行为。小接口 + 大实现 = 深；接口跟实现一样复杂 = 浅 |
| **缝（seam）**         | 可以「不改这里却改变行为」的位置；接口所在之处                                                                      |
| **适配器（adapter）**  | 缝上满足接口的具体实现。「一个适配器 = 假设的缝，两个适配器 = 真实的缝」                                            |
| **杠杆（leverage）**   | 深度给调用方的回报：一份实现，在 N 个调用点、M 个测试上摊薄                                                         |
| **局部性（locality）** | 深度给维护者的回报：改动、缺陷、知识、验证集中在一处，而非散落各调用方                                              |

**删除测试（deletion test）**：设想删掉这个模块——复杂度是**消失**了，还是**又出现在 N 个调用点**？「消失」= 它是直通层；「转移」= 它值这个位置。

本报告对每个候选都要过一遍删除测试。

---

## 二、现状架构（as-is）

### 2.1 crate 分层（**这一层是干净的**）

```
┌─────────────────────────────────────────────────────────────┐
│  React 19 + TS strict  (src/)                                │
│    components / hooks / store(Zustand)                       │
│         │  只经 src/services/backend.ts 调 IPC               │
└─────────┼───────────────────────────────────────────────────┘
          │ Tauri IPC（40 个 #[tauri::command]）
┌─────────┼──────────────────┐
│  gui/（crate `patheditor`） │  ← 薄包装：参数转换 + 错误透传
└─────────┼──────────────────┘
          │ 直接调 core
┌─────────┴───────────────────────────────────────────────────┐
│  core/（crate `path-editor-core`，**零 Tauri 依赖**）         │
│    依赖：serde / winreg / windows-sys / dirs / chrono / log  │
└─────────┬───────────────────────────────────────────────────┘
          │ 直接调 core（同一个 crate）
┌─────────┴──────────┐
│  cli/（bin-only）   │  ← 薄：Clap + 命令分派 + 错误呈现
└────────────────────┘
```

**crate 级缝是成立的**：`core` 不依赖 Tauri（可用 `cargo tree` 佐证），`gui`/`cli` 都是薄适配器。**风险不在这一层。**

### 2.2 core 内部模块（一屏看清）

| 模块                                                                          | 生产行数\* | 职责                                                                  | 错误契约     |
| ----------------------------------------------------------------------------- | ---------- | --------------------------------------------------------------------- | ------------ |
| `registry/env_var.rs`                                                         | ~958       | 通用环境变量 CRUD（保留/保护/敏感/revision）                          | `CoreError`  |
| `backup.rs`                                                                   | ~1288      | config.ini / env 备份 / path 备份 / 轮换 / 列表 / 恢复预览 / 恢复执行 | `CoreError`  |
| `fs.rs`                                                                       | ~557       | 受限读文件 + 导入导出（json/csv/txt）                                 | **`String`** |
| `profiles.rs`                                                                 | ~499       | 配置 CRUD                                                             | `CoreError`  |
| `disabled.rs`                                                                 | ~316       | sidecar（disabled.json / 快照 / pending）                             | `CoreError`  |
| `service.rs`                                                                  | ~293       | 应用服务层（F-08：统一 GUI/CLI 的「保存 PATH + sidecar」编排）        | `CoreError`  |
| `reg_store.rs`                                                                | ~324       | `EnvHiveStore` trait + `WinregHive` / `MemoryHive`                    | 混合         |
| `scanner.rs`                                                                  | ~218       | 单次枚举、冲突与工具清单                                              | **`String`** |
| `registry/path.rs`                                                            | ~201       | PATH 读写 / 拆分 / 拼接 / 校验 / 清理                                 | **`String`** |
| `env_var.rs`                                                                  | ~403       | 契约（保留/保护/敏感判定、revision、preview 净化）                    | —            |
| `persist.rs` / `system.rs` / `error.rs` / `capabilities.rs` / `path_entry.rs` | ~490       | 落盘/系统/错误/能力/契约                                              | 混合         |

\* 生产行数 = 文件总行数减去 `mod tests` 之后的量。**巨头多为测试撑起来的**：`backup.rs` 3319 行里约 2031 行是内联测试；`registry/env_var.rs` 1753 行里约 795 行测试；`cli/env_ops.rs` 1881 行里约 1225 行测试。**「文件很大」本身不是问题**——问题是下面的内聚性与重复。

### 2.3 前端 `src/` 分层

```
components/  ← 渲染
hooks/       ← useAppActions(256) / useKeyboard / usePathValidation
store/       ← Zustand：app-store(424, PATH) / env-store(266, 环境变量) / theme-store
services/    ← backend.ts(489, 唯一 IPC 边界+运行时形状校验) / path-session.ts(213, 保存编排)
core/        ← 声称「纯逻辑、零框架依赖」
```

---

## 三、结论速览

| #       | 发现                                                                                                                   | 类型              | 删除测试                            | 建议强度           | 优先级 |
| ------- | ---------------------------------------------------------------------------------------------------------------------- | ----------------- | ----------------------------------- | ------------------ | ------ |
| **F-1** | 存在**两份**「保存 PATH + sidecar」编排：TS 的 `path-session.ts`（活的）与 Rust 的 `core::service`（半成品、生产死码） | 重复 + 半成品迁移 | 部分：CLI 仍依赖其中 2 个函数       | **Strong**         | P0     |
| **F-2** | 前端 3 个模块**只被自己的测试引用**，运行时零调用（`path-manager.ts` / `import-export.ts` / `validation.ts`）          | 死码（夹具）      | 删除 → 复杂度消失                   | **Strong**         | P0     |
| **F-3** | 同一条 IPC 缝speaks **两种错误契约**（22 个 `Result<_, String>` vs 9 个 `Result<_, CoreError>`）                       | 缝不一致          | ——                                  | Worth exploring    | P1     |
| **F-4** | `backup.rs` 是**低内聚上帝模块**：4 类职责挤在一个模块，局部性差                                                       | 内聚性            | 删除 → 复杂度转移（保留），但应拆分 | Worth exploring    | P2     |
| **F-5** | `registry/env_var.rs` 每个操作 **3 层包装链**（`X`/`X_with_store`/`X_in_store`），中层偏薄                             | 浅层堆积          | 中层是文档化不变式的载体，可保留    | Worth exploring    | P3     |
| **F-6** | **7 个已注册的 IPC 命令前端从不调用**（含 F-1 的 4 个 service 命令）                                                   | 死接口面          | 删除 → 复杂度消失                   | **Strong（清理）** | P1     |
| **F-7** | `EnvVar` 与 `PathEntry` **双通路**并存（有意设计，但前端成本翻倍）                                                     | 已接受成本        | ——                                  | Speculative / 记录 | P3     |

**一句话结论**：**crate 分层是健康的，无需推翻重来；真正的问题集中在「一次未完成的迁移」（F-1）与被它拖出来的死码簇（F-2 / F-6），以及一个低内聚的 `backup.rs`（F-4）。** 不需要架构级重构，需要的是**把半成品迁移收口 + 删死码**。

---

## 四、逐项发现

### F-1【Strong / P0】两份「保存 PATH + sidecar」编排 —— F-08 迁移半途而废

**证据链**

1. `core/src/service.rs` 模块文档自述目标（`service.rs:1-5`）：

   > 把 GUI（`path-session.ts`）与 CLI（`runtime.rs`）各自编排的「读 → 写注册表 → 广播 → 落 sidecar 快照 → 失败落 pending」用例编排下沉到此处，**消除两端的策略分叉**。

2. 实际调用图（本次全仓 grep 实测）：
   - **活的**：`commit_sidecar_snapshot` + `retry_pending_path_state` —— 被 `cli/src/runtime.rs:168 / :195` 调用。
   - **死的（生产）**：`save_path_with_sidecar` / `apply_path_snapshot` / `apply_profile` —— **仅**被 `gui/src/commands/service.rs` 包装，而这 4 个 Tauri 命令在 `src/` 与 `e2e/` 中**零调用**（见 F-6）。

3. GUI 的**实际**保存路径根本没走它：
   `src/store/app-store.ts:365` → `savePathSnapshot`（`src/services/path-session.ts:66`）→ `backend.saveSystemPaths / saveUserPaths / broadcastEnvChange / savePathSnapshot`。
   即 GUI 用的是**裸命令** `save_system_paths` / `save_user_paths` / `save_path_snapshot`，编排逻辑全在 **TypeScript** 里。

4. **两者并不等价**——所以收口不是「拨开关」：
   | 职责 | `path-session.ts`（活的 TS） | `core::service::save_path_with_sidecar`（死的 Rust） |
   | --- | --- | --- |
   | 写前备份 | ✅ `backend.backupRegistry()` | ❌ **无**（`registry/path.rs:48 save_paths` 内部也不备份） |
   | 乐观并发校验（`original`） | ✅ 传给 save 命令 | ❌ 明确不做（`service.rs:191-192` 注释） |
   | 长度上限校验 | ✅ 客户端算 | ❌ 靠 `save_paths` 内的 32767 校验 |
   | sidedcar + pending | ✅ | ✅ |

   → **Rust 服务层相对活的 TS 路径缺了「备份 + 并发 + 长度预检」三项能力**。这正是 `gui/src/commands/service.rs:5-6` 说「完整切换延后到 Wave 2 收口（保守方案）」的原因。

**删除测试**
删除 `core::service` → 复杂度**不会消失**，因为 CLI 依赖其中 2 个函数。所以**模块本身值这个位置**；真正的问题是：**它的 5 个公开函数里 3 个是死重，而它本该取代的那份 TS 编排还活着**。

**问题本质**：这不是「多写了一个模块」，而是**一次分层迁移做到一半**——core 侧建好了目的地，但 GUI 侧没有搬过去，于是**两个适配器（TS 编排 / Rust 编排）同时存在，且行为已经分叉**（备份/并发/长度三项）。分叉会随时间拉大：任何一侧修 bug，另一侧不会同步。

**建议方向**（接口留待开发窗口设计）

- **二选一，不要维持双份**：
  - **方案 A（推荐，收口方向）**：把「备份 + 乐观并发 + 长度预检」补进 `core::service`，让 `save_path_with_sidecar` 成为**唯一**的保存编排；GUI 的 `path-session.ts` 退化为**薄适配器**（只做 i18n 文案与 UI 状态映射），或直接删除其编排职责。
  - **方案 B（止损方向）**：若近期不打算收口，则**明确冻结并标注** `core::service` 中那 3 个死函数（`#[allow(dead_code)]` + 注释指向本报告），并**在两侧各加一条「行为一致性」测试**钉住分叉点——但这只是止血，不解决双份实现。
- 无论哪条，**先补一条「GUI 保存 == CLI 保存」的契约测试**（同一输入 → 同一注册表/sidecar 终态），把当前分叉点显式化。

**建议强度：Strong。** 这是本报告最高的架构风险点：一个被测试覆盖、看似更优的模块**在生产里根本没接线**，而它的替代品已经悄悄缺了三个安全相关能力。

---

### F-2【Strong / P0】前端「只被自己测试引用」的死码簇

**证据链**（导入方全仓实测，排除 `node_modules`）

| 模块                        | 行数 | 运行时导入方 | 测试导入方                                           | 现状 |
| --------------------------- | ---- | ------------ | ---------------------------------------------------- | ---- |
| `src/core/path-manager.ts`  | 54   | **无**       | 仅 `tests/unit/path-manager.test.ts`                 | 死码 |
| `src/core/import-export.ts` | 237  | **无**       | 仅 `import-export.test.ts` + `import-parity.test.ts` | 死码 |
| `src/core/validation.ts`    | 35   | **无**       | 仅 `validation.test.ts`                              | 死码 |

- 三个模块**在 `src/`、`e2e/` 中零运行时调用**（`components/` / `hooks/` / `store/` / `services/` 均不引用）。
- 且它们各自**已经有 Rust 对应实现**：
  - `path-manager.ts::pathClean` ↔ Rust `registry::clean_path_entries`（`registry/path.rs:148`，CLAUDE.md 明确「清理走 Rust」）。
  - `import-export.ts` ↔ Rust `fs::import_paths/export_paths`（`fs.rs:70/292`），且文件头自述「**仅作为历史兼容/测试夹具保留**」。
  - `validation.ts::split_path` 自述「**仅保留为测试夹具**；正式 PATH 解析以此处（Rust）为准」（`registry/path.rs:109` 反向印证）。

**关键反诘：那个 `import-parity.test.ts` 真的在验「一致性」吗？**
读 `tests/unit/import-parity.test.ts`：它 import 的是**TS 版自己**（`../../src/core/import-export`），只断言 TS 版的输入输出。**它没有与 Rust 版做任何交叉比对**——所谓「parity」只是给 TS 单测起了个名字。Rust 侧的真正契约由 `core/src/registry/golden/*.json` + `golden_tests.rs` 守护。→ **这个「一致性测试」是自证自的循环，不构成保留 TS 版实现（vs 仅保留夹具数据）的理由。**

**删除测试**
删除这三个模块 → 复杂度**消失**（Rust 已完整拥有对应能力的实现与测试）。它们的存在只制造「同一规则有两处实现、会漂移」的假象。

**建议方向**

- **删除 `path-manager.ts` 与 `import-export.ts`**，连同其独占的测试文件（`path-manager.test.ts`、`import-export.test.ts`、`import-parity.test.ts`）。
  - 若「parity 夹具」确有价值，**保留的应是跨 Rust/TS 的 golden 数据**（像 `tests/fixtures/path-capabilities.json` 那样），而不是 TS 的第二份实现。
- `validation.ts`：确认 `is_valid_path_format` 确实无运行时使用（本次 grep：`is_valid_path_format` 在 `src/` 中零命中），删除；若 `join_path` 仍被测试依赖，先迁移该测试。
- ⚠️ **涉及删除文件，须用户书面同意**（Global Constraints）。本报告只作删除**建议**，不代为执行。

**建议强度：Strong。** 这是最干净的一类——**删除即减复杂度，零行为风险**。

---

### F-3【Worth exploring / P1】同一条 IPC 缝 speak 两种错误契约

**证据**

- `gui/src/commands/*.rs` 实测：**22 个** `Result<_, String>`，**9 个** `Result<_, CoreError>`。
  - `String` 一族：`registry` / `fs` / `scanner` / `profiles` / `disabled` / `service` 的命令。
  - `CoreError` 一族：`env_var` / `backup`（env 备份）的命令。
- core 内部同样分裂：`registry/path.rs` / `fs.rs` / `scanner.rs` 返回 `String`；`registry/env_var.rs` / `backup.rs` / `profiles.rs` / `disabled.rs` / `service.rs` 返回 `CoreError`。`error.rs:96` 有 `impl From<String> for CoreError` —— 是**过渡期桥**。
- 前端 `services/backend.ts:146 parseCoreError` 注释直言：「**双形状兼容（过渡期，Wave 2 收口时复核是否收紧）**」。

**问题**
同一条缝（Tauri IPC）对调用方说**两种方言**。调用方必须**按命令**知道它返回哪种——`backend.ts` 正是靠「env 命令走 `parseCoreError`、PATH 命令走裸 `invoke`」的记忆来分别处理。这是缝泄漏（leak across the seam）：错误处理的复杂度没有沉到一处，而是散落在每个调用点的「我知道这条命令是哪种」。

**删除测试**：这不是删模块的问题，是**统一缝的方言**。目标是让 IPC 缝只有一种错误契约（结构化 `CoreError`），`parseCoreError` 的双形状分支收成单形状。

**建议方向**

- 把 PATH 子系统的 `String` 错误逐步迁移为 `CoreError`（`From<String>` 桥已是现成踏脚石），直至 `parseCoreError` 可删掉兼容分支。
- 这是一次**贯穿多模块的迁移**，建议**按子系统分批**（先 `scanner`/`fs`，再 `registry/path`），每批附「同一错误码在新旧路径一致」的契约测试。
- CLAUDE.md 已把它登记为「尚未完成」——本项即**把它从「已知债」变成「有批次、有测试」的计划**。

**建议强度：Worth exploring。** 收益真实（缝收敛、文档可简化），但工作量大、非阻塞。

---

### F-4【Worth exploring / P2】`backup.rs`：低内聚上帝模块

**证据**：`core/src/backup.rs` 生产代码约 1288 行，混装至少 4 类职责：

1. **配置**：`config_file_path` / `read_backup_keep` / `path_backup_keep` / `env_backup_keep`
2. **env 备份**：`collect_env_backup` / `write_env_backup_to` / `rotate_env_backups` / `list_env_backups*` / `backup_env_vars`
3. **path 备份**：`rotate_path_backups` / `backup_registry`
4. **恢复**：`validate_backup_path` / `read_env_backup` / `preview_restore*` / `restore_env_backup_from` / `restore_in_stores` / `diff_one_hive`

**问题（局部性差）**：想改「env 备份轮换」，得在「path 备份」「恢复执行」的代码之间穿行；想在恢复路径上定位一个公共 helper，前缀是 `backup::`，与 PATH 备份同名同模块。**知识没有集中在它该在的地方。**

**删除测试**：删除 `backup.rs` → 复杂度**转移**到多个调用方，**不会消失**。所以它**值这个位置**，问题是**一颗模块装了四颗模块的量**。

**建议方向**：拆为 `backup/mod.rs` + `backup/config.rs` + `backup/env.rs` + `backup/path.rs` + `backup/restore.rs`。**纯搬家、零行为变化**——仓库已有先例（`registry.rs` 拆分，见其模块文档「W2 Task 4，纯搬家零行为变化」），照抄该做法即可，风险极低。**不建议**改接口，只改内部文件组织。

**建议强度：Worth exploring。** 低风险、纯局部性收益；优先级低于 F-1/F-2。

---

### F-5【Worth exploring / P3】`registry/env_var.rs` 的三层包装链

**证据**：5 个写操作各有一组三层函数（`registry/env_var.rs`）：

```
update_env_var                (489)  ← 公开：open WinregHive → 转调
  └─ update_env_var_with_store (504)  ← 校验 + 写前备份 + *_in_store + 广播
       └─ update_env_var_in_store (530) ← 真正读→算→比对→写
```

同构还有 `create_*` / `delete_*` / `update_*_force` / `delete_*_force`——**5 组 × 3 层 = 15 个函数**。

**评估**：中层 `_with_store`（`env_var.rs:504-523`）约 15 行，**不是无意义直通**——它承载一条**文档化不变式**：`golden/README.md` 第 6 类「`*_in_store` 不含广播」，即「公开 API 层才做备份与广播」。这是**刻意的缝**：`_in_store` 是单元测试面（配 `MemoryHive`），`_with_store` 是「公开 API 不变式」面。**可保留。**

**但**：5 组同构三层 = 45 个函数名要记，且 `with_store`/`in_store` 的样板（校验顺序、备份位、广播位）在 5 处重复。**"探索"的价值在于**：能否用一个小结构（如「操作上下文」把 `store + hive + 不变式钩子` 打包）把中层从 5 份压成 1 份，同时**保住那条不变式**。

**建议方向**：**低优先**。先评估「是否存在一个接口能同时表达『公开 API 层负责备份+广播』与『in_store 层负责 CAS』」；若设计的接口比现状更深（更少函数名、不变式不丢），再动；否则**维持现状**——现状是「啰嗦但正确」，不是缺陷。

**建议强度：Worth exploring（倾向不动）。** 明确写出来，是为了**防止后来者把它当"重复"顺手"统一"掉**——那条不变式会一起丢。

---

### F-6【Strong（清理）/ P1】7 个已注册、前端从不调用的 IPC 命令

**证据**（命令名在 `src/` + `e2e/` 的字符串命中数，实测）：

| 命令                       | 前端命中         | 归属                           |
| -------------------------- | ---------------- | ------------------------------ |
| `save_path_with_sidecar`   | **0**            | F-1 的 service 层              |
| `apply_path_snapshot`      | **0**            | F-1                            |
| `retry_pending_path_state` | **0**            | F-1                            |
| `apply_profile`            | **0**            | F-1                            |
| `scan_conflicts`           | 0（仅 e2e mock） | 「兼容包装」（CLAUDE.md 自述） |
| `scan_tools`               | 0（仅 e2e mock） | 「兼容包装」                   |
| `get_appdata_dir`          | 0（仅 e2e mock） | 疑似遗留                       |

（注：`list_env_backups` 一度被本报告初判为死码，复核后发现它经 `backend.ts:464` 的**变量 label** 调用——**它是活的**。此处如实更正，避免误伤。）

**问题**：注册进 Tauri 的每个命令都是**对外接口面**——它进入 `invoke_handler` 白名单、进入安全边界、进入「未来维护者以为有人在用」的认知负担。7 个从不被调用的命令 = 接口面的净负担。

**删除测试**：删除 → 复杂度消失（无调用方）。**但 F-1 的 4 个命令的生死取决于 F-1 的决策**：

- 若采纳 F-1 方案 A（收口）→ 这 4 个命令**将被接线**，不能删。
- 若采纳 F-1 方案 B（冻结）→ 应连同死函数一起标注或删除。

`scan_conflicts` / `scan_tools` / `get_appdata_dir` 与 F-1 无关，可直接评估删除（先确认 CLI 侧不经 Tauri 用到它们——CLI 直连 core，不经 gui，故与它们无关）。

**建议方向**：与 F-1 决策**绑定**处理。**先定 F-1 方向，再清 F-6。**

**建议强度：Strong（清理），但依赖 F-1 决策。**

---

### F-7【Speculative / 记录】`EnvVar` 与 `PathEntry` 双通路

**证据**：core 同时存在通用环境变量通路（`registry/env_var.rs` + `env_var.rs`）与 PATH 专用通路（`registry/path.rs` + `disabled.rs`）。PATH 被列入 `RESERVED_NAMES`（`env_var.rs:13`），被 `list_all_env_vars` 过滤、被通用写入口拒绝。

**这是有意的**（CLAUDE.md 明说：PATH 必须经专用通路，否则绕过 `disabled.json` 与快照事务）。**缝是正当的。**

**代价**：前端整条链路翻倍——两个 store（`app-store` 管 PATH / `env-store` 管环境变量）、两套表格、两族弹窗、两套 `PathEntry` vs `EnvVarMeta` 契约。**这不是缺陷，是「两通路」的必然成本。**

**建议方向**：**不改**。只做一件事——**加一条「两通路不可交叉」的守卫测试**（如：断言通用写入口对 `Path` 恒拒绝、断言两个 store 的键空间无覆盖），钉住这条缝不被未来改动模糊掉。**建议强度：Speculative（记录 + 一条守卫，不动结构）。**

---

## 五、正向发现（这些深度/杠杆在起作用，**不要动**）

评审不只看缺陷。以下是被反复验证的良好结构，**后续重构应以它们为模板**：

1. **`EnvHiveStore` trait（`reg_store.rs:17`）是教科书级的「真实缝」**——两个适配器（`WinregHive` 真注册表 / `MemoryHive` 内存），符合「两个适配器 = 真实缝」。它让所有 env 写逻辑可在**不碰真注册表**的前提下单测。**杠杆极高**：一份 `_in_store` 逻辑，被真机与测试同一接口驱动。
2. **`backend.ts` 是唯一 IPC 边界 + 运行时形状校验（`backend.ts:60-379`）**——把「上游可能返回坏形状」的复杂度**集中到一处**，组件/store 不直接 `invoke`。**局部性极佳**：契约校验改一次，全前端受益。
3. **`CoreError` / `ErrorCode` 结构化契约（`error.rs`）**——判定只认 `code`（serde camelCase），**禁止按 message 文本做控制流**。已被 F-3 的迁移方向验证为「正确的目标形态」。
4. **`src/core/` 的「零框架依赖 + 安全判定不下放」纪律**——`env-var.ts:1-6` 明确「安全判定全部在 Rust 侧计算并下发，本模块不重复实现」。这是 F-2 之外**该保留**的 `src/core` 模块（`env-var.ts` / `path-capabilities.ts` / `path-entry.ts` / `undo-redo.ts` / `env-backup.ts`）的共同特征：**它们只含类型契约与纯展示逻辑，不含第二份安全规则**。
5. **golden 契约测试**（`core/src/registry/golden/*.json` + `golden_tests.rs`）——把「不可变式」固化成数据。**这正是 F-2 里 `import-parity.test.ts` 应当效仿而未效仿的形态。**

**判断标准（给开发窗口）**：一个 `src/core/*.ts` 模块该留还是该删，看它是否包含**第二份实现**（该删）还是**仅类型 + 纯展示**（该留）。按此标准：`path-manager`/`import-export`/`validation` 含第二份实现 → 删；其余留。

---

## 六、顶层建议（先做哪个）

**第一个该动的不是最大的，而是最便宜的净收益：F-2。**

- F-2 是**纯删除**：零行为风险，删完立刻减少「两份规则」的漂移面。
- F-1 是**最高风险**但也**最需要先决策**的（收口 vs 冻结），且它的决策**决定 F-6 的 4 个命令生死**。所以 F-1 应当**紧接着**做，但**先要开发窗口选定方向**（本报告给方向，不给接口）。
- F-3 / F-4 / F-5 / F-7 是**可延后**的结构优化。

**推荐顺序：F-2（立即）→ F-1 决策并收口（本波主线）→ F-6 据 F-1 清理 → F-4 纯搬家 → F-3 分批次 → F-5/F-7 视情。**

---

## 七、建议路线图（供开发窗口排波次）

| 波次    | 内容                                                              | 风险               | 前置                           |
| ------- | ----------------------------------------------------------------- | ------------------ | ------------------------------ |
| **W-1** | **F-2 删除三个死码模块**（+ 处理其测试）                          | 极低（纯删）       | 用户书面同意删除               |
| **W-2** | **F-1 收口或冻结** + 补「GUI 保存 == CLI 保存」契约测试           | 中（涉及保存路径） | 开发窗口定方向；真机回归需授权 |
| **W-3** | **F-6 清理**（据 F-1 结果删/留 4 个 service 命令 + 3 个兼容命令） | 低                 | W-2 完成                       |
| **W-4** | **F-4 `backup.rs` 纯搬家拆分**（照 `registry.rs` 先例）           | 低（零行为变化）   | 无                             |
| **W-5** | **F-3 错误契约分批迁移**（`scanner`/`fs` → `registry/path`）      | 中（跨模块）       | 无                             |
| **W-6** | **F-5 / F-7 评估**（倾向不动 + 各加一条守卫测试）                 | 低                 | 无                             |

**每波的质量门沿用现状**：`npm run verify:all` + `cargo test --workspace` + `cargo clippy --workspace --all-targets -D warnings` + `cmp CLAUDE.md AGENTS.md`。**行为变化必须有回归测试**（仓库硬性要求）。

---

## 八、范围外与待裁断

**明确范围外（本报告不建议动）**：

- crate 分层（`core`/`gui`/`cli`）——健康，**不重构**。
- `src/core/` 中「类型契约 + 纯展示」模块（`env-var` / `path-capabilities` / `path-entry` / `undo-redo` / `env-backup`）——**保留**。
- `EnvHiveStore` 缝、`backend.ts` IPC 边界、`CoreError` 契约——**保留并作为重构模板**。
- CLAUDE.md 已登记的**规格级**已知问题（`RestorePreview.modified`/`RestoreOutcome.skipped`/`variableCount` 恒为 0；TOCTOU 窗口）——**不在本报告范围**，属既有取舍。

**待用户/开发窗口裁断**：

1. **F-1 方向**：收口（方案 A）还是冻结标注（方案 B）？这一问决定 F-6 的一半。
2. **F-2 删除授权**：三文件 + 三测试文件的删除需**用户书面同意**（Global Constraints）。
3. **F-3 是否入波**：这是贯穿多模块的迁移，需评估是否与后续功能开发争资源。
4. **本报告的行动化**：是否将 W-1/W-2 转为 `docs/superpowers/specs/` 下的设计文档 + `plans/` 下的实施计划，走**既有双窗口流程**（本报告是「发现」，不是 spec）。

---

## 附录：本次评审的证据命令（可复现）

```bash
# 1. 生产/测试行数（mod tests 以内为测试）
grep -nE '^mod .*tests|^#\[cfg\(test\)\]' core/src/backup.rs core/src/registry/env_var.rs cli/src/env_ops.rs
# 2. 死模块：某 src/core 模块的运行时导入方
grep -rn "core/path-manager\|core/import-export\|core/validation" src --include=*.ts --include=*.tsx
# 3. core::service 的调用图
grep -rn "save_path_with_sidecar\|apply_path_snapshot\|apply_profile\|commit_sidecar_snapshot\|retry_pending_path_state" core gui cli --include=*.rs
# 4. 注册但前端零调用的命令
grep -oE 'commands::[a-z_]+::[a-z_]+' gui/src/lib.rs | sed 's/.*:://' | sort -u   # 注册集
grep -rhoE "'[a-z_]+'" src/services/backend.ts | tr -d "'" | sort -u            # 调用集（需含变量 label）
# 5. 两种错误契约
grep -rho "Result<[^,]*, String>" gui/src/commands/*.rs | wc -l   # 22
grep -rho "Result<[^,]*, CoreError>" gui/src/commands/*.rs | wc -l # 9
# 6. EnvHiveStore 的两个适配器（真实缝的判据）
grep -n "impl EnvHiveStore for" core/src/reg_store.rs
```

---

**报告结束。** 本报告为**发现与方向**，下一步请开发窗口：① 就 F-1 选定方向；② 就 F-2/F-6 删除征得用户书面同意；③ 将选定的波次转为 `specs/` + `plans/` 走既有双窗口流程。
