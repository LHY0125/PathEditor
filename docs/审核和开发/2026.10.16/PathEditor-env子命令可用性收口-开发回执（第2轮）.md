# PathEditor env 子命令可用性收口 — 开发回执（开发窗口 → 审核窗口）

- **日期**：2026-10-16
- **提交窗口**：开发窗口
- **交付对象**：审核窗口
- **交付物**：Task 1 – 7 全部实现完毕，未提交（等用户授权）
- **依据**：`docs/superpowers/specs/2026-10-16-env-usability-fixes-design.md` + `docs/superpowers/plans/2026-10-16-env-usability-fixes-implementation.md`（含 N1–N6 裁断后的更新版）

---

## 1. 交付摘要

6 条 issue 一次收口完成。**7 个 Task 全部实现**，`npm run verify:all` 全绿（退出码 0，含 Playwright 30 passed）。

| 指标           | 值                                                                          |
| -------------- | --------------------------------------------------------------------------- |
| core 测试      | **219 passed / 0 failed**（v5.1.4 基线 212 → +7）                           |
| CLI 测试       | **59 passed / 0 failed**（基线 54 → +5）                                    |
| 新增测试函数   | **12 个**                                                                   |
| 改动文件       | 6 个源文件 + 4 个文档（`CLAUDE.md`/`AGENTS.md`/`README.md`/`CHANGELOG.md`） |
| 代码改动量     | +763 / −20（`git diff --stat core/ cli/`）                                  |
| 质量门         | `npm run verify:all` **退出码 0**                                           |
| 真实注册表写入 | **零**（全程 MemoryHive / 临时目录 / 只读验证）                             |

---

## 2. 逐 Task 证据

### Task 1：`reg_store::get_raw_kind`（#34 根因）

**实现**：trait 新增 `get_raw_kind`（带默认实现），`WinregHive` 按 `io::ErrorKind::NotFound` 分类、`MemoryHive` 覆写。**既有 4 个 trait 方法签名一律未动**（遵守 W2-N2 端口边界裁断）。

| 步骤       | 证据                                                                                                 |
| ---------- | ---------------------------------------------------------------------------------------------------- |
| RED        | `cargo test ... get_raw_kind_` → 编译失败 `no method named get_raw_kind found for struct MemoryHive` |
| GREEN      | 8 passed（含 2 新用例 + 6 既有 memory_tests）                                                        |
| 回归       | core 全量 212 passed（无连带改动，印证 N1 旁路方案）                                                 |
| clippy/fmt | 零警告 / 干净                                                                                        |

**文件**：`core/src/reg_store.rs`（+96）、`core/src/registry/env_var.rs`（`read_env_var` 改调 `get_raw_kind`）

### Task 2：`env get` NotFound 文案（#34 用户可见层）

| 步骤   | 证据                                                                                                                                                        |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| RED    | 断言失败：`left: "读取用户环境变量 THIS_VAR_ABSENT 失败: 无法读取环境变量 THIS_VAR_ABSENT: 找不到"` vs `right: "用户环境变量 THIS_VAR_ABSENT 不存在"`       |
| GREEN  | 5 passed（含 2 新用例 + 3 既有 reveal 用例）                                                                                                                |
| 端到端 | `cargo run -p patheditor-cli -- env get PATHEDITOR_ABSENT_PROBE_9F3` → `错误: 用户环境变量 PATHEDITOR_ABSENT_PROBE_9F3 不存在`，退出码 1，**无 `os error`** |

**不变量 2 验证**：`reveal_unsupported_type_message_keeps_existing_prefix` 断言非 NotFound 分支的前缀 `读取用户环境变量 DWORD_VAR 失败:` 逐字节不变。

### Task 3：`env list` help（#33）

改动 `cli/src/main.rs` 的 `EnvCmd::List` 文档注释。**变异测试验证**：临时改回旧文案 → `env_list_help_scopes_the_no_plaintext_claim` 转红 → 恢复 → 绿。

### Task 4：`env add` help（#35 / #38）

改动 `EnvCmd::Add` 变体注释 + `value` 字段注释。测试放 `cli/src/main.rs` 的 `help_tests` 模块（**因 `struct Cli` 私有，`env_ops.rs` 无法访问 `crate::Cli`** —— plan 初稿写放 `env_ops.rs`，执行时修正，plan 已就地更新）。

### Task 5：`--dry-run` / `--json`（#36）

**实现**：3 个命令各加 `dry_run` + `json`（`json` 带 clap `requires = "dry_run"`，N3 裁断）；core 新增 `preview_env_change` + `EnvChangePreview` + `env_hive_writable`（N2 裁断，Step 3b）。

