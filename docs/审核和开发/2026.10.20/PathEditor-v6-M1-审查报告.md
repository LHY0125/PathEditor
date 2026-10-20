# PathEditor v6.0.0 M1（存储与历史）审查报告

- **审查日期**：2026-10-20
- **被审分支 / 提交**：`v6/m1-storage-history` @ `5277407`
- **基准点**：`main @ 9e674ec`
- **范围**：14 个提交，42 个文件，+4847 / −28 行
- **功能**：SQLCipher 加密 sidecar 数据库；PATH 快照与操作事件持久化；配置方案与备份索引入 DB；`.pebak` 加密备份；v5 文件迁移；GUI「历史记录」面板
- **规格来源**：`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`、`docs/superpowers/plans/2026-10-19-v6-sqlite-m1-plan.md`
- **规范来源**：`AGENTS.md` / `CLAUDE.md`、`docs/审核和开发/2026.10.19/PathEditor-v6-M1-plan核对轮交接.md`
- **复现命令**：`git diff main...v6/m1-storage-history`

---

## 提交清单

```
5277407 chore(v6): 锁定 SQLCipher 依赖树（rusqlite/blake3/uuid/tempfile）
e2e1486 docs(v6): M1 开发回执
42efd8d docs(v6): M1 文档同步（CHANGELOG 与 README 历史记录章节）
c9743f1 feat(v6): 历史面板（时间线、快照预览、敏感值掩码）
e85c39c feat(v6): 前端历史 IPC 方法与运行时形状校验
1b56883 feat(v6): 历史/配置/备份 IPC 命令（细粒度、分页钳制、降级错误）
d286c6f feat(v6): v5 文件一次性迁移（幂等、排除 Path）
bba6634 feat(v6): env 备份改为 .pebak（DPAPI），备份索引无需解密
8d4c1df feat(v6): 配置方案迁入 DB（标签、描述、应用计数、软删除同名重建）
291155c feat(v6): DbHistory 实现 HistorySink，事件落库
b8c2e2a feat(v6): 快照与事件读写（单事务、敏感值掩码、内容哈希）
923d972 feat(v6): DPAPI 密钥生命周期（互斥量 + create_new 原子创建）
5566332 feat(v6): Db 句柄与 v1 schema（SQLCipher、PRAGMA、版本迁移）
d7bf3f2 feat(v6): 新增 HistorySink 接缝，服务层不再直接依赖存储
9e674ec docs(v6): M1 实施计划与核对轮裁断          ← main
```

> 回执 §9 列了 12 个 Task 提交；实际 14 个（多出 `42efd8d` 文档同步与 `5277407` 依赖锁定两个提交，回执 §9 表格中未列）。功能范围一致。

---

## 一、验证证据

以下命令均由审核窗口**独立执行**，不采信回执数字。

| #   | 命令                                                    | 结果                                                               |
| --- | ------------------------------------------------------- | ------------------------------------------------------------------ |
| 1   | `cargo fmt --all -- --check`                            | exit 0                                                             |
| 2   | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0（`Finished dev profile in 1.36s`）                          |
| 3   | `cargo test --workspace`                                | exit 0；14 个测试二进制全绿，`302` 个 `test ... ok`                |
| 4   | 7 个 `core/tests/db_*.rs` 逐文件                        | 各 1 passed（`grep -c '#\[test\]'` 逐一验证均为 1）                |
| 5   | `npm run verify`                                        | exit 0（format → lint → build → coverage → cargo fmt/clippy/test） |
| 6   | `npx vitest run`                                        | `Test Files 27 passed (27)`，`Tests 330 passed (330)`              |
| 7   | `npm run test:e2e`                                      | `41 passed (1.4m)`                                                 |

**与回执的差异（如实记录）**：

| 项                            | 回执称 | 实测    | 判定                                                                                                              |
| ----------------------------- | ------ | ------- | ----------------------------------------------------------------------------------------------------------------- |
| `cargo test --workspace` 总数 | 301    | **302** | 回执少算 1。`running 231 tests` 实为 230 passed + 1 内联，回执按 229 计。**不影响结论**，但回执的算术与证据不符。 |
| `git log` 中 Task 提交数      | 12     | 14      | 多两个提交未在 §9 表格登记。                                                                                      |

> **绿灯 ≠ 规格场景全覆盖**。上表只证明「已存在的测试通过」，不证明规格里的每条要求在实现中被覆盖。下文 §三 列出未覆盖项。

---

## 二、Standards 轴

### 硬性违反

#### V-1【阻塞】密钥文件 DACL 完全未实现

