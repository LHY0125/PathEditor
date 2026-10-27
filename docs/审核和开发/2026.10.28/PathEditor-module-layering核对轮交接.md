# 模块分层重组 — 计划核对轮交接

**提交窗口**：开发窗口
**日期**：2026-10-28
**交付对象**：审核窗口
**核对对象**：`docs/superpowers/plans/2026-10-28-module-layering-implementation.md`（7 Task / 50 Step）
**对应 spec**：`docs/superpowers/specs/2026-10-28-module-layering-design.md`
**仓库基线**：分支 `refactor/module-layering @ ae0a522`，工作区干净
**结论**：**四层划分、目录映射、Task 顺序均无异议**。发现 **2 项阻塞、1 项验证手段失效（高风险）、1 项事实错误、1 项规格要求遗漏、1 项检查不对称**。两项阻塞都会在计划自己指定的验收步骤（`cargo check`、文档重写）处炸掉，但计划没有给出处置步骤。

---

## 0. 核对方法

逐 Task 核对：文件路径是否存在、符号与行号是否对得上、命令的真实输出、任务之间与任务自身的自洽性、以及**每条验证命令是否真的能验证它声称的东西**。所有结论均来自本机实际执行的 `grep` / `ls` / `sed -n` 输出，命令与原始输出附在各条目内。

已确认的**前置条件**：计划 Global Constraints 说的「5 个未提交的文档改动」已由 `4734188` 提交到 main，本分支工作区干净，`git status --short` 无输出。

---

## 1. 阻塞项

### B1 —— Task 3 的 sed 漏掉 3 处「大括号内」模块导入，Step 7 编译**必然失败** ⛔

**计划原文**（Task 3 Step 4）：

```bash
grep -rlE --include='*.rs' '(crate|path_editor_core)::(service|bootstrap|disabled)\b' src \
| xargs sed -i -E \
    -e 's/\bcrate::service\b/crate::app::service/g' \
    ...
```

**实测**（`src/core/tests/` 下三处导入把模块名写在花括号**内部**）：

```text
src/core/tests/db_current_snapshot_read.rs:13:use path_editor_core::{service, PathEntry};
src/core/tests/db_profile_apply.rs:9:use path_editor_core::{bootstrap, CoreError, EnvHive, PathEntry, PathSnapshot};
src/core/tests/db_restore_snapshot.rs:9:use path_editor_core::{bootstrap, CoreError, EnvHive, PathEntry, PathSnapshot};
```

这三处在替换后仍然写作 `path_editor_core::{service, …}` / `path_editor_core::{bootstrap, …}`，而 Task 3 Step 3 的最终 `lib.rs` 只剩 `app / db / domain / platform` 四个顶层模块，`path_editor_core::service` 与 `path_editor_core::bootstrap` 不再存在 → **E0432 unresolved import**。

两个原因叠加导致它不会被计划自己抓住：

1. **Step 4 的 sed 匹配不到**：`\bpath_editor_core::service\b` 要求模块名紧跟 `::`；而实际文本是 `path_editor_core::{service`，中间隔了一个 `{`。用同一模式扫全仓库，命中的 12 处 `path_editor_core::service` 全是紧邻写法（`path_editor_core::service::{…}`），花括号写法是唯一的例外：
2. **Step 5 的检查也看不见**：计划给的检查是

   ```bash
   grep -rnE --include='*.rs' '\bcrate::(service|bootstrap|disabled)\b' src | grep -v 'app::'
   ```

   它只查 `crate::` 前缀，**完全不覆盖 `path_editor_core::`**，更看不见花括号写法。所以 Step 5 会「Expected: 无输出」地通过，然后在 Step 7 `cargo check --workspace --all-targets` 才失败。

**实测：Task 1 与 Task 2 没有这个问题**（用同一组替换目标扫描，花括号导入只命中上面 3 行，全部属于 Task 3 范围）：

```text
$ grep -rnE --include='*.rs' '(crate|path_editor_core)::\{[^}]*\b(error|path_entry|env_var|history|persist|fs|system|scanner|capabilities|registry|backup|service|bootstrap|disabled)\b' src
src/core/tests/db_current_snapshot_read.rs:13:use path_editor_core::{service, PathEntry};
src/core/tests/db_profile_apply.rs:9:use path_editor_core::{bootstrap, CoreError, EnvHive, PathEntry, PathSnapshot};
src/core/tests/db_restore_snapshot.rs:9:use path_editor_core::{bootstrap, CoreError, EnvHive, PathEntry, PathSnapshot};
```

