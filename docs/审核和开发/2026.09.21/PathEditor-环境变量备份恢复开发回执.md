# PathEditor 5.1.4 环境变量备份与恢复 — 开发回执

**日期**: 2026-09-22
**分支**: `worktree-env-backup-restore`（worktree 隔离）
**范围**: 5.1.4「环境变量备份与恢复」特性，Task 1–8 实现 + Task 9 收口
**BASE**: `0da96a7` → **HEAD（Task 9 提交前）**: `455c376`
**收口轮提交**: 见文末第 7 节

## 1. 任务映射

| 任务 | 内容                                                        | 提交                                                                 |
| ---- | ----------------------------------------------------------- | -------------------------------------------------------------------- |
| T1   | 环境变量备份数据模型与采集（含 B1 的 Unsupported 跳过语义） | `26b262a`                                                            |
| T2   | env 备份落盘、`Versioned` 信封与保留策略轮换                | `45efc2c`, `787d560`, `c77adc5`                                      |
| T3   | 5 个 env 写入口挂载写前备份，返回 `WriteOutcome`            | `13d1334`, `648bc98`（补回丢失的 `broadcast_env_change`）, `93ef8e4` |
| T4   | env 备份列表与恢复路径校验                                  | `358c4b7`, `0188eb1`, `3a9dc14`                                      |
| T5   | env 备份差异计算（新增 / 删除 / 冲突）                      | `fe7d1d9`                                                            |
| T6   | env 备份恢复执行与冲突中止语义                              | `fa40703`, `f4ee616`, `a38992b`, `0648392`                           |
| T7   | CLI `env backup` / `env backups` / `env restore` 三个子命令 | `f980c61`, `0619bc0`                                                 |
| T8   | GUI 备份与恢复界面，含差异预览与冲突二次确认                | `2faa0f1`, `455c376`                                                 |
| T9   | 文档同步与收口（本文件）                                    | 见第 7 节                                                            |

累计 19 个提交，`git log --oneline 0da96a7..HEAD` 可复现；
`git diff --stat 0da96a7..HEAD` = **30 files changed, 6438 insertions(+), 86 deletions(-)**。

## 2. Execution Notes（计划原文 / 实际 / 处理）

> 本波计划在执行中暴露了 13 处「计划与实现不符」或「计划代码有实质缺陷」。逐条列出，
> **无一项被静默丢弃**。

