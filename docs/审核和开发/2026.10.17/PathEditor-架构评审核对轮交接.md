# PathEditor 架构评审 — 核对轮交接（开发窗口 → 审核窗口）

- **日期**：2026-10-17
- **提交窗口**：开发窗口
- **交付对象**：审核窗口
- **核对对象**：`docs/审核和开发/2026.10.17/PathEditor-架构评审报告.md`（审核窗口 2026-10-17 产出）
- **当前状态**：**第 1 轮交接**。开发窗口已对报告的**事实层逐条独立核实**（Grep/Read 只读），**未开工、未改任何源文件、未删除任何文件**。
- **本轮用户授权**：提交 / 推送 / 升版本号 / 真实注册表写入 / 删除文件 —— **全部未授权**（沿用默认）。

> 本文件是「核心细节必须落盘」规则下的产物。审核窗口看不到开发窗口的会话，以下每条都自带 `file:line` 或可复现的核实结论，不依赖对话上下文即可复核。
>
> **核实方法**：全部使用 Grep / Glob / Read 只读检索。**本轮未运行任何构建或测试**，因此本文件不含质量门结论，只含事实核实结论。凡检出「无法核验」的，明确标注为未覆盖（见 §2.3）。

---

## 1. 本轮性质：这不是「读计划提异议」，而是「读评审报告提异议」

按 `patheditor-two-window-flow` 的交付物表，`spec` 与 `plan` 的作者是**审核窗口**。本轮的输入不是 spec/plan，而是审核窗口的**架构评审发现**（报告自述「是发现，不是 spec」）。因此开发窗口对本轮的动作是：

1. 把报告的**事实层逐条去代码里复现**（开发窗口 skill ① 的硬要求：报告是没看过实现细节时写的，几乎必然有与现状不符处）；
2. 就 **F-1 方向**给出开发窗口的选择与理由；
3. 把报告**遗漏的、会改变实施成本的**事实补上；
4. 把需要用户书面同意的事项（删除 / 授权）显式列出。

**本轮不改代码。** 方向未收敛前不动手（skill ② 的收敛判据）。

---

## 2. 事实核实结果

### 2.1 已核实：**逐条吻合**（本节所有结论均来自本次实际检索）

