# 模块分层重组 — 开发回执

**提交窗口**：开发窗口
**日期**：2026-10-28
**交付对象**：审核窗口
**分支**：`refactor/module-layering`
**版本**：6.1.1（未升级）
**状态**：**开发完成，质量门全绿，未推送**

---

## 1. 交付内容概述

| 交付项            | 内容                                                                               | 关键决策（选了什么 + 为什么）                                                                     |
| ----------------- | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `domain` 契约层   | `error` / `path_entry` / `env_var` / `history`                                     | 只放零 OS、零 IO 的契约；`capabilities_for` 因要探测注册表权限，按裁断移入 `platform`（Task 3.5） |
| `platform` 适配层 | `persist` / `fs` / `system` / `scanner` / `capabilities` / `registry/` / `backup/` | `registry` 与 `backup` 互为 SCC-1，强制同层；`persist` 因被 backup 使用且做文件 IO，不放进 domain |
| `db` 数据层       | 路径不变；`backups.rs` → `backup_index.rs`                                         | 保持顶层以免 50 处 `path_editor_core::db::` 变长；改名只为消除与 `platform/backup/` 的同义词歧义  |
| `app` 编排层      | `service/` / `bootstrap` / `disabled`                                              | 唯一同时依赖 `platform` 与 `db` 的层                                                              |
| CLI               | 6 个 `*_ops.rs` + `env_ops/` → `commands/`                                         | 分类信息从文件名移到目录，顶层 11 项 → 5 项                                                       |
| 文档              | architecture / README / CLAUDE / AGENTS + 源码注释                                 | 只改当前态文档；历史文档（plans/specs/审核和开发）未改                                            |

**零行为变更**：只移动文件 + 改路径；未改任何业务判定、DB schema、同步包格式、CLI 退出码、IPC 契约。唯一非纯移动的改动是 Task 3.5（函数换层，签名与行为不变）。

## 2. 代码改动清单

### 2.1 Task 1（`b45a6f7`，86 files changed，141 / 135）

```text
rename src/core/src/{ => domain}/env_var.rs (100%)
rename src/core/src/{ => domain}/error.rs (99%)
rename src/core/src/{ => domain}/history.rs (98%)
create mode 100644 src/core/src/domain/mod.rs
rename src/core/src/{ => domain}/path_entry.rs (100%)
```

### 2.2 Task 2（`5e31fcb`，65 files changed，141 / 134）

`persist / fs / system / scanner / capabilities` 五个文件 + `registry/`（含 `golden/` 20 个 JSON 夹具）+ `backup/` 整目录移动，rename 相似度 90–100%；新增 `platform/mod.rs`。

### 2.3 Task 3（`1adcd5c`，23 files changed，39 / 34）

```text
rename src/core/src/{ => app}/bootstrap.rs (98%)
rename src/core/src/{ => app}/disabled.rs (100%)
create mode 100644 src/core/src/app/mod.rs
rename src/core/src/{service.rs => app/service/mod.rs} (99%)
rename src/core/src/{ => app}/service/{profile,restore,tests}.rs (100%)
```

`rmdir src/core/src/service` 已执行（空目录）。

### 2.4 Task 3.5（`b49e816`，2 files changed，30 / 26）

`capabilities_for` 与它的两个单测从 `domain/env_var.rs` 移入 `platform/capabilities.rs`；`capabilities_for_with` 的文档注释去掉 `crate::` 前缀。签名、行为、断言均未改。

### 2.5 注释假阳性修正（`a435b21`，1 file，1 / 1）

`platform/registry/golden_tests.rs` 的模块说明里 `crate::app::disabled` → `app::disabled`（prose，不是依赖；否则 `platform → app` 契约检查永远红）。

### 2.6 Task 4（`816c488`，15 files changed，35 / 29）

11 个 rename（100%）：`path_ops→commands/path`、`profile_ops→commands/profile`、`scan_ops→commands/scan`、`sync_ops→commands/sync`、`maintenance_ops→commands/maintenance`、`import_export→commands/import_export`、`env_ops/→commands/env/`（5 个文件）；新增 `commands/mod.rs`。

### 2.7 Task 5（`888aa0f`，9 files changed，13 / 13）