**位置**：`core/src/db/crypto.rs`（全文 277 行）、`core/src/db/mod.rs:191`

- 规格：spec §5.1 明确要求「密钥文件 DACL：仅当前用户 + SYSTEM；启动时校验，权限过宽则告警（不自动修改）」；spec §十二 验收标准单独列为一条。
- 计划：M1 计划 `:597`（Task 3 规格表）、`:651`、`:654-678`（实现骨架 + 威胁模型）、`:1947`（核对轮裁断「补充-2」），要求实现 `set_key_dacl(path)` 与新增测试 `key_dacl_is_restricted`。
- 实际：`grep -rn "DACL\|set_key_dacl\|SetNamedSecurityInfo\|Win32::Security::Authorization" core/ gui/ cli/` **零命中**（`Win32::Security` 仅在 `dpapi.rs:13` 命中的是 `Cryptography`，与 ACL 无关）。7 个 `db_*.rs` 测试中也无 `key_dacl`。

**为什么重要**：DPAPI 与 DACL 是两个不同威胁模型——项目自己在计划 `:669-678` 里写过这张表：

| 威胁                                        | DPAPI    | DACL |
| ------------------------------------------- | -------- | ---- |
| 同一台机器的**另一个用户读取**密钥          | 挡       | —    |
| 同一台机器的**另一个用户删除/替换**密钥文件 | **不挡** | 挡   |

缺失 DACL 的后果不是「信息泄露」，而是**另一个用户可以把 `db.key.dpapi` 删掉或换成自己的密钥**。进程下次启动发现文件「存在但解不开」→ 按 D16 走 `create_key` 的 `AlreadyExists` 分支 → 返回 `Crypto` 错误、进入降级模式——**历史永久不可读，且用户无法自行恢复**。这条路径在实现里是死循环（文件在、解不开、不覆盖、不提示重建）。

`C:\Users\<name>\.patheditor` 在默认用户配置下并非所有用户不可写；一旦被外部进程/SMB 写入（仓库里 NAS/SMB 场景真实存在）改了 ACL，就可能出现上述状态。

**这不是「未覆盖项」，是回执未声明的规格缺失**：回执 §5「未覆盖项」列了 8 条 M1 内未完成项，**没有一条提到 DACL**；§6「需要审核窗口重点关注的项」也未提。按 `patheditor-two-window-flow` 的「未覆盖项、已知保留项如实列出，不粉饰」，这一条本该出现在回执里。

**整改**：在 Task 3 补 `set_key_dacl(path)`（写后立即设置，失败则删除半成品并返回 `Crypto` 错误）+ `open_default()` 启动校验（过宽只 `log::warn!`，不自动改）+ 新增核内测试 `key_dacl_is_restricted`。计划 `:656-678` 已给出实现要点，可直接照做。

---

#### V-2【阻塞】`DbHistory` 未接入任何生产路径——数据库里永远不会有事件

**位置**：`gui/src/commands/service.rs:7,9,21,30,36,42`；`cli/src/runtime.rs:81`；`gui/src/lib.rs:34`

- `DbHistory` 在全仓库只有两处引用：`core/src/db/history_sink.rs` 的定义、`core/tests/db_history_sink.rs:31` 的测试。**没有任何生产代码构造它**。
- GUI 的 4 个写命令全部传 `&NoopHistory`：`save_path_with_sidecar`、`apply_path_snapshot`、`retry_pending_path_state`、`apply_profile`。CLI 同样传 `NoopHistory`。
- 代码已自我承认：`gui/src/commands/service.rs:7` 的注释写「命令层先统一传 [`NoopHistory`]，历史落库由 Task 9 的 `DbState` 接入后替换」——但 Task 9 提交（`1b56883`）最终没有替换。

**为什么重要**：这直接让 spec §六「历史时间线」在最主要的入口上失效。GUI 与 CLI **永远不写 `events` 表**。可达的唯一写入点是 `migrate_v5::migrate_legacy_with`，而 `migrate_legacy` 在生产中**也没有调用者**（`grep -rn "migrate_legacy"` 仅命中定义与测试）。

**端到端后果**（叠加 V-3 后完全闭环）：

1. 用户启动 GUI → `open_default` → 建库 → `migrate_to_current` → 空库（9 张表，0 行）。
2. 用户保存 PATH → `commands::service::save_path_with_sidecar` → 注册表写入成功 → `NoopHistory::record` → **无事发生**。
3. 用户打开「历史记录」标签页 → `history_list_events` → `SELECT ... FROM events` → **空列表**，面板显示「还没有历史记录」。

