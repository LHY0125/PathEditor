# 模块分层重组 — 开发回执（阻塞待裁断）

**提交窗口**：开发窗口
**日期**：2026-10-28
**交付对象**：审核窗口
**分支**：`refactor/module-layering`
**版本**：6.1.1（未升级）
**状态**：**阻塞**。Task 1–3 已完成并提交，编译全绿；**Task 4–7 未开始**。阻塞原因是 spec 的依赖方向契约与仓库实际不符（详见第 5 节），需裁断后才能继续。

---

## 1. 交付内容概述

已完成的三层抽取：

| 层         | 内容                                                                               | 状态                       |
| ---------- | ---------------------------------------------------------------------------------- | -------------------------- |
| `domain`   | `error` / `path_entry` / `env_var` / `history`                                     | 已提交 `b45a6f7`           |
| `platform` | `persist` / `fs` / `system` / `scanner` / `capabilities` / `registry/` / `backup/` | 已提交 `5e31fcb`           |
| `app`      | `service/` / `bootstrap` / `disabled`                                              | **已实施未提交**（工作区） |

`src/core/src` 顶层已收敛为 `app db domain lib.rs platform`（Task 3 后实测）。

## 2. 代码改动清单

### 2.1 已提交

**Task 1（`b45a6f7`，86 files changed，141 insertions / 135 deletions）**

```text
rename src/core/src/{ => domain}/env_var.rs (100%)
rename src/core/src/{ => domain}/error.rs (99%)
rename src/core/src/{ => domain}/history.rs (98%)
create mode 100644 src/core/src/domain/mod.rs
rename src/core/src/{ => domain}/path_entry.rs (100%)
```

**Task 2（`5e31fcb`）**：`persist / fs / system / scanner / capabilities` 五个文件 + `registry/`（含 `golden/` 20 个 JSON 夹具）+ `backup/` 整目录移动，rename 相似度 90–100%；新增 `src/core/src/platform/mod.rs`。

### 2.2 Task 3（`1adcd5c`，23 files changed，39 insertions / 34 deletions）

```text
rename src/core/src/{ => app}/bootstrap.rs (98%)
rename src/core/src/{ => app}/disabled.rs (100%)
create mode 100644 src/core/src/app/mod.rs
rename src/core/src/{service.rs => app/service/mod.rs} (99%)
rename src/core/src/{ => app}/service/profile.rs (100%)
rename src/core/src/{ => app}/service/restore.rs (100%)
rename src/core/src/{ => app}/service/tests.rs (100%)
```

`rmdir src/core/src/service` 已执行（空目录）。`src/core/src` 顶层已收敛为 `app db domain lib.rs platform`。

## 3. 提交清单

```text
1adcd5c refactor: 抽出 core::app 编排层
b153ec6 docs: 提交模块分层重组开发回执（Task 1-2 完成，Task 3 阻塞待裁断）
5e31fcb refactor: 抽出 core::platform 适配层
b45a6f7 refactor: 抽出 core::domain 契约层
acc9878 docs: 按核对轮裁断修订模块分层计划（v2）
```

> 注：回执先在 `a41fc82` 提交过一次，随后发现 `git mv` 会自动暂存，把 Task 3 的 6 个 rename 一并带进了那个 docs 提交（内容还是 sed 之前的，属编译不过的中间态）。已用 `git reset a41fc82^`（mixed，不动工作区）回退，拆成 `b153ec6`（纯回执）与 `1adcd5c`（Task 3）两个提交。详见偏离 D-7。

## 4. 测试与质量门（已完成部分）

| 项                    | 命令                                         | 结果                                                                                                               |
| --------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| 基线（Task 1 Step 1） | `cargo test --workspace`                     | **324 passed / 0 failed / 2 ignored**                                                                              |
| 行尾基线              | `git ls-files --eol src \| grep -c 'w/crlf'` | **283**                                                                                                            |
| Task 1 编译           | `cargo check --workspace --all-targets`      | `Finished`，无 error / warning                                                                                     |
| Task 2 编译           | `cargo check --workspace --all-targets`      | `Finished`，无 error / warning                                                                                     |
| Task 3 编译           | `cargo check --workspace --all-targets`      | `Finished`，无 error / warning                                                                                     |
| 行尾（每一步后复查）  | 同上                                         | 全程未降（283 → Task 3 提交后 284，+1 是 `w/lf` 被 checkout 成 CRLF，方向安全）；`sed --binary` 未造成任何 CRLF→LF |