**建议改法**：

1. Task 3 Step 4 之后补一步手工改这 3 行（保持裸模块名可用，因为下游用的是 `service::…` / `bootstrap::…`）：
   - `db_current_snapshot_read.rs:13` → `use path_editor_core::{app::service, PathEntry};`
   - `db_profile_apply.rs:9` → `use path_editor_core::{app::bootstrap, CoreError, EnvHive, PathEntry, PathSnapshot};`
   - `db_restore_snapshot.rs:9` → 同上

   依据：三处随后分别以裸名调用 `service::load_path_snapshot_db(…)`（`db_current_snapshot_read.rs:44`）与 `bootstrap::ensure_baseline_from(…)`（另两文件 `:71` / `:64`），`app::service` 这种引入方式仍然把 `service` / `bootstrap` 带进作用域。

2. Task 3 Step 5 的检查补一条花括号扫描：
   ```bash
   grep -rnE --include='*.rs' '(crate|path_editor_core)::\{[^}]*\b(service|bootstrap|disabled)\b' src
   ```

### B2 —— Task 6 Step 3 的行号区间「6–68」会把两个二级标题一起删掉 ⛔

**计划原文**：

> 把第 6–68 行的目录树替换为下列内容（保留其后的「模块边界速记」「数据流」两节不变）

**实测**（`docs/dev/architecture.md` 的真实结构）：

````text
$ awk 'NR>=5 && NR<=9 {print NR": "$0} NR>=64 && NR<=72 {print NR": "$0}' docs/dev/architecture.md
5:
6:## 完整目录树
7:
8:```text
9:PathEditor/
64:├── docs/dev/                    # 本目录：按需读取的开发细节
65:└── docs/审核和开发/YYYY.MM.DD/  # 审核与开发记录
66:```
67:
68:## 模块边界速记
69:
70:| 层                                    | 职责 …
````

- 第 6 行是 `## 完整目录树`（标题本身）；
- 目录树代码块是 **8–66**（含围栏），树体是 9–65；
- 第 68 行正是计划说「要保留」的 `## 模块边界速记`。

按「6–68」字面执行会同时删掉 `## 完整目录树` 与 `## 模块边界速记` 两个标题，与计划自己的「保留其后的两节」自相矛盾。

**建议改法**：把区间改为 **8–66**（连同围栏整体替换），或改成「定位 `## 完整目录树` 标题与下一个 `##` 标题之间的内容」这类不依赖行号的写法。

---

## 2. 验证手段失效（高风险）

### H1 —— Task 6 Step 6 与 spec 1.3 第 4 条的 grep 对**树形写法完全失明**，改动前就「空过」

**计划原文**（Task 6 Step 6）与 **spec 1.3 第 4 条**：

```bash
grep -rnE 'src/core/src/(error|path_entry|env_var|history|persist|fs|system|scanner|capabilities|registry|backup|service|bootstrap|disabled)\.rs' src docs/dev README.md CLAUDE.md AGENTS.md
# Expected: 无输出
```

**实测（在**未做任何改动**的当前分支上直接跑）**：

```text
$ grep -rnE 'src/core/src/(error|…|disabled)\.rs' src docs/dev README.md CLAUDE.md AGENTS.md
$ echo "退出码=$?"
退出码=1
```

**当前态文档里这些旧模块名明明还在**——只是写成目录树形式（`│   │   ├── service.rs`），路径前缀不是完整的 `src/core/src/`：