用户看到的界面是「历史记录功能上线了，但我做的每一次操作都不在里面」。这是本里程碑最核心的对外承诺，当前实现为**恒空**。

回执 §5.2 列了「`EventView.snapshotUid` 未回填」「面板没有恢复按钮」，但没有列出「历史根本不写入」——§2 表格中 `DbHistory` 被标为「新增 ~80 行 / `DbHistory` 实现 `HistorySink`」，读起来像是已接通。

**整改（两个层次，缺一不可）**：

1. `gui/src/commands/service.rs`：4 个写命令改为从 `State<DbState>` 取 `&mut Db`，构造 `DbHistory::new(db)` 传入。`&mut Db` 与 `MutexGuard` 的借用需要生命周期整理（`DbHistory<'a>` 持有 `&'a mut Db`，而 `State` 的 guard 生命周期跨不过 `tauri::command` 的返回），建议在 `commands::history` 里加一个 `with_db_sink` 辅助函数把「锁 → 构 sink → 调 service」收在一处。
2. 启动序列补 `migrate_v5::migrate_legacy(&mut db, &crypto::config_dir())`——当前**没有任何生产代码调用它**，`disabled.json` / `profiles/*.json` / 老备份文件都不会被迁移。

> **第 2 条的归因是计划缺陷，不是开发窗口的遗漏。** 我核对了计划 Task 9 Step 4 的启动片段，它只写：
>
> ```rust
> let db = path_editor_core::db::open_default().ok();
> app.manage(DbState(Mutex::new(db)));
> ```
>
> **没有** `migrate_legacy`。计划 Task 8 只写了 `migrate_legacy` 的实现与测试，而全计划的 5 处 `migrate_legacy` 引用中**没有一处位于启动序列**（`:1457` 签名、`:1466/:1496/:1499` 测试）。所以开发窗口是**照着计划做的**。
>
> 这条要由审核窗口（本报告作者）承担责任：M1 计划在「把 pending 回放移出迁移函数、交给启动序列」这个裁断后，**没有回头把迁移本身也接进启动序列**。开发窗口偏离 #2 的推理（避免集成测试污染真实 `~/.patheditor/`）是**正确的**；问题出在计划端没有闭合这个环。
>
> 整改时请把这一条与 V-2 第 1 条一起做，并同步修订计划 Task 9 的启动片段。

---

#### V-3【阻塞】`.pebak` 写入器无生产调用者，README/CHANGELOG 的对外声明不成立

**位置**：`core/src/db/backups.rs:88`（`write_env_backup_pebak`）；`core/src/backup.rs:314,337,664-668`；`cli/src/env_ops.rs:555`；`gui/src/commands/backup.rs:18-19`

- `write_env_backup_pebak` 的调用者只有 `core/tests/db_backups_index.rs:43`。
- 生产入口 `backup::backup_env_vars()`（`:664`）→ `write_env_backup_to(&dir, &payload)` → `:337` 仍然 `dir.join(format!("env_backup_{}.json", timestamp))`。**仍然写明文 JSON**。
- CLI `env backup`（`cli/src/env_ops.rs:555`）与 GUI `backup_env_vars`（`gui/src/commands/backup.rs:18`）都走这条老路径。
- `read_env_backup_any` 同样无生产调用者——`restore_env_backup_from` 走的是老 `read_env_backup`。

**对外声明与实现的落差**：

- `CHANGELOG.md` 「M1 新增」段：`**`.pebak`加密备份**：… 遗留`.json` 仍可一次性读取（带明文警告）`
- `README.md:281-287`：「环境变量备份从本版起写 `.pebak`（DPAPI 加密），遗留的 `env_backup_*.json` 仍可一次性恢复（会提示明文警告）」

**这两句在合并后即为不实陈述**：用户 `env backup` 得到的是明文 JSON，`env restore` 也不会识别 `.pebak`，也没有「明文警告」。

同时 spec §十二 验收标准「新 env 备份为 `.pebak`（DPAPI），磁盘上不再新增明文敏感文件」**未达成**；spec §7 第 9 条「CLI 备份语义（H6，已定）」列出的 5 条（写 `.pebak`、`env backups` 带 `encrypted` 标志与变量数、`env restore` 支持两种、`--json` 新增字段、DPAPI 失败 exit 5）**一条都没落地**。

回执 §5 未列出此项。