```text
rename src/core/src/db/{backups.rs => backup_index.rs} (100%)
```

`db/mod.rs` 的 `pub mod backups;` → `pub mod backup_index;`；7 处 `db::backups::` → `db::backup_index::`。SQL 表名 `backups` 与 `platform::backup`（6 处）未被误伤。

### 2.8 Task 6（`027573f`，34 files changed，181 / 137）

- 3 个 `.rs` 的注释路径同步；`golden/README.md` 的 6 条规则；
- `docs/dev/architecture.md` 第 8–66 行整块重写（标题存活已验证）；`README.md` 第 580–616 行重写；`CLAUDE.md` 第 71–91 行重写并 `cp` 同步 `AGENTS.md`；
- `cargo fmt` 重排了 20+ 个文件的长行（路径变长导致超过 100 列）。

## 3. 提交清单

```text
027573f docs: 同步模块分层后的目录树与路径引用
888aa0f refactor: db::backups 改名为 db::backup_index
816c488 refactor: CLI 命令实现收进 commands/
a435b21 refactor: 注释去掉 crate:: 前缀，避免依赖方向检查假阳性
b49e816 refactor: 把 capabilities_for 移出 domain，消除 domain -> platform 反向边
0ab6a86 docs: 更正依赖矩阵并裁断开发轮阻塞（spec v2 / 计划 v3）
3d006f2 docs: 回执更新 Task 3 已提交与 D-7 提交操作偏离
1adcd5c refactor: 抽出 core::app 编排层
b153ec6 docs: 提交模块分层重组开发回执（Task 1-2 完成，Task 3 阻塞待裁断）
5e31fcb refactor: 抽出 core::platform 适配层
b45a6f7 refactor: 抽出 core::domain 契约层
acc9878 docs: 按核对轮裁断修订模块分层计划（v2）
bd587d9 docs: 补充模块分层核对轮的基线结果与工作区异常
6c00dbc docs: 提交模块分层重组计划核对轮意见
ae0a522 docs: 新增模块分层重组的 spec 与实施计划
```

本轮回执提交后紧随其后再追加一个 `docs:` 提交。

## 4. 测试与质量门

### 4.1 逐任务

| 任务     | 命令                                                   | 结果                                                              |
| -------- | ------------------------------------------------------ | ----------------------------------------------------------------- |
| Task 1   | `cargo check --workspace --all-targets`                | `Finished`，无 error / warning                                    |
| Task 2   | 同上                                                   | `Finished`                                                        |
| Task 3   | 同上                                                   | `Finished`                                                        |
| Task 3.5 | `cargo check` + `cargo test -p path-editor-core --lib` | `Finished`；**206 passed / 0 failed / 2 ignored**（与搬移前一致） |
| Task 4   | `cargo test -p patheditor-cli --bins`                  | **66 passed / 0 failed**                                          |
| Task 5   | `cargo check --workspace --all-targets`                | `Finished`                                                        |
| Task 6   | `cargo fmt --all -- --check` + `cargo check`           | 两条均通过                                                        |

### 4.2 Task 7 全量质量门（原始输出）

```text
$ cargo fmt --all -- --check
（无输出）                                   exit=0

$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.35s   exit=0

$ cargo test --workspace
TOTAL passed=324 failed=0 ignored=2          exit=0
（与 Task 1 Step 1 基线**完全一致**）

$ cargo test -p patheditor-cli --bins
test result: ok. 66 passed; 0 failed; 0 ignored

$ npm run verify
 Test Files  37 passed (37)
      Tests  413 passed (413)
                                             exit=0
```

### 4.3 结构与依赖契约

```text
$ ls -1 src/core/src
app
db
domain
lib.rs
platform

$ ls -1 src/cli/src
cli.rs
commands
main.rs
output.rs
runtime.rs

$ grep -rn 'crate::\(platform\|db\|app\)' src/core/src/domain
（无输出 ✓）

$ grep -rn 'crate::\(db\|app\)' src/core/src/platform | grep -v '/backup/'
（无输出 ✓；/backup/ 是 spec §3.3 登记的唯一白名单 SCC-2）

$ grep -rn 'crate::app' src/core/src/db
（无输出 ✓）

$ cmp CLAUDE.md AGENTS.md && echo IDENTICAL
IDENTICAL
```