| #   | 计划原文                                                                         | 实际                                                                                                                                                               | 处理                                                                                                                                       |
| --- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | Task 3 的 `WriteOutcome` 定义未声明 serde 派生                                   | Tauri command 的返回值出境必须可序列化；缺派生则 `gui` crate 编译不过                                                                                              | 补 `Serialize` / `Deserialize`                                                                                                             |
| 2   | Task 4 `validate_backup_path_rejects_invalid` 的两条断言                         | 两条断言各自被**另一条规则**兜住（`SAM` 无扩展名但会被来源规则拒；`C:\some\other\file.json` 不存在会被存在性规则拒）——删掉任一条规则断言照样通过，是**自证的断言** | 新增 `validate_backup_path_rejects_each_rule_in_isolation`，为每条规则各造一个「只有该规则能拒」的输入；原测试保留（覆盖面不同）           |
| 3   | Task 5 `preview_classifies_added_modified_removed`                               | **不可满足**：`revision_of` 是 `(name, type, value)` 的纯函数，「值变了」与「备份已过期」是同一条件，`Modified` 变体在当前实现下不可达                             | 改写为 `preview_classifies_added_conflict_removed`，并显式断言 `modified == 0` 钉住不可达性                                                |
| 4   | Task 5 `preview_ignores_unsupported_current_vars` 用 `REG_DWORD`                 | `REG_DWORD` 本就**解码失败**（`String::from_reg_value` 对非字符串类型返回 `Err`），该变量在解码阶段即被滤掉 → `is_writable()` 判定**删掉也照样通过**，不可证伪     | 改用 `REG_MULTI_SZ`（可解码成字符串，只有 `is_writable()` 能拦下）；`REG_DWORD` 作为实际最常见类型保留为第二例                             |
| 5   | Task 2 的 S1 对抗性测试用 9 个唯一文件名                                         | `keep = 20`，候选数 ≤ keep 时 `rotate_env_backups` **提前返回、一份不删** → 测试**空转**，「未删除外部文件」的断言失去意义                                         | 改为 25 个唯一名，并加前置断言 `removed.len() == 5`，删除未发生即失败                                                                      |
| 6   | Task 2 计划「先轮换、后写入」                                                    | 该顺序下目录稳态为 `keep + 1` 份（`keep` 份时轮换无可删、写入后变 `keep+1`），与验收标准 11 的端到端口径不符                                                       | 改为**先写入、后轮换**，稳态恰为 `keep` 份；新增端到端测试 `write_env_backup_steady_state_is_exactly_keep_files`（函数级测试看不到该偏差） |
| 7   | **Task 3 brief 的示例代码**                                                      | 泛型化改写时 `broadcast_env_change()` 在 5 个写入口**整体丢失**——**Critical**：写注册表后不广播，已运行进程读不到新值                                              | 5 处全部补回（`648bc98`），并加守护测试                                                                                                    |
| 8   | Task 3 计划抽 `apply_concurrency` / `apply_core_result`                          | 泛型化方案落地后这两处成为**死代码**                                                                                                                               | 删除                                                                                                                                       |
| 9   | Task 7 brief 的 `test_persist_lock`                                              | 该函数是 `core` crate 内部（`#[cfg(test)] pub(crate)`）的**同进程**锁；`--bins` 是独立进程，进程内锁跨进程无意义，且从 `cli` crate 不可达                          | 删掉取锁行，注释说明理由                                                                                                                   |
| 10  | Task 7 计划用 `env!("CARGO_BIN_EXE_patheditor")` 定位测试二进制                  | 该变量**只对 `tests/` 下的集成测试可用**，对 `--bins` 单测不可用                                                                                                   | 改为测试内 `cargo build` + `cargo metadata` 读 target 目录（不写死 `target/`）                                                             |
| 11  | Task 3 的 `broadcast_after_write` doc 曾写「生产构建不含此分支（`cfg!(test)`）」 | **写反了**：`cfg!(test)` 在测试构建里为真，doc 描述的分支方向与实际相反                                                                                            | 改为如实描述（`93ef8e4`）                                                                                                                  |
| 12  | Task 6 空备份 doc 曾写「默认模式不写任何内容」                                   | 与**自己的配对测试**矛盾：空备份产生零冲突，`conflicts > 0` 的中止条件根本不触发，默认模式会照常删光                                                               | 改为如实陈述，并在 doc 中显式固定该后果（`0648392`）                                                                                       |
| 13  | 计划原文含 2 处 U+FFFD                                                           | 其中一处会落入源码 doc comment（复制粘贴所致）                                                                                                                     | 已修                                                                                                                                       |

## 3. 质量门终态数字

**口径声明**：worktree 路径
`D:\Code\doing_exercises\programs\PathEditor\.claude\worktrees\env-backup-restore`；
分支 `worktree-env-backup-restore`；HEAD `455c376`（Task 9 文档改动前）；
时间点 2026-09-22 04:51–04:56 (+08:00)；工具链 Node v24.19.0 / Vitest 4.1.7 /
cargo 1.96.0 (30a34c682 2026-05-25) / rustc 1.96.0 (ac68faa20 2026-05-25)。
**所有数字逐项取自下方命令的原始输出**，未做换算或估算。

| 质量门                   | 命令                                                    | 实测结果                                                                                                                                                                                                 |
| ------------------------ | ------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Prettier                 | `npm run format:check`                                  | 通过（`All matched files use Prettier code style!`）                                                                                                                                                     |
| ESLint                   | `npx eslint .`                                          | `✖ 3 problems (0 errors, 3 warnings)`——3 条均为 React Compiler 对 `useVirtualizer()` 的已知跳过提示（`react-hooks/incompatible-library`），非本波引入                                                    |
| 构建                     | `npm run build`                                         | `tsc -b` 零错误；`vite build` ✓ 107 modules transformed，`built in 1.88s`                                                                                                                                |
| Vitest（覆盖率模式）     | `npm run test:coverage`                                 | **21 files / 305 passed**（`Test Files 21 passed (21)` / `Tests 305 passed (305)`）                                                                                                                      |
| 覆盖率                   | `npm run test:coverage`                                 | Statements **86.61%** (731/844) / Branches **77.17%** (372/482) / Functions **90.54%** (134/148) / Lines **88.34%** (644/729)；行覆盖门槛 80%（`vitest.config.ts:18-20`）**达标**                        |
| `cargo fmt --check`      | `cargo fmt --check`                                     | 通过（无输出）                                                                                                                                                                                           |
| Clippy                   | `cargo clippy --workspace --all-targets -- -D warnings` | 通过（`Finished \`dev\` profile ... in 1.29s`，零警告）                                                                                                                                                  |
| `cargo test --workspace` | `cargo test --workspace`                                | **260 passed / 2 ignored / 0 failed**——按 target 分行：`path_editor_core` **207 passed / 2 ignored**；`app_lib`（gui lib）0；`PathEditor`（gui bin）0；`patheditor`（cli bin）**53 passed**；Doc-tests 0 |
| CLI 单测                 | `cargo test -p patheditor-cli --bins`                   | **53 passed / 0 failed / 0 ignored**（与 workspace 中的 cli 行一致）                                                                                                                                     |
| Playwright E2E           | `npm run test:e2e`                                      | **30 passed (17.8s)**                                                                                                                                                                                    |