**整改**：把 `backup_env_vars()` / `write_env_backup_to()` 改走 `write_env_backup_pebak`（需要 `KeyMaterial`，即 `backup_env_vars` 要能拿到 `Db` 或密钥）；`read_env_backup_from` / `restore_env_backup_from` 改用 `read_env_backup_any`；CLI `env backups` 列表补 `encrypted` / `format` 字段。**若确认 M1 不做，则必须回退 README/CHANGELOG 的这两段措辞**，不能带着不实声明合并。

---

#### V-4【记录，不阻塞】计划 Task 12 的交付物命名与仓库固定约定冲突

**这是计划的问题，开发窗口做对了。**

- `patheditor-two-window-flow` 的交付物表规定：开发窗口产出 **`开发回执`**。
- 计划 Task 12 原文写的是 `PathEditor-v6-M1-完成报告.md`（名称非标准，与固定约定冲突）。

开发窗口偏离 #11 明确说明「遵循 `patheditor-two-window-flow` 的固定路径约定」，并实际命名为 `PathEditor-v6-M1-开发回执.md`——**符合约定，是正确的**。

**待办在计划端**：M1 计划的 Task 12 没改，下一轮 M2 计划照抄模板时会继续复制这个错误名称。见 J-1。

### 判断性意见

#### J-1 修计划 Task 12 的交付物名称

把 `docs/superpowers/plans/2026-10-19-v6-sqlite-m1-plan.md` Task 12 里的 `PathEditor-v6-M1-完成报告.md` 改为 `PathEditor-v6-M1-开发回执.md`，并在计划末尾的核对轮裁断表补一行说明。否则 M2 计划会继承这个错误。

#### J-2 硬编码中文文案，绕过 i18n

**位置**：`src/components/history/HistoryPanel.tsx:6-25`（`ACTION_LABEL` / `SOURCE_LABEL` 两个常量表，共 17 个中文标签）、`:127`（`'部分成功'` / `'失败'`）、`:137`（`'系统'` / `'用户'`）；`src/components/history/SnapshotDetail.tsx:11`（`'（空）'`）

**为什么重要**：仓库有完整的 i18n 基础设施（`src/i18n/index.ts`，`zh-CN` + `en` 双语）。同一次提交里 `src/i18n/locales/{zh-CN,en}.json` **只加了 `tab.history` 一个键**（`git diff` 确认），面板内部却把 `t('history.title', '历史记录')` 的 fallback 当真值用——**英文用户**会看到：

- 标签页名：`History`（来自 locale）
- 面板标题：`历史记录`（fallback 中文）
- 所有事件动作/来源：中文
- `部分成功` / `失败` / `系统` / `用户`：中文

这是「半个 i18n」：既有翻译框架又在框架外写死文案。既有组件（`MergePreview.tsx:21,28,50,51`、`PathTable.tsx:103`）全部走 `t('key')` 无 fallback，风格不一致。

**注意**：`t('key', 'fallback')` 这个写法本身在仓库里是新引入的模式——17 个键全部只有 fallback、locale 里没有对应条目，说明作者本意是「先写死中文，以后补」。但 `en.json` 同步只加 `tab.history`，等于把这个「以后」漏在了当前版本。

#### J-3 三处硬性事实与代码不符的注释/文档

| 位置                             | 说法                                                      | 事实                                  |
| -------------------------------- | --------------------------------------------------------- | ------------------------------------- |
| `gui/src/commands/service.rs:7`  | 「历史落库由 Task 9 的 `DbState` 接入后替换」             | Task 9 已提交且**没有替换**（见 V-2） |
| `core/src/db/migrate_v5.rs:53`   | 「执行迁移并上报事件（GUI/CLI 注入 `DbHistory` 时使用）」 | GUI/CLI **没有任何注入点**            |
| `README.md:281` / `CHANGELOG.md` | 「本版起写 `.pebak`」                                     | 生产路径仍写 `.json`（见 V-3）        |

「行文与实现不符的注释」是 `patheditor-review-window` 4.2 明列的必查项。这三处会让下一个维护者（和下一个 AI 会话）相信功能已通。

#### J-4 降级条幅永远不会显示具体原因

**位置**：`src/store/history-store.ts:34`；`src/components/history/HistoryPanel.tsx:83`

`parseCoreError`（`backend.ts:242-263`）在失败时 **`throw` 一个 `RejectionPayload` 对象**（`{code, message}`，普通对象，不是 `Error` 实例）。而 store 写的是：

```ts
set({ loading: false, error: e instanceof Error ? e.message : String(e) });
```

`e instanceof Error` 恒为 `false` → 走 `String(e)` → **`"[object Object]"`**。

