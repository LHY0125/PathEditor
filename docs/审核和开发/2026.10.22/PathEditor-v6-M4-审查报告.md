# PathEditor v6.0.0 M4 审查报告

- **审查日期**：2026-10-22
- **被审对象**：`main` @ `0a97867`（已推送，与 `origin/main` 一致）
- **基准点**：`1b978b0`（M4 规划起点）
- **范围**：15 个提交，40 个文件，+5221 / −115
- **功能**：配置同步落库（profile / revision / 墓碑 / 同名改名）、保留策略清理、`.v5.bak` 归档、遗留明文备份转换清理、v5 配置导入、版本 6.0.0
- **规格来源**：`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` §4.2 第 10 条、§5.3、§6.3–§6.5、§7.4、§十二、§十三
- **规范来源**：`CLAUDE.md` / `AGENTS.md`、`CONTRIBUTING.md`
- **开发回执**：`docs/审核和开发/2026.10.21/PathEditor-v6-M4-开发回执.md`（333 行）
- **复现命令**：`git diff 1b978b0..0a97867`

## 提交清单

```text
0a97867 docs(v6): M4 开发回执
ecf300e chore(v6): 版本号升至 6.0.0
0aa6214 docs(v6): M4 功能与 6.0.0 发布日志
5f18dbc feat(v6): 支持从 v5 profiles/*.json 导入配置
26dcc4d feat(v6): 遗留明文 env 备份先转 .pebak 再删除
1d604cf feat(v6): v5 原件归档为 .v5.bak（位置改到启动序列末尾）
9121322 feat(v6): 保留策略清理（changes 90 天 / 冲突 30 天）
74d2d51 test(v6): 配置同步重复导入保持幂等
0ef05b5 test(v6): 墓碑不会被低 lamport 的旧内容复活
2d6ba81 feat(v6): 同步导入支持配置墓碑软删除
d1d853f feat(v6): 配置同步遇到同名时自动改名并在导入提示中计数
034ffbe feat(v6): M4 配置同步落库与依赖排序
1c0c294 docs(v6): M4 核对轮交接（S-1/S-2 已裁断，C-1~C-5 按建议执行）
a65f650 docs(v6): M4 计划与 spec §6.5（配置同步/清理/发布）
435bc76 docs(v6): M4 开工提示词
```

## 一、验证证据（审核窗口独立执行）

| #   | 命令                                                    | 结果                                             |
| --- | ------------------------------------------------------- | ------------------------------------------------ |
| 1   | `cargo fmt --all -- --check`                            | exit 0                                           |
| 2   | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0，无 error / warning                       |
| 3   | `cargo test --workspace`                                | **56 binaries, 349 passed, 0 failed, 2 ignored** |
| 4   | `npx tsc --noEmit`                                      | exit 0                                           |
| 5   | `npx vitest run`                                        | **33 files, 398 passed**                         |
| 6   | `npm run test:e2e`                                      | **54 passed (1.5m)**                             |
| 7   | 版本号四处 + 锁文件                                     | 全部 **6.0.0**，无 5.1.7 残留                    |
| 8   | `git rev-parse HEAD` vs `origin/main`                   | 一致（`0a97867`）                                |
| 9   | `git tag --list`                                        | **无 v6.0.0**（未打 tag）                        |
| 10  | `CLAUDE.md` / `AGENTS.md` 差异                          | 本轮未修改                                       |

全部数字与回执 §4 一致：Rust 48/340 → 56/349（+8 binaries / +9 tests），前端 397 → 398，E2E 54 → 54。**无一项下降。**

> 绿灯不等于规格全覆盖。以下结论来自逐文件阅读与 `file:line` 核对，不是「测试过了」的同义反复。

## 二、Standards 轴

### 硬性违反

**无。**

- `gui` / `cli` 只做参数转换与呈现：`maintenance_ops.rs` 只负责打开库、打印结果；`prune` / `archive` / `cleanup` / `import_profile_v5` 的业务规则全在 `core`。
- CLI 侧无重复的安全判定：`--yes` 是**确认门**，不是业务规则；真正的删除条件在 `maintenance.rs`。
- 组件与 Store 只经 `src/services/backend.ts` 调 IPC；`SyncCenter.tsx` 仍只消费 store。
- 所有新增 `pub fn` / `pub(crate) fn` 有中文 `///`；`ProfileSyncContent` 也有。
- 无死导出：`scan_legacy_env_backups`（CLI dry-run）、`prune_plan`（CLI）、`prune_now`（GUI）、`archive_v5_sources`（GUI+CLI）、`import_profile_v5`（CLI）、`import_profile_content_tx`（`apply_entity_tx`）全部有生产调用者。
- Conventional Commits 合规；未删除任何文件；未修改 `CLAUDE.md` / `AGENTS.md`。