## 4. 未覆盖项（如实登记，不美化）

1. **真实注册表恢复闭环未做**。恢复会写 HKLM/HKCU，E2E 一律走 mock IPC，**本波未获授权**做真实注册表写入，因此「`env restore` 打通真实注册表」这条端到端路径**从未被真实执行过**。CLI 侧 `env restore` 的重启后可观测结果、GUI 侧经 Tauri IPC 的真实写入均未验证。
2. **备份目录 ACL / 文件权限收紧未做**（spec §S2.1，核对轮 C5 裁断登记）。Windows 上需 `icacls` 或 Win32 API，实现复杂度未评估。**本波收到一条独立的安全复审告警（MEDIUM：凭据写入未收紧文件权限）——该告警确认了 S2.1 登记的风险真实可触发**，不是理论风险。
3. **`restore_env_backup_from` 无单测**。该函数的第一行 `validate_backup_path` 与最后的 `broadcast_env_change` 都不在 core 单测覆盖内：前者由 `validate_backup_path` 自身的测试覆盖，后者是 Win32 `SendMessageTimeoutW` 调用（生产约 4s，core 单测里既不该触发也无法观测）。core 单测只覆盖到 `restore_in_stores`（决策层），**恢复的公开入口本身未被单测执行过**。
4. **`collect_env_backup` 硬编码真实 hive**（`WinregHive::open(...)`），只读、不可注入存储，因此 core 单测覆盖的是 `collect_hive_vars_in_store`（可注入版）而非公开入口。
5. **GUI 手工冒烟未执行**。GUI 备份/恢复对话框的实际渲染、点击、`window.confirm` 替换后的异步对话框交互均未在真实 Tauri 运行时下人工验证；覆盖它的是 mock IPC 的 E2E 与 jsdom 单测。
6. **CI 不跑任何测试**。仓库唯一 workflow 是 `.github/workflows/release.yml`（tag 触发），**零测试步骤**（`grep -n "vitest\|cargo test\|npm test\|playwright" .github/workflows/*.yml` 无命中）。`npm run verify` **不含 e2e**（`package.json` 的 `verify` 脚本链：format:check → lint → build → test:coverage → cargo fmt → clippy → cargo test）。因此**本波的全部质量门证据都是本地证据**，无任何 CI 背书。
7. **恢复的 `1 MiB` 大小上限、`--dry-run` 与校验之间无 TOCTOU 防护**（`validate_backup_path` 原样返回路径、不做 canonicalize，随后的读取与校验是两次独立文件系统调用）。CLI 为单线程，该窗口是理论性的，本波不解决，已在 doc 中如实标注。
8. **`validate_backup_path` 的目录判断用 `parent == env_backup_dir()`**：入参目录若被 `PATHEDITOR_BACKUP_DIR` 设成相对路径，该等值判断与用户给的绝对路径不相等，只能靠 `env_backup_` 前缀兜底。不构成安全缺陷（前缀规则本就独立生效），但「在备份目录内即可」这条语义在相对路径下**不成立**。

## 5. 需要审核窗口重点关注的项

### 5.1 待裁断项（规格级，本波未改）