```text
$ grep -nE '│.*(error|path_entry|env_var|history|persist|fs|system|scanner|capabilities|registry|backup|service|bootstrap|disabled)\.rs' docs/dev/architecture.md | head -12
25:│   │   ├── service.rs           # 保存编排：save_path_with_db / save_path_registry_only
26:│   │   ├── capabilities.rs      # 按 hive 探测 PathCapabilities
27:│   │   ├── system.rs            # 权限、路径验证、变量展开、环境广播
28:│   │   ├── disabled.rs          # 快照合并、pending 磁盘文件
29:│   │   ├── env_var.rs           # 通用环境变量契约与保留/保护/敏感判定
30:│   │   ├── path_entry.rs        # PathEntry / PathSnapshot 契约
31:│   │   ├── error.rs             # 结构化 CoreError / ErrorCode（含 exit_code）
32:│   │   ├── persist.rs           # 持久化文件解析与损坏隔离
33:│   │   ├── scanner.rs           # scan_paths 单次枚举、冲突和工具清单
34:│   │   ├── fs.rs                # 受限文件读取、导入导出
35:│   │   └── backup.rs            # 注册表备份、env 备份（.pebak）
```

四份文档含旧模块名的行数（正则按「裸文件名」统计）：

| 文件                       | 命中行数 |
| -------------------------- | -------- |
| `docs/dev/architecture.md` | 16       |
| `README.md`                | 8        |
| `CLAUDE.md`                | 3        |
| `AGENTS.md`                | 3        |

**后果**：这条检查无论 Task 6 Step 3/4/5 做得对不对、甚至完全不做，都会「无输出」地通过。它是本次验收里唯一一条针对文档的机械检查，失效后整节文档同步只剩人工自觉。

**建议改法**：把检查换成对裸文件名的扫描（并对四份文档分别断言零命中），例如：

```bash
grep -nE '\b(error|path_entry|env_var|history|persist|fs|system|scanner|capabilities|registry|backup|service|bootstrap|disabled)\.rs\b' \
  docs/dev/architecture.md README.md CLAUDE.md AGENTS.md
```

注意：该正则会命中 `db/` 下合法的 `history.rs`、`fs.rs`（`platform/fs.rs`）等同名文件，因此不能简单要求零命中；更稳的做法是**在计划里逐文档给出「改后应有的条目清单」**，由开发窗口逐条核对，而不是靠一条 grep。这条需要审核窗口定夺采用哪种形式。

---

## 3. 事实错误

### F1 —— Task 4 Step 3 说「九条 mod 声明」，实际是 **10 条**

**计划原文**：

> 把第 5–14 行的九条 `mod` 声明替换为四条

**实测**：

```text
$ sed -n '5,14p' src/cli/src/main.rs
mod cli;
mod env_ops;
mod import_export;
mod maintenance_ops;
mod output;
mod path_ops;
mod profile_ops;
mod runtime;
mod scan_ops;
mod sync_ops;
```

第 5–14 行恰好 10 行、10 条声明（`cli / env_ops / import_export / maintenance_ops / output / path_ops / profile_ops / runtime / scan_ops / sync_ops`），替换为 4 条（`cli / commands / output / runtime`）。

**影响**：行号区间（5–14）是对的，只有「九条」这个数词错了。按行执行不受影响，但会让开发窗口误以为漏了一条而回头翻找。建议改为「十条 → 四条」。

---

## 4. 规格要求遗漏

### F2 —— `registry/golden/README.md` 的既有漂移，Task 6 Step 2 **没有修**

**spec 原文**（第九节文档同步清单）：

| 文件                                              | 动作                                                                                                                 |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `docs/dev/architecture.md`                        | 重写「完整目录树」块；**顺带修正既有漂移（`registry/env_var.rs` 实为 `env_edit.rs`；`backup.rs` 已拆成 `backup/`）** |
| `src/core/src/platform/registry/golden/README.md` | **同上**                                                                                                             |

**计划 Task 6 Step 2 只做了前缀替换**：

```bash
sed -i -E \
  -e 's|core/src/registry/|core/src/platform/registry/|g' \
  -e 's|core/src/backup\.rs|core/src/platform/backup/mod.rs|g' \
  -e 's|core/src/service\.rs|core/src/app/service/mod.rs|g' \
  src/core/src/platform/registry/golden/README.md
```

**实测该文件里仍会有三处失真**：