后果：降级条幅显示 `历史不可用（降级模式）：[object Object]`，spec §十二「启动校验失败有明确告警」的用户可见部分失效。

仓库里已有正确做法可参照：`src/store/env-store.ts:57-63` 的 `errorMessage(err)` 用 `isCoreError(err)` 判定并优先取 `err.message`。建议直接复用。

#### J-5 新增 IPC 命令 `profile_mark_applied` 未被 `apply_profile` 调用

`gui/src/commands/service.rs:41-43` 的 `apply_profile` 只调 `service::apply_profile(&name, &NoopHistory)`，**没有**在成功后调 `profiles::mark_profile_applied`。虽然前端暴露了 `backend.profileMarkApplied`，但全仓库**没有任何前端代码调用它**（`grep -rn "profileMarkApplied" src/` 仅命中 `backend.ts` 的定义）。

后果：`profiles.apply_count` / `last_applied_at` 恒为 0 / NULL。spec §六「配置元数据」的「应用次数」不生效。

#### J-6 `history_get_current` / `backup_list` / `profile_list` / `profile_delete` 无前端消费者

`grep` 确认这 4 个 backend 方法（`backend.ts:866-889`）在前端**零调用**。它们是给 M2 准备的，但当前属于**死代码**——`patheditor-review-window` 4.2 要求「查是否有死导出」。

不阻塞（M2 会用到），但建议在回执里像 §5.2 那样明确标出「已实现未接入」，避免 M2 重复造。

#### J-7 `app_state` 行只在 v5 迁移时创建——新装用户永远没有

**位置**：`core/src/db/migrate_v5.rs:68-75`（唯一的 `INSERT INTO app_state`）

新库建好后 `app_state` 为**空**。后果链：

1. `history::app_state_version(db)` → `query_row` → 无行 → `Err`。
2. `history::set_current_snapshot(db, uid, expected_version)` 的 CAS 语句 `WHERE id = 1 AND version = ?2` → **匹配 0 行** → 返回 `Ok(false)`。**CAS 永远失败**（spec §十二「并发写：两进程同时写，CAS 冲突返回 conflict」在当前状态下不可能通过，因为 CAS 从来不成功）。
3. `DbHistory::record` 用 `COALESCE((SELECT device_id FROM app_state WHERE id = 1), 'local')` 兜底成 `'local'`。

回执 §5.3 R-03 **只写了第 3 条**（device_id 兜底），没写第 1、2 条更严重的后果。M3 同步依赖 `device_id` 与 `lamport`，届时必须建库即写 `app_state`；但 CAS 失败是**当下**就存在的缺陷。

#### J-8 事件模型的可用性缺口（非阻塞，但需要在 M2 前决策）

- `history_list_events` 的 `EventFilter` 只有 `limit/offset/action/source`（`gui/src/commands/history.rs:22-49`），spec §六要求 `hive` / `target` / `result` / 时间范围，spec §3.2.1 规则 1 要求「所有列表命令接受 `{limit, offset}`」。
- `history-restore-snapshot` 缺失（回执已声明）。
- `events.snapshot_id` 从不回填 → `EventView` 无 `snapshotUid` → 前端 `selectEvent` 拿 **事件 uid 去查快照**（`history-store.ts:47`），必然 `NotFound` → `catch` 清空详情。
  **净效果**：历史面板点击任何一条事件，右侧详情永远空白。这是**功能性失效**，不是「M1 最小可用」——面板在 M1 事实上只有左半边可用。

回执 §5.2 写了这条（「无法定位到具体快照」），但措辞是「查不到时清空详情」，弱化了「**每次点击都查不到**」。建议整改或明确降级为「M1 面板只读事件列表」。

#### J-9 降级条幅只存在于历史面板内部，不是「常驻」

**位置**：`src/components/history/HistoryPanel.tsx:74-87`（唯一的条幅）

`grep -rn "降级\|degraded" src/components/layout/*.tsx` **零命中**——`AppShell.tsx` 没有任何降级提示。

spec §7 第 10 条（DB 不可用 / B7）要求：「fail-open …… **常驻错误条幅**；降级操作不写历史（**状态栏明示**）」。

实际效果：DB 打不开时，用户**只要不切到「历史记录」标签页，就完全看不到任何提示**。用户会以为历史功能正常，直到某天点进去才发现全是空——而此时可能已经积累了大量「应该有记录却没有」的操作。

**整改**：把降级状态提升到 store（或 AppShell 启动时探一次 `history_get_current`），在 `AppShell` 顶部渲染常驻条幅，而不是只在历史面板里。