| #      | 问题                                 | 背景                                                                                                                                                                                                                                                                                                                                              | 建议方向                                                                                                                                                                                                             |
| ------ | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **D1** | `Modified` / `Conflict` **不可共存** | `revision_of` 是 `(name, type, value)` 的纯函数，「值变了」与「备份已过期」**是同一个条件**。`diff_one_hive` 一律判 `Conflict`，`Modified` 变体**当前不可达**（保留在枚举中，供恢复执行侧合并 `Modified \| Conflict`）。后果是四个计数里 `modified` 恒 0                                                                                          | 是否把「冲突」改为 `RestoreChange` 上的**布尔属性**，使 `Added` / `Modified` / `Removed` 三个 kind 都是**动作**，`conflicts` 成为其中「已过期」者的计数？这样四类计数**全部有意义**，`--force` 的 dry-run 也不再低报 |
| **D2** | `RestoreOutcome` **无 hive 级字段**  | spec 写「退出码取最严重者」（`docs/superpowers/specs/2026-09-21-env-backup-restore-design.md:132`），但 `RestoreOutcome { applied, skipped, failures }` 里没有任何 hive 维度的成功/失败标记，**该句在现有类型里无法表达**。本波裁定接受现状：枚举失败在任何写入前**整体中止**，比 spec 字面**更安全**（宁可什么都不做，也不留「半个 hive 已改」） | spec 措辞是否随之修订为「任一 hive 枚举失败即整体中止」？若坚持 hive 级语义，需给 `RestoreOutcome` 加 hive 维度字段                                                                                                  |
| **D3** | **`modified` 恒 0 导致的低报**       | `--dry-run` 与 GUI 确认弹窗的「修改 N」**恒显示 0**；`--force` 下被冲突覆盖的变量**确实被改写却计入 `conflicts`**——两者都**低报** force 模式的实际改动量。这是 D1 的直接后果，**不是已修复项**。CLI 与 GUI 都被明文禁止自行换算（那会复制 core 的判定）                                                                                           | 与 D1 合并裁决                                                                                                                                                                                                       |
| **D4** | **e2e 不在 CI**                      | `npm run verify` 不含 e2e；唯一 workflow 是 `release.yml` 且零测试步骤 → 本波质量门证据**全部是本地证据**。是否加 CI 测试步骤？                                                                                                                                                                                                                   | **范围外决策，本波未做**。建议下一波或独立任务处理                                                                                                                                                                   |
| **D5** | **未提权恢复不可用且无降级路径**     | 普通用户无法以写权限打开 HKLM，`restore_env_backup_from` **恒**返回 `permissionDenied`（退出码 1，spec 允许）；即使备份只含 user hive 变量也一样失败。GUI 恢复入口在未提权时**不可用**，且 **UI 不提供**「仅恢复 user hive」这类降级路径。控制器已裁定 GUI 采用「**提示需管理员、不禁用按钮**」                                                   | 现状已登记。是否需要降级路径（仅恢复 user hive）？                                                                                                                                                                   |
| **D6** | **`env restore` 有逐条失败仍退 0**   | 与 brief 一致（恢复是 best-effort），但脚本**无法从退出码检出部分失败**——只能解析 `--json` 输出的 `failures` 数组，或 grep stderr 的中文警告                                                                                                                                                                                                      | 是否引入「有失败则非零」的模式（如 `--strict`）？                                                                                                                                                                    |

### 5.2 其他需关注项

- **e2e 覆盖面**：`e2e/tests/env-backup.spec.ts` 有 4 例（确认弹窗内容、取消不恢复、`code=conflict` 二次确认、权限不足提示），均为 mock IPC。**没有一例走真实恢复**（见未覆盖项 1）。
- **恢复不产生新备份**（C7 裁断）：CLI 与 GUI 的提示都是**打印命令**、绝不自动执行（自动备份会与保留策略互相吞噬）。核对该行为是否符合运维直觉。
- **空备份语义已被显式固定**（`restore_in_stores` doc）：`{"capturedAt":1,"hives":{}}` 反序列化成功 → 当前**全部**可写变量判 `Removed` → 等价于「清空环境变量」。默认模式的冲突中止**根本不会触发**（零冲突）。该行为由两个测试钉住，若将来要加防护它们会先红——**那是有意的**。

## 6. 反思与改进建议（供下一波写计划参考）

### 6.1 「测试看似通过但测不到东西」——本波**六次**命中

①断言了永远不成立的结论（`modified == 1`）；②对抗性测试因取值不当未进入目标路径（9 个文件名 vs `keep=20`）；③两条 `.is_err()` 断言被**其它规则**兜住（自证断言）；④`Unsupported` 测试被「解码失败跳过」兜住；⑤`WriteOutcome` 缺 `backup` 键的分支从未取到；⑥`parseEnvBackupInfo` 的逐字段检查未被覆盖。

**其中两次是靠变异验证（临时改错实现、看断言是否失败）才暴露的。**

**建议**：

