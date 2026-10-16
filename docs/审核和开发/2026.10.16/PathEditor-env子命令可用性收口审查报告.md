# PathEditor env 子命令可用性收口 审查报告

- **审查日期**：2026-10-16
- **被审改动**：工作区未提交改动（baseline = HEAD `433bbfe`「chore: 发布 5.1.4」）
- **基准点**：`433bbfe`
- **范围**：10 个已改文件（6 源 + 4 文档），**+813 / −32 行**；另 3 个未跟踪文件（spec / plan / 本目录文档）
- **功能**：修复 GitHub issues #33 / #34 / #35 / #36 / #37 / #38（`env` 子命令可用性）
- **规格来源**：`docs/superpowers/specs/2026-10-16-env-usability-fixes-design.md`、`docs/superpowers/plans/2026-10-16-env-usability-fixes-implementation.md`
- **规范来源**：`AGENTS.md` / `CLAUDE.md`、`CONTRIBUTING.md`
- **开发回执**：`docs/审核和开发/2026.10.16/PathEditor-env子命令可用性收口-开发回执（第2轮）.md`
- **复现命令**：`git diff`（工作区未提交）；`cargo test --workspace`；`npm run verify:all`

> **审查环境说明**：本波开发窗口**未提交**（用户未授权），故审查对象是工作区 diff，非某分支。`git diff --stat` 与回执声明的 +763/−20（仅 `core/ cli/`）之差 = 文档改动量。

## 提交清单

无（未提交）。工作区改动：

```
M AGENTS.md  M CHANGELOG.md  M CLAUDE.md  M README.md
M cli/src/env_ops.rs  M cli/src/main.rs
M core/src/backup.rs  M core/src/reg_store.rs  M core/src/registry.rs  M core/src/registry/env_var.rs
?? docs/superpowers/{specs,plans}/2026-10-16-*  ?? docs/审核和开发/2026.10.16/
```

## 一、验证证据

**审核窗口亲自执行**（未采信回执数字）：

| 命令                                                    | 结果                                                                                      |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `cargo test --workspace`                                | core **219 passed / 0 failed / 2 ignored**；CLI **59 passed / 0 failed**                  |
| `cargo clippy --workspace --all-targets -- -D warnings` | **零警告**                                                                                |
| `cargo fmt --check`                                     | 干净（exit 0）                                                                            |
| `cmp CLAUDE.md AGENTS.md`                               | 无输出（**字节级一致**）                                                                  |
| `npm run verify:all`                                    | **退出码 0**（Prettier→ESLint→构建→覆盖率→fmt→Clippy→Rust 测试→Playwright **30 passed**） |
| `git diff` 新增 `unsafe`                                | **无**                                                                                    |
| 真实注册表写入                                          | **零**（测试走 `MemoryHive` / 临时目录）                                                  |

与回执声明**逐项吻合**。

**⚠️ 绿灯 ≠ 规格场景全覆盖**：上述 16 条验收标准中，**第 5 条（`env set/add/remove --dry-run` 不写注册表 / 不产生备份 / 不广播）没有自动化测试覆盖** —— 它只经开发窗口的**手工端到端**验证。详见 §三 (a)-1。

## 二、Standards 轴

（本轴由审核窗口 + 一个独立 `general-purpose` 子代理**背对背**完成，两者独立得出同一处硬性违反 —— 见 §二.1。）

### 硬性违反

**S-1：`core/src/registry/env_var.rs:394-404` 文档注释与实现不符（"注释撒谎"）**

```rust
/// 读取某变量的注册表类型标签（`"string"` / `"expand"`），供预览用。
///
/// 读取失败时回落 `"string"`（预览是尽力而为；变量不存在已在 `reveal_env_var_in_store` 处返回 `Err`）。
fn env_kind_label(store: &dyn EnvHiveStore, name: &str) -> Result<String, CoreError> {
    let raw = store.get_raw_kind(name)?;   // ← `?` 直接传播 Err，无任何回落
    Ok(match EnvValueKind::from_reg_type(raw.vtype) {
        EnvValueKind::ExpandString => "expand",
        _ => "string",
    }
    .to_string())
}
```