| #   | 报告主张                                                                                                         | 核实方式                                         | 结论                                                                                                                                                                                 |
| --- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------- | ------------------------------------------------------------ |
| 1   | F-1：`save_path_with_sidecar`/`apply_path_snapshot`/`apply_profile` 生产侧仅被 gui 包装，前端零调用              | Grep `**/*.rs` + Grep `src/`                     | ✅ Rust 侧仅出现在 `gui/src/lib.rs:54/55/57`（注册）、`gui/src/commands/service.rs:13/22/37`（包装）、core 定义与自测；**`src/` 全域零命中**                                         |
| 2   | F-1：`commit_sidecar_snapshot` + `retry_pending_path_state` 被 `cli/src/runtime.rs:168 / :195` 调用              | Grep                                             | ✅ 行号**逐字符吻合**                                                                                                                                                                |
| 3   | F-1 能力分叉表（写前备份 / 乐观并发 / 长度预检：TS 有、Rust 无）                                                 | Read `path-session.ts` + `service.rs`            | ✅ 逐格成立，详见 §2.2                                                                                                                                                               |
| 4   | F-1：`service.rs:191-192` 注释明说「不做乐观并发校验」                                                           | Read                                             | ✅ 原文：`**注意**：本函数不做乐观并发校验（读-比-写）。CLI 的 verify_and_save 保留在 CLI 层（保守方案，见 Task 5 报告）`                                                            |
| 5   | F-1：`registry/path.rs:48 save_paths` 内部也不备份                                                               | Read `path.rs:48-71`                             | ✅ `fn save_paths` 只做 `validate_and_join_paths` + `set_raw_value`，**无任何备份调用**                                                                                              |
| 6   | F-2：`path-manager.ts` / `import-export.ts` / `validation.ts` 运行时零导入                                       | Grep 路径串 + Grep 导出符号 + 查 barrel          | ✅ 三重验证：路径串只命中 `tests/unit/*.test.ts` 与 docs；**导出符号的运行时引用全部落在三个模块自身内部**；`src/core/` **无 barrel**（8 文件无 `index.ts`），`from '@/core'` 零命中 |
| 7   | F-2：三模块行数 54 / 237 / 35                                                                                    | Read 至末行                                      | ✅ 精确（54 / 237 / 35）                                                                                                                                                             |
| 8   | F-2：`import-parity.test.ts` 只 import TS 版自己，未与 Rust 交叉比对                                             | Read                                             | ✅                                                                                                                                                                                   |
| 9   | F-2：三模块各有 Rust 对应实现                                                                                    | Read `fs.rs:1`、`path.rs:109` 注释 + Glob golden | ✅ 双向印证                                                                                                                                                                          |
| 10  | F-3：gui 命令 22 个 `Result<_, String>` vs 9 个 `Result<_, CoreError>`                                           | Grep count                                       | ✅ **精确 22 / 9**（String：disabled 3 / backup 1 / fs 2 / profiles 5 / registry 4 / scanner 3 / service 4；CoreError：backup 4 / env_var 5）                                        |
| 11  | F-4：`backup.rs` 混装 4 类职责                                                                                   | Grep `^pub fn                                    | ^fn                                                                                                                                                                                  | ^const` | ✅ 函数清单坐实四簇：config / env 备份 / path 备份 / restore |
| 12  | F-5：三层包装链 `update_env_var`(489) → `_with_store`(504) → `_in_store`(530)，共 5 组                           | Grep                                             | ✅ 行号精确；5 组齐备（update 489/504/530、create 590/604、delete 706/716/739、update_force 798/809、delete_force 884/891）                                                          |
| 13  | F-6：7 个已注册命令前端零调用                                                                                    | Grep `src/`                                      | ✅ 零命中                                                                                                                                                                            |
| 14  | F-6 自我更正：`list_env_backups` **是活的**（经变量 label）                                                      | Grep `backend.ts`                                | ✅ `backend.ts:463-464`：`listEnvBackups: async () => { const label = 'list_env_backups';`。**报告把自己初判的死码改判为活，诚实且正确**                                             |
| 15  | §2.2：`backup.rs` 3319 行、`registry/env_var.rs` 1753 行                                                         | Grep count `^`                                   | ✅ 精确                                                                                                                                                                              |
| 16  | §五.5：`core/src/registry/golden/*.json` + `golden_tests.rs` 存在                                                | Glob `**/*.json`                                 | ✅ **25 个 JSON** + `golden_tests.rs`                                                                                                                                                |
| 17  | F-1 证据链 3：`src/store/app-store.ts:365` → `savePathSnapshot`（`path-session.ts:66`）→ save/broadcast/snapshot | Grep `src/`                                      | ✅ 三处行号全部吻合（365 / 66 / 168；`backupRegistry` 在 `:131`）                                                                                                                    |

**结论**：报告在本轮可核验的事实层上，**未发现任何失实**；引用的行号亦逐条吻合。这是罕见的命中率，如实记录。

### 2.2 F-1 能力分叉表 —— 逐格核实（这是 P0 建议的全部依据）

| 职责              | 报告的判断      | 本次核实                                                                                                                                                                                                                            |
| ----------------- | --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 写前备份          | TS ✅ / Rust ❌ | ✅ `path-session.ts:129-134` 确有 `await backend.backupRegistry()`（`backend.ts:414`，默认 `customDir=null`）；`service.rs` 的 use 列表（disabled / env_var / error / path_entry / profiles / registry / system）**无 backup 模块** |
| 乐观并发          | TS ✅ / Rust ❌ | ✅ TS `:136-151` 由 `savedSys/savedUser` 算出 `origSys/origUser` 传给 save 命令；Rust `service.rs:191-192` 注释明说不做                                                                                                             |
| 长度预检          | TS ✅ / Rust ❌ | ✅ TS `:109-127` 用 `appConfig.path.*` 计算；Rust 侧只有 `path.rs:129-143 validate_and_join_paths` 内的硬编码 `MAX_PATH_LEN = 32767`。**但两者不等价，见 §3.1**                                                                     |
| sidecar + pending | TS ✅ / Rust ✅ | ✅ 两侧都有（语义有差异，见 §3.3）                                                                                                                                                                                                  |

### 2.3 未核验项（如实列出，**不得**当作已证实）