1. 写计划时，**每一条断言都必须能指出「删掉哪一行实现会让它失败」**。指不出来的断言是装饰，不是测试。
2. 把**变异验证列为固定动作**——至少对每个「守卫型」判定（跳过、拒绝、兜底）做一次：注释掉那行守卫，测试必须变红。

### 6.2 同一论证在三个地方各存一份

代码 doc / 回执报告 / 测试 docstring 三处各自复述同一段论证，导致「修一处、留一处」反复复发（本波 Task 3、Task 6 各中一次，`0648392` 之前修了三轮）。

**建议：论证只写一处，其余引用**（与项目「判定只在 core 一处」同源）。doc 写行为契约，测试 docstring 写「为什么这个取值能证伪」，不要复述实现理由。

### 6.3 控制器的技术建议同样须经验证

本波控制器三次给出**未经验证**的技术建议并被实测驳回：`cfg!(test)` 语义、mtime 加固、单条正则断言足以闭合分支。

**建议：控制器的建议同样须标注为假设，由实施者验证；不得因来自协调者而免除验证。** 这一条与「判定只在 core 一处」无关，是**信息溯源**问题：协调者的直觉与实现者的实测不是同一等级的证据。

### 6.4 编码 / 字节损坏

本波共 **7 处**：计划原文、`core/src/service.rs`（**先于本波存在**）、以及多次因
heredoc / 正则编辑中文文本引入的 U+FFFD 与控制字节。

**可复现核验**（Task 9 结束时实测；口径：仓库全部文本文件，排除
`target/` `node_modules/` `.git/` `dist/` `coverage/` `test-results/`；
`REPL = chr(0xFFFD)`，控制字符指 `< 0x20` 且不属于 TAB/LF/CR 的字节）：

```python
import io, pathlib
REPL = chr(0xFFFD)
for f in pathlib.Path('.').rglob('*'):
    p = str(f).replace(chr(92), '/')
    if any(s in p for s in ('/target/', '/node_modules/', '/.git/', '/dist/')):
        continue
    if not f.is_file():
        continue
    t = io.open(f, encoding='utf-8', errors='replace').read()
    n = t.count(REPL)
    c = [hex(ord(ch)) for ch in t if ord(ch) < 32 and ch not in '\t\n\r']
    if n or c:
        print(p, 'FFFD=', n, 'CTRL=', c[:5])
```

**实测结果（本波提交范围之外，全部遗留）**：

| 文件                                                                     | U+FFFD | 行号      | 归属                           |
| ------------------------------------------------------------------------ | ------ | --------- | ------------------------------ |
| `docs/superpowers/plans/2026-09-21-env-backup-restore-implementation.md` | 4      | 1165,2494 | 计划原文（审核窗口维护）       |
| `core/src/service.rs`                                                    | 2      | 58        | **先于本波**（`0da96a7` 即有） |
| `docs/审核和开发/2026.09.19/PathEditor-Wave1数据一致性审查报告.md`       | 2      | 82        | 更早波次的历史文档             |
| `docs/审核和开发/2026.09.20/PathEditor-Wave2架构收口开发回执.md`         | 2      | 137       | 更早波次的历史文档             |

**本波提交范围（`455c376..52823e1`）实测 U+FFFD = 0、控制字符 = 0**；六个 blob 逐个复核
（含提交后的 `git show` 复扫）= `checked 6 bad 0`。

**本波自身仍犯了 3 次**：回执初稿 1 处、`task-9-report` 1 处、本节的代码块 1 处
（写检测示例时把 U+FFFD 写成了字面量而非 `chr(0xFFFD)`），均用精确替换修复。**第 3 次尤其说明问题**：想写一段
「如何检测 U+FFFD」的示例，反而自己写出了一个 U+FFFD——所以示例必须用 `chr(0xFFFD)` 而非字面量。

**处置**：上表四处**均未修改** —— plan 按指令不得改（审核窗口的产物），
另外三处属本波范围外的既有遗留，改它们会把无关文件拖进本波 diff。

**建议**：改中文文本一律用精确 `Edit`（不用 heredoc / sed / 正则）；提交前对**提交范围内的**
文本文件自查 U+FFFD 与控制字符；写「检测损坏字符」的示例时用 `chr(0xFFFD)`，不要写别名。

### 6.5 「字节级一致」的文档对里，只有一份过质量门