- 注释声称「**读取失败时回落 `"string"`**」，但实现用 `?` **直接传播 `Err`**，不存在读取失败回落分支。真正的「回落」是 `match` 的 `_ => "string"` —— 那是把 **`Unsupported` 类型**映射成标签，与「读取失败」是两个不同场景。
- **为什么重要**：这正是本仓 `CLAUDE.md` 点名的历史事故类型（W2-B1 声明行为与实现不符；本波 #34 的根因就是这类）。读者据此注释会误以为「预览不会因读取失败报错」，从而误判 `preview_env_change` 的错误契约。**违反 Global Constraints 的注释一致性要求。**
- **修复**：注释改为如实描述 —— 「`Unsupported` 类型回落 `"string"`；读取失败经 `?` 传播」。一行改动。

### 判断性意见

**J-1：`env_var.rs:406-418` `env_hive_writable` 与 `reg_store.rs:79-84` `hive_writable` 是重复实现**

两函数体逐字相同（`System → check_admin` / `User → can_write_user`）。分叉风险真实：一旦将来一侧调整（如加 UAC 虚拟化判定），权限判定会出现两套 —— 正是分层约束要防的。建议把 `reg_store::hive_writable` 提为 `pub(crate)` 复用。**非阻断**（本波只读预览用，两处当前一致）。

**J-2：`env_var.rs:380-392` `preview_env_change` 对同一变量做两次注册表读取**

先 `reveal_env_var_in_store`（内部 `get_raw_kind`），再 `env_kind_label`（又一次 `get_raw_kind`）。两次调用之间变量可能被外部改写，使 `old_value` 与 `kind` 来自不同时刻 —— 同一「预览时刻」内自相矛盾。`reveal_env_var_in_store` 内部本就有 `vtype`（见 `:463`），可一并返回复用，省一次 IO 且消除窗口。**非阻断**（CHANGELOG 的 TOCTOU 免责覆盖了跨时刻；此为同一时刻内的粗糙）。

**J-3：`env_ops.rs:164-189` `render_change_preview_text` 用 JSON 值比较判 hive**

`if v["hive"] == "system"` —— 呈现层判定（选中文标签），不涉安全规则，符合「CLI 只做呈现」。但比传 `EnvHive` 枚举脆弱。**非阻断**。

### 合规亮点

- **分层边界严守**：新增三个判定入口（`preview_env_change` / `env_hive_writable` / `get_raw_kind`）**全在 core**；`env_ops.rs` 无任何保留名/保护名单/hive 权限/revision 的本地判定复制。CLI 只消费 `preview.old_value/kind/writable`。
- **错误契约严守**：`NotFound` 判定用 `e.code == ErrorCode::NotFound`（结构化 code）；全仓库无新增「按 message 文本做控制流」。
- **`get_raw_kind` 默认设计**：trait 默认实现退回 `get_raw` 包 `Io`，两个 impl 覆写 —— 对第三方实现零破坏，是 N1 旁路方案的干净落地。
- **`#[allow]` 使用精确**：仅 `cmd_env_set`（9 参）与 `cmd_env_add`（8 参）加 `#[allow(clippy::too_many_arguments)]`（阈值 7 确实触发）；`cmd_env_remove`（6 参）**未**加 —— 无滥用。
- **注释坦诚度（加分）**：`backup.rs` 主动声明「轮换失败只 `log::warn!`，CLI 未初始化 logger 故不可见……**不要**误以为用户会看到提示」；`env_var.rs:157-163` 的「邻近改动禁区」注释防止后来者误「统一」两套读法。这**正是**本仓要求的「如实措辞」文化的体现。
- **测试可证伪性**：`backup_registry_triggers_path_rotation`（接线测试，注释明说「单测证明不了被接线」）、`rotate_path_backups_never_deletes_foreign_files`（对抗性零删除）、help 文案测试（断具体短语而非「解析成功」）—— 均非空转测试。

## 三、Spec 轴

### (a) 规格要求但缺失或只做一半

**SP-1【唯一实质缺口】验收标准 5 的 `env set/add/remove --dry-run` 写安全，无自动化测试**

spec 验收标准 5 要求：「`--dry-run` 不写注册表、不产生备份、不广播」。实测：

- 新增的 12 个测试函数中，`--dry-run` 相关**只有 3 个**（`change_preview_json_has_stable_contract` / `_nulls_are_correct_per_operation` / `_text_mentions_no_write`），全部只测**纯辅助函数** `build_change_preview` / `render_change_preview_text`。
- `cmd_env_set/add/remove` 三个函数**从未被任何测试调用**（grep 确认：仅 `:386/439/481` 三处定义，无调用点）。
- 对比：`env restore --dry-run` **有**真正的 `--bins` 进程级测试（`env_restore_dry_run_previews_without_writing:1354`），证明它「不写注册表、不新增文件」。
- 结果：**「CLI dry-run 不写注册表」这一 P0 安全性，本波只经开发窗口手工端到端验证，无回归防护**。将来若有人在 dry-run 分支后误加一行写入，测试不会红。