| 项                                                                                                                                                                                                                                            | 状态                                                                              |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| §2.2 全表的**生产/测试行数拆分**（如 `backup.rs` 3319 行中约 2031 是测试、`env_var.rs` 1753 中约 795、`cli/env_ops.rs` 1881 中约 1225）                                                                                                       | **未核**。我只核了三个文件的总行数（3319 / 1753 精确命中），未逐文件拆分生产/测试 |
| §2.2 其余模块的行数与错误契约标注（`fs.rs` 557、`profiles.rs` 499、`disabled.rs` 316、`service.rs` 293、`reg_store.rs` 324、`scanner.rs` 218、`registry/path.rs` 201、`env_var.rs` 403、`persist/system/error/capabilities/path_entry` ~490） | **未核**（`registry/path.rs` 除外，见 §3.4 的混合契约发现）                       |
| F-7 双通路细节（`RESERVED_NAMES` 含 `Path`、`list_all_env_vars` 过滤、前端两 store 键空间）                                                                                                                                                   | **未核**                                                                          |
| F-5 引用的 `golden/README.md` 第 6 类「`*_in_store` 不含广播」条文                                                                                                                                                                            | **未核**（golden 目录确有 25 个 JSON，但未读 README）                             |
| F-6 表中 `scan_conflicts` / `scan_tools` / `get_appdata_dir` 的「仅 e2e mock」                                                                                                                                                                | **部分未核**。我核了 `src/` 零命中；**未核 `e2e/`**                               |
| 附录 6 条证据命令的逐条可复现性                                                                                                                                                                                                               | **未逐条跑**。我用的是等价的 Grep 检索，命令文本本身未执行                        |

---

## 3. 开发窗口的异议与补充（报告未覆盖、且会改变实施成本的事实）

**本节不含「报告写错了」的指控** —— 事实层我没找到错。以下都是「报告**没写**、但动手前必须知道」的成本项。

### 3.1 【最重要的补充】F-1 方案 A 的「长度预检」不是搬运，是**跨界配置裁断**

报告建议「把「备份 + 乐观并发 + 长度预检」补进 `core::service`」。前两项是纯逻辑搬运，第三项不是：

1. **限制值的来源在前端。** `path-session.ts:113` 读 `appConfig.path`，值在 **`src/config/default.json:14-18`** —— 一个**前端文件**，core 读不到：
   ```json
   "path": { "maxSystemLength": 32767, "maxUserLength": 32767, "maxCombinedLength": 32767 }
   ```
2. **两者语义不等价。** 三个值当前都是 32767，于是：
   - TS 检查三条：`sys ≤ 32767`、`user ≤ 32767`、**`sys + user ≤ 32767`** —— 第三条组合约束才是实际生效的那条；
   - Rust `validate_and_join_paths`（`path.rs:129-143`）按**单 hive UTF-16 长度**判 32767，**没有组合约束**。
   - 所以「把长度预检补进 core」会**引入一条新约束**，属于行为变化，不是等价搬运。
3. **失败形态也不同。** TS 是 `{ kind: 'warning', reason: 'lengthExceeded' }`，用户可用 `force` **继续**（`path-session.ts:118 if (!force && lengthExceeded)`）；Rust 是**硬 `Err`**，无逃逸口。

**→ 收口前必须裁断**：限制值归 core、还是由 GUI 传入？保留「warning + force」还是改成硬拒绝？（见 §5 待裁断 A）

### 3.2 两 hive 的写入模型不同：**并行 vs 顺序**

- TS：`path-session.ts:138-151` 用 `Promise.all([saveSystemPaths, saveUserPaths])` —— **并行**。
- Rust：`service.rs:202-209` 先 system 后 user —— **顺序**。

收口会改变时序与「一个 hive 失败时另一个是否已落地」的窗口。这会影响 §5 待裁断 B 的契约测试怎么写（同一输入 → 同一终态，但中间态不同）。

### 3.3 「备份」这一项与本波刚做的改动**耦合**

TS 的备份是 `backend.backupRegistry()`（`path-session.ts:131`，**无参** → `backend.ts:414` 默认 `customDir = null`）。若把它搬进 `core::service`，会直接碰上**本波（5.1.5 未发布）刚加的** `path_backup_keep` 轮换与 N4 裁断（`custom_dir` 指定目录**不参与**轮换，理由：可能混有用户自己的文件）。**报告未提这条耦合。** 收口时须确认：core 侧的新备份调用同样只走默认目录轮换。