### 判断性意见

#### J-1【应改·安全边界】远端 `profile.name` 未做校验

- **位置**：`core/src/db/profiles.rs:213-223`（反序列化 + `unique_name_tx`）、`:257-294`（INSERT/UPDATE）
- **事实**：`import_profile_content_tx` 直接使用 `c.name`，没有调用同文件的 `validate_name`（`:41`）。
  一个同步包可以把**空名、超长名、含控制字符的名称**写进 `profiles.name`，随后进入 UI。
  若与本地重名，`unique_name_tx` 还会把空名变成 `" (来自 dev-x)"`。
- **为什么重要**：这是「外部数据跨越信任边界」的经典入口。包虽然用口令加密，但发送端可能是
  更新版本、有 bug 的版本，或被用户分享口令的另一台机器；名称会持久化并被渲染。
- **建议**：在 `import_profile_content_tx` 开头调用 `validate_name(&c.name)`，失败返回
  `SyncFormat`（整包回滚）。同步导入的自动改名仍走 `unique_name_tx`，它生成的名字已受 255 字节约束。
  同理建议对 `description` 做长度上限（可选）。

#### J-2【应改·注释】`import.rs:88-89` 的注释已过期

```rust
/// 比 [`APPLICABLE_ENTITY_TYPES`] 宽：白名单里的类型**都是合法输入**，
/// 只是其中一部分当前版本还落不了库。
```

M4 后两个常量内容完全相同（都是 5 种），这句话与事实相反。开发窗口在 Task 11 未修。
建议改成：「两者当前一致；保留分离是为了将来新增已知类型时先接受、后实现」。

#### J-3【应改·文档】计划 Execution Notes 未回填

`docs/superpowers/plans/2026-10-21-v6-sqlite-m4-plan.md` 的 `## Execution Notes` 仍是占位符。
开发窗口把偏离记录全部放在回执 §3，但**计划是两窗口的权威信道**；M3 的偏离是回填进计划的。
本轮已由审核窗口据回执 §2/§3/§5/§6 代填，并在文中注明来源。请开发窗口确认无遗漏。

#### J-4【判断】`prune --dry-run` 仍会创建密钥与数据库

- **事实**：`cli/src/maintenance_ops.rs:15-20` 用 `open_default()`；后者在
  `core/src/db/mod.rs:224-248` 调用 `load_or_create_key` 并打开/创建 `patheditor.db`。
  在从未运行过 v6 的机器上，`maintenance prune --dry-run` 会创建**密钥文件 + 空库 + schema**。
- **对照**：回执 U-1 只写了「创建真实库文件」，漏了密钥文件。
- **建议**：若要求 dry-run 严格纯读，可在 `open_db()` 前判断 DB 文件是否存在，不存在则直接打印
  「无数据库，0 条」并返回。当前不影响数据安全（不删不改已有数据），**非阻塞**。

#### J-5【判断】增量同步包不自包含

- **事实**：`export_bundle` 只收集 `exported_at IS NULL` 的变更。若配置的创建快照已在**更早的包**中导出，
  后续包只含 `profile` 变更；该变更的 `content.snapshotUid` 指向那个旧快照。
  一台**新设备**若只拿到后续包，本地没有该快照 → 按 spec §6.5.1 返回 `SyncFormat` 整包回滚。
- **影响**：这是增量同步的固有语义，不是 M4 引入的缺陷；但 README 没有说明「同步包必须按顺序应用」。
- **建议**：在 README 同步章节补一句「同步包是增量包，新设备需要从第一份包开始按顺序应用；
  只拿到后续包会因引用悬空而拒绝」。**非阻塞**。

#### J-6【发布门】远端没有任何独立验证

- **事实**：推送 `0a97867` 未触发 CI；仓库只有 `release.yml`（`v*` tag 触发）与路径过滤的探针流水线。
  `release.yml` 只构建、不跑测试（M3 审查报告 §六已记录）。
