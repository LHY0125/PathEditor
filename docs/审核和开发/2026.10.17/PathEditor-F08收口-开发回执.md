# F-08 收口 + 死码清理 — 开发回执

- **日期**：2026-10-17
- **提交窗口**：开发窗口
- **交付对象**：审核窗口
- **依据**：`docs/superpowers/specs/2026-10-17-f08-closeout-and-deadcode-removal-design.md`（含 D13–D16 / K7）+ `docs/superpowers/plans/2026-10-17-f08-closeout-and-deadcode-removal-implementation.md`（v2 修订版）
- **状态**：**Task 1–7 全部实现、质量门全绿**；**Task 8 的真机回归被既有环境问题阻塞**（详见 §5）。未提交、未推送、未升版本号。

---

## 1. 交付摘要

| 指标                                                    | 值                                                                         |
| ------------------------------------------------------- | -------------------------------------------------------------------------- |
| core 测试                                               | **224 passed / 0 failed / 2 ignored**（基线 219 → +5 契约测试）            |
| CLI 测试                                                | **65 passed / 0 failed**（基线 59 → +6）                                   |
| 前端测试                                                | **260 passed / 0 failed**（基线 305 → −45，删除了 3 个死模块的 45 个用例） |
| E2E                                                     | **30 passed**                                                              |
| 前端覆盖率                                              | **87.58% 行**（门槛 80%）                                                  |
| `npm run verify:all`                                    | **真实退出码 0**                                                           |
| `cargo clippy --workspace --all-targets -- -D warnings` | **退出码 0，零警告**                                                       |
| 新增契约测试                                            | **5 个**（K7，经 fake `PathHiveIo`，零真实注册表）                         |
| 删除文件                                                | 7 个（3 模块 + 4 测试，均在本波授权清单内）                                |
| 真实注册表写入                                          | **零**（Task 1–7；Task 8 被阻塞，见 §5）                                   |

**核心成果**：`core::service::save_path_with_sidecar` 成为 GUI + CLI **唯一**的「保存 PATH + sidecar」编排；`PathHiveIo` 注入缝使成功路径**可单测**（消解 K5 与 Global Constraints 的冲突）。

---

## 2. 逐 Task 证据

### Task 1：`PathHiveIo` 注入缝 + 契约测试

**实现**：`pub(crate) trait PathHiveIo { read_enabled / write_enabled / backup }` + `RealPathHiveIo`；`save_path_with_sidecar` 编排体抽到 `save_path_with_sidecar_io(io, ..)`（**纯重构**）。

| 步骤                  | 证据                                                                         |
| --------------------- | ---------------------------------------------------------------------------- |
| 纯重构无回归          | 抽取后 `cargo test -p path-editor-core service` → **6 passed**（与基线一致） |
| RED                   | `backup_called_once_when_write_happens` → `left: 0, right: 1`                |
| GREEN（顺序）         | `writes_are_sequential_system_then_user` 直接 GREEN（顺序本就成立）          |
| GREEN（无写入不备份） | `no_backup_when_both_hives_skipped` GREEN                                    |

**偏离**：plan Task 1 Step 3 的 `concurrent_mismatch_*` 测试**移到 Task 3**（它引用 `expected`，Task 1 时该字段不存在，`#[ignore]` 挡不住编译错误）。

**偏离（重要）**：`EnvHive` **未派生 `Hash`**，fake 内部原用 `HashMap<EnvHive, Vec<String>>` 编译失败。**未给 `EnvHive` 加 `#[derive(Hash)]`**（那会扩进 `env_var.rs`，属 F-3 领地）——改用有序 `Vec<(EnvHive, Vec<String>)>`。

### Task 2：补写前备份（经缝）

| 步骤         | 证据                                                                                   |
| ------------ | -------------------------------------------------------------------------------------- |
| 实现         | 编排层在「校验通过、确实将写入」后调 `io.backup()`；失败 `log::warn!` 不阻断           |
| GREEN        | `backup_called_once_when_write_happens` 转 GREEN（9 passed）                           |
| **变异测试** | 把条件改为 `if false` → 该测试转红（`left: 0, right: 1`）→ 恢复 → 绿。**可证伪性成立** |

### Task 3：补乐观并发（签名变更）

**实现**：`PathHiveWrite { entries, expected }`；`check_expected()`（只读校验）与 `write_hive_registry()`（纯写）分离，编排层先校验两个 hive、再备份、再写。