```text
$ grep -nE 'core/src/registry|core/src/backup|crate::(system|disabled)|env_var\.rs' src/core/src/registry/golden/README.md
13:- `core/src/registry/golden_tests.rs`（挂 `registry/path.rs` 下，访问其私有 fn）
14:- `core/src/registry/golden/merge_golden_tests.rs`（挂 `crate::disabled` 下，访问 `merge_hive`）
47:> `core/src/backup.rs` 的 `restore_env_backup_from` 实现，差异计算与写入判定
56:**广播端口尚未抽出**（F-08 只迁移了部分）。当前 `crate::system::broadcast_env_change()`
59:- `core/src/registry/env_var.rs`：`update_env_var` / `create_env_var` /
73:> 而是加了一个全局观察标志：`core/src/registry/env_var.rs` 的
```

按 Step 2 的 sed 执行后：

| 行    | 现状                                  | Step 2 之后                             | 仍然错误的地方                                      |
| ----- | ------------------------------------- | --------------------------------------- | --------------------------------------------------- |
| 14    | `crate::disabled`                     | 不变                                    | Task 3 后应为 `crate::app::disabled`                |
| 56    | `crate::system::broadcast_env_change` | 不变                                    | Task 2 后应为 `crate::platform::system::…`          |
| 59/73 | `core/src/registry/env_var.rs`        | `core/src/platform/registry/env_var.rs` | 该文件实为 **`env_edit.rs`**（spec 点名要修的漂移） |

**建议改法**：Step 2 补两条 `sed`（或改为逐个手工改）：

```bash
-e 's|platform/registry/env_var\.rs|platform/registry/env_edit.rs|g'
-e 's|crate::system|crate::platform::system|g'
-e 's|crate::disabled|crate::app::disabled|g'
```

> **附带观察（非本轮要求）**：该 README 第 14 行引用的 `core/src/registry/golden/merge_golden_tests.rs` **在仓库中不存在**（`ls src/core/src/registry/golden/` 只有 `README.md` 与 20 个 JSON）。这是既有失效引用，不属于本次重构的处置范围，仅记录备查。

---

## 5. 检查不对称（轻微）

### M1 —— Task 2 Step 5 只查 `crate::`，不查 `path_editor_core::`

Task 1 Step 6 给了两条检查（`crate::` 与 `path_editor_core::` 各一条），Task 2 Step 5 只给了 `crate::` 一条：

```bash
grep -rnE --include='*.rs' '\bcrate::(persist|fs|system|scanner|capabilities|registry|backup)\b' src | grep -v 'platform::'
```

而 Step 4 的 sed 是**两个前缀都替换**的，且 `src/core/tests/` 里有 `path_editor_core::backup::{…}` 这样的真实引用（2 个文件）。建议补齐：

```bash
grep -rnE --include='*.rs' '\bpath_editor_core::(persist|fs|system|scanner|capabilities|registry|backup)\b' src | grep -v 'platform::'
```

---

## 6. 已核对无误的项（供审核窗口确认核对覆盖面）

| 核对项                                                     | 结果                                                                            |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Task 1 Step 4 给出的 `lib.rs`（Task 1 态）                 | 与当前 `lib.rs` 的模块清单、`pub(crate) mod persist`、6 条 re-export 逐条对得上 |
| Task 2 Step 3 给出的 `lib.rs`（Task 2 态）                 | 模块与 re-export 均对得上                                                       |
| Task 3 Step 3 给出的 `lib.rs`（最终态）                    | 四个 `pub mod` + 6 条 re-export，与前两态自洽                                   |
| `service.rs` 尾部的 `mod profile; mod restore; mod tests;` | 确实在（`:583`、`:584`、`:640-641`），随文件搬移无需改                          |
| Task 4 Step 5 的三处注释行号                               | `cli.rs:3`、`main.rs:140`、`runtime.rs:253` **全部精确命中**，原文与表格一致    |
| `env_ops/` 的 `super::render` / `super::value`             | 整目录下移一层后 `super` 仍指向同级文件（`commands/env/`），**无需改**          |
| `registry/path.rs:241` 的 `#[path = "golden_tests.rs"]`    | 相对路径，`golden_tests.rs` 与 `path.rs` 同目录一起移动，**保持有效**           |
| `db/mod.rs` 的 `pub mod backups;`                          | 在该文件第 6 行，Task 5 Step 2 的 sed 锚点精确                                  |
| Task 5 Step 4 的「裸 `backups::`」检查                     | 当前无输出，与计划预期一致                                                      |
| `src/cli` 里的 `backups` 命中                              | 全部是 `env backups` 子命令文本，不会被 `\bdb::backups\b` 误伤                  |
| `path_editor_core::persist` 在集成测试中的可见性           | 无引用，`pub(crate)` 不构成阻塞                                                 |
| 空目录                                                     | `find src -type d -empty` 当前无输出                                            |
| `probes/` 之外的 workspace 外 `.rs` 引用                   | 无；`probes/v6-sqlcipher` 已在 `workspace.exclude` 中                           |