**为什么重要**：dry-run 的核心价值就是「绝不产生副作用」；这正是最该被测试钉住的性质。本波为它写了纯函数测试，却漏了「命令真的没写」这一层 —— 属「只做一半」。

**建议**：补一个 `--bins` 进程级测试（仿 `env_restore_dry_run_previews_without_writing`）：跑 `env add <PROBE> v --dry-run --json`，断言退出码 0 + stdout 含 `"oldValue":null`，并断言**未产生备份文件**（`PATHEDITOR_BACKUP_DIR` 指向临时目录，断言目录为空）。**注册表零写入**在无提权测试环境下无法直接断言（不能写 HKCU），故至少钉住「不备份」这一可观测副作用；若用户授权，可加真实注册表前后比对。

**SP-2【需审核窗口裁断 → 见下】`writable` 语义**

开发回执 §4 主动请裁断：`preview_env_change` 的 `writable` 采用「**纯 hive 写权限**」（不含保护名单/类型）。**审核窗口裁断：正确，维持。** 理由：spec §命令规格 4 的字段表写「该 hive 是否可写」，验收标准 9 明确要求「`add` 预览对已存在同名变量**照常展示** `writable`，不在预览层拒绝」。若掺入保护名单，「windir」这类会显示 `writable:false`，但那是**执行阶段**才该拒绝的事 —— 预览层拒绝会让用户以为「预览说不行、执行也一定不行」，与「保护名单照常展示、执行时由 core 拒」的既有 E5 裁断（#36 spec）一致。**维持开发窗口的选择。**

### (b) 规格没要求但做了的行为（scope creep）

**合理但未在规格内**：

- **`backup_registry_triggers_path_rotation` 接线测试** —— plan 之外的补强。开发窗口理由：「单测只证明函数对，证明不了它被调用」。**属正当补强**，且经变异测试证明有效（禁用接线 → 转红）。建议回填 spec 测试策略段（非必须）。
- **`#[allow(clippy::too_many_arguments)]` 加到 `cmd_env_add`** —— 非规格要求，但 `cmd_env_add` 增至 8 参确实触发 clippy，属必要技术债处理。

### (c) 实现与规格/计划不符

**无实质偏离。** 开发回执 §3 列的 7 处均为**plan 缺陷的就地修正**（N6 `mut`、Task 4 测试位置、Step 7 命令笔误、Step 3b 示意代码失实、`read_backup_keep` 3 参、接线测试缺失、文档不一致），审核窗口逐条复核**全部成立且已折入 plan**。特别是 `read_backup_keep` 从 2 参改 3 参（fallback 由调用方给，而非函数内硬编码 key→fallback 映射）—— **这是对审核窗口 plan 的正确改进**（原 2 参版把「哪个 key 对应哪个回落值」的知识硬编码进通用函数，是「伪装成通用化的特化」）。

**测试缺口复核**（skill 要求单独较真）：

- 公开 API 是否有直接调用测试？`preview_env_change` / `env_hive_writable` / `get_raw_kind` **均有**（通过 `reveal_*` / `get_raw_kind_*` 用例间接覆盖 core 行为）。✅
- P0 安全场景是否只在 mock/前端单测里间接覆盖？**是** —— SP-1 指出的 dry-run 写安全即此问题（仅手工验证，无自动化）。⚠️
- `npm run verify` 全绿**不能**证明上述被覆盖 —— 已在此明确指出。

## 四、总结

**质量评价：高。** 6 条 issue 一次收口，改动与 spec/plan 逐处对应，7 处 plan 缺陷被开发窗口逐条发现并就地修正（其中 3 处是「plan 描述函数长什么样、却跳过它如何被使用/被测」类），2 处对 plan 的正确改进（`read_backup_keep` 3 参、接线测试补强）。分层、错误契约、文档一致、测试可证伪性均达标。

**合并前建议整改（按优先级）**：

1. **【必须改】S-1**：`env_var.rs:396` 注释改如实（「`Unsupported` 回落 `string`；读取失败经 `?` 传播」）。一行。属仓库点名的「注释撒谎」类型。
2. **【建议改】SP-1**：补 `env set/add/remove --dry-run` 的 `--bins` 写安全测试（至少钉「不产生备份」）。这是验收标准 5 的唯一实质缺口。
3. **【可延后】J-1**：`env_hive_writable` 与 `hive_writable` 去重（提 `pub(crate)`）。
4. **【可延后】J-2**：`preview_env_change` 复用 `reveal_env_var_in_store` 的 `vtype`，省一次读、消除同刻窗口。
5. **【可延后】J-3**：`render_change_preview_text` 改传 `EnvHive` 枚举。