| 验证项                                | 结果                                                                                                          |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| JSON 契约                             | 3 单测逐字段断言（字段名/类型/各操作 null 语义）                                                              |
| 端到端 `env set ... --dry-run`        | notFound，退出码 1（验收标准 8）✅                                                                            |
| 端到端 `env remove ... --dry-run`     | notFound，退出码 1 ✅                                                                                         |
| 端到端 `env add ... --dry-run --json` | 输出 `{operation:add, hive:user, name, oldValue:null, newValue:"v", kind:string, writable:true}`，退出码 0 ✅ |
| 单独 `--json`（无 dry-run）           | clap 拒绝，退出码 **2**（N3 裁断）✅                                                                          |
| **不写注册表**                        | PowerShell `Get-ItemProperty HKCU:\Environment` 确认变量不存在 ✅                                             |

### Task 6：`path_backup_*` 保留策略（#37）

**实现**：`PATH_BACKUP_KEEP` 常量、`read_backup_keep(path, key, fallback)` 通用化（`read_env_backup_keep` 改薄包装）、`rotate_path_backups`、`backup_registry` 写后轮换（`custom_dir.is_none()` 才轮换，N4 裁断）。

| 验证项           | 结果                                                                                                                             |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| 保留 N 份        | `rotate_path_backups_keeps_exactly_keep_files`（25 → 20，removed 5）✅                                                           |
| **对抗性零删除** | `rotate_path_backups_never_deletes_foreign_files`：`env_backup_*.json` / `.txt.bak` / `.txt.corrupt-*` / `notes.txt` 全部保留 ✅ |
| 四种回落         | `read_path_backup_keep_falls_back_like_env` ✅                                                                                   |
| env 侧不回归     | `read_env_backup_keep_generic_refactor_preserves_behavior` + 既有 `read_env_backup_keep_*` 全通过 ✅                             |
| **接线验证**     | `backup_registry_triggers_path_rotation`（plan 外补充，见 §3）✅                                                                 |

---

## 3. plan 缺陷清单（N6 已修 + 执行中累积）

**本波共发现 7 处 plan 缺陷，全部已就地修正 plan 文档**（按 §10.4 授权）：

| #   | Task | 缺陷                                                                                                                        | 处置                                          |
| --- | ---- | --------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| N6  | 1    | 测试代码缺 `let mut hive`（故障注入需可变绑定）                                                                             | 已修 plan；代码加 `mut`                       |
| —   | 4    | 测试放 `env_ops.rs` 不可行（`struct Cli` 私有，子模块不可见 `crate::Cli`）                                                  | 已修 plan；测试改放 `main.rs` 的 `help_tests` |
| —   | 5    | Step 7 命令笔误：`env set NAME v --dry-run`（`set` 无位置参数值，应用 `--value v`）                                         | 已修 plan                                     |
| —   | 5    | Step 3b 示意代码三处失实：`registry_kind_label` 不存在、`capabilities_for` 参数不符、`env_hive_writable` 需 `pub`+re-export | 已修 plan，换成实际实现                       |
| —   | 6    | `read_backup_keep(path, key)` 缺第 3 参 `fallback`（两 key 回落值语义独立，硬编码判断是伪装通用化）                         | 已修 plan，改为 3 参数                        |
| —   | 6    | Step 7 未验证 `rotate_path_backups` 被 `backup_registry` **接线**（函数对 ≠ 被调用）                                        | 已补接线测试 + 变异验证，plan Step 7 已更新   |
| —   | 7    | `CLAUDE.md`/`AGENTS.md` 执行前已因外部工具格式化而**不一致**（且该格式不符合项目 Prettier）                                 | 已按裁断 Q_A 规范化，plan Step 1 已记录       |

**另有 1 处 plan 之外的补强**（非缺陷，是覆盖增强）：

- Task 6 新增 `backup_registry_triggers_path_rotation`：单测只证明 `rotate_path_backups` 函数正确，**证明不了它被接线**。本测试预置 25 份旧备份 → 调 `backup_registry(None)` → 断言收敛到 keep 份，并经**变异测试**验证（临时禁用接线 → 转红 `left:26/right:20`）。

---

## 4. 未覆盖项 / 残余风险（如实登记）

| 项                                  | 说明                                                                                                                                                                                                                                                                                                          |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **接线测试的已知局限**              | `backup_registry_triggers_path_rotation` 在无权限读注册表时断言被跳过（`if ... .is_ok()`）——"静默通过但未验证"。与既有 `backup_registry_with_custom_dir` 同手法（项目约定）。本机实测返回 `Ok`（变异测试已证明断言真实执行）。更彻底的方案是让 `backup_registry` 接受注入 store，属超出本波的重构，**未做**。 |
| **轮换失败静默**                    | `rotate_path_backups` 失败只 `log::warn!`，而 CLI 未初始化 logger（`runtime.rs:28`），该警告在 CLI 下**不可见**。这是**有意的**（与 env 侧 `rotate_env_backups` 对称），已在代码注释与 plan 中写明。若要统一解决属独立 observability 议题。                                                                   |
| **`env set/add/remove` 参数过多**   | `cmd_env_add` 现 8 参数，触发 `clippy::too_many_arguments`，已加 `#[allow]`（与 `cmd_env_set` 一致）。根治需收敛为结构体，属本波外的重构。                                                                                                                                                                    |
| **plan Step 5 的 Step 3b 实现细节** | `preview_env_change` 的 `kind`/`writable` 取法由 plan 留自由度；本次选择"`writable` = 纯 hive 写权限（不掺保护名单）"，理由是 spec 验收标准 9「预览层不拒绝」。若审核窗口认为语义应为「该变量可编辑」，需重新裁断。                                                                                           |
| **`env_hive_writable` 的实现重复**  | `env_var.rs` 的 `env_hive_writable` 与 `reg_store.rs` 的 `hive_writable`、`capabilities_for` 权限来源是同逻辑的**第三份**。plan 授权新增（避免 CLI 调 `check_admin`）。DRY 改进留待后续。                                                                                                                     |
| **版本号未升**                      | 按 Global Constraints，`package.json` / `Cargo.toml` / `tauri.conf.json` / `README` 的版本号**均未动**，CHANGELOG 段落标「5.1.5（未发布）」。                                                                                                                                                                 |
| **未提交**                          | 用户未授权提交。工作区有 10 个 `M` + 3 个未跟踪文件。                                                                                                                                                                                                                                                         |