Task 1 / Task 2 提交前的 `git status` 均含 `R`（rename），无 `deleted:` + `new file:`。

---

## 5. 阻塞项：spec 的依赖方向契约与仓库实际不符

### 5.1 现象

计划 Task 3 Step 6 要求三条依赖方向检查全部无输出：

```bash
$ grep -rn 'crate::\(platform\|db\|app\)' src/core/src/domain
src/core/src/domain/env_var.rs:231:        EnvHive::System => crate::platform::system::check_admin(),
src/core/src/domain/env_var.rs:232:        EnvHive::User => crate::platform::registry::can_write_user(),

$ grep -rn 'crate::\(db\|app\)' src/core/src/platform
（13 处真实代码引用，全部在 platform/backup/，另 1 处为 golden_tests.rs 的文档注释）

$ grep -rn 'crate::app' src/core/src/db
（无输出 ✓）
```

### 5.2 这些边**改造前就存在**，不是本次移动引入的

```bash
$ git show acc9878:src/core/src/env_var.rs | grep -n 'crate::system\|crate::registry'
231:        EnvHive::System => crate::system::check_admin(),
232:        EnvHive::User => crate::registry::can_write_user(),

$ git show acc9878:src/core/src/backup/env.rs | grep -n 'crate::db::'
163:    match crate::db::crypto::load_or_create_key(...)
187:    material: &crate::db::crypto::KeyMaterial,
189:    let path = crate::db::backups::write_env_backup_pebak(...)
392:            crate::db::backups::read_pebak_header(&path)
```

### 5.3 根因一：spec §2 的依赖矩阵记错了两行

spec 第 62 行写 `env_var, path_entry, system, scanner → （无）`，但 `env_var` 实际依赖 `system` 与 `registry`（权限探测）。第 70 行写 `backup/* → registry, persist, env_var, error`，漏了 `db`。

### 5.4 根因二：`db ↔ platform::backup` 是**第二个强连通分量**

```bash
$ grep -rn 'crate::platform::backup' src/core/src/db | grep -vE ':\s*(//|///|//!)'
db/backups.rs:24   use crate::platform::backup::{EnvBackupPayload, EnvBackupVar};
db/backups.rs:219  return crate::platform::backup::read_env_backup(path);
db/backups.rs:309  match crate::platform::backup::read_env_backup(&path) {
db/backups.rs:397  refresh_and_list_at(db, &crate::platform::backup::env_backup_dir())
db/maintenance.rs:14  use crate::platform::backup::EnvBackupPayload;
db/maintenance.rs:259 use crate::platform::backup::{EnvBackupHives, EnvBackupVar};

$ grep -rn 'crate::db::' src/core/src/platform/backup | grep -vE ':\s*(//|///|//!)' | wc -l
13
```

`db` 引用 `platform::backup`，`platform::backup` 又引用 `db::crypto` 与 `db::backups` —— **双向环**。spec §2.1 断言「`registry` 与 `backup` 是唯一强连通分量」，该断言不成立。

### 5.5 后果

- spec §6 验收标准「1.3 节的三条依赖方向检查全部无输出」**无法达成**；
- 计划 Task 3 Step 6 与 Task 7 Step 3 必然失败；
- 这不是编译问题（三处 `cargo check` 全绿），而是**契约本身与现状矛盾**。

### 5.6 需要裁断的选项

| 方案              | 做法                                                                                                                                                                            | 改动量                   | 影响                                         |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------ | -------------------------------------------- |
| **A（本轮推荐）** | 把两条边登记为**已知例外**，检查命令加白名单：`domain→platform` 仅 `env_var.rs:231-232`；`platform↔db` 仅 `backup/` 与 `db::{crypto,backups}`。spec §3.3 改写为「按白名单豁免」 | 零代码改动               | 符合「纯重构、零行为变更」；契约有洞但显式化 |
| B                 | 把 `env_var.rs` 的权限探测改为调用方注入，使 `domain` 变纯；`db↔backup` 环另议                                                                                                  | 改公开 API 签名          | 超出「纯移动」范围                           |
| C                 | 把 `db::crypto` / `db::backups` 或整个 `platform::backup` 重新归位，消除环                                                                                                      | 大，触及 spec D1/D2 定案 | 需重写 spec 与计划                           |

---

## 6. 本轮已记录的偏离（与计划的差异）