| 步骤         | 证据                                                                                                            |
| ------------ | --------------------------------------------------------------------------------------------------------------- |
| GREEN        | `concurrent_mismatch_writes_nothing_and_no_backup` + `concurrent_match_proceeds_with_write`                     |
| 三条「不」   | 断言「不写」（`write_order().is_empty()`）+「不备份」（`backup_count()==0`）+「不产 pending」                   |
| **变异测试** | 使 `check_expected` 恒通过 → 转红 → 恢复 → 绿                                                                   |
| N10          | `golden/README.md:62` 的 `service.rs:207` 引用**已漂移**（实际广播在 `:381`）→ 改为**符号名锚定**，去掉易腐行号 |
| 转调         | `apply_path_snapshot` / `apply_profile` 改为 `expected: None`                                                   |

**偏离（安全）**：两个既有测试（`registry_stage_failure_*` / `oversized_user_entry_*`）原用**真实现** `RealPathHiveIo`。Task 2 加入无条件备份后，它们会**落真实备份文件到 `~/.patheditor/backups/`**（副作用）。**改为经 fake IO**（`fail_write=true` 模拟），彻底消除副作用。

### Task 4：CLI 6 入口全收口 + 删广播 + 删死函数

**实现**：新增 `save_path_write(system, user)` 作为 CLI 唯一保存出口；**6 个入口**全部改用它：

| 入口                | 命令                      | 位置                       |
| ------------------- | ------------------------- | -------------------------- |
| `load_and_save`     | add / move-up / move-down | `cli/src/runtime.rs`       |
| `load_operate_save` | remove / edit             | `cli/src/runtime.rs`       |
| `clean_one`         | clean                     | `cli/src/main.rs`          |
| `cmd_toggle`        | enable / disable          | `cli/src/main.rs`          |
| `cmd_import`        | import                    | `cli/src/import_export.rs` |
| `profile_apply`     | profile apply             | `cli/src/profile_ops.rs`   |

- **删 7 处调用方广播**：`main.rs`（add / move / clean / toggle）、`import_export.rs`、`profile_ops.rs`、`runtime.rs`（`load_operate_save`）——广播下沉 core。
- **删 3 个死函数**（E，已授权）：`verify_and_save` / `persist_snapshot` / `flush_pending_snapshot`，及 `SaveFn` 类型别名。
- **N9**：`clean_one` 构造 `PathEntry` 时写死注释说明 `enabled: true` 的等价性。
- **退出码不变**：并发冲突仍走 `exit_err`（退出码 1）。
- **D14（行为变化）**：`clean` 现在**顺带提交 sidecar**（此前只写注册表）。

| 步骤     | 证据                                                                        |
| -------- | --------------------------------------------------------------------------- |
| CLI 测试 | **65 passed / 0 failed**                                                    |
| clippy   | workspace 零警告（**含无 `dead_code`** —— 证明 6 入口全接线、3 死函数删净） |

### Task 5：GUI 切换（`path-session.ts` 薄化）

**实现**：

- `backend.ts` 新增 `savePathWithSidecar` + `parseApplyOutcome`（运行时形状校验 `HiveOutcome`/`SidecarOutcome`，不靠 TS 断言）。
- `path-session.ts::savePathSnapshot` 只保留三件事：**权限预检**（`blocked`）+ **组合长度软提醒**（`warning`，K1）+ **`ApplyOutcome`→`SaveSnapshotOutcome` 映射**。
- **删除**：自理备份、并行写、store 内 pending。
- `default.json` 删 `maxSystemLength`/`maxUserLength`（保留 `maxCombinedLength`）。
- **N7**：`app-store.test.ts` 的 3 个 `savePaths` 测试按**新单次调用契约**重写。

| 步骤        | 证据                        |
| ----------- | --------------------------- |
| TS 类型检查 | `npx tsc --noEmit` 退出码 0 |
| 前端测试    | **305 passed / 0 failed**   |
| 覆盖率      | 88.34% 行（改后）           |
| 构建        | `npm run build` 退出码 0    |

**语义变化（须关注）**：`isModified` 不再反映「sidecar 未落盘」（pending 移到 core 磁盘）。GUI 不再就快照失败单独提示，改为由 core 的 `retry_pending_path_state` 自愈。**这是 D6/K4 的明确裁定**，已写进测试注释。

### Task 6：F-2 删除三个死码模块

**删前复核**：路径串 + 导出符号 + barrel 三重检索，确认 `src/`、`e2e/` **零运行时引用**。

**删除**（均在授权清单）：`src/core/{path-manager,import-export,validation}.ts` + `tests/unit/{path-manager,import-export,import-parity,validation}.test.ts`。