**本轮判定：需要整改后复审（仅 1 项必须改 + 1 项建议补）。**

S-1 是一行注释修正、SP-1 是一个测试补充，均无架构影响。整改后由开发窗口写**复审回执**，审核窗口复验 S-1/SP-1 落地即可放行。

**未覆盖项 / 残余风险（照实登记，不替开发窗口兜底）**：

| 项                                 | 说明                                                                                                                                                                                  |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| dry-run 写安全的自动化缺口         | 见 SP-1，本波仅手工验证                                                                                                                                                               |
| 接线测试的跳过断言                 | `backup_registry_triggers_path_rotation` 在无权限读注册表时断言被 `if is_ok()` 跳过（与既有 `backup_registry_with_custom_dir` 同手法）。本机实测返回 `Ok`，变异测试已证明断言真实执行 |
| 轮换失败静默                       | `rotate_path_backups` 失败只 `log::warn!`，CLI 不可见。**有意**（与 env 侧对称），属独立 observability 议题                                                                           |
| `cmd_env_add` 参数过多             | 8 参触发 clippy，已 `#[allow]`；根治需收敛为结构体，属本波外重构                                                                                                                      |
| `env_hive_writable` 第三份权限实现 | 见 J-1                                                                                                                                                                                |
| 版本号未升                         | 按 Global Constraints，四处版本号均未动，CHANGELOG 标「5.1.5（未发布）」                                                                                                              |
| 未提交、issue 未回填               | 用户未授权；`gh issue comment 34` 草稿就绪待授权                                                                                                                                      |

**下一步**：开发窗口整改 S-1（必须）+ SP-1（建议）→ 写复审回执 → 审核窗口复验。

---

## 五、补充：提交后安全审查（2026-10-16，审核窗口）

在 6 个提交落盘后、**推送前**，一次后台安全审查报出 2 条发现（均在 `cli/src/env_ops.rs` 的 `--dry-run` 预览路径）。审核窗口独立核实**两条均成立**，并在推送前修复（追加提交 `a931288`）。

### 发现 1【高危】敏感值明文泄漏 —— 已修复

`build_change_preview` 的 `oldValue` 来自 `preview_env_change → reveal_env_var_in_store`，返回**完整明文且无 `is_sensitive` 检查**。后果：

```bash
patheditor env set MY_API_TOKEN --value "new" --dry-run
# 修复前：将更新用户变量 MY_API_TOKEN：sk-realkey... → new（未写入任何内容）
```

**旧密钥明文打印到终端**（并可能进 shell 历史 / CI 日志）。**这与仓库既有约定冲突**：`env list` 对敏感值打码 `(敏感)`，`env get` 被文档明确为「CLI 侧**唯一**明文出口」。新 dry-run 路径**新开了第二个明文出口**，而 spec 未覆盖此点。

**修复**（`a931288`）：`preview_env_change` 命中 `is_sensitive` 时不回传明文（`old_value = None`、新增 `sensitive: true` 字段）；`env add` 的新值经新增的 `mask_new_value` 同样打码；文本预览显示 `(敏感)`。JSON 契约加 `sensitive` 字段以区分「打码」与「空值」。

### 发现 2【中危】输出注入（ANSI 逃逸）—— 已修复

`render_change_preview_text` 把注册表值**原样**格式化进终端输出，未做控制字符净化。且既有 `sanitize_preview`（`env_var.rs`）**只剔除 `\r`/`\n`/`\0`，不剔除 `\x1b`（ESC）** —— 即它对「多行日志注入」有效，对「终端 ANSI 转义注入」无效。

**修复**（`a931288`）：把 `sanitize_preview` 的剔除范围扩展为**全部 C0 控制字符（U+0000–U+001F）与 DEL（U+007F）**，覆盖 ESC。这是对**所有**展示路径（`env list` / `--dry-run`）的严格改进，既有 `sanitize_preview` 测试无回归。

### 证据