#### J-10 `ErrorCode` 新增 4 个码，但前端只用到 2 个

`src/core/env-var.ts` 补了 `database` / `crypto` / `sync` / `migration`，`backend.ts:203` 的 `ERROR_CODES` 白名单同步扩充。但前端只有 `history-store.ts` 依据 `code === 'database'` 做降级判定（且该判定因 J-4 实际不可达）。`crypto`（密钥错误 / DPAPI 失败）是 spec §8 里对应 exit 5 的**高价值分支**——用户换机后所有历史打不开，此时应给出「密钥与新机器不匹配」的明确文案，而不是笼统的「不可用」。

M1 可延后，但需在回执里登记为「码已定义、分支未使用」。

### 合规亮点

- **`HistorySink` 接缝设计干净**：`core/src/history.rs` 的 trait 不要求 `Send + Sync`，与 M1 同步单线程前提一致；`NoopHistory` 让既有测试零改动。回执偏离 #1（`RefCell<&mut Db>`）是**正确的编译层面取舍**，`history_sink.rs:23-31` 的文档注释把「为什么不是裸 `&mut Db`」讲清了。
- **敏感值掩码在 Rust 侧完成**：`core/src/db/history.rs:551` 的 `if reveal || sensitive == 0 { raw } else { None }`——前端从不接收未请求的明文，`backend.ts:861-867` 的注释也明确了这条边界。`insert_snapshot_tx:305` 的 `sensitive` 用 `env_var::is_sensitive` **重新计算并覆盖**调用方传值，是纵深防御的正确做法。
- **`content_hash` 的三处防碰撞设计**（`history.rs:313-342`）：区分 hive、插入互不相同的分隔字节、env 按名排序——这是把「哈希相等 = 语义相等」当契约来写的，超出一般实现水平。
- **一文件一测试**执行彻底：7 个 `db_*.rs` 逐一 `grep -c '#\[test\]'` 全为 1，注释里写明了 `STATUS_STACK_OVERFLOW` 的原因与出处（M0 §5.3）。
- **总览账目正确**：`insert_snapshot` 不写 `events`（由 `HistorySink` 负责），`save_path_with_sidecar_io` 内的 pending 回放传 `NoopHistory`（避免一次保存两条事件）——两处都是「避免重复记账」的主动设计，且有测试断言（`db_history_roundtrip.rs:86`）。
- **DB 打开的 PRAGMA 顺序**（`db/mod.rs:41-63`）：先 key、再只读查询验 HMAC、最后设 PRAGMA，注释写明了「否则错误密钥会先被 `journal_mode=WAL` 触发，错误语义模糊」。
- **`db_err` 写成二元函数**（`db/mod.rs:181`）并注释了「为什么不能返回闭包」，12 处调用点全部正确改写。
- **前端 IPC 边界守住**：`grep -rn "invoke(" src/ | grep -v backend.ts` → **0 命中**，所有 IPC 都经 `backend.ts`。
- **`npm run verify` / E2E 独立复跑全绿**，回执数字除 301→302 外均可复现。

---

## 三、Spec 轴

### (a) 规格要求但缺失或只做一半