| 步骤   | 证据                                               |
| ------ | -------------------------------------------------- |
| 测试   | 260 passed（删了 45 个）                           |
| 覆盖率 | **87.58% 行 ≥ 80%**（分母确实变了：88.34 → 87.58） |
| 构建   | 退出码 0                                           |

### Task 7：F-6 删除三个孤儿 IPC 命令

**复核**：`scan_conflicts`/`scan_tools`/`get_appdata_dir`/`save_system_paths`/`save_user_paths` 在 `src/` 零**真实**调用（仅 e2e/单测 mock 提及）。

**删除**：`gui/src/lib.rs` 6 个注册项 + `scanner.rs`/`backup.rs`/`registry.rs` 对应包装；`backend.ts` 的 `saveSystemPaths`/`saveUserPaths` 封装；e2e/单测 mock 同步清理。

**保留**：`core::backup::get_appdata_dir()`、`core::scanner::{scan_conflicts,scan_tools}`（core 函数，被内部使用）。

| 步骤               | 证据             |
| ------------------ | ---------------- |
| Rust 构建 + clippy | 退出码 0，零警告 |
| E2E                | **30 passed**    |

---

## 3. plan 缺陷清单（执行中发现并就地处理）

| #   | 缺陷                                                          | 处置                             |
| --- | ------------------------------------------------------------- | -------------------------------- |
| 1   | Task 1 Step 3 的 `concurrent_*` 测试引用尚不存在的 `expected` | 移到 Task 3                      |
| 2   | fake 用 `HashMap<EnvHive, _>` 编译失败（`EnvHive` 无 `Hash`） | 改 `Vec`，**不给生产类型加派生** |
| 3   | 两个既有测试用真实现，Task 2 后会写真实备份                   | 改为 fake（安全修复）            |
| 4   | `golden/README.md:62` 行号引用已漂移                          | 改符号名锚定（N10）              |
| 5   | `app-store.test.ts` 3 个测试按旧 IPC 序列 mock                | 按新单次契约重写（N7 预判）      |

---

## 4. 质量门（实际输出）

```
$ npm run verify:all
=== REAL EXIT CODE: 0 ===
（Prettier → ESLint → Vite 构建 → 覆盖率 87.58% → cargo fmt --check
 → cargo clippy --workspace --all-targets -- -D warnings → cargo test --workspace
 → Playwright 30 passed）
```

```
$ cargo test --workspace
core:  224 passed; 0 failed; 2 ignored
cli:    65 passed; 0 failed
gui:     0 passed（无单测）
```

---

## 5. 真机回归：**被既有环境问题阻塞**（如实登记）

### 5.1 做了什么

按 plan Task 8 Step 3，用**唯一探针路径** `D:\__W30B_PROBE__` 跑 `patheditor add`（唯一授权写真实注册表处），并记录前后快照。

### 5.2 结果：**保存被并发校验拒绝**

```
$ patheditor add "D:\__W30B_PROBE__"
错误: 注册表已被其他进程修改，请重新执行操作
--- exit code: 1 ---
```

注册表未变（user 仍 1 条……实际 10 条，见下），探针未写入。

### 5.3 根因：既有数据陈旧（**已证明非本波引入**）

| 项                                                     | 值                                                                                 |
| ------------------------------------------------------ | ---------------------------------------------------------------------------------- |
| `HKCU:\Environment\Path` 条目数                        | **10**                                                                             |
| `~/.patheditor/disabled.json` 的 `userSnapshot` 条目数 | **11**                                                                             |
| 差异                                                   | 快照多一条 `D:\settings\settings\Scoop\apps\git\current\cmd`（**已从注册表消失**） |

`load_path_snapshot` 经 `merge_hive` 把该陈旧条目并入 → `expected`（11 条）≠ `current`（10 条）→ **保存被拒**。

**决定性对照（A/B）**：`git stash` 回到 **HEAD（`9f9c0d8`，本波之前）** 的代码，重建 CLI，跑**同一命令**：

```
$ target\release\patheditor.exe add "D:\__W30B_HEAD_PROBE__"
错误: 注册表已被其他进程修改，请重新执行操作
--- exit code: 1 ---
```

⇒ **HEAD 报完全相同的错误**。**本波改动不是原因。**（stash 已 pop 恢复。）

### 5.4 影响与结论

- **Task 8 的真机回归在本机无法完成**：保存路径被既有数据问题阻塞，形成死锁（快照陈旧 → 校验拒绝 → 无法保存 → 快照永修不好）。
- **可视为「并发冲突拒绝」被意外验证**：拒绝逻辑**确实生效**（只是触发源是陈旧快照而非真实并发）。
- **不自行修复**：要让保存能通过，需删除 `disabled.json` 中那条陈旧条目 —— 这**改动用户数据**，且**不在本波授权范围**，**未做**。