| #   | 位置                  | 计划原文                                                                                    | 实际                                                                                                                                                                  | 处置                                                                                                                                   |
| --- | --------------------- | ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| D-1 | Task 1/2/3 Step 5/4   | 只覆盖 `crate::` 与 `path_editor_core::` 两种前缀                                           | 仓库还有 **`core::` 别名**（11 处 `use path_editor_core as core;`，共 67 处 `core::<模块>`）                                                                          | 补了第三个前缀的 sed；`core::ffi`（Rust 标准库）不受影响，已单独验证                                                                   |
| D-2 | 所有 Task 的 `sed -i` | `sed -i -E`                                                                                 | 本机 MSYS 的 `sed -i` 会把**整文件 CRLF 翻成 LF**（实测 269 行 CRLF → 0）；虽经 `.gitattributes` 归一后 `git diff --numstat` 仍只显示目标行，但 `w/crlf` 计数会掉     | 全部改用 **`sed --binary -i`**，实测行尾完整保留，283 基线全程未变                                                                     |
| D-3 | Task 2 Step 1         | 只移动文件                                                                                  | `capabilities.rs` 内含相对路径 `include_str!("../../../src/gui/web/tests/fixtures/path-capabilities.json")`，下移一层后失效                                           | 改为 `../../../../src/...`；已用 Python 验证路径可解析                                                                                 |
| D-4 | Task 3 Step 5 检查 3  | `grep -E '(path_editor_core\|crate)::\{[^}]*\b(service\|bootstrap\|disabled)\b'` 期望无输出 | 修复后 `{app::service,` 仍被正则命中（`[^}]*` 吃掉 `app::`），该检查**永远无法变空**                                                                                  | 用 `\| grep -v 'app::'` 复核，确认无真实残留                                                                                           |
| D-5 | Task 2 Step 1         | `git mv src/core/src/registry src/core/src/platform/registry`                               | 目录 rename 报 `Permission denied`（Windows 占用）                                                                                                                    | 逐文件 `git mv`，随后 `rmdir` 空目录；rename 状态正常                                                                                  |
| D-6 | Task 1 Step 7         | 期望一次编译通过                                                                            | 首次 `cargo check` 出现一次 E0583（未复现），随后两次连续 `Finished` 无 error/warning                                                                                 | 记录备查；未改变任何代码                                                                                                               |
| D-7 | 提交操作              | `git add -A` + commit                                                                       | **`git mv` 会自动把 rename 放进暂存区**。首次提交回执时未意识到这点，把 Task 3 的 6 个 rename（sed 之前的旧内容）一并带进了 docs 提交 `a41fc82`，形成编译不过的中间态 | `git reset a41fc82^`（mixed，工作区不动）回退，改为先 `git add <回执>` 单独提交（`b153ec6`），再 `git add -A` 提交 Task 3（`1adcd5c`） |

## 7. 未覆盖项（如实列出）

- **Task 4–7 未开始**（CLI `commands/` 收口、`db/backups` 改名、文档同步、全量质量门）。
- **未跑完整全量质量门**（`clippy -D warnings`、`cargo test --workspace` 全量、`npm run verify`）——只跑了每任务的 `cargo check`。Task 1/2 提交后未做全量回归，留待 Task 7。
- **未做真实注册表闭环测试**（本轮为纯重构，计划未要求；所有测试仍走 `TempDir`）。
- **未推送、未升级版本号、未合并到 main**。

## 8. 交付物位置索引

| 交付物     | 路径                                                                 |
| ---------- | -------------------------------------------------------------------- |
| 本回执     | `docs/审核和开发/2026.10.28/PathEditor-module-layering开发回执.md`   |
| 核对轮交接 | `docs/审核和开发/2026.10.28/PathEditor-module-layering核对轮交接.md` |

## 9. 合并与分支状态

- 分支 `refactor/module-layering`，领先 `main` 6 个提交（3 个 docs + 3 个 refactor）。
- 工作区**干净**：`git status --porcelain` 无输出；Task 1–3 均已提交（`b45a6f7` / `5e31fcb` / `1adcd5c`）。
- 三次 `cargo check --workspace --all-targets` 全绿，零行为变更成立。
- **等待审核窗口对第 5.6 节方案 A/B/C 的裁断**。裁断后：选 A 则直接继续 Task 4；选 B/C 则需先改 spec 与计划，并在 Task 3 之上追加整改提交。

---

**签章**：以上结论均基于本机实际执行的命令输出；未做的项已在第 7 节如实列出。