- **影响**：v6.0.0 的全部质量证据来自本机 GNU 工具链；MSVC（CI 实际使用的工具链）从未跑过测试。
- **建议**：在打 `v6.0.0` tag **之前**二选一：
  1. 给 CI 加一个测试 job（`cargo test --workspace` + `npx vitest run`；MSVC 下若遇
     `0xC0000139`，按 `gui/build.rs` 的提示处理 comctl32 清单）；
  2. 明确接受「发布基于本地证据」，并在 CHANGELOG/回执中写明。
     **这不阻塞代码合并，但阻塞「发布」这个动作。**

#### J-7【判断】S-2 的启动顺序没有回归测试

- **位置**：`gui/src/lib.rs:136-154`
- **事实**：归档在 ⑤/⑥ 之后，推理正确、注释详尽，但这个顺序没有任何测试守卫；
  将来一次重构把归档挪回 ③b，现有测试不会红。
- **建议**：可在 `gui` 侧把 `setup_database` 的步骤拆成可测的小函数并断言顺序，或至少在
  `archive_v5_sources` 的调用处留一个 `// 顺序是安全性：不要前移` 的显式标记（现有注释已接近）。
  **非阻塞**。

### 合规亮点

- **S-1 的发现与处置**：开发窗口在写代码前就识别出 M3 的守卫测试会被 Task 1 淘汰，并给出改写口径。
  这比「测试红了再改」高一个层级。
- **S-2 的失败链推理**：从 `load_path_snapshot` 的回落路径反推出「文件已改名 + 读路径仍依赖文件」，
  并主动把归档挪到最后。这是本轮最有价值的一条。
- **C-4 的反向验证**：换回字段级 LWW → `left: 0, right: 1` → 还原后 `git diff` 为空。测试有证伪能力。
- **遗留备份的转换顺序**：写 `.pebak` → 回读 → 删 `.json`；任一失败保留原文件。
  `Option<&KeyMaterial>` 让「dry-run 却真去加密」在类型上不可表达。
- **严格文件名匹配**：只处理 `env_backup_YYYYMMDD_HHMMSS_mmm.json`，不碰用户自己的 `env_backup_x.json`。
- **归档的 `is_file()` 与排序**：防止目录名为 `xxx.json` 时整次归档中止；输出可复现。
- **版本一致性**：`package.json` / `Cargo.toml` / `Cargo.lock` 三个 crate / `tauri.conf.json`（含窗口标题）/
  `package-lock.json` / README 两处，全部 6.0.0。
- **未碰真实用户文件**：只跑了 `archive-v5 --dry-run` 与 `cleanup-legacy-backups --dry-run`；
  明确拒绝启动真实 GUI；U-1/U-2/U-3 如实披露。
- **回执质量**：§0 事故、§4 分节点测试数、§5 未覆盖、§6 残余风险都写实，没有「全部通过」的空话。

## 三、Spec 轴

### (a) 规格要求但缺失或只做一半

| #   | 规格                                          | 状态                                                                                                  |
| --- | --------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| a-1 | §十二「`CLAUDE.md` / `AGENTS.md` 字节级一致」 | **未做，且与当前 `CLAUDE.md` 自述冲突**。本轮未修改任何 CLAUDE.md，回执 R-7 已登记，待用户裁断        |
| a-2 | §6.5.3 接口表                                 | 实现多出 `prune_plan` / `prune_now` / `scan_legacy_env_backups`；审核窗口已回填 spec §6.5.3           |
| a-3 | GUI 的 v5 导入入口                            | spec §6.5.3 只列 CLI，**不算缺失**；GUI 的 `profileExportV5` 仍无 UI 调用者（既有问题，不在 M4 范围） |

其余 §6.5.1 / §6.5.2 / §6.5.4 / §6.5.5 逐条对照，**全部实现且与规格一致**：
可落库类型、依赖排序、uid 保留、usage 统计本地、同名改名、墓碑、实体级 LWW、悬空引用回滚、
保留谓词、归档跳过、转换顺序、`--yes`、不在范围项均未越界。

### (b) 规格没要求但做了的行为（scope creep）

| #   | 行为                                   | 判定                                               |
| --- | -------------------------------------- | -------------------------------------------------- |
| b-1 | `scan_legacy_env_backups`              | **合理**：C-2 要求 dry-run 不创建密钥；已回填 spec |
| b-2 | `prune_plan` / `prune_now`             | **合理**：dry-run 计数与 GUI 时间源；已回填 spec   |
| b-3 | `map_change_row` 共用                  | **合理**：避免两处 13 列映射漂移                   |
| b-4 | 归档 `is_file()` + `candidates.sort()` | **合理**：防御性，行为不变                         |
| b-5 | 维护 CLI 文案「将归档到」              | **合理**：消除歧义                                 |