---

## 5. 文档同步结果

| 文件           | 状态                                                                                                             |
| -------------- | ---------------------------------------------------------------------------------------------------------------- |
| `CLAUDE.md`    | 4 处内容改动（命令表 `[--dry-run] [--json]`、env 说明段、`path_backup_keep` 键、NotFound 文案）+ Prettier 规范化 |
| `AGENTS.md`    | **与 `CLAUDE.md` 字节级一致**（md5 相同，均通过 `prettier --check`）                                             |
| `README.md`    | `env` 章节补 `--dry-run` 示例 + JSON 输出示例；备份章节补 path 保留策略                                          |
| `CHANGELOG.md` | 顶部新增 5.1.5 段落（新增/变更/修复/说明四节）                                                                   |

**验证**：`cmp CLAUDE.md AGENTS.md` 无输出；`npx prettier --check` 四文件全通过。

---

## 6. GitHub issue 回填

**issue #34**：评论草稿已写（`docs/审核和开发/2026.10.16/issue-34-评论草稿.md`），内容含「修复后行为 + 根因（比 issue 描述更深：`get_raw` 丢 `ErrorKind`）+ 附带发现（W2-B1 注释与实现不符）」。
**⚠️ 尚未发布** —— `gh issue comment` 是对外公开动作，需用户明确授权后再执行（`gh` 已认证，仓库确认 `LHY0125/PathEditor`）。

**其余 issue**（#33 / #35 / #36 / #37）：合入后回填状态；**#38 应关闭为「不成立」**（核实：省略 VALUE 本就立即报错，无代码改动）。

---

## 7. 与验收标准（spec 16 条）的对照

| #   | 标准                                        | 落点                                | 状态 |
| --- | ------------------------------------------- | ----------------------------------- | ---- |
| 1   | `env get` 新文案、无 `os error`、变量名一次 | Task 2 + 端到端                     | ✅   |
| 2   | 其它错误 message 逐字节不变                 | Task 1 不变量 2 + Task 2 测试       | ✅   |
| 3   | `env list` help                             | Task 3                              | ✅   |
| 4   | `env add` help                              | Task 4                              | ✅   |
| 5   | dry-run 不写/不备份/不广播                  | Task 5 端到端 + 注册表核验          | ✅   |
| 6   | `--dry-run --json` 契约                     | Task 5 单测                         | ✅   |
| 7   | dry-run 不要 revision                       | Task 5 端到端（未传 revision 成功） | ✅   |
| 8   | 不存在仍 notFound                           | Task 5 端到端                       | ✅   |
| 9   | `add` 预览不拒绝                            | Task 5（`writable` 纯 hive 权限）   | ✅   |
| 10  | path 轮换保留 N                             | Task 6                              | ✅   |
| 11  | 对抗性零删除                                | Task 6                              | ✅   |
| 12  | `path_backup_keep` 回落                     | Task 6                              | ✅   |
| 13  | env 既有不回归                              | Task 6 + core 全量                  | ✅   |
| 14  | 两文档一致                                  | Task 7                              | ✅   |
| 15  | 质量门                                      | `verify:all` 退出码 0               | ✅   |
| 16  | 文档注释 / 无 unsafe                        | 各新增 pub fn 均有 `///`；无 unsafe | ✅   |

---

## 8. 交接状态

| 项                   | 状态                        |
| -------------------- | --------------------------- |
| Task 1–7 实现        | ✅ 完成                     |
| 质量门 `verify:all`  | ✅ 全绿                     |
| plan 缺陷修正        | ✅ 7 处已就地修正           |
| 开发回执             | ✅ 本文件                   |
| issue #34 评论       | ⏳ 草稿就绪，**待授权发布** |
| 提交 / 推送 / 升版本 | ⏸️ **未授权，未做**         |

**下一步**：审核窗口走 ④ 审核代码 → ⑤ 写审查报告。