**Task 9 实测发现**：`AGENTS.md` 被 Prettier 检查（且被 lint-staged 在提交时自动改写），
`CLAUDE.md` 却被 `.gitignore:42` 忽略、绕过 Prettier——**两份要求「字节级一致」的文件，
只有一份接受格式化**。后果是「保持两份同步」与「满足 Prettier」可能互相推开：
本波给 IPC 表加行后列宽变化，`AGENTS.md` 被 prettier 判为不合规，而 `CLAUDE.md` 不受检，
若只跑 `prettier --check` 不看 `AGENTS.md` 就会漏掉。

本次处理：先 `prettier --write AGENTS.md`，再 `cp AGENTS.md CLAUDE.md`，两边同时合规且哈希一致
（`E1AADF6B…`）。**但这是手工步骤，没有自动化守卫。**

**建议**：把 `CLAUDE.md` 从 `.gitignore` 移出（或给两份文件加一个「哈希一致」的 pre-commit 钩子），
并让 `npm run format:check` 覆盖 `CLAUDE.md`——否则下一次同样的失误仍会漏检。

## 7. 提交信息

| 提交      | 说明                                                                                 |
| --------- | ------------------------------------------------------------------------------------ |
| `5b6a43a` | `docs: 同步 env 备份恢复的命令与 IPC 文档，落盘开发回执`（Task 9 全部产物）          |
| `a0bbd7c` | `docs: 勘正回执第 8 节的产出文件清单`（初稿清单凭印象写，与命令输出不符；见第 8 节） |

> **版本号未升（本波明令禁止）**：`package.json` / `Cargo.toml` / `gui/tauri.conf.json` / README 徽章
> 均仍为 `5.1.3`，但 `CHANGELOG.md` 顶部已有 `## 5.1.4` 段落。**这不是漂移**——本波按指令只落盘
> 变更日志，不升版本、不打 tag。将来发版时须先把四处版本号升到 `5.1.4`，否则
> `release.yml` 的「校验项目版本」步骤会因与 tag 不一致而整条失败。

## 8. 本波产出文件清单

**T1–T8（代码与测试）**——精确清单：`git diff --name-status 0da96a7..455c376`
= **8 added / 22 modified / 30 total**（逐项如下）。

新增（8）：

```text
src/core/env-backup.ts
src/components/dialogs/EnvBackupDialog.tsx
src/components/dialogs/env-backup/EnvBackupPanel.tsx
src/components/dialogs/env-backup/use-env-backup.ts
tests/unit/env-backup.test.ts
tests/unit/env-backup-panel.test.tsx
tests/unit/env-backup-dialog.test.tsx
e2e/tests/env-backup.spec.ts
```

修改（22）：

```text
core/src/{backup.rs,lib.rs,registry.rs,registry/env_var.rs}
cli/src/{main.rs,env_ops.rs,runtime.rs}
gui/src/{lib.rs,commands/backup.rs,commands/env_var.rs}
src/services/backend.ts
src/core/env-var.ts
src/store/env-store.ts
src/components/env-list/EnvVarToolbar.tsx
src/components/layout/AppShell.tsx
src/i18n/locales/{zh-CN.json,en.json}
tests/unit/app-shell-env-vars.test.tsx
tests/unit/backend-env-contract.test.ts
tests/unit/env-store.test.ts
tests/unit/env-var-toolbar.test.tsx
e2e/mocks/ipc.ts
```

> **两处澄清（初稿清单有误，此处以命令输出为准）**：
>
> - `core/src/reg_store.rs` **不是**本波产出——它在 `ff5b4f8`（更早的波次）已存在，
>   本波只是**使用** `EnvHiveStore` / `WinregHive`（`git log 0da96a7..455c376 -- core/src/reg_store.rs` 为空）。
> - `core/src/persist.rs` **未改动**——本波复用既有的 `Versioned` 信封与 `parse_error_quarantined`
>   （`git log 0da96a7..455c376 -- core/src/persist.rs` 为空）。
> - `core/src/env_var.rs` 亦未改动；`WriteOutcome` / `backup_before_write` 落在
>   `core/src/registry/env_var.rs`。
> - spec / plan 两文档由审核窗口维护，不在本波代码提交内。

**T9（文档收口，本提交 `5b6a43a`）**

修改：`CLAUDE.md`、`AGENTS.md`（两份字节级一致，`E1AADF6B…`）、`README.md`、`CHANGELOG.md`、
`docs/审核和开发/2026.09.19/PathEditor-备份体系未覆盖环境变量登记.md`（关闭登记）。
新增：`docs/审核和开发/2026.09.21/PathEditor-环境变量备份恢复开发回执.md`（本文件）。
`git show --stat 5b6a43a` = **6 files changed, 312 insertions(+), 41 deletions(-)**。