### (c) 实现与规格/计划不符

| #   | 计划原文                                | 实际                                     | 裁断                                                       |
| --- | --------------------------------------- | ---------------------------------------- | ---------------------------------------------------------- |
| c-1 | `-p path_editor_core`                   | 包名是 `path-editor-core`                | **计划笔误**，开发窗口已改用连字符；审核窗口已修正计划文本 |
| c-2 | 归档在 ③b 之后                          | 移到 `setup_database` 末尾               | **接受**（核对轮 S-2）                                     |
| c-3 | `db_sync_import_limits.rs` 保持 M3 断言 | 按 S-1 改写为 M4 语义                    | **接受**（核对轮裁断）                                     |
| c-4 | Task 6 CLI 用 `open_and_migrate`        | 改用 `open_default + migrate_to_current` | **接受**（C-1）                                            |
| c-5 | Task 8 CLI 先加载密钥                   | dry-run 走 `scan_legacy_env_backups`     | **接受**（C-2）                                            |
| c-6 | CHANGELOG 顶部新增 `## 6.0.0`           | 直接改写已有「未发布」标题               | **接受**（语义等价，release 正则可命中）                   |

## 四、总结

### 判定

**通过（代码层无阻塞项）。**

M4 的 11 个 Task 全部落地；质量门全绿且与回执逐项一致；S-1/S-2 两条阻塞项在开工前被开发窗口
自己抓住并正确处置；测试有证伪能力（C-4 反向验证）。没有发现功能性缺陷、数据安全缺陷或规格越界。

**发布前**（打 `v6.0.0` tag 前）建议完成：

1. **J-6 CI 决策** —— 加测试 job，或明确接受本地证据。这是唯一真正的发布门。
2. **J-1 `profile.name` 校验** —— 外部输入跨信任边界，建议同批修（代码 3 行）。
3. **J-2 / J-3 文档收尾** —— 过期注释；Execution Notes 已代填，确认无遗漏。
4. **J-5 README 增量包说明** —— 一句话，避免新设备导入失败时无解释。

### 合并前建议整改（按优先级）

| 优先级 | 项                                                                      | 类型     |
| ------ | ----------------------------------------------------------------------- | -------- |
| P0     | 无                                                                      | ——       |
| P1     | J-6 CI 测试决策                                                         | 发布门   |
| P1     | J-1 `profile.name` 校验                                                 | 安全加固 |
| P2     | J-2 过期注释；J-3 Execution Notes 确认                                  | 文档     |
| P2     | J-4 dry-run 纯读（可选）；J-5 README 增量说明；J-7 顺序回归测试（可选） | 判断性   |

### 审核窗口自己的失误（必须记录）

1. **计划里的 `-p path_editor_core` 是错的**（应为 `path-editor-core`）。开发窗口在 D-1 发现并绕过；
   审核窗口已修正计划文本。
2. **计划漏了「被本次变更淘汰的守卫测试」**：`db_sync_import_limits.rs` 是 M3 的 R-1 守卫，
   Task 1 使 `profile` 可落库后它必然变红。开发窗口在 S-1 抓到，审核窗口没有。
3. **计划的 dry-run 设计有副作用**：`open_and_migrate` 会导入 v5 文件、`load_or_create_key` 会建密钥。
   开发窗口在 C-1/C-2 抓到。这三条都与 M3 的 `find_cycles` / 白名单误判同源——**计划只描述增量，
   没有系统检查增量的反作用力**。

### 本轮未执行（如实列出）

- 未启动真实 GUI（`setup_database` 会在真实目录归档）；未对真实 `~/.patheditor` 执行
  `prune` 实跑或 `cleanup-legacy-backups --yes`。
- 未验证 MSVC 工具链；CI 未跑测试。
- 未做真机双端同步（无第二台机器）。
- 未打 tag、未创建 Release。
- 未运行 `npm run verify:all` 全量串联：各分项（fmt / clippy / cargo test / tsc / vitest / E2E）
  已独立执行，结果见 §一。

---

**审查窗口签章**：§一 的每条结果均来自本轮独立执行的命令；§二/§三 的每条判断均附 `file:line`
或规格条目。J-1 / J-4 / J-5 是基于代码阅读的判断，未做运行时复现，已在正文注明。