另：TS 的 pending 是**请求/返回里携带**的 store 级状态（`pendingSys/pendingUser` 进出 `SaveSnapshotRequest`/`SaveSnapshotOutcome`），Rust 的 pending 是**磁盘文件**（`disabled::save_pending_path_snapshot`）。两者**同概念、不同机制**，收口时要选定一种。

### 3.4 §2.2 错误契约表有一处标注不准：`registry/env_var.rs` 是**混合**的

报告 §2.2 把 `registry/env_var.rs` 的错误契约标为 `CoreError`（未标混合）。实测该模块内还有：

- `pub fn validate_env_name(name: &str) -> Result<(), String>`（`registry/env_var.rs:215`）
- `pub fn validate_env_value(value: &str, label: &str) -> Result<(), String>`（`registry/env_var.rs:232`）

**→ 该模块是 `String` + `CoreError` 混合**，与 `reg_store.rs` 同。这会影响 **F-3 的迁移范围估算**（迁移 `registry/*` 时，`env_var.rs` 也在范围内）。

### 3.5 F-3 可以有一个**更锋利**的证据：单文件内即已分裂

报告给的是总数 22 : 9。实测 **`gui/src/commands/backup.rs` 一个文件内就两种契约**（1 个 `Result<_, String>` + 4 个 `Result<_, CoreError>`）。这比「跨文件总数」更能直接支撑「同一条缝 speak 两种方言」的论点，建议在 spec 里用作首选证据。

（来源：`gui/src/commands/backup.rs` 的 `backup_registry` 走 `String`，其 core 实现 `backup.rs:1228 pub fn backup_registry(custom_dir: Option<String>) -> Result<String, String>`；同文件的 env 备份命令走 `CoreError`。）

### 3.6 附录证据命令 #3 的**边界**：它不含前端，证明不了「前端零调用」

附录 #3：

```bash
grep -rn "save_path_with_sidecar\|apply_path_snapshot\|apply_profile\|commit_sidecar_snapshot\|retry_pending_path_state" core gui cli --include=*.rs
```

这是 **Rust-only** 检索。「前端零调用」这一结论实际由附录 #4（命令名在 `src/` 命中数）支撑。报告正文 §F-1 证据链 2 写「本次全仓 grep 实测」，措辞略宽。

**→ 建议**：在 spec / 后续文档里把两条证据的**各自适用范围**写清，避免后来者误引「全仓」这个词。这不影响结论（两条证据合起来是充分的）。

### 3.7 命名冲突：报告的 `W-1…W-6` 与仓库历史的 `Wave 0/1/2` 并存

仓库既有代号：**Wave 0**（注册表端口）、**Wave 1**（数据一致性）、**Wave 2**（架构收口，`service.rs` 模块文档即写「F-08，Wave 2 Task 5」）。报告新引入 `W-1…W-6` 排波次，两套编号并存易混。

**→ 建议**改名为不冲突的一套（如「阶段 A–F」或「Wave 3.x」），并在 spec 里一句话交代新旧对应关系。

### 3.8 F-6 的 `get_appdata_dir`：要删的是 **IPC 命令**，不是 core 函数

`core/src/backup.rs:165` 有 `pub fn get_appdata_dir() -> String`，它被 `backup_base_dir()`（`backup.rs:147`）**内部使用**。报告说的是「命令」，表述无误，但需在 spec 里写死，防止执行时误删 core 函数。

### 3.9 方案 A 的终态措辞需澄清：GUI 侧**不会归零**

报告写 `path-session.ts` 应「退化为薄适配器（只做 i18n 文案与 UI 状态映射），或直接删除其编排职责」。但 TS 侧还有两类逻辑是 `ApplyOutcome` **表达不了**的：

- `kind: 'blocked'` —— 来自 `canWriteTarget(isAdmin, capabilities, target)`（`src/core/path-capabilities.ts`，报告 §五.4 明确**要保留**的模块）；
- `kind: 'warning'` —— 长度预检（§3.1）。

**→ 所以终态实际是「core 负责写与事务；GUI 保留权限预检、长度预检与文案映射」，而不是「GUI 零逻辑」。** 这不是反驳，是给 spec 定接口时必须写清的边界。

---

## 4. F-1 方向：开发窗口的选择