### 4.4 行尾与残留

```text
字节级统计（git ls-files src 共 320 个文件）：
  pure-CRLF=284  pure-LF=17  mixed=18（其中 2 个 .rs + 16 个二进制图标）  no-newline=1

$ git status --porcelain
（无输出）

$ find src -type d -empty
（无输出）

整文件重写检查：对每个改动 .rs 比较 diff 行数与文件总行数，
无任何文件超过 80% —— 纯移动可审性成立。
```

## 5. 真实注册表闭环测试

**未做**。本轮为纯重构（零行为变更），计划未要求真实注册表闭环；所有测试仍走 `TempDir`，未打开真实 `~/.patheditor`，未写注册表。

### 未覆盖项（如实列出）

1. **Playwright E2E 未跑**：计划只要求 `npm run verify`（不含 E2E）；`verify:all` 才包含。本轮未执行 E2E。
2. **真实注册表闭环未做**：见上。
3. **`db_backups_index.rs` 测试文件名未改**：模块已改名 `backup_index`，但测试二进制文件名仍含 `backups`。计划未要求，属观感不一致。
4. **`golden/README.md` 第 14 行仍提到不存在的 `merge_golden_tests.rs`**：既有漂移，计划 §7 明确范围外。
5. **行尾基线是 284，不是 Task 1 记录的 283**：原因见偏离 D-11；Task 3 之后已记录为 284，本轮全程未降。
6. **未推送、未升级版本号、未合并到 main**。

## 6. 需要审核窗口重点关注的项

### 6.1 已知保留项

- **SCC-2 白名单**：`platform/backup/` ↔ `db` 的双向引用按裁断保留，检查命令统一带 `/backup/` 过滤。这是唯一例外；除它之外三个方向的检查全部为空。
- **`cargo fmt` 的连带重排**：Task 6 Step 7 的 `cargo fmt` 修好了路径变长导致的超长行，涉及 20+ 文件。这是「改引用」的必然结果，不是额外重构。

### 6.2 本轮做的清理

- `capabilities_for` 移出 `domain`（Task 3.5），`domain` 现在真正零跨层引用。
- `golden_tests.rs` 的 prose 去掉 `crate::`，避免契约检查假阳性。
- `.rs` 注释里的 `registry/env_var.rs` 一并修正为 `registry/env_edit.rs`（见 D-10）。

### 6.3 环境异常（需要你判断）

**`CLAUDE.md` 在 01:49:39 被本窗口之外的东西改写**，内容是把表格分隔线加长、插入多余空行——与 2026-10-28 00:50 那次 `spec` 被改写是**同一签名**（都是「去 prettier 化」，HEAD 版本 prettier 干净、工作区版本不干净）。

- 本窗口在 Task 6 中写入并 prettier 格式化过 `CLAUDE.md`/`AGENTS.md`，提交 `027573f` 时两者 `cmp` IDENTICAL；
- `npm run verify` 只执行 `format:check`（不写文件），但该文件在 verify 运行期间被改写；
- 已用 `git checkout -- CLAUDE.md` 还原，还原后 `cmp` IDENTICAL、`prettier --check` 通过，并持续观察到收尾未再变化；
- 并行会话 `PathEditor审核 [18cc9c]` 当时为 idle。

**猜测**：环境里有编辑器/VSCode 的自动保存或 markdown 表格格式化在监听这些文件。**建议**核对是否存在此类 watcher；若再次出现，本轮的 `npm run verify` 绿是在文件仍格式化时取得的，结论不受影响，但工作区会被污染。

## 7. 偏离记录（与计划的差异）