| #    | Spec 条目                                                                      | 状态                 | 证据                                                                                                                                                                                                                                                                            |
| ---- | ------------------------------------------------------------------------------ | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| a-1  | §5.1 / §十二「密钥文件 DACL 仅当前用户 + SYSTEM；启动校验失败有明确告警」      | **完全缺失**         | 见 V-1，零命中                                                                                                                                                                                                                                                                  |
| a-2  | §六「历史时间线」：`list_events` 等实际产生数据                                | **写入未接通**       | 见 V-2，GUI/CLI 全传 `NoopHistory`                                                                                                                                                                                                                                              |
| a-3  | §7.4「原件保留为 `*.v5.bak`；不自动删除」                                      | **未做**             | `migrate_v5.rs:6` 的模块文档写「`.v5.bak` 归档与删除留给 M4（spec §7.4）」，但 **spec §7.4 并没有把它划给 M4**——spec §7 第 4 条是迁移规则的一部分，M4 只被 §7 第 6 条授权处理「清理 `env_backup_*.json`」。这是一处**规格误读**，开发窗口把 M4 的清理授权扩大到了 M1 的归档动作 |
| a-4  | §7.8「迁移成功后正常模式不再写 `disabled.json`」                               | **未做**             | `disabled.rs` 未改动，仍写 v5 文件                                                                                                                                                                                                                                              |
| a-5  | §7.9 CLI 备份语义 5 条（`.pebak` 写/列表/恢复/JSON 字段/exit 5）               | **0/5 落地**         | 见 V-3                                                                                                                                                                                                                                                                          |
| a-6  | §3.2.1 规则 1「所有列表命令接受 `{limit, offset}`」                            | **部分**             | `backup_list` / `profile_list` **无分页参数**（`gui/src/commands/history.rs:114,142`），回执 §5.2 已承认                                                                                                                                                                        |
| a-7  | §3.2.1「`profile_save(request)` / `profile_apply(uid)` / `profile_export_v5`」 | **缺失**             | 回执 §5.2 承认；`profile_apply` 由既有 `apply_profile` 承担，但**不记 `apply_count`**（见 J-5）                                                                                                                                                                                 |
| a-8  | §3.2.1「`history_reveal_env_value`」                                           | **缺失**             | 回执 §5.2 承认                                                                                                                                                                                                                                                                  |
| a-9  | §十二「并发写：两进程同时写，CAS 冲突返回 `conflict`，无 lost update」         | **不可达成**         | 见 J-7，`app_state` 无行 → CAS 恒返回 `false`                                                                                                                                                                                                                                   |
| a-10 | §3.2.1 测试要求「E2E 只覆盖 3–4 条主路径（历史面板、恢复、配置应用）」         | **0 条**             | `e2e/tests/` 12 个 spec 文件无 `history` 相关（`grep -rln history e2e/` 零命中）                                                                                                                                                                                                |
| a-11 | §7.1「首次启动先调用 `retry_pending_path_state`」                              | **未在启动序列实现** | `grep migrate_legacy` 无生产调用者                                                                                                                                                                                                                                              |
| a-12 | §7.2 导入规则（4 类文件）                                                      | **已实现但有缺陷**   | `migrate_v5.rs` 实现了 4 类，但**无人调用**                                                                                                                                                                                                                                     |

### (b) 规格没要求但做了的行为（scope creep）

| #   | 行为                                                          | 判定                                                                                   |
| --- | ------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| b-1 | `Db::query_row_optional`（`db/mod.rs:154-169`）               | 合理补全——注释解释了「`CoreError` 下 `.optional()` 不适用」，集成测试需要              |
| b-2 | `backup.rs:395-405` 的 `rotate_env_backups` 同时收集 `.pebak` | **合理但当前无效果**——因为 `.pebak` 从不被写（V-3）；逻辑本身正确，是为 V-3 整改预铺的 |
| b-3 | `Db::table_exists`（`db/mod.rs:109`）                         | 合理补全（测试/诊断）                                                                  |
| b-4 | `ErrorCode::Crypto → exit 5` 分步实现（`Sync → 4` 留 M3）     | 合理，回执 §6 已请确认                                                                 |

### (c) 实现与规格/计划不符

| #    | 计划原文                                                                           | 实现                                     | 回执是否登记                                                                    |
| ---- | ---------------------------------------------------------------------------------- | ---------------------------------------- | ------------------------------------------------------------------------------- |
| c-1  | `DbHistory<'a>(pub &'a mut Db)`                                                    | `DbHistory<'a>(pub RefCell<&'a mut Db>)` | ✅ 偏离 #1，理由成立                                                            |
| c-2  | Task 8 步骤 1「先 `retry_pending_path_state(&NoopHistory)`」在 `migrate_legacy` 内 | 移出                                     | ✅ 偏离 #2。**但移出后未在启动序列补上**（V-2 整改第 2 条）                     |
| c-3  | `DDL_V1` 中 `app_state` 在 `snapshots` 之前                                        | `snapshots` 在前                         | ✅ 偏离 #3，理由正确（外键依赖）                                                |
| c-4  | `tx.pragma_update(None,"user_version",1)`                                          | `execute_batch`                          | ✅ 偏离 #4                                                                      |
| c-5  | `assert_eq!(key_hex.len(), 64)`                                                    | 返回 `CoreError`                         | ✅ 偏离 #5，理由正确（库函数不 panic）                                          |
| c-6  | `EventView` 5 字段                                                                 | 含 `hive` / `target`                     | ✅ 偏离 #6                                                                      |
| c-7  | Task 9「11 个 `#[tauri::command]`」                                                | **7 个**                                 | ✅ 偏离 #7。但 §5.2 的措辞是「未完成」，而实际是「规格要求 11 个，只做了 7 个」 |
| c-8  | `EventFilter::from_request` 关联函数                                               | 自由函数                                 | ✅ 偏离 #8                                                                      |
| c-9  | `ErrorCode::Database` 用于「快照不存在」                                           | `NotFound`                               | ✅ 偏离 #9，**判断正确**（语义更精确）                                          |
| c-10 | Task 7「扫描目录」                                                                 | 三格式（`.pebak`/`.json`/`.txt`）        | ✅ 偏离 #10                                                                     |
| c-11 | 计划 Task 12 `M1-完成报告.md`                                                      | `M1-开发回执.md`                         | ✅ 偏离 #11，**命名正确**（见 V-4/J-1）                                         |
| c-12 | 计划未提 UPSERT                                                                    | 「先查后写」                             | ✅ 偏离 #12                                                                     |