---

## 7. 建议处理汇总

| 编号 | 严重度 | 位置                       | 建议改法                                                                   |
| ---- | ------ | -------------------------- | -------------------------------------------------------------------------- |
| B1   | 阻塞   | Task 3 Step 4/5            | 补 3 行花括号导入的手工改写 + 补花括号扫描检查                             |
| B2   | 阻塞   | Task 6 Step 3              | 行号区间 6–68 → 8–66（或不依赖行号的定位写法）                             |
| H1   | 高     | Task 6 Step 6、spec 1.3 #4 | 换成对裸文件名的逐文档清单核对，或按文档给出「改后应有条目」               |
| F1   | 低     | Task 4 Step 3              | 「九条」→「十条」                                                          |
| F2   | 中     | Task 6 Step 2              | 补 `env_var.rs → env_edit.rs`、`crate::system`、`crate::disabled` 三条改写 |
| M1   | 低     | Task 2 Step 5              | 补一条 `path_editor_core::` 前缀的检查                                     |

---

## 8. 待裁断

以上 **B1、B2、H1、F2** 需要审核窗口裁断并更新计划；**F1、M1** 属措辞/覆盖补强，若审核窗口认为可并入同一轮修订，开发窗口按修订后的计划执行。

**开发窗口未做任何代码改动。** 代码与测试文件保持 `ae0a522` 原样；本轮只新增本交接文档（提交 `6c00dbc`）。

---

## 9. 开工前基线（已实测，供裁断后直接引用）

按 Task 1 Step 1 的要求，改动前已跑完 `cargo test --workspace`：

```text
$ cargo test --workspace > baseline-full.txt; echo exit=$?
exit=0
$ grep -E '^test result:' baseline-full.txt | wc -l
56
# 逐条求和（python 解析全部 test result 行）
passed=324 failed=0 ignored=2
```

**结论：324 passed / 0 failed / 2 ignored，与计划 Step 1 的期望值完全一致**，闸门通过。

（同轮已确认：`target/` 处于暖态，增量全量测试约 1 分钟；首次冷编译仍需约 24 分钟。）

---

## 10. 工作区异常（非开发窗口造成，需审核窗口确认）

开工时的 `git status --short` 为**空**。核对过程中发现工作区出现一处**非开发窗口造成的**改动：

```text
$ git status --short
 M docs/superpowers/specs/2026-10-28-module-layering-design.md
```

**性质**：纯格式差异，无实质内容改动（`git diff -w --ignore-blank-lines` 仅剩表格分隔行与空行的空白变化；`git diff --numstat` 为 `21 16`）。

**方向是「去格式化」而非格式化**：

```text
$ npx prettier --check docs/superpowers/specs/2026-10-28-module-layering-design.md
[warn] ... Code style issues found in the above file.
$ git show HEAD:docs/superpowers/specs/2026-10-28-module-layering-design.md > /tmp/spec_head.md
$ npx prettier --check /tmp/spec_head.md
All matched files use Prettier code style!
```

即 **HEAD 版本是 prettier 干净的，工作区版本反而不干净**。文件 mtime 为 `00:50:28`，而开发窗口的提交在 `01:04:11`，时间上早于提交、晚于开工。

**已知线索**：`ListAgents` 显示存在并行会话 `PathEditor审核 [18cc9c]`（54 分钟前启动，当前 idle）。该改动很可能是审核窗口在自己会话里落笔但未提交的内容。

**开发窗口未处置该文件**——按硬约束「不改写历史文档」，也没有擅自 `git checkout --` 丢弃他人改动。**需要用户/审核窗口决定**：

