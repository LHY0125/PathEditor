# PathEditor env 子命令可用性收口 — 核对轮交接（审核窗口 → 开发窗口）

- **日期**：2026-10-16
- **提交窗口**：审核窗口
- **交付对象**：开发窗口
- **核对对象**：
  - `docs/superpowers/specs/2026-10-16-env-usability-fixes-design.md`（本轮新建，未提交）
  - `docs/superpowers/plans/2026-10-16-env-usability-fixes-implementation.md`（本轮新建，未提交）
- **当前状态**：**第 1 轮交接**。spec 与计划已写完，等开发窗口逐行核对、提异议。**未开工。**
- **来源**：GitHub issues [#33](https://github.com/LHY0125/PathEditor/issues/33) / [#34](https://github.com/LHY0125/PathEditor/issues/34) / [#35](https://github.com/LHY0125/PathEditor/issues/35) / [#36](https://github.com/LHY0125/PathEditor/issues/36) / [#37](https://github.com/LHY0125/PathEditor/issues/37) / [#38](https://github.com/LHY0125/PathEditor/issues/38)

> 本文件是「核心细节必须落盘」规则下的产物。开发窗口看不到审核窗口的会话，以下每条都自带 `file:line` 或核实结论，不依赖对话上下文即可复核。

---

## 1. 本波范围与用户已拍板的决策

用户 2026-10-16 确认：**窗口角色 = 审核窗口**；**范围为 6 条 issue**（#32 排除）；**不授权提交/推送/升版本号**；**删除仅限自产备份文件**；**#37 默认保留 20 份**。

| Issue | 做什么                                         | 性质              | 核实结论                                                  |
| ----- | ---------------------------------------------- | ----------------- | --------------------------------------------------------- |
| #34   | `env get` 报「变量不存在」而非 `os error 2`    | core 错误映射     | ✅ 完全属实，根因链可溯源                                 |
| #33   | `env list` help 文案（「不含明文」→ 限定措辞） | 纯文案            | ⚠️ 属实，但 issue 的「preview 未截断」判断**错误**        |
| #35   | `env add` help 补「纯新建语义」                | 纯文案            | ✅ 属实，答案是「纯新建」（安全）                         |
| #38   | `env add` help 补「省略值报错」                | 纯文案 + 关 issue | ❌ **不成立**（会立即报错，不阻塞）                       |
| #36   | `env set/remove/add` 加 `--dry-run` / `--json` | 新功能（小）      | ⚠️ env 侧属实；PATH `clean` 与 `env restore` 已有 dry-run |
| #37   | `path_backup_*` 保留策略                       | 新功能（小）      | ⚠️ **env 侧已有轮换（20 份）**，只有 path 侧无            |

**#32 排除原因**：其诉求③（批量往返）需跨命令 session 状态，属架构变动，另开 spec。诉求①②（错误提示、help 措辞）已由 #34/#33 的文案改动间接覆盖。

## 2. 三条「描述与代码不符」的更正（开发窗口务必先看）

这三条若照 issue 原文去修，会做错方向。**核实证据见各 issue 的评论**（已发布）。

### 2.1 #38 — issue 推断不成立，无代码改动

issue 说「省略 `VALUE` 时可能阻塞」。核实：

- `cli/src/env_ops.rs:353`：`resolve_value(value, stdin, value_file, true)` —— 第 4 参 `required=true`。
- `env_ops.rs:39-44`：`count == 0 && required` → **立即 `exit_err`**，不读 stdin。
- 只有显式 `--stdin` 才走 `read_value` 的 `Stdin` 分支（`env_ops.rs:70-77`）。

**结论**：省略 `VALUE` + 不给 `--stdin`/`--value-file` = 立即报错退出码 1。**不阻塞、不交互、不取空值**。本波只补 help 文案 + 回填 issue 关闭为「不成立」。

### 2.2 #33 — 「preview 未截断」是误判

`core/src/env_var.rs:197-211` 的 `sanitize_preview` **已实现截断**（超 `PREVIEW_MAX_CHARS` 加 `…`）。issue 举的两个例子（`D:\settings\Language\Rust\.cargo`、`https://mirrors.aliyun.com/pypi/simple/`）都短于阈值，所以*看起来*完整。**issue 的「方向 2（加截断）已实现，不需做」**；本波只改 help 文案。

### 2.3 #37 — env 侧已有轮换，只有 path 侧缺

`core/src/backup.rs:24` 有 `ENV_BACKUP_KEEP = 20`，`rotate_env_backups`（`:361-406`）每写必轮换。而 `path_backup_*.txt` **只在 `backup.rs:1177` 被写入，无任何轮换调用**。issue 实测的 250 份吻合。**本波只给 path 侧加策略**，env 侧不动。

---

## 3. 待裁断（开发窗口请在核对轮作答）

### 待裁断 A（计划 Task 5）：`--json` 是否必须与 `--dry-run` 同用？

- (a) **推荐**：单独 `--json` 报错「--json 需与 --dry-run 同用」；
- (b) 静默忽略。

理由：静默忽略会让脚本作者以为拿到了 JSON 摘要。

### 待裁断 B（计划 Task 6）：path 轮换是否作用于 `backup_registry` 的 `custom_dir`？

- (a) **推荐**：否，只在默认目录轮换（与 env 侧「自定义目录不参与轮换」一致）；
- (b) custom_dir 也轮换。

理由：用户指定目录可能混有他自己的文件，轮换风险高于收益。

### 待裁断 C（计划 Task 1 / Task 5，K1 实施自由度）：`reg_store` 错误类型的实现方案

本计划选的是「`EnvHiveStore::get_raw` 返回 `Result<RegValue, CoreError>`」。spec 的 K1 明确允许更小方案，**唯一不变量**：

1. `ErrorKind::NotFound` 的结构信息必须到达 `read_env_var`；
2. 其余错误的 message 文本与 v5.1.4 **逐字节一致**。

若开发窗口有更小改动（例如只加一个旁路函数），在核对轮提出并同步更新计划 Task 1。**不得**退回「在 `read_env_var` 里文本匹配『找不到』」—— 那是按文本做控制流，违反错误契约硬约束，且非中文 Windows 直接失效。

---

## 4. 已知的代码事实核对（供开发窗口复查，非异议项）

| 事实                                                                        | 位置                                                | 用途                                                                              |
| --------------------------------------------------------------------------- | --------------------------------------------------- | --------------------------------------------------------------------------------- |
| `update_env_var` 公开签名 `(hive, name, value, expected_revision)`          | `registry/env_var.rs:389-397`                       | Task 5 参考                                                                       |
| `reveal_env_var` 返回 `RevealedValue { value, revision }`                   | `registry/env_var.rs:356-382`、`env_var.rs:116-119` | Task 5 取 oldValue                                                                |
| `attach_read_context` 补「读取X环境变量 Y 失败: 」前缀                      | `registry/env_var.rs:186-191`                       | Task 2 保留非 NotFound 分支                                                       |
| `MemoryHive` 在 `reg_store::memory`（`#[cfg(test)]`），**未经根 re-export** | `reg_store.rs:1207` 的注释                          | Global Constraints                                                                |
| `config_file_path` 支持 `PATHEDITOR_CONFIG_FILE` 重定向                     | `backup.rs:44-54`                                   | Task 6 测试隔离                                                                   |
| `should_warn_on_read_error`：NotFound 静默、其它 warn                       | `backup.rs:63-65`                                   | Task 6 复用                                                                       |
| `with_temp_backup_dir` 测试辅助（既有）                                     | `backup.rs` 测试模块                                | Task 6 复用                                                                       |
| CLI 未初始化 logger，core 的 `log::warn!` 被丢弃                            | `runtime.rs:28` 注释                                | Task 6 的轮换失败 warn 在 CLI 下不可见 —— **是否需要 CLI 侧显式提示？见下方观察** |

### 观察（留给开发窗口判断，非阻断）

Task 6 Step 5 的轮换失败只 `log::warn!`。按 `runtime.rs:28` 的实证，CLI 未初始化 logger，**该 warn 在 CLI 下不可见**。env 侧既有轮换（`rotate_env_backups`）同样是 `log::warn!`，即**现状已接受「轮换失败静默」**。本波与 env 侧保持一致即可，但请开发窗口确认：这是否符合「如实报告」的期望？若认为需要可见，可在核对轮提议（会扩大范围，需用户拍板）。

---

## 5. 本波不做什么（范围边界）

- **#32 批量往返**：另开 spec。
- **GUI 侧 env 预览**：本波 6 条全在 CLI/core，`src/` 与 `gui/` 零改动。
- **#33 的 expand 展开值新列**：issue 自身说可另开。
- **`env add` 加 revision 参数**：#35 核实后确认纯新建语义已安全。
- **备份目录 ACL 收紧 / `RestorePreview.modified` 恒为 0**：既有已知项，本波不动。
- **升版本号 / 提交 / 推送**：用户未授权。

---

## 6. 交接链状态

| 文档                                                    | 状态                                                           |
| ------------------------------------------------------- | -------------------------------------------------------------- |
| spec `2026-10-16-env-usability-fixes-design.md`         | ✅ 已写（未提交）→ 第 1 轮裁断后已更新                         |
| plan `2026-10-16-env-usability-fixes-implementation.md` | ✅ 已写（未提交）→ 第 1 轮裁断后已更新                         |
| 本核对轮交接                                            | ✅ 本文件                                                      |
| 开发回执（开发窗口）                                    | ✅ 第 1 轮已交（`PathEditor-env子命令可用性收口-开发回执.md`） |
| 开发回执                                                | ⏳ 待开发窗口                                                  |

**下一步**：开发窗口读 spec + 计划 → 提异议（含待裁断 A/B/C 的作答）→ 审核窗口裁断并更新文档 → 收敛后放行开发。

---

## 7. 第 1 轮裁断结果（审核窗口，2026-10-16）

开发窗口第 1 轮回执提出 **1 处头号异议（§2）+ 3 处事实性缺陷（§3）**。审核窗口逐条独立核实（**以源码为准，非采信回执**），结论：**四条全部成立**，含一条对审核窗口自身疏漏的指正。**已全部裁断并折入 spec 与计划。**

### 7.1 头号异议 N1（成立 → 采纳旁路方案）

- **异议**：plan Task 1 要把 `EnvHiveStore::get_raw` 签名改为 `Result<_, CoreError>`，与 W2-N2 裁断（`reg_store.rs:48`「trait 的其他方法保持 `Result<_, String>`，Wave 0 端口边界不动」）**正面冲突**，且 plan 未提及该裁断。
- **审核窗口独立核实**：读 `reg_store.rs:41-48`，确认 W2-N2 原文存在且明确。grep 统计 `set_raw`/`delete_value`/`enum_names` 生产调用点 = **7 处**（`registry/env_var.rs:203/685/852`、`backup.rs:214/809`、`registry/env_var.rs:268/581`），确认改签名会连带改动 `backup.rs` 恢复路径。
- **裁断**：**采纳开发窗口的旁路方案** —— trait 既有签名不动，新增带默认实现的 `get_raw_kind`。改动面从「7 处调用点 + 恢复路径」缩到「2 个 impl + 1 个调用点」。K1 已重写（spec §关键设计决策），plan Task 1 全节重写。
- **这是审核窗口的疏漏**：写 spec 时没翻到离被引行约 60 行的架构裁断注释。已如实记入 spec 的核对轮裁断记录。

### 7.2 事实性缺陷 N2 / N3 / N4（均成立）

| #   | 缺陷                                                                                                | 核实                                                                                                                | 裁断                                                                 |
| --- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| N2  | Task 5 引用不存在的 `kind_label_for` / `parse_kind_label`；「core 加预览函数」被列为推荐而非必选    | ✅ `kind_label(EnvValueKind)` 在 `env_ops.rs:116`、`parse_kind(&str)->EnvValueKind` 在 `:303`；两个 plan 名确不存在 | `preview_env_change` 提为**必选**（新增 Task 5 Step 3b）；修正函数名 |
| N3  | 待裁断 A 补充：`--json` 应用 clap `requires = "dry_run"`（退出码 2）而非手写 `exit_err`（退出码 1） | ✅ 与 `CLAUDE.md` 退出码表一致                                                                                      | **采纳**                                                             |
| N4  | 待裁断 B 补充：`custom_dir` 在作用域内可直接判断；要求 Task 6 注释写明「轮换失败 CLI 不可见」       | ✅ `backup_registry(custom_dir: Option<String>)` 在 `backup.rs:1155`                                                | **采纳**；B 裁断为 (a)                                               |

### 7.3 开发窗口的额外发现（高价值，须回填 issue #34）

开发窗口在核对 `env_var.rs:152-155` 时发现：**W2-B1 注释声称「三类错误在函数内部分类完成……不再整体包成 Io」，但当前代码（`:157-159`）恰恰在整体包 Io** —— 该注释描述的行为**从未实现**（或被回退）。即 #34 的根因不只是「漏判 NotFound」，而是「注释与实现不符」。

- **审核窗口复核**：读 `env_var.rs:153-159`，确认注释与代码矛盾属实。
- **裁断**：开发窗口完成 Task 1 后，须在 **issue #34 补一条评论**记录此发现（`gh issue comment 34`）。已写入 plan Task 1 Step 4 与 spec 的核对轮裁断记录。

### 7.4 行号勘误（已就地修正）

- `main.rs`：`List` 描述在 `:169`（plan 原写 `:169` ✅ 可接受）；`Add` 变体在 `:206-220`（plan 原写 `:205-219`，差 1 行，已改）。
- `env_ops.rs`：`resolve_value` 在 `cmd_env_add` 内为 `:357`（spec §2.1 原引 `:353`，方向正确，已修正）。

### 7.5 打磨收敛状态

**已收敛**：审核窗口对「做什么、不做什么、怎么验证」与开发窗口无分歧；N1–N4 全部裁断并折入文档（非口头共识）。**放行开发**。

**开发窗口开工时须知**：

1. plan 已更新，**以更新后的 plan 为准**（Task 1 全节重写、Task 5 新增 Step 3b、Task 1 Step 4 新增「邻近改动禁区」）。
2. 三个待裁断全部有答案，无需再问。
3. 仍**不授权提交/推送/升版本号**（用户未变更授权）。
4. 完成后写**开发回执**（`docs/审核和开发/2026.10.16/PathEditor-env子命令可用性收口开发回执.md`，注意与「核对轮回执」区分命名）。
5. 提交前勿忘：**issue #34 补评论**（§7.3）。

---

## 8. 第 2 轮裁断（审核窗口，2026-10-16）

开发窗口复核了第 1 轮裁断的落盘质量（确认 spec/plan 时间戳为本轮、Task 1 无旧方案残留、`get_raw_kind`/`preview_env_change`/`requires` 均已落位、函数名已修正），并新提出 **1 项自洽性要求（N5）+ 1 处口径分歧**。审核窗口逐条独立核实后裁断如下。

### 8.1 N5（成立 → 折入 plan）

- **发现**：Task 1 改完后，`registry/env_var.rs` 内会并存两种读法 —— `:158` `read_env_var` 用 `get_raw_kind`（需分类），`:284` `list_env_vars` 保持 `get_raw`（列表路径要一致文案）。开发窗口指出：**必须写明这是有意的**，否则后人看到同文件两套读法会「统一」，而那个统一正是 bug（会把 `list_env_vars` 错误文案也改掉）。
- **审核窗口独立核实**：
  - `env_var.rs:284-292`：确为 `with_hive(ErrorCode::Io, "list_env_vars", format!("读取{}环境变量列表失败: …"))` —— 列表级统一文案，**不需要** NotFound 分类。
  - `backup.rs:226-232`：同为 `map_err` 到 `Io`，理由一致。
  - `backup.rs:821-823`：是 `let Ok(raw) = store.get_raw(&name) else { continue }` —— **跳过不构造错误**，与 `:226` 机制不同但同样不需要分类。
- **裁断**：**折入 plan**（Task 1 Step 4 新增「邻近改动禁区」表，并要求落注释）。**不采纳**开发窗口「自行遵守、不折入文档」的提议 —— 理由：计划是下一个会话（下一个 AI）的权威依据，「我会注意」不持久，而这里的误改代价是静默回归。**计划的职责之一，就是把「看似该改、其实不该改」的邻近改动圈出来。**

### 8.2 口径分歧（更正，非异议）

- 第 1 轮 K1 写「改签名牵动 7 处调用点」；开发窗口 grep 到 26 处。**两者都对，集合不同**：26 = 全部 `.get_raw(` 文本出现（含 22 处测试）；7 = `set_raw`/`delete_value`/`enum_names` 的生产调用点；而 `get_raw` 自身的生产调用点 = **4 处**（`env_var.rs:158/284`、`backup.rs:226/821`）。
- **裁断**：已在 spec K1 与 plan Task 1 更正为准确口径（`get_raw` 4 处 + 其它方法 7 处）。这一更正对结论无影响（旁路仍显著更小），但口径要说清。

### 8.3 收尾建议的裁断

| 开发窗口建议                                                                                    | 裁断                                            |
| ----------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| issue #34 评论**留到开工完成后**回填（现在是半截状态；且 GitHub MCP 连接失败、`gh` 配置未验证） | **采纳** —— 回填应是「已修复 + 根因」的完整叙事 |
| 开工前先 `git diff README.md` 定性那个来历不明的改动                                            | **采纳**（见 §9）                               |
| 「A → B」顺序（先清基线再开工）                                                                 | **采纳**                                        |

---

## 9. README.md 意外改动（本期唯一非本波产物）

`git status` 显示 `README.md` 有改动，**不是本波任何一方的产物**。经核实：改动是 Markdown 表格分隔行对齐（`| --------- |` → `| ----------- |`）+ 表格前加空行，共 4 增 2 删，是 **Prettier 对 Markdown 表格的格式化输出**。最可能来源是编辑器保存自动格式化。

**处置**：定性为「历史遗留的格式化改动」，**与本波无关**，建议用户在开工前自行决定（保留并单独提交 / `git checkout` 丢弃）。审核窗口与开发窗口**均不擅自 `checkout` 或提交**（守「不删除文件、不擅自提交」约束）。

---

## 10. 第 3 轮：Task 1 / Task 2 实施与复验（2026-10-16）

开发窗口完成 **Task 1 + Task 2**（两者同为 #34，一并验证），交来进度汇报。审核窗口**独立复验**（不采信回执数字）：

### 10.1 复验结果（审核窗口亲自跑的）

| 检验   | 命令                                                            | 结果                                                      |
| ------ | --------------------------------------------------------------- | --------------------------------------------------------- |
| 单测   | `cargo test -p path-editor-core`                                | **214 passed / 0 failed / 2 ignored**（较基线 212 +2） ✅ |
| Lint   | `cargo clippy -p path-editor-core --all-targets -- -D warnings` | 零警告 ✅                                                 |
| 格式   | `cargo fmt --check`                                             | 干净（exit 0） ✅                                         |
| 改动量 | `git diff --stat core/`                                         | `reg_store.rs` +96、`env_var.rs` +71/−5 ✅                |

### 10.2 实施中发现并裁断的 3 项（N6 + 2 项）

| #      | 事项                                                                                               | 核实                                                                                                                        | 裁断                                                        |
| ------ | -------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| **N6** | plan Task 1 Step 1 测试误写 `let hive`，`hive.fail_get = Some(...)` 需 `let mut hive`              | ✅ 成立。既有 `memory_hive_injects_failures`（`reg_store.rs:363`）本就带「需要可变绑定，此处加 `mut`」注释，plan 抄写时丢了 | **已就地修正** plan                                         |
| —      | 开发窗口**自证非缺陷**：Task 2 的 `let mut v = …; v.vtype = …` 是 `RegValue` 局部可变，非 hive mut | ✅ 确认。`seed_raw(&self, …)` 签名不可变                                                                                    | 采纳，Task 2 测试**不改**（此为开发窗口自我纠错，值得肯定） |
| —      | 补充：Task 2 测试模块名实为 `env_var_tests`（非通用 `tests`）                                      | ✅ 成立                                                                                                                     | 已补进 plan Task 2 Files                                    |

### 10.3 N5 的落实确认

开发窗口把「邻近改动禁区」注释落在 `read_env_var` 的**文档注释块**（plan 允许位置之一），内容正确（点名 `list_env_vars` 与 `backup.rs` 采集/恢复预览路径不得「统一」）。✅ 确认落位。

### 10.4 放行决定

**Task 1 / Task 2 验收通过，继续 Task 3 → Task 6。** 理由：独立复验与回执一致（214 passed、零警告、fmt 干净），改动与 plan 逐处对应，无偏离。

**节奏决定（回应开发窗口的 A/B/C/D 选项）**：**采 A → B 连贯**（Task 3+4 纯文案 → Task 5 新功能 → Task 6 新功能），**不要**每 Task 打断一次汇报。但要求：

1. 每 Task 结束**跑对应质量门**（Task 3/4 只涉 CLI，跑 `cargo test -p patheditor-cli --bins` + `clippy`；Task 5/6 跑 core 全量 + CLI --bins）。
2. 累积的 plan 缺陷**边做边就地修正文档**（像 N6 这样），**不必等汇报**；在开发回执里列一份完整清单即可。
3. **Task 5 开工前**——因为 Task 5 含 core 新增 `preview_env_change`（本波唯一的 core 行为新增）——**回头看一眼本交接 §10.2 的三项**，确认理解无歧义再动手。
4. 全部 6 个 Task 完成后写**开发回执**（`PathEditor-env子命令可用性收口开发回执.md`），含：逐 Task 的证据、plan 缺陷清单、未覆盖项、issue #34 的 W2-B1 回填。

**仍不授权提交/推送/升版本号**（用户未变更）。**未授权前不要 commit。**

---

## 11. 第 4 轮：Task 3–6 完成 + 格式改动裁断（2026-10-16）

开发窗口完成 **Task 3 → Task 6 全部**，全量质量门通过（core 222 passed、CLI 59 passed、clippy 零警告、fmt 干净）。Task 1–6 仅剩 Task 7（文档同步）未做。

### 11.1 用户裁断（2026-10-16 明确指示）

**Question A：`CLAUDE.md` / `AGENTS.md` / `README.md` 的 Prettier 格式改动怎么处置？**

**裁断：(a) 接受 Prettier 格式。** 用户 2026-10-16 明确。理由：Prettier 是项目自带工具（`npm run verify` 会跑），`.md` 跟随项目格式化是合理的；回退（选项 b）只会让下次 Prettier 重新产生同样 diff。

**执行要求**：

1. 把 **`AGENTS.md` 也格式化到与 `CLAUDE.md` 一致**（两者必须恢复字节级一致）。
2. `README.md` 同样接受该格式（它已被格式化，无需回退）。
3. 三个文件的格式改动**非本波产物**，但既然接受，就在 Task 7 一并纳入。

**Question B：Task 7 是否授权开发窗口修改 `CLAUDE.md`？**

**裁断：授权。** 用户 2026-10-16 明确授权，按 plan Task 7 的 4 项内容改动（CLI 命令表补 `[--dry-run] [--json]`、env 说明段、`config.ini` 的 `path_backup_keep` 键、`env get` 错误文案）。

> ⚠️ **此授权仅限本次 Task 7 的这 4 项内容改动** —— 即「plan 已列明、用户已批」的范围。开发窗口**不得**借机扩大 CLAUDE.md 的改动范围（不重排其它章节、不改无关表述）。改完后必须 `diff CLAUDE.md AGENTS.md` 为空。

### 11.2 Task 7 执行须知

1. **先格式化 `AGENTS.md`**（与 CLAUDE.md 对齐），再在其上做内容改动 —— 否则内容改动会与格式改动混在一起，diff 难读。
2. `CLAUDE.md` 与 `AGENTS.md` 的**内容改动必须逐字节同步**（改一份立即复制另一份）。
3. `README.md`：补 `env` 子命令的 `--dry-run` 说明 + 备份章节的 path 保留策略与「含明文路径」提示。
4. `CHANGELOG.md`：顶部写 5.1.5 段落（**不升版本号**，段内可标「未发布」）。
5. 跑 `npm run verify:all`，**必须全绿**。
6. **不提交**（用户未授权提交；本轮授权仅限「改文件」）。

### 11.3 完成后

写**开发回执** `PathEditor-env子命令可用性收口开发回执.md`，含：

- 逐 Task 证据（6 个 Task）
- 实施中发现的 plan 缺陷清单（N6 已修；其余累积项）
- 未覆盖项、已知残余风险
- **issue #34 的 W2-B1 回填**（「注释与实现不符」的发现）—— 用 `gh issue comment 34`，GitHub MCP 本次连接失败
- `diff CLAUDE.md AGENTS.md` 为空的证据