| #    | 位置                 | 计划原文                                                                    | 实际                                                                                  | 处置                                                                                             |
| ---- | -------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| D-1  | Task 1/2/3           | 只覆盖 `crate::` 与 `path_editor_core::`                                    | 还有 **`core::` 别名**（11 处 alias / 67 处引用）                                     | 补第三/第四种形态的 sed；`core::ffi` 不受影响                                                    |
| D-2  | 所有 `sed -i`        | `sed -i -E`                                                                 | 本机 `sed -i` 会把整文件 CRLF 翻成 LF                                                 | 全部改 `sed --binary -i`，实测行尾完整保留                                                       |
| D-3  | Task 2 Step 1        | 只移动文件                                                                  | `capabilities.rs` 的 `include_str!` 相对路径下移一层后失效                            | 改为 `../../../../src/...`，Python 验证可解析                                                    |
| D-4  | Task 3 Step 5 检查 3 | `\{[^}]*\b(service\|…)` 期望无输出                                          | `{app::service,` 仍命中，检查永远无法变空                                             | 复核时加 `\| grep -v 'app::'`                                                                    |
| D-5  | Task 2 Step 1        | 目录 `git mv`                                                               | `registry/` rename 报 `Permission denied`                                             | 逐文件 `git mv` + `rmdir` 空目录                                                                 |
| D-6  | Task 1 Step 7        | 一次编译通过                                                                | 首次 `cargo check` 出现一次 E0583（未复现）                                           | 记录备查，未改代码                                                                               |
| D-7  | 提交操作             | `git add -A`                                                                | **`git mv` 会自动暂存**，首次提交回执把 Task 3 的 rename 一并带进 docs 提交 `a41fc82` | `git reset`（mixed）回退，拆成 `b153ec6` + `1adcd5c`                                             |
| D-8  | Task 3.5 Step 3      | 片段写 `crate::system::check_admin()` / `crate::registry::can_write_user()` | Task 2 已把它们移到 `platform`，片段已过期                                            | 使用 `system::` / `registry::`（文件顶部已 `use crate::platform::{registry, system}`），行为不变 |
| D-9  | Task 3/7 契约检查    | `platform → app` 要求无输出                                                 | `golden_tests.rs` 的**文档注释**含 `crate::app::disabled`，会造成假红                 | prose 改为 `app::disabled`（`a435b21`）；Task 6 Step 2 的 README 同样改用 `app::disabled`        |
| D-10 | Task 6 Step 1        | 只改路径前缀，行号不校正                                                    | 3 处 `.rs` 注释指向的 `registry/env_var.rs` 实际是 `env_edit.rs`                      | 按「改为新路径」一并修正；行号未动（一处仍在 `backup/env.rs`，详见下方注）                       |
| D-11 | Task 6 Step 7        | `cargo fmt` 后重跑                                                          | `cargo fmt` 把 20+ 文件写成 LF，`w/crlf` 从 284 掉到 262                              | 对 fmt 改写的文件恢复 CRLF；其中 5 个原本就是 LF，已改回 LF。最终字节级 pure-CRLF=**284**        |
| D-12 | 环境                 | —                                                                           | `CLAUDE.md` 在 01:49:39 被外部进程改写（去 prettier 化，同 00:50 spec 事件）          | `git checkout -- CLAUDE.md` 还原；`cmp` IDENTICAL、prettier 干净                                 |

> D-10 补充：`env_edit.rs` 自身没有 `registry/env_var.rs` 注释；两处出现在 `platform/backup/env.rs`，已修正。全仓库已无指向 `registry/env_var.rs` 的注释。

## 8. 交付物位置索引

| 交付物      | 路径                                                                  |
| ----------- | --------------------------------------------------------------------- |
| 本回执      | `docs/审核和开发/2026.10.28/PathEditor-module-layering开发回执.md`    |
| 核对轮交接  | `docs/审核和开发/2026.10.28/PathEditor-module-layering核对轮交接.md`  |
| v2 设计文档 | `docs/superpowers/specs/2026-10-28-module-layering-design.md`         |
| v3 实施计划 | `docs/superpowers/plans/2026-10-28-module-layering-implementation.md` |

## 9. 合并与分支状态

- 分支 `refactor/module-layering`，领先 `main` 15 个提交（6 docs + 1 计划修订 + 7 refactor + 1 docs 回执）。
- 工作区**干净**：`git status --porcelain` 无输出。
- **未推送、未升级版本号、未合并到 main**——三项均待用户明确授权。

---

**签章**：以上结论均基于本机实际执行的命令输出；未做的项已在「未覆盖项」如实列出。