| 方案      | 做法                                                                                           | 影响                                                           |
| --------- | ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| A（推荐） | 审核窗口确认这是否是它的在途改动：是则自行 commit，否则 `git checkout --` 还原                 | 恢复「工作区干净」前置条件                                     |
| B         | 授权开发窗口执行 `git checkout -- docs/superpowers/specs/2026-10-28-module-layering-design.md` | 丢弃该格式改动，同样恢复干净                                   |
| C         | 保持现状直接开工                                                                               | **违反** Global Constraints 的「开工前工作区必须干净」，不建议 |

> 在 A/B 落定前，Task 1 的「工作区干净」前置条件不成立，开发窗口不会开工。

---

## 11. 审核窗口裁断（2026-10-28）

审核窗口逐条复跑，**未采信交接文档的结论**。下列证据均为本机实测。

### 11.1 B1 —— 成立，已修

```text
$ grep -rnE 'path_editor_core::\{|crate::\{' src | grep -E 'service|bootstrap|disabled'
src/core/tests/db_current_snapshot_read.rs:13:use path_editor_core::{service, PathEntry};
src/core/tests/db_profile_apply.rs:9:use path_editor_core::{bootstrap, CoreError, EnvHive, PathEntry, PathSnapshot};
src/core/tests/db_restore_snapshot.rs:9:use path_editor_core::{bootstrap, CoreError, EnvHive, PathEntry, PathSnapshot};
```

恰好 3 处，全部在 Task 3 范围内，交接文档的「全仓库花括号导入只有这 3 行」属实。

**处置**：Task 3 Step 4 增加花括号专用 sed：

```bash
sed -i -E 's/path_editor_core::\{([^}]*)\b(service|bootstrap|disabled)\b/path_editor_core::{\1app::\2/g'
```

结果形如 `use path_editor_core::{app::service, PathEntry};`（合法 use 语法，行数不变）。Step 5 增加花括号守卫检查。

### 11.2 B2 —— 成立，已修

````text
$ sed -n '6p;8p;66p;68p' docs/dev/architecture.md
## 完整目录树
```text
````

## 模块边界速记

````

第 6 行标题、第 8 行开围栏、第 66 行收围栏、第 68 行标题。交接文档的「真实区间是 8–66」属实。

**处置**：Task 6 Step 3 改为「替换第 8–66 行」，并增加替换后的标题存活检查（`grep -n '^## 完整目录树$\|^## 模块边界速记$\|^## 数据流'`）。

### 11.3 H1 —— 成立，已重写

```text
$ grep -rnE 'src/core/src/(error|...|disabled)\.rs' src docs/dev README.md CLAUDE.md AGENTS.md
（无输出，退出码 1）
````

在未做任何改动的树上就已空过——交接文档判断正确。

**处置**：Task 6 Step 6 重写为四条检查 (a)(b)(c)(d) + 一条正向断言，并要求「开工前先在未改动的树上跑，必须各有输出」。

审核窗口在重写过程中又发现一个更隐蔽的同类问题：**`[├└]` 这类含多字节字符的括号表达式在 Git Bash 下会静默命中 0 行**。第一版改写用的正是 `[├└]`，实测 (c)(d) 只命中 2 行，改用 `(├──|└──)` 字面量择一后才全部命中。已写进计划作为 ⚠️。

改动前实测应有：(b) 10 行、(c) 8 行、(d) 3 行、(e) 正向断言 `0`（改后应为 `3`）。

### 11.4 F1 —— 成立，已修

`src/cli/src/main.rs` 第 5–14 行实为 10 条 `mod`（`cli` / `env_ops` / `import_export` / `maintenance_ops` / `output` / `path_ops` / `profile_ops` / `runtime` / `scan_ops` / `sync_ops`）。已改「九条」→「十条」。

### 11.5 F2 —— 成立，已修，且范围比交接文档写的更大

```text
$ grep -n 'crate::\|core/src/' src/core/src/registry/golden/README.md
14:（挂 `crate::disabled` 下，访问 `merge_hive`）
56:当前 `crate::system::broadcast_env_change()`
59:core/src/registry/env_var.rs
62:core/src/service.rs
73:core/src/registry/env_var.rs
47:core/src/backup.rs
```

交接文档只指出了 `env_var.rs`；实测还有 `crate::disabled` 与 `crate::system::` 两处 Rust 模块路径，通用前缀替换同样碰不到。