**偏离登记质量**：12 条偏离**全部有据可查、理由成立**，这是本轮回执质量最高的部分。问题集中在**未登记的部分**（V-1 DACL、V-2 未接线、V-3 `.pebak` 未接通、J-7 `app_state`），而不是已登记的偏离。

---

## 四、总结

### 合并前建议整改（按优先级）

**必须改（阻塞合并）**

1. **V-1 补 DACL**：`set_key_dacl` + `open_default` 启动校验 + `key_dacl_is_restricted` 测试。计划 `:654-678` 已给骨架。
2. **V-2 接通 `DbHistory`**：`commands::service` 的 4 个写命令改传真实 sink；补 `migrate_legacy` 的启动调用。**否则「历史记录」面板对所有用户恒空**。
3. **V-3 二选一**：要么把 `backup_env_vars` / `restore_env_backup_from` 改走 `.pebak`，要么**回退 README/CHANGELOG 的对外声明**。带着「本版起写 `.pebak`」合并而实际写明文，是错误披露。
4. **J-4 修降级条幅**：`history-store.ts:34` 改用 `isCoreError` 判定（参照 `env-store.ts:57-63`），否则告警文案是 `[object Object]`。
5. **J-9 条幅提升到全局**：降级提示目前只在历史面板里，切到别的标签页就看不到；spec §7.10 要求「常驻条幅 + 状态栏明示」。
6. **J-8 决策**：历史面板点击事件必然空白（`snapshot_id` 未回填 + `selectEvent` 用错 uid）。要么回填 `events.snapshot_id`，要么在 M1 明确「面板只显示事件列表」，并去掉会误导的右侧详情区。

**应该改**

7. **J-5**：`apply_profile` 成功后调 `mark_profile_applied`，否则 `apply_count` 恒 0。
8. **J-7**：建库时写 `app_state` 单行（`device_id` 真实值 + `version=1`），否则 `set_current_snapshot` 的 CAS 恒返回 `false`（当前无测试覆盖这条路径）。
9. **J-3**：修 3 处与实现不符的注释/文档。
10. **J-2**：17 个硬编码中文标签 + `'部分成功'`/`'失败'`/`'系统'`/`'用户'`/`'（空）'` 补进 `locale`。
11. **J-1**：修计划 Task 12 的交付物名称，避免 M2 计划继承。

**可延后（记入 M2 计划）**

12. **J-6**：4 个无消费者的 backend 方法，在回执里标「已实现未接入」。
13. **J-10**：`crypto` / `sync` / `migration` 三个码已定义但无分支，登记即可。
14. **a-6 / a-7 / a-8**：分页、`profile_save`、`reveal` 入口 —— 回执已列为 M2。
15. **a-10**：历史面板 E2E —— spec §3.2.1 要求 M1 覆盖 3–4 条主路径，建议至少补 1 条（打开面板 → 空态 → 降级条幅）。
16. **a-11 / a-12**：`retry_pending_path_state` 启动序列 —— 与 V-2 第 2 条合并处理。

### 本轮判定

**需要整改后复审。**

本轮的**工程质量是高的**：偏离登记诚实完整、接缝设计干净、掩码与哈希的正确性考虑周全、测试组织严格遵循一文件一测试。`npm run verify` 与 E2E 在审核窗口独立复跑均全绿，回执的真诚度也高于前几轮（主动列出 8 条未覆盖项与 7 条残余风险）。

但**「绿灯 ≠ 规格全覆盖」在这轮体现得格外明显**：所有门都是绿的，而三个核心承诺——历史写入、`.pebak` 备份、密钥文件 DACL——在生产路径上都不成立。前两项使 README/CHANGELOG 的对外声明失真，第三项是 spec 验收标准里单独列出的一条。

整改后请按 `patheditor-two-window-flow` 交 **`PathEditor-v6-M1-复审回执.md`**，逐条给出 `file:line` 证据。复审窗口只核对上述 5 条阻塞项 + 4 条「应该改」，不重新审全量。