| 检验                                                 | 结果                                                 |
| ---------------------------------------------------- | ---------------------------------------------------- |
| `cargo test --workspace`                             | core 219 / CLI **62**（+3 安全测试）passed，0 failed |
| `cargo clippy --workspace --all-targets -D warnings` | 零警告                                               |
| `cargo fmt --check`                                  | 干净                                                 |
| `npm run verify:all`                                 | 退出码 0（E2E 30 passed）                            |
| 端到端 `env add MY_API_TOKEN sk-secret123 --dry-run` | 输出 `(敏感)`，**无明文**                            |
| 端到端 `env add <非敏感> hello --dry-run`            | 正常显示值                                           |
| `cmp CLAUDE.md AGENTS.md`                            | 字节一致（JSON 契约的 `sensitive` 字段已同步）       |

### 对本报告判定的影响

- **S-1** 已在提交前修复（`env_kind_label` 注释如实化）。
- **SP-1**（dry-run 写安全无自动化测试）**仍为未覆盖项** —— 本次新增的 3 个测试测的是**打码与净化**（纯函数），**未**测「命令真的不写注册表/不备份」。SP-1 结论不变。
- 新增发现 1 / 2 **已修复并测试**，不构成新的待整改项。

**修正后的判定：S-1 已修、发现 1/2 已修；仅 SP-1（测试缺口）作为已知未覆盖项保留。** 是否补 SP-1 由用户决定（本波已提交，补 SP-1 需再追加提交）。

---

## 六、SP-1 关闭（2026-10-16，审核窗口）

用户要求发版前解决 SP-1。已补 3 个 `--bins` 进程级测试（提交 `5a6d9ef`）：

| 测试                              | 断言的可观测副作用                                                                               |
| --------------------------------- | ------------------------------------------------------------------------------------------------ |
| `probe_absent_after_dry_run_add`  | dry-run 后该变量在注册表中**不存在**（用只读 `env get` 反证：退出码 1 = notFound；若误写则为 0） |
| `dry_run_produces_no_backup_file` | dry-run 后 `PATHEDITOR_BACKUP_DIR` 指向的目录**为空**（无 `env_backup_*.json`）                  |
| `dry_run_json_end_to_end_shape`   | `--json` 端到端的字段契约（operation / name / oldValue=null / newValue / sensitive）             |

**可证伪性已验证（变异测试）**：在 `cmd_env_add` 的 dry-run 分支注入一次 `create_env_var` 后，前两个用例**转红**（`dry_run_produces_no_backup_file`、`probe_absent_after_dry_run_add` 均 FAILED）；移除变异后恢复全绿。证明这两个用例确实在守护「dry-run 不写」这一性质，而非空转。

**副作用清理**：变异测试期间向真实 HKCU 写入了 3 个探针变量（`PATHEDITOR_SP1_*`），已全部删除（复核计数为 0）。

**结果**：CLI 测试 59 → **65**（+3 SP-1 + 早前 +3 安全），全绿；`npm run verify:all` 退出码 0。**SP-1 关闭，三档发现（S-1 / 发现 1 / 发现 2 / SP-1）全部处置完毕。**

## 七、发布（2026-10-16，审核窗口）

- 版本号升级至 **5.1.5**（`package.json` / `package-lock.json` / `Cargo.toml` / `Cargo.lock` / `gui/tauri.conf.json`（version + 窗口标题）/ README 徽章与安装包名，六处对齐；CHANGELOG 段落改为「5.1.5 (2026-10-16)」）。提交 `4e2c9c4`。
- `cargo build -p patheditor-cli` 后 `patheditor --version` 输出 **`patheditor 5.1.5`**。
- **审查判定：全部整改完成，可发布。**

## 八、Issue 回填（2026-10-16，审核窗口）

v5.1.5 发布后回填全部 issue：

| Issue | 处置                             | 说明                                                           |
| ----- | -------------------------------- | -------------------------------------------------------------- |
| #34   | ✅ 关闭（completed）             | 已有修复评论（含「根因比 issue 更深 + W2-B1 注释与实现不符」） |
| #33   | ✅ 关闭（completed）             | 回填：改文案；并更正「preview 未截断」的误判（截断早已实现）   |
| #35   | ✅ 关闭（completed）             | 回填：纯新建语义是正确答案；已把豁免理由写进 help              |
| #36   | ✅ 关闭（completed）             | 回填：--dry-run/--json 用法与语义；含敏感值打码加固            |
| #37   | ✅ 关闭（completed）             | 回填：更正「env 侧已有轮换」；path 侧补保留策略                |
| #38   | ✅ 关闭（completed，**不成立**） | 回填：省略 VALUE 本就立即报错，无代码缺陷                      |
| #32   | ⏸ **保持开放**                   | 批量往返需跨命令 session，属架构变动，明确不在本波             |

回填评论原文留档于本目录 `issue-3X-回填.md`。