**处置**：Task 6 Step 2 补三条规则，并固定顺序——`env_var.rs → env_edit.rs` 必须排在通用前缀替换**之前**，否则第 2 条先生成 `platform/registry/env_var.rs`，第 1 条就匹配不到。

**范围外**：该 README 第 14 行提到的 `core/src/registry/golden/merge_golden_tests.rs` 在仓库中不存在（`golden/` 下只有 `README.md` 与 JSON 夹具）。这是另一处既有漂移，本轮不改。

### 11.6 M1 —— 成立，已修

Task 2 Step 5 补 `path_editor_core::` 一条与花括号守卫一条，与 Task 1 / Task 3 的检查对齐。

### 11.7 审核窗口另补三条（开发窗口未提）

| 编号 | 问题                                                     | 实测                                                           | 处置                                                            |
| ---- | -------------------------------------------------------- | -------------------------------------------------------------- | --------------------------------------------------------------- | --------------------------------------- |
| E1   | `sed` 的 `$` 行尾锚点在 CRLF 下永不匹配                  | `db/mod.rs` 为 `w/crlf`；`s/^pub mod backups;$/…` 会静默不生效 | 全局禁止 `$` 锚点；Task 5 命令去掉 `$`                          |
| E2   | README 与 CLAUDE.md 的替换只说「按四层改写」，无具体内容 | 两处各是一整块代码树                                           | 给出两个完整替换块 + 精确行区间（README 580–616、CLAUDE 71–91） |
| E3   | `[├└]` 多字节括号表达式静默失效                          | 见 11.3                                                        | 改 `(├──                                                        | └──)`；并加「改动前必须有输出」的硬步骤 |

另：Task 1 Step 1 增加行尾基线（`git ls-files --eol src \| grep -c 'w/crlf'`，2026-10-28 实测 `283`），Task 7 Step 4 增加行尾比对——防止某条 `sed -i` 把 CRLF 整体写成 LF，让「改一行」变成「全文件重写」。

### 11.8 第 10 节工作区异常的裁断：采用 B，已执行

**采用方案 B**：`git checkout -- docs/superpowers/specs/2026-10-28-module-layering-design.md`，已执行。

依据：

```text
$ git diff -w --numstat -- <spec>
10	5	docs/superpowers/specs/2026-10-28-module-layering-design.md
$ npx prettier --check <spec>          # 还原前
[warn] 工作区版本不合格
$ npx prettier --check <spec>          # 还原后
All matched files use Prettier code style!
```

- 差异只有表格分隔行的横线长度与若干空行，**无任何语义改动**，还原不丢内容。
- HEAD 版本 prettier 干净，工作区版本不干净；保留工作区版本会让 `npm run verify` 的 `format:check` 不稳定。
- 该文件属于审核窗口的交付物，审核窗口有权定夺。

**给并行会话的提示**：`PathEditor审核 [18cc9c]` 若仍持有该文件的内容，请勿再次落笔覆盖；如需修改，请先 `git pull`/`git status` 确认基线。

### 11.9 结论

- **计划已升到 v2**，修订明细见计划文件顶部 `## 修订记录`。
- 交接文档的 6 条全部采纳，无一条驳回。
- **开发窗口可以从 Task 1 重新开工**，按 v2 执行。
- 开工前确认：`git status --porcelain` 为空（审核窗口提交完 v2 后即恢复干净）。

---

## 12. 开发轮裁断（Task 1–3 完成后，2026-10-28）

开发窗口在 Task 3 Step 6 撞到依赖方向契约不可达成，停下待裁断。**裁断结论：spec 有错，代码没错。**

### 12.1 根因：审核窗口的矩阵统计方法有缺陷

spec §2 的依赖矩阵在 v1 是用 `grep -o 'use crate::[a-z_]*'` 统计的——**只覆盖 `use` 语句，漏掉函数体内联的 `crate::x::f()`**。开发窗口报的三条边全部属实：

```text
$ grep -rn 'crate::platform' src/core/src/domain
231:        EnvHive::System => crate::platform::system::check_admin(),
232:        EnvHive::User   => crate::platform::registry::can_write_user(),

$ grep -rn 'crate::platform::backup' src/core/src/db
6 处（backups.rs:24/219/309/397、maintenance.rs:14/259）

$ grep -rn 'crate::db::' src/core/src/platform
6 处 backups + 8 处 crypto，全部在 platform/backup/ 内
```