**开发窗口选 A（收口），但要求分步执行，且以「契约测试」为先决条件。**

### 4.1 为什么不选 B（冻结标注）

- B 把「两份实现长期并存」**正式化**，与 F-08 的目标（`service.rs:1-5` 自述「消除两端的策略分叉」）和 `CLAUDE.md` 的相关表述相悖；
- 分叉会**继续拉大**：任何一侧修 bug，另一侧不同步。而缺的那三项里有**两条是安全相关**（备份、并发保护）；
- B 的成本不是零 —— 它要求「两侧各加一条行为一致性测试钉住分叉点」，等于**用测试固化一个已知错误的状态**。

### 4.2 为什么 A 不能一步到位

三项能力的实施成本**不对称**：

| 能力     | 成本       | 是否改签名                       | 阻塞点                                   |
| -------- | ---------- | -------------------------------- | ---------------------------------------- |
| 写前备份 | 低（纯增） | 否                               | 与 `path_backup_keep` 轮换的耦合（§3.3） |
| 乐观并发 | 中         | **是**（需接受 `original` 参数） | 接口设计（spec 环节）                    |
| 长度预检 | 中         | 视方案                           | **跨界配置裁断**（§3.1）                 |

### 4.3 建议的分步（A′）

0. **先决**：补「GUI 保存 == CLI 保存」契约测试，把当前分叉**显式化**（报告 §F-1 也建议了）—— 没有它，后续每一步都无从验证等价性。
1. `core::service` 补**写前备份**（纯增，低风险）。
2. `core::service` 补**乐观并发**（签名变更，需 spec 定接口）。
3. **长度预检**：先完成 §5 待裁断 A，再实施。
4. GUI 切换；`path-session.ts` 退化为薄适配器（权限预检 + 长度预检 + 文案映射，见 §3.9）。

**全程冻结项**：core::service 那 3 个死函数加 `#[allow(dead_code)]` + 注释指向报告，并**禁止**两侧各自修 bug（否则分叉在收口期间继续拉大）。报告的方案 B 里这条是对的，即便选 A 也应保留。

### 4.4 对报告推荐顺序的认同

**F-2（纯删）→ F-1（收口）→ F-6（据 F-1 清理）→ F-4（纯搬家）→ F-3（分批）** 的顺序我**同意**。F-2 与 F-1 触及的文件不重叠（前者删 `src/core/*`，后者改 `path-session.ts` / `service.rs`），无冲突。

---

## 5. 待裁断（开发窗口无法自行拍板）

### 待裁断 A（F-1 方案 A 的「长度预检」归属）

- (a) **限制值下沉到 core**（core 定义常量，前端从 core 取）—— 与「业务规则放 core」的项目纪律一致，但改动面更大；
- (b) **由 GUI 传入限制值**给 core —— 改动小，但把一条业务规则留在前端；
- (c) **不搬长度预检**，GUI 继续在本地预检（只搬备份 + 并发）。

并需同时决定：**保留「warning + force 可继续」还是改硬拒绝？**（§3.1）

**开发窗口倾向 (a)**：`CLAUDE.md` 明写「保留名、保护名单、hive 写权限、revision 校验全部由 core 判定，CLI 仅透传错误文本」，长度上限属同类规则。但这是 spec 级决策，请审核窗口定。

### 待裁断 B（F-2 删除授权）

删除 `src/core/path-manager.ts`、`src/core/import-export.ts`、`src/core/validation.ts` 及其独占测试（`tests/unit/path-manager.test.ts`、`import-export.test.ts`、`import-parity.test.ts`）**需要用户书面同意**（Global Constraints）。

**开发窗口的建议**：同意删（事实层已三重核实为死码，且各有 Rust 实现）。另需在实施时**实测一次 `npm run test:coverage`** —— 删模块会改变覆盖率分母，须确认仍过 80% 门槛。

### 待裁断 C（F-6 清理授权）

`scan_conflicts` / `scan_tools` / `get_appdata_dir` 三个命令与 F-1 无关，可否删除？同样需用户同意。**F-1 的 4 个命令的生死挂在待裁断 A/§4 的方向上**（选 A 则保留并接线）。

### 待裁断 D（本轮授权）

提交 / 推送 / 升版本号 / 真实注册表写入 —— 均未授权。W-2（F-1 收口）的**真机回归**需用户显式授权并记录备份与回滚。

