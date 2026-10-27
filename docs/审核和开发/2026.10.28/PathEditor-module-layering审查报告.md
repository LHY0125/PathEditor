# PathEditor 模块分层重组 审查报告

- **审查日期**：2026-10-28
- **被审分支 / 提交**：`refactor/module-layering` @ `f190e03`
- **基准点**：`main` @ `4734188`
- **范围**：17 个提交，140 个文件，+2,908 / −464 行
- **功能**：把 `src/core/src` 的 17 个并列项收敛为 `domain / platform / db / app` 四层；把 `src/cli/src` 的 `*_ops.rs` 收敛进 `commands/`
- **规格来源**：`docs/superpowers/specs/2026-10-28-module-layering-design.md`（v2）、`docs/superpowers/plans/2026-10-28-module-layering-implementation.md`（v3）
- **规范来源**：`AGENTS.md` / `CLAUDE.md`、`CONTRIBUTING.md`、`docs/dev/*`
- **复现命令**：`git diff main...refactor/module-layering`

## 提交清单

```text
f190e03 docs: 修正回执中的提交计数
6217de9 docs: 模块分层重组开发回执（Task 1-7 完成，质量门全绿）
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

## 一、验证证据

审核窗口**未采信回执数字**，全部自跑。

| 命令                                                                        | 结果                                                                         |
| --------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `cargo fmt --all -- --check`                                                | exit 0，无输出                                                               |
| `cargo clippy --workspace --all-targets -- -D warnings`                     | exit 0，`Finished`                                                           |
| `cargo test --workspace`                                                    | **324 passed / 0 failed**（逐条求和）                                        |
| `cargo test -p patheditor-cli --bins`                                       | **66 passed / 0 failed**                                                     |
| `npm run verify`                                                            | **exit 0**（format:check + eslint + build + coverage + fmt + clippy + test） |
| `ls -1 src/core/src`                                                        | `app db domain lib.rs platform`                                              |
| `ls -1 src/cli/src`                                                         | `cli.rs commands main.rs output.rs runtime.rs`                               |
| `grep -rn 'crate::\(platform\|db\|app\)' src/core/src/domain`               | 0 行                                                                         |
| `grep -rn 'crate::\(db\|app\)' src/core/src/platform \| grep -v '/backup/'` | 0 行                                                                         |
| `grep -rn 'crate::app' src/core/src/db`                                     | 0 行                                                                         |
| `cmp CLAUDE.md AGENTS.md`                                                   | `IDENTICAL`                                                                  |
| `git status --porcelain`                                                    | 空                                                                           |

**绿灯 ≠ 规格全覆盖。** 上述结果只证明「编译、测试、契约检查通过」，不证明文档与代码一致、不证明 D4 的补偿措施已落地——见第三节。

### 重命名相似度（判断是否为纯移动）

```text
R100  path_entry.rs → domain/path_entry.rs
R100  sync_ops.rs   → commands/sync.rs
R100  backup/config.rs → platform/backup/config.rs
R099  error.rs      → domain/error.rs
R098  service.rs    → app/service/mod.rs
R098  db/backups.rs → db/backup_index.rs
R095  path_ops.rs   → commands/path.rs
R087  env_ops/cmd.rs → commands/env/cmd.rs
R075  import_export.rs → commands/import_export.rs（40 行的小文件，几行 import 即占 25%）
R077  scan_ops.rs   → commands/scan.rs（72 行，同理）
```

全部 87–100%，**纯移动成立**，没有整文件重写。

## 二、Standards 轴

### 硬性违反

无。

### 判断性意见

| 编号 | 位置                             | 问题                                                                                                                                                                                                                                            | 严重度 |
| ---- | -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| S-1  | `src/core/src/db/mod.rs:1-4`     | **D4 的补偿措施未落地**。spec D4 决定不做 `db/security/` 分组，代价交换是「在 `db/mod.rs` 顶部用文档注释标注安全边界文件清单」。实测该文件头部仍是原来的 4 行（SQLCipher / BEGIN IMMEDIATE / CAS），没有 `crypto` / `dpapi` / `dacl` 的说明。   | 中     |
| S-2  | `src/core/src/platform/mod.rs:5` | 行文与实现不符：写「拆开会产生 **storage** ↔ platform 环」，但本仓库没有 `storage` 层（那是 v1 spec 的废弃命名），实际环是 `db` ↔ `platform::backup`。                                                                                          | 低     |
| S-3  | 计划 Task 3.5 代码片段           | 计划里写的是 `crate::system::check_admin()`，而 Task 2 已把 `system` 移入 `platform`，照抄不编译（开发窗口已按现有 `use` 改写为 `system::check_admin()`）。**这是计划文档的缺陷，不是实现的缺陷**，但说明计划里新增的代码片段没有经过编译验证。 | 低     |

### 合规亮点

- 全部质量门由审核窗口独立复跑，与回执数字一致（324/0/2、CLI 66、verify exit 0）。
- 依赖方向契约三条全零，且 `platform` 那条正确带了 `/backup/` 白名单过滤。
- `a435b21` 只改 1 行注释（`crate::app::disabled` → `app::disabled`），是消除假阳性所必需，且有 commit message 说明——不是「为过检查而改代码」。
- `lib.rs` 只导出新路径，**没有保留任何过渡别名**。
- `docs/dev/architecture.md` 的「完整目录树」已重写，三个二级标题（完整目录树 / 模块边界速记 / 数据流）都在，依赖方向说明已补在 58 行。
- `README.md` 与 `CLAUDE.md` 的目录树已同步到四层；`cmp` 字节级一致。
- 当前态文档里已无旧路径（`grep -rnE 'src/core/src/(error|…|disabled)\.rs'` 为空）。
- 未覆盖项与 12 条偏离全部如实列在回执里，没有粉饰。

## 三、Spec 轴

### (a) 规格要求但缺失或只做一半

- **S-1（同上）**：D4 的文档补偿缺失。这是本节唯一一条实质缺口。D4 拒绝 `db/security/` 的理由是「3 个文件相邻的收益不值 55 处路径改动」，替代方案就是注释标注；不做等于这个交换只做了一半。

### (b) 规格没要求但做了的行为（scope creep）

无。`a435b21` 的注释改动属消除假阳性的必要处置，`027573f` 的文档同步属 Task 6 计划内。

### (c) 实现与规格/计划不符

- **S-2（同上）**：`platform/mod.rs` 用了废弃层名 `storage`。
- **S-3（同上）**：计划自身代码片段过期，实现按实际 import 处理是正确的偏离。
- **计划外的事实**：spec §1.3 / Task 6 Step 6 的旧路径检查在**改造前就是空过的**（正则只认 `src/core/src/` 前缀，而树形图写成 `│ │ ├── service.rs`），已在核对轮 H1 修正。本条记录在案，不再计为实现问题。

### 测试缺口

- 本次是纯移动，**没有新增测试**，324 与基线一致——这是正确结果，不是缺口。
- `capabilities_for` 从 `domain` 移到 `platform` 时，其两个单测随之搬移，测试数量未变。
- Playwright E2E 未跑：`npm run verify` 不含 E2E（`verify:all` 才含），计划只要求 `verify`。**E2E 与本次改动无交集**（前端零改动），但仍属未覆盖项。

### 未覆盖项（如实列出，承自回执）

1. Playwright E2E 未跑（计划未要求）。
2. 真实注册表闭环未做（纯重构，全部走 `TempDir`）。
3. `src/core/tests/db_backups_index.rs` 文件名仍含 `backups`（模块已改名，计划未要求改文件名）。
4. `platform/registry/golden/README.md` 第 14 行仍提到不存在的 `merge_golden_tests.rs`（计划 §7 明确范围外）。
5. 工作区 16 个 `.rs` 为 LF（索引侧统一 LF，不影响 `git diff`；但与 `.gitattributes` 的 `eol=crlf` 期望不一致，属环境卫生）。

## 四、总结

### 判定

**可以合并。** 需要合并前修掉 S-1 与 S-2——两处都是文档级，一个小提交即可，不涉及代码逻辑。

### 合并前建议整改（按优先级）

1. **[必须，中]** `db/mod.rs` 顶部补安全边界说明，例如：

   ```rust
   //! **安全边界文件**：`crypto.rs`（密钥派生/加载）、`dpapi.rs`（Windows DPAPI）、
   //! `dacl.rs`（密钥文件 ACL 校验）。审查密钥与权限相关改动时，这三个文件必须一起看。
   ```

2. **[建议，低]** `platform/mod.rs:5` 把「storage ↔ platform 环」改为「db ↔ platform::backup 环」，并把「唯一的层内强连通分量」补一句「另有跨层的 `db ↔ platform::backup`，见 spec §3.3 白名单」。

3. **[可延后]** 工作区 LF 文件恢复为 CRLF（`git checkout` 即可，索引侧无差异）；`db_backups_index.rs` 文件名是否随模块改名，留待下次。

### 版本与发布建议

- **不建议升版本**。本次零行为变更、零用户可见改动，版本号应留给真正的功能变更；升 6.2.0 会让 CHANGELOG 出现一条「内部重构」，对用户无信息量。
- 合并用 `--no-ff` + `chore: 合并模块分层重组（core 四层 / CLI commands）`，与 `main` 上既有的 `chore: 合并 …` 惯例一致。
- 不推送、不合并、不打 tag 需用户单独授权；本报告不含这些动作。