**重构前就存在**：

```text
$ git show 4734188:src/core/src/env_var.rs | grep -n 'crate::system\|crate::registry'
231:        EnvHive::System => crate::system::check_admin(),
232:        EnvHive::User => crate::registry::can_write_user(),
```

开发窗口的「§2 矩阵写错、§2.1 唯一 SCC 不成立」判断正确。

### 12.2 裁断一：`domain → platform` 必须修，不白名单

`capabilities_for` 与同文件的 `capabilities_for_with(writable, …)` 是同一套判定的两种包装——后者已参数化，注释甚至写明「避免本模块依赖注册表探测」。**设计意图本来就是参数化，只是便捷包装放错了层。**

实测调用点：

```text
$ grep -rn 'capabilities_for\b' src --include='*.rs' | grep -v capabilities_for_with
src/core/src/domain/env_var.rs:237  （定义）
src/core/src/domain/env_var.rs:349  （测试）
src/core/src/domain/env_var.rs:357  （测试）
```

除自身单测外**零调用**。

**决定**：新增 **Task 3.5**，把 `capabilities_for` 连同两个测试移入 `platform/capabilities.rs`。签名不变、行为不变，纯移动。注意 `capabilities_for_with` 的文档注释里那行 `crate::platform::capabilities` 也要改——否则 `grep` 会把注释算进去造成假红。

### 12.3 裁断二：`db ↔ platform::backup` 登记白名单（方案 A）

**采用开发窗口推荐方案 A。**

依据：

- 消除它要动 `db/backups.rs`（497 行，SQL 与 `.pebak` 编解码混合）并搬 `db/{crypto,dpapi,dacl}`（约 55 处路径引用），是第二个重构轮的量。
- 该环源于 `.pebak` 横跨「文件」与「数据库索引/密钥」，不是目录划分失误。
- 白名单**精确可校验**：`grep -rn 'crate::\(db\|app\)' src/core/src/platform | grep -v '/backup/'` 必须为空。

**方案 B 否决**：与方案一收益重叠，但要动公开签名，违背「零行为变更」。
**方案 C 记为后续候选**：三步纯移动的做法已写进 spec §7，作为独立一轮。

### 12.4 其余 7 处偏差：全部确认属实

| 编号 | 开发窗口的发现                                     | 审核窗口复核                                                                                                                         |
| ---- | -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| D-1  | `use path_editor_core as core;` 造成第四种路径形态 | 实测 11 处 alias、80 处 `core::<模块>`（platform 50 / db 12 / domain 9 / app 9）；`core::ffi` 等标准库靠「只替换目标模块名」天然避开 |
| D-2  | `sed -i` 把整文件 CRLF 翻成 LF                     | 属实。`--binary` 是正解；`w/crlf` 计数校验保留为兜底                                                                                 |
| D-3  | `include_str!` 相对路径随移动失效                  | 属实，已逐处补 `../`                                                                                                                 |
| D-4  | Task 3 Step 5 检查 3 伪阳性                        | 复现确认：`use path_editor_core::{app::service, PathEntry};` 仍命中 `\bservice\b`。已改为「`{` 或 `,` 后紧跟裸模块名」               |
| D-5  | `registry/` 目录 rename 被 Windows 占用            | 属实，逐文件 `git mv` 是正确处置                                                                                                     |
| D-6  | `git mv` 自动暂存导致提交混入                      | 属实，提交前 `git status` 逐项确认                                                                                                   |
| D-7  | 文档与代码混提交                                   | 属实，`git reset` 拆开                                                                                                               |

### 12.5 结论

- **spec 升到 v2**：§2 矩阵更正、§2.1 改为两个 SCC、§3.3 加白名单、§6/§7 同步、新增 §11 裁断记录。
- **计划升到 v3**：新增 Task 3.5；Global Constraints 补「四种路径形态」与 `sed --binary`；修正 Task 3 Step 5/6 与 Task 7 Step 3。
- **开发窗口从 Task 3.5 继续**，然后 Task 4 → 7。Task 3 保持已提交状态，不重做。