### 待裁断 E（流程归属）

报告末行写「下一步请开发窗口：③ 将选定的波次转为 `specs/` + `plans/` 走既有双窗口流程」。但按 `patheditor-two-window-flow` 的交付物表，**spec 与 plan 的作者是审核窗口**。

**→ 请确认**：是审核窗口据本交接落笔 spec/plan（**符合现行流程，开发窗口建议这条**），还是本轮改由开发窗口写？若改由开发窗口写，请明示（这会改变流程约定）。

---

## 6. 我同意、且不建议动的（对报告 §五「正向发现」的确认）

以下结论我**独立认同**，并**不主张改动**：

| 项                                       | 位置                                         | 认同理由                                      |
| ---------------------------------------- | -------------------------------------------- | --------------------------------------------- |
| crate 分层（core/gui/cli）健康           | `core` 不依赖 Tauri                          | 本轮未发现反例                                |
| `EnvHiveStore` 是真实缝（两适配器）      | `reg_store.rs`                               | 本轮 F-1/F-3 的核实过程反复依赖它             |
| `backend.ts` 是唯一 IPC 边界 + 形状校验  | `backend.ts`                                 | 与 F-3 的收敛目标一致                         |
| `CoreError` / `ErrorCode` 是正确目标形态 | `error.rs`                                   | F-3 的迁移方向即向它收敛                      |
| golden 契约测试是模板                    | `registry/golden/*.json` + `golden_tests.rs` | 已核实存在；F-2 的「parity 夹具」应是它的形态 |
| F-5 倾向不动                             | `registry/env_var.rs` 三层链                 | 同意：中层承载文档化不变式，是「啰嗦但正确」  |
| F-7 双通路不改                           | `env_var.rs` + `disabled.rs`                 | 同意：缝正当，只加守卫测试                    |

---

## 7. 本波（F-1 收口）明确不做什么

- **不改 crate 分层**。
- **不动 `src/core/` 中「类型契约 + 纯展示」模块**（`env-var` / `path-capabilities` / `path-entry` / `undo-redo` / `env-backup`）。
- **不动 `EnvHiveStore` 缝、`backend.ts` IPC 边界、`CoreError` 契约**。
- **不碰** `CLAUDE.md` 已登记的规格级已知问题（`RestorePreview.modified` / `RestoreOutcome.skipped` / `variableCount` 恒为 0；TOCTOU 窗口）。
- **不做** F-3 / F-4 / F-5 / F-7（可延后）。
- **不删除任何文件**（未获用户书面同意前）。
- **不提交 / 不推送 / 不升版本号 / 不写真实注册表**（本轮未授权）。

---

## 8. 一处开发窗口的自我更正（诚实记录）

核实过程中我曾判定「报告引用的 `core/src/registry/golden/*.json` **不存在**」——Glob `**/golden*` 只返回了 `golden_tests.rs`。改用 `**/*.json` 后确认 **25 个 JSON 确实存在**。

**这是我的检索模式不够（`golden/` 是目录名，`**/golden\*` 只匹配文件名前缀），不是报告的问题。** 记在此处：本轮对报告的怀疑中，**没有一条成立\*\*——包括上面这条。

---

## 9. 交接链状态

| 文档                                          | 状态                                 |
| --------------------------------------------- | ------------------------------------ |
| `PathEditor-架构评审报告.md`（审核窗口）      | ✅ 已产出（未提交，工作树唯一新增）  |
| 本核对轮交接（开发窗口）                      | ✅ 本文件                            |
| spec `docs/superpowers/specs/2026-10-17-*.md` | ⏳ 待审核窗口（依待裁断 E 确认作者） |
| plan `docs/superpowers/plans/2026-10-17-*.md` | ⏳ 同上                              |
| 开发回执                                      | ⏳ 待开发（方向收敛后）              |

**下一步（开发窗口的请求）**：

1. 审核窗口就 **§5 待裁断 A / D / E** 给出裁断，并就 **B / C** 向用户征得**书面同意**；
2. 本节 §3 的九项补充**折入 spec**（尤其 §3.1 长度预检、§3.9 终态边界）；
3. 方向收敛后，开发窗口按 §4.3 的分步实施。

**开发窗口本轮未开工，等方向收敛。**