### 5.5 顺带发现的**既有测试缺陷**（本波之外，**未修改**）

`core/src/backup.rs:1317` 的 `backup_registry_default_dir_no_panic`：

```rust
fn backup_registry_default_dir_no_panic() {
    // 验证不传参时不会 panic
    let _ = backup_registry(None);
}
```

- **无目录隔离** → 直接写用户真实备份目录（`~/.patheditor/backups/`）。
- **丢弃返回值**（`let _ =`）→ 注册表不可读时也「通过」，**实际验证不了任何东西**。
- **已实测坐实**：设 `PATHEDITOR_BACKUP_DIR` 到临时目录后单跑该测试 → 临时目录**出现 1 个 `path_backup_*.txt`**。
- **这是 HEAD 中既有的测试**（`git log -S` 溯源：`21af268`），**非本波引入**。
- **未修改**：不在本波授权（授权为删 3 模块 + 3 命令 + 3 CLI 死函数）。**建议**：加 `with_temp_backup_dir` 隔离，或改为断言返回值。

> **判据教训（供后续参考）**：起初我用「备份文件计数」判断测试是否写入，得 21→21「无写入」的**错误结论**——因为 `path_backup_*` 有 **20 份轮换上限**，新写入会删旧文件，**净数量恒定**。改用**「隔离目录能否从无到有重建」**才坐实。**计数在饱和系统里不反映写入次数。**

---

## 6. 未覆盖项 / 残余风险（如实列出）

| 项                                         | 说明                                                                                                                                                                                                                |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **真机回归未完成**                         | 被既有 `disabled.json` 陈旧阻塞（§5）。**GUI 侧保存流程由 E2E 30 passed 间接覆盖（mock IPC）**，但真实 Tauri + 真实注册表端到端**未验证**。                                                                         |
| **`isModified` 语义收窄**                  | GUI 不再反映「sidecar 未落盘」（D6 裁定）。若审核窗口认为用户应被告知，需重新裁断。                                                                                                                                 |
| **CLI 新增备份文件**                       | 每次 CLI 写 PATH 产生 `path_backup_*.txt`（D15，行为变化）。**待 Task 8 Step 2 写入 CHANGELOG/README**（见下）。                                                                                                    |
| **文档未同步**                             | `CLAUDE.md`/`AGENTS.md`/`README.md`/`CHANGELOG.md` **未改** —— **本波未获 `CLAUDE.md` 修改授权**（5.1.5 的授权仅限当次 4 项）。现文档已与代码**不符**（命令表仍列已删命令、说明仍称「GUI 暂未接线」）。**待授权**。 |
| **`backup_registry_default_dir_no_panic`** | 既有测试写用户真实目录（§5.5），**未修改**。                                                                                                                                                                        |
| **未提交**                                 | 工作树 14 `M` + 7 `D` + 3 untracked；**未提交、未推送、未升版本号**。                                                                                                                                               |

---

## 7. 交付物位置索引

| 交付物            | 路径                                                                            |
| ----------------- | ------------------------------------------------------------------------------- |
| 本开发回执        | `docs/审核和开发/2026.10.17/PathEditor-F08收口-开发回执.md`                     |
| 第 1 轮核对轮交接 | `docs/审核和开发/2026.10.17/PathEditor-F08收口核对轮交接.md`                    |
| 第 2 轮裁断       | `docs/审核和开发/2026.10.17/PathEditor-F08收口裁断.md`                          |
| spec / plan       | `docs/superpowers/{specs,plans}/2026-10-17-f08-closeout-and-deadcode-removal-*` |

---

## 8. 待裁断 / 待授权

1. **文档同步授权**：`CLAUDE.md`/`AGENTS.md`/`README.md`/`CHANGELOG.md` 需改（现与代码不符）。**是否授权？授权范围？**
2. **既有测试缺陷**（§5.5）：`backup_registry_default_dir_no_panic` 写用户真实目录 —— **是否本波修，还是另开？**
3. **真机回归阻塞**（§5）：`disabled.json` 陈旧条目的处置（需改用户数据）—— **谁来做、如何做？**
4. **`isModified` 语义收窄**（§6）：是否接受，或需 GUI 提示快照失败？

---

**开发窗口签章**：本回执所有结论均来自本次实际执行的命令输出（`cargo test` / `npm run verify:all` / `git stash` A/B 对照 / PowerShell 注册表读取）。**未跑过的未写为通过**；未覆盖项已如实列出。
