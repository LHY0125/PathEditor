# PathEditor v6.2.0 全项目审查报告

- **审查对象**：`main @ a6d347d`（`chore: 版本升至 6.2.0`），工作区仅有 `CLAUDE.md` / `AGENTS.md` 的基线单行改动
- **审查日期**：2026-11-05
- **审查方式**：只读静态审查 + 质量门实测；**未执行任何真实注册表写入**
- **结论**：**Changes Requested** —— 1×P0 / 13×P1 / 32×P2 / 6×P3

---

## 0. 结论摘要

| 维度       | 结论                                                                                                                 |
| ---------- | -------------------------------------------------------------------------------------------------------------------- |
| 分层与契约 | 好。`domain → platform → db → app` 方向由 grep 可验证；`src/gui/web/src/core` 零框架依赖；组件/Store 无直连 `invoke` |
| 安全边界   | 好。SQLCipher 密钥不落日志/IPC；bundle 的 KDF/AEAD 构造正确；`EnvVarMeta` 双端拒绝 `value`；FTS 不索引变量值         |
| 数据一致性 | 有系统性缺口。同步冲突解决不落实体、FTS 不随写更新、配置/恢复失败被当成功                                            |
| CLI 契约   | 有 1 个**破坏性**缺陷（`import --target` 可清空整条 PATH）与多条「帮助/文档 ↔ 实现」不一致                           |
| GUI 正确性 | 有 5 处确定性缺陷（拖放失效、撤销按钮、关窗判定、刷新缺失、草稿残留）                                                |
| 工程门禁   | **CI 明显弱于本地 `verify`**：前端类型检查是空转命令，lint/Prettier/覆盖率门槛在 CI 不执行                           |
| 可维护性   | 良。但文档与代码存在 15 处事实不符（版本号、测试基线、路径、CI 描述）                                                |

**必须先修的 5 项**

1. `patheditor import <file> --target system|user` 在该侧条目为空时把整条 PATH 写成空串（P0）。
2. `apply_profile` / `restore_snapshot` 把「逐 hive 写入失败」当成功，CLI 打印「已写入注册表」退出 0（P1）。
3. 同步冲突选「采纳远端」不写实体，用户的选择永远不生效（P1）。
4. GUI 拖放在真实 Windows/Tauri 下完全失效（P1）。
5. CI 的 `npx tsc --noEmit` 实际检查 0 个文件；lint/Prettier/覆盖率门槛在 CI 缺失（P1）。

---

## 1. 实测基线（本轮亲自执行）

| 检查             | 命令                                                                                        | 结果                                                                                                                                                                                           |
| ---------------- | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust 全量        | `cargo test --workspace`（GNU，按 `CLAUDE.md` 设 `OPENSSL_SRC_PERL` / `PERL5LIB` / `PATH`） | **exit 0**；core 单测 206 passed / 2 ignored，core 集成 53 个单测试文件各 1 passed，CLI 69 passed，GUI lib 1 passed → **327 passed / 0 failed / 2 ignored**，与 `docs/dev/testing.md` 基线一致 |
| 前端单测         | `npx vitest run`                                                                            | **37 files / 413 passed**                                                                                                                                                                      |
| 覆盖率           | `npx vitest run --coverage`                                                                 | **行 82.89%（596/719）**，门槛 80 通过；统计范围仅 `core/store/hooks`                                                                                                                          |
| ESLint           | `npm run lint`                                                                              | exit 0，**3 warnings**（React Compiler 跳过 `useVirtualizer`）                                                                                                                                 |
| 类型检查         | `npx tsc --noEmit`                                                                          | exit 0 但 **`--listFilesOnly` 输出 0 行**（solution 式配置，见 P1-7）                                                                                                                          |
| 类型检查（真实） | `npx tsc -p tsconfig.app.json --noEmit --listFilesOnly`                                     | 178 个文件，无错误                                                                                                                                                                             |
| 依赖审计         | `cargo audit`                                                                               | **quick-xml 0.39.4 × 2 high（RUSTSEC-2026-0194/0195）**、rkyv 0.7.46、proc-macro-error / unic-\* unmaintained                                                                                  |
| npm 审计         | `npm audit`                                                                                 | 南大镜像 `/npm/v1/security/advisories/bulk` 404，无法取得结论（非项目缺陷）                                                                                                                    |
| 版本一致性       | `package.json` / `Cargo.toml` / `tauri.conf.json` / `README` 徽章                           | 前三处 6.2.0 一致；README 徽章 6.2.0 但测试徽章仍 737                                                                                                                                          |

> 本地 GNU 构建需要三条环境变量（`OPENSSL_SRC_PERL` / `PERL5LIB` / `PATH` 追加 Git 的 `usr/bin`），
> 而 `perl5lib` 目录被 `.gitignore` 排除（P2-28）——新克隆按文档设置会失败。

---

## 2. P0（数据丢失 / 破坏性）

### P0-1 `import --target system|user` 在该侧为空时把整条 PATH 写成空串

**位置**：`src/cli/src/commands/import_export.rs:9-16`

```rust
"system" => {
    save_path_write(Some(sys_entries), None);   // 不检查 sys_entries 是否为空
    println!("已导入到系统 PATH");
}
"user" => {
    save_path_write(None, Some(usr_entries));
```

`"both"` 分支刻意写了 `(!sys_entries.is_empty()).then_some(...)`（`:20-21`），单 hive 分支漏掉了同一守卫。

**触发路径（确定性，无需构造）**：

- `.txt` 导入永远返回 `(vec![], entries)`（`src/core/src/platform/fs.rs:241-258`，注释「TXT 格式全部导入为用户路径」）
  → `patheditor import paths.txt --target system` ⇒ `sys_entries == []`
- JSON/CSV 只含一侧时同理（`fs.rs:118-153`、`fs.rs:186-238`）

**后果**：`write_hive_registry` 过滤 `enabled` 后得空列表 → `registry::save_paths` → `join_path([]) == ""`
→ `HKLM\...\Environment\Path` 或 `HKCU\Environment\Path` 被写成空字符串，命令仍打印「已导入」并 exit 0。
写前备份只是让事故可恢复，不改变破坏性；用户 PATH 被清空的后果立竿见影（新开的终端全部找不到命令）。

**修复**：

```rust
"system" if sys_entries.is_empty() => {
    exit_err("源文件的系统 PATH 段为空，已中止：继续会把系统 PATH 清空");
}
```

`user` 分支同样加 `usr_entries.is_empty()` 守卫；并在 core 的 `save_paths` 层面拒绝「显式写入空 PATH」——
这是所有调用方（GUI 清空、配置应用）共用的语义决策，不能只修一个入口。

---

## 3. P1（正确性 / 契约违背 / 安全风险）

### P1-1 `apply_profile` / `restore_snapshot` 把逐 hive 失败包装成成功

**位置**：`src/core/src/app/service/mod.rs:355-360`（`save_path_registry_io` 恒 `Ok`，失败只装进 `HiveOutcome::Failed`）、
`src/core/src/app/service/profile.rs:46-75`、`src/core/src/app/service/restore.rs:37-48`

**消费侧**：

- `src/cli/src/commands/profile.rs:84-87`：`unwrap_or_else` 后**无条件**打印「配置文件 "x" 已写入注册表。」，exit 0
- `src/gui/web/src/components/dialogs/profile/use-profiles.ts:82-83`：`await backend.profileApply(selected); onClose();` —— 失败也关窗、无提示
- `src/gui/web/src/store/history-store.ts:165-176`：`await backend.historyRestoreSnapshot(uid)` 后只清 `detail`

**影响**：非提权时「配置含系统 PATH」或「历史快照的系统侧与当前不同」→ system 写失败 / user 写成功，
用户仍得到「已写入」「已恢复」。同一份 `ApplyOutcome` 在 GUI 保存链路（`path-session.ts:146-170` 逐 hive 检查）
被正确解释，在另外三处被忽略——契约同一、解释两套。

**修复**：在 app 层把「请求过的 hive 有 `Failed`」抬成 `Err`（`HiveOutcome::Failed(e) => return Err(e.clone())`），
或让三个消费点统一检查；并补一条 `apply_profile` 目标 hive 失败的用例（当前 `service/tests.rs` 无任何 `apply_profile` 覆盖）。

---

### P1-2 同步冲突选「采纳远端」不落实体，用户的选择永远不生效

**位置**：`src/core/src/db/sync/import.rs:236-250`（冲突分支不写实体、不写 change）、
`src/core/src/db/sync/conflict.rs:194-240`（`resolve_conflict` 只 `record_change_tx`）、
`src/core/src/db/schema.rs:193-204`（`sync_conflicts` 无 `content` 列）

```rust
MergeDecision::Conflict => {
    record_conflict_tx(tx, ...)?;
    report.conflicts += 1;
    continue;   // 实体内容在此丢弃
}
```

**影响链**：用户对「改名 vs 删除」或「配置内容」冲突选 `remote` 后——(a) 本地实体保持原状；
(b) 下次导出用 `entity_content` 取**本地仍存活的实体**，与刚记的 `new_value`（远端值）自相矛盾；
(c) 第三台设备按 `content` 落地，把「输掉的本地值」当成用户选择传播；
(d) 冲突时没写 remote change，重新导入原包会因本地已有更高 lamport 的解决记录而 `KeepLocal`，远端内容**永久不可恢复**。

**修复**：`sync_conflicts` 增 `remote_content TEXT` 并在导入冲突分支落库；`resolve_conflict` 在同一事务内
按胜方 content 调 `apply_entity_tx` / `apply_tombstone_tx`，并让追加 change 的 `new_value` 与实际写入一致。

---

### P1-3 FTS 只随「current 快照」增量维护：配置与同步导入的数据永远搜不到

**位置**：`src/core/src/db/search.rs:162-205`（`index_snapshot_tx` 只处理 `path_entries` / `env_values`）、
`src/core/src/db/profiles.rs`（全文无 `search` 引用）、`src/core/src/db/sync/import.rs:407-420`（不调索引）、
`src/core/src/app/bootstrap.rs:162-173`（仅 `index_count == 0` 时全量重建）

`rebuild_index` 确实索引 `profiles`（`search.rs:116-139`），但生产写入路径没有任何一处为 profile 写 FTS 行，
且导入不调 `index_snapshot_tx`。

**影响**：首次启动后新建/改名/删除的配置、以及**全部**同步导入的快照与配置永久搜不到；已删除配置的旧索引行永久残留
（返回死引用）。无自愈路径（只有清空 `search_fts` 才会触发重建）。

**修复**：新增 `index_profile_tx` / `remove_profile_index_tx` 并在 `save_profile_tx` / `rename_profile` /
`delete_profile` / 导入 profile 中调用；导入快照后同事务 `index_snapshot_tx`；把「仅空时重建」改为按水位增量比较。

---

### P1-4 CLI `clean --json` 静默不写注册表却 exit 0；`clean` 还会丢弃禁用项

**位置**：`src/cli/src/commands/path.rs:168-196`、`src/cli/src/cli.rs:113-124`

```rust
if json_out { println!(...); }            // 直接结束，不写
else if dry_run { ... }
else { save_path_write(...); }
```

- `cli.rs:122-124` 的 `--json` 只写「以 JSON 输出」，与 `env set/add/remove` 的 `requires = "dry_run"`（`cli.rs:379/410/431`）不一致；
  脚本看到 exit 0 + `removed` 列表，以为已清理。
- 非 dry-run 分支把 `load_*_paths()`（只含**启用**项）的清理结果整体作为该 hive 的 `current` 提交
  （`service/mod.rs:530-545` 是整体替换）。`patheditor disable 3` 之后再 `clean`，该禁用条目从 DB 快照中彻底消失，
  `list` 再也看不到、无法重新启用；而 GUI 的 `clean_path_entries` 接收完整 `Vec<PathEntry>` 并保留 `enabled`
  （`src/gui/src-tauri/src/commands/registry.rs`）——CLI 与 GUI 语义分叉。

**修复**：`Clean.json` 加 `requires = "dry_run"`，JSON 里补 `"dryRun"`；清理改用 `load_path_snapshot_any()`
的完整条目并调用 `clean_path_entries`，保留禁用项。

---

### P1-5 41 个 IPC 命令全部是同步函数，注册表 / SQLCipher / 目录枚举跑在 UI 线程

**位置**：`src/gui/src-tauri/src/commands/*.rs`（41 个 `#[tauri::command]`，`#[tauri::command(async)]` 0 个，
`spawn_blocking` / `async fn` 0 处）、`src/gui/src-tauri/src/lib.rs:88-...`（`generate_handler!`）、
`src/gui/src-tauri/src/lib.rs:63-77`（`setup_database` 在窗口创建后同步执行）

**已从依赖源码核实**：`tauri-macros-2.6.2/src/command/wrapper.rs:249-266` 把非 async 命令归为
`ExecutionContext::Blocking` / kind `"sync"`；`tauri-runtime-wry-2.11.2/src/lib.rs:5383-5406` 的 IPC handler
在 wry 回调里**同步调用**，即 Windows 消息循环线程。

**影响**：以下路径期间窗口不重绘、不响应输入，前端连 loading 都渲染不出来：

- `scan_paths`：对每个 PATH 目录 `read_dir` 全量枚举（`core/src/platform/scanner.rs:40`），System32 一档数千项
- `save_path_with_sidecar`：两次注册表写 + 备份 + 快照 + FTS/引用图增量
- `sync_export` / `sync_import`：Argon2id（m=64 MiB、t=3）+ 整包加解密 + 单事务
- 启动序列（迁移、pending 补写、备份索引、派生表重建、FTS 首次全量）

**修复**：对重型命令加 `#[tauri::command(async)]`（函数体保持同步写法，宏内部走 `async_runtime::spawn`），
至少覆盖 `scan_paths`、`save_path_with_sidecar`、`sync_export/import`、`backup_env_vars`、`profile_apply`、
`history_restore_snapshot`、`history_get_snapshot`、`history_get_current`、`history_list_events`、
`stats_dashboard`、`search`。`validate_path` / `expand_env_vars` 取 `&str` 且返回非 `Result`，保持同步。

---

### P1-6 CI 的「前端类型检查」是空转命令；lint / Prettier / 覆盖率门槛在 CI 缺失

**位置**：`.github/workflows/ci.yml:98-102`、根 `tsconfig.json:1-8`、`vitest.config.ts:17-21`、`package.json` 的 `verify`

```
$ npx tsc --noEmit --listFilesOnly   → 0 行
$ npx tsc -p tsconfig.app.json --noEmit --listFilesOnly → 178 行
```

根 `tsconfig.json` 是 `"files": []` + 三个 `references` 的 solution 配置，只有 `tsc -b` 才会构建被引用工程。
CI 也不跑 `npm run lint`、`npm run format:check`、`npm run test:coverage`、`npm run build`，
因此 `thresholds.lines = 80` **在 CI 永不生效**。

**影响**：TS 类型错误、ESLint 回归、Prettier 漂移、覆盖率跌破 80% 都能绿灯合入，直到打 tag 时的
`npx tauri build`（`beforeBuildCommand: npm run build` → `tsc -b`）才暴露。

**修复**：`npx tsc -b`（或 `npm run build`）；补 `npm run lint`、`npm run format:check`；
`npx vitest run` 改 `npm run test:coverage`。当前三道门实测均绿，补入不会立刻弄红。

---

### P1-7 GUI 拖放添加 PATH 在真实 Windows / Tauri 下完全失效

**位置**：`src/gui/web/src/components/layout/AppShell.tsx:270-281`、`src/gui/src-tauri/tauri.conf.json:13-19`

```tsx
onDrop={(e) => {
  ...
  const entry = e.dataTransfer.items[i].webkitGetAsEntry();
  const file = e.dataTransfer.files[i] as TauriFile;
  if (file.path) addPath(file.path, ...);
}}
```

Tauri 2 的 `dragDropEnabled` 默认 `true`，其配置 schema 明确写着
「Disabling it is required to use HTML5 drag and drop on the frontend on Windows」。
Windows WebView2 下文件拖放被 Tauri 拦截，DOM `drop` 到不了该 handler；`file.path` 也只在 Tauri v1 存在。

**影响**：README 与 `status.dragFolderOnly` 描述的「拖文件夹添加 PATH」在真实应用中不可用；
E2E 用 `page.evaluate` 合成 `DragEvent`（`e2e/tests/env-vars.spec.ts:74-91`），mock 环境无法暴露。
非目录项被静默忽略，对应 i18n 文案从未被使用。

**修复**：改用 `getCurrentWindow().onDragDropEvent`，从 `event.payload.paths` 取原生路径，按当前 tab 与写权限过滤。

---

### P1-8 保存进行中的编辑会被判成「已保存」，关窗时静默丢失

**位置**：`src/gui/web/src/store/app-store.ts:360-385`

```ts
const state = get();                     // await 前固化
const outcome = await savePathSnapshot({ sysPaths: state.sysPaths, ... });
set({ isModified: !(arraysEqual(state.sysPaths, outcome.savedSys) && ...) });  // 旧快照
```

`cleanPaths`（`app-store.ts:204-224`）同型：`await backend.cleanPathEntries(list)` 后直接写回 `kept`，
await 期间的新编辑被整体覆盖。

**影响**：保存链路含备份 + 两次注册表写 + DB 提交，耗时足以让用户继续编辑/勾选/撤销；返回后 `isModified` 被强制改回
`false`，而 store 里已有新改动。`AppShell.tsx:96-98` 的关窗确认读的正是 `isModified` → X / Alt+F4 不再提示，
改动静默丢失。

**修复**：`await` 后重新 `const current = get()` 并用 `current.sysPaths` 计算 `isModified`；`isSaving` 期间禁用编辑
（或至少在 `cleanPaths` 返回后检测列表是否被并发修改）。

---

### P1-9 撤销 / 重做按钮在等长操作后不刷新可用状态

**位置**：`src/gui/web/src/components/toolbar/UndoRedoButtons.tsx:13-15`

```tsx
useAppStore((s) => s.sysPaths.length); // 只订阅长度
useAppStore((s) => s.userPaths.length);
const undoRedo = useAppStore((s) => s.undoRedo); // 可变对象，引用不变
```

编辑已有条目、切换 `enabled`（长度不变）后 `push` 发生但三个订阅值都没变 → 组件不重渲染：
首次编辑后「撤销」保持禁用；撤销一次编辑后「重做」仍禁用。Ctrl+Z/Ctrl+Y 可用，工具栏按钮与实际历史脱节。

**修复**：改成原始值选择器 `useAppStore((s) => s.undoRedo.canUndo())` / `canRedo()`，或给 store 加 `historyVersion` 计数。

---

### P1-10 编辑弹窗经 Escape / 遮罩关闭时不清草稿：明文滞留 + 永久「未保存」提示

**位置**：`src/gui/web/src/components/dialogs/EditEnvVarDialog.tsx:33`（注释承诺「成功或取消时清除（c2）」）、
`EditEnvVarDialog.tsx:108`（`<Modal onClose={onCancel}>`）、`EditEnvVarDialog.tsx:150-156`（只有按钮取消会清）、
`src/gui/web/src/components/layout/AppShell.tsx:368`（`onCancel={() => setEditVarKey(null)}`）

**影响**：Escape / 点遮罩这条取消路径不清 `draft`：敏感变量输入的明文继续留在 zustand store
（与「明文不进入持久状态」的红线相悖），且 `hasDrafts()` 永为 `true` —— 之后每次关窗都弹「有未保存修改」，
即使什么都没改。测试只覆盖按钮取消。

**修复**：抽 `closeEditVar(key)`：先 `clearDraftByKey(key)` 再 `setEditVarKey(null)`，两处共用。
（注意区分「提交失败」——那条路径应保留草稿。）

---

### P1-11 应用配置 / 恢复历史快照后主界面 PATH 不刷新，后续保存必冲突

**位置**：`use-profiles.ts:82-83`、`history-store.ts:165-176`；`loadPaths()` 仅两个调用点
（`app-store.ts:415` 初始化、`StatusBar.tsx:36` 重试）

**影响**：两者都会写注册表，但界面仍显示旧 PATH 与旧 `_savedSys`。用户点「应用」看不到变化；
此后任何保存都携带过期 `expected` 被 core 以 conflict 拒绝，或用户误以为应用失败。
历史恢复还会恢复 env 值，`env-store` 同样不刷新；工具栏没有「重新加载 PATH」入口，只能重启。

**修复**：成功后调 `useAppStore.getState().loadPaths()`；历史恢复再调 `useEnvStore.getState().load()`。

---

### P1-12 GUI 明文备份回落无任何用户可见提示

**位置**：`src/core/src/platform/backup/env.rs:161-174`（仅 `log::warn!`）、`env.rs:498-505`
（`BackupOutcome::Created(PathBuf)` 不携带格式）、`src/gui/web/src/store/env-store.ts:46-50`
（`savedStatusMessage` 只看「备份失败」，不看是否明文）

**影响**：DB 降级（密钥被换 / DPAPI 失效 / 换机器）是**受支持的正常状态**。此时 GUI 里任何一次环境变量编辑
都会把两个 hive 的全部变量明文（含 `*_TOKEN` / `*_KEY` / `*_SECRET`）写成 `env_backup_*.json`，
界面只显示「已保存」。CLI 有明确 stderr 警告（`src/cli/src/commands/env/cmd.rs:209-219`），GUI 没有——
与 `docs/dev/data-and-security.md`「回落明文并打印警告」的承诺不一致。

**修复**：`BackupOutcome::Created` 增加 `encrypted: bool`，GUI 在 `false` 时显示常驻警告并提供「重新加密」入口。

---

### P1-13 `env restore` 逐条失败仍 exit 0

**位置**：`docs/dev/interfaces.md:82` 已注明该语义；实现见 `src/core/src/platform/backup/restore.rs`（best-effort
逐条执行，失败进 `failures`）

**影响**：脚本无法从退出码检出部分失败。文档诚实，但契约本身对自动化不友好：恢复是**写注册表**的高危操作，
「部分成功」应以非零退出码表达（例如 6 或 1 + `--strict`）。

**建议**：至少新增 `--strict`（任一变量失败即非零），默认保持现状并保留现有文档措辞。

---

## 4. P2（正确性风险 / 安全加固 / 可维护性）

### 4.1 CLI

| #     | 位置                                                        | 问题                                                                                                                                                                             | 建议                                                           |
| ----- | ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| P2-1  | `src/cli/src/commands/path.rs:83-95`                        | `add ""` / `add "   "` 静默无效（`join_path` 过滤空串）却打印「已添加」，并在 DB 快照里留一条幽灵空条目                                                                          | 写前拒绝 `path.trim().is_empty()`                              |
| P2-2  | `src/cli/src/commands/path.rs:122-127`                      | `--steps` 无范围限制，`index + steps` 溢出：debug 构建 panic（exit 101 绕过 0–5 契约），release 回绕成顶部                                                                       | `index.saturating_add(steps).min(max)`                         |
| P2-3  | `src/cli/src/cli.rs:113-118` vs `commands/path.rs:139-141`  | 帮助说「`--system` 与 `--user` 同给则两者都清」，代码直接 `exit_err`；`clean_sys/clean_usr` 的「两者都清」成了死代码                                                             | 删守卫让行为与 `list` 对齐，或改文案                           |
| P2-4  | `src/cli/src/cli.rs:339-343`                                | `env list` 帮助「与 `--user` 同给则两者都看」，但 `conflicts_with = "system"` 会 exit 2                                                                                          | 改文案或去掉 `conflicts_with`                                  |
| P2-5  | `src/cli/src/cli.rs:152-154`                                | `import --target` 无 `value_parser`；`--target sys` 落到 `_` 分支**同时写两个 hive**                                                                                             | `value_parser = ["system","user","both"]`                      |
| P2-6  | `src/cli/src/commands/maintenance.rs:45-47`                 | `cleanup-legacy-backups` 缺 `--yes` 时不打印计划，只报错——与 `--help` / `interfaces.md:76` 承诺不符                                                                              | 打印 `scan_legacy_env_backups` 结果后 exit 1                   |
| P2-7  | `src/cli/src/commands/maintenance.rs:24`                    | `prune` 输出「（dry-run）将删除 changes 3 条」夹一个多余英文词                                                                                                                   | 删除 `changes` 字样                                            |
| P2-8  | `src/cli/src/commands/scan.rs:7-12`、`:40-45`               | `conflicts` / `scan` 吞掉注册表读取错误（`if let Ok`），读失败伪装成「无冲突」并 exit 0                                                                                          | 全部失败时 `exit_core_error`，或输出 `partial: true`           |
| P2-9  | `src/cli/src/runtime.rs:115-122` + `service/mod.rs:250-257` | DB 不可用的**受支持**降级模式下，每次成功的 PATH 编辑都以 `SidecarOutcome::Failed` 收尾：exit 1 + 文案「待补写状态记录失败，请手工核对」（实际根本没尝试记录 pending）           | 增加 `SidecarOutcome::Skipped` 变体，CLI 对该变体不退出非零    |
| P2-10 | `src/cli/src/commands/env/render.rs:229-232`                | 跨 hive 提示恒为「请加 `--system`」，当变量其实在 HKCU 时该提示必然再失败一次                                                                                                    | 按 `other` 分支给指令                                          |
| P2-11 | `src/core/src/db/sync/export.rs:143-161`                    | 退出码 4 实际由「待导出变更过多 / payload 序列化失败」（`ErrorCode::Sync`）产生，而 `sync import` 的合并冲突返回 exit 0；`interfaces.md:76` 又写「口令/篡改 → 1」（实际 exit 5） | 把 export 侧改成 `SyncFormat`/`Internal`；冲突占用 4；同步文档 |
| P2-12 | `src/cli/src/cli.rs:438-456`、`:284-294`                    | `env backup/backups/restore --json`、`profile list --json`、`profile save/load/apply/delete/rename` 的参数无 `///`，`--help` 里是空说明                                          | 补中文文档注释（可加 help 回归断言）                           |

### 4.2 core / platform

| #     | 位置                                                                                                                                 | 问题                                                                                                                                                                              | 建议                                                                      |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| P2-13 | `src/core/src/platform/registry/path.rs:41-49`、`:52-57`                                                                             | PATH 用 `get_value::<String>`（接受 `REG_SZ/REG_EXPAND_SZ/REG_MULTI_SZ`，`from_utf16_lossy` 解码），写回时 `REG_MULTI_SZ` 被改成 `REG_EXPAND_SZ`；未配对代理项被静默替换为 U+FFFD | 改 `get_raw_value` + 类型白名单，非 `SZ/EXPAND_SZ` 返回 `UnsupportedType` |
| P2-14 | `src/core/src/platform/registry/path.rs:162-184` vs `platform/fs.rs:14`                                                              | 保存通路不拒绝 `;`，`path add "C:\a;C:\b"` 写进去后变成两条；导入通路却过滤 `;`——同一非法输入两条通路语义相反                                                                     | `validate_and_join_paths` 补 `;` 校验                                     |
| P2-15 | `src/core/src/platform/fs.rs:353-355`                                                                                                | `ensure_not_system_dir` 黑名单漏 `Program Files (x86)`、无尾 `\` 的 `C:\Windows`、非 C 盘、`\\?\` / 8.3 短名                                                                      | 用 `canonicalize` + 组件比较，盘符取 `%SystemDrive%`                      |
| P2-16 | `src/core/src/platform/fs.rs:19-24`                                                                                                  | `atomic_write` 用可预测的 `path.with_extension("tmp")`，不带 `create_new`、跟随符号链接；`read_text_file_scoped` canonicalize 后又按原路径二次打开（TOCTOU）                      | 随机临时名 + `create_new`；读取复用同一 handle                            |
| P2-17 | `src/core/src/platform/scanner.rs:98-103`、`:81-85`                                                                                  | 同一目录在 PATH 里出现两次时，每个 exe 都被报成「两处冲突」且 `dir` 相同（纯误报）；扫描线程 panic 被 `let Ok(..) = join()` 静默吞掉，仍返回 `Ok`                                 | 按 `(name, dir)` 去重；线程结果改为 `Result` 汇总                         |
| P2-18 | `src/core/src/platform/capabilities.rs:30-36`、`registry/store.rs:80-85`、`registry/env_edit.rs:429-434`、`platform/system.rs:16-18` | 写权限判定四处独立实现；`check_admin` 硬编码 HKLM 键路径而不是 `SYS_REG_PATH`                                                                                                     | 收敛到 `store::hive_writable`（`pub(crate)`）；`check_admin` 用常量       |
| P2-19 | `src/core/src/app/disabled.rs:173-179`                                                                                               | 非法 UTF-8 的 pending 文件返回 `Io` 而不隔离 → 每次启动都告警、补写永久失效（与 `persist::read_versioned_file` 同病）                                                             | `InvalidData` 走 `parse_error_quarantined`                                |
| P2-20 | `src/core/src/app/service/disabled.rs` / `service/mod.rs:323-327`                                                                    | PATH 写前备份失败只 `log::warn!`（CLI 无 logger），`ApplyOutcome` 无 `backup` 字段；env 通路有 `WriteOutcome.backup`，PATH 通路没有                                               | 给 `ApplyOutcome` 加 `backup: Option<String>`                             |
| P2-21 | `src/core/src/platform/persist.rs`（`Versioned` 信封）                                                                               | 版本号/迁移只对 JSON 解析失败隔离，非 UTF-8 同上                                                                                                                                  | 同 P2-19                                                                  |

### 4.3 core / db

| #     | 位置                                                               | 问题                                                                                                                                    | 建议                                                                                   |
| ----- | ------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| P2-22 | `src/core/src/db/mod.rs:93-108`                                    | `schema_version()` 在事务**外**读取；GUI 与 CLI 并发首次迁移时后到者按过期版本重跑 DDL，报 `table already exists`                       | 进入 `BEGIN IMMEDIATE` 后重读 `PRAGMA user_version`                                    |
| P2-23 | `src/core/src/db/history.rs:492-505`、`db/scan_history.rs:236-250` | 任何 SQL 错误都降级为 `NotFound`；导出侧把 `NotFound` 当「实体已删除」，一次 `SQLITE_BUSY` 就会产出 `content=null` 并 `mark_exported`   | 只有 `QueryReturnedNoRows` 才映射 `NotFound`                                           |
| P2-24 | `src/core/src/db/sync/import.rs:189-196`、`history.rs:264-275`     | 只按 `changes.uid` 判重，`snapshots.uid` UNIQUE 冲突时整包以 `Database` 失败（保留策略清掉 changes 后重新导入同一包即可触发）           | 实体表先查 uid，或 `ON CONFLICT DO NOTHING`                                            |
| P2-25 | `src/core/src/db/maintenance.rs:37`、`db/sync/import.rs:209-232`   | 90 天清理删掉 `changes` 后 LWW 失去本地基线：旧包可**复活已删除配置**；墓碑也一并消失                                                   | 每组 `(entity_type, entity_uid)` 保留最新一条与全部墓碑                                |
| P2-26 | `src/core/src/db/derived.rs:20-31`                                 | 水位不含 `scan_runs` 与 `app_state.current_snapshot_id`，`record_run` 注释承诺的「下次启动重算」不成立                                  | 水位纳入两者                                                                           |
| P2-27 | `src/core/src/db/crypto.rs:59-63`                                  | 首次 `read_key` 在互斥量**外**：另一进程可能读到 `create_new` 后尚未写完的文件 → `Crypto` 错误并 fail-closed                            | 读也进临界区，或写「临时文件 + rename」                                                |
| P2-28 | `src/core/src/db/dacl.rs:107`、`:224`                              | SDDL 用 `D:` 而非 `D:P`，「仅当前用户 + SYSTEM」不是结构性保证；校验是黑名单（`BA` / 具名 SID 不报）；`set_key_dacl` 只在首次创建时执行 | `D:P(...)` + `keys` 目录加固；校验改白名单                                             |
| P2-29 | `src/core/src/db/sync/import.rs:138`、`platform/fs.rs:33-50`       | 同步包与导入文件读取无大小上限（`.pebak` 有 1 MiB 上限，说明项目已有纪律）                                                              | `metadata().len()` 上限（bundle 64 MiB、文本 8 MiB），`split()` 对 `header_len` 加上限 |
| P2-30 | `src/core/src/db/sync/import.rs:296-299`                           | `base_lamport` 来自明文 header，可设到 `i64::MAX`：SQLite 溢出转 REAL 后 `r.get::<i64>` 抛错，此后本地记账永久失败                      | 导入时钳制；`+1` 改 `saturating_add`                                                   |
| P2-31 | `src/core/src/db/sync/import.rs:476-507`                           | 导入快照丢弃 `content.createdAt`，历史时间与统计日被改写成导入时刻（profile 侧却保留远端时间戳）                                        | `NewSnapshot` 增 `created_at: Option<i64>`                                             |
| P2-32 | `src/core/src/db/schema.rs`（v2 DDL）                              | `backups.file_path`、`var_references.from_name/to_name` 无索引，全表扫描                                                                | v4 迁移补三个索引                                                                      |

### 4.4 GUI（Rust / Tauri）

| #     | 位置                                                                                                  | 问题                                                                                                                                                                                            | 建议                                                                                                            |
| ----- | ----------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| P2-33 | `src/gui/src-tauri/capabilities/default.json:6-21`                                                    | `core:default` 展开含 image（`from_path` 可读任意绝对路径图片）、menu/tray、webview devtools；`core:event:default`、`dialog:allow-open/save/confirm`、`log:default` 与 default 重复或前端零调用 | 收敛到实际用到的集合（`core:event:default`、`core:window:allow-destroy`、`core:app:default`、`dialog:default`） |
| P2-34 | `src/gui/src-tauri/src/commands/system.rs:15-17`                                                      | `expand_env_vars` 可展开**任意**变量明文（`%*_TOKEN%`），绕过 `reveal_env_var` 的敏感掩码                                                                                                       | 只允许 PATH 语境白名单；敏感名保留原文                                                                          |
| P2-35 | `src/gui/src-tauri/src/commands/sync.rs:28-49`                                                        | `sync_export` / `sync_import` 的 `path` 无来源校验（同层 `import_file` / `preview_env_backup` 都有）                                                                                            | 复用 core 的路径校验，导入限 `.pesync`                                                                          |
| P2-36 | `src/gui/src-tauri/src/lib.rs:45-50`、`Cargo.toml:24-28`                                              | 无单实例保护：双开会重复跑启动序列、并发写同一 DB 与注册表                                                                                                                                      | 接入 `tauri-plugin-single-instance`                                                                             |
| P2-37 | `src/gui/src-tauri/tauri.conf.json:31`、被跟踪的 `WebView2Loader.dll`、`src/gui/src-tauri/Cargo.lock` | MSVC 构建链接 `WebView2LoaderStatic`，打包的 DLL 是死文件（且是经典 DLL 劫持目标名）；子 crate 的 `Cargo.lock` 停在 5.1.2，cargo 忽略它                                                         | 删除三者，GNU 本地构建从 `OUT_DIR` 取                                                                           |

### 4.5 GUI（Web）

| #     | 位置                                                                                                                                                             | 问题                                                                                                                                            | 建议                                                             |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| P2-38 | `src/gui/web/src/store/search-store.ts:68-78`                                                                                                                    | 过期响应 `return` 未清 `loading`：请求期间改输入框 → 按钮永久「搜索中」                                                                         | seq 守卫 + 当前请求必清 `loading`                                |
| P2-39 | `src/gui/web/src/components/history/HistoryPanel.tsx:134,143,285,311`                                                                                            | `className="toolbar-btn"` 在 `globals.css` 中不存在（只有 `.toolbar` / `.toolbar-sep` / `.toolbar-search`），四个按钮无样式且绕过 `Button` 原语 | 改用 `Button` 或补 CSS；设计系统测试加「引用的类必须已定义」断言 |
| P2-40 | `src/gui/web/src/components/dialogs/analyze/use-analyze-data.ts:36`                                                                                              | `String(scanError)` → 界面显示「分析失败: [object Object]」                                                                                     | 用 `core/error.ts` 的 `errorMessage`                             |
| P2-41 | `src/gui/web/src/components/dialogs/profile/use-profiles.ts:20-52`                                                                                               | 全部 IPC 无 `catch`；`handleLoad` 无代次守卫（快速点两个配置时右侧显示旧内容）                                                                  | 每个 handler 落错误条 + `selectedSeq`                            |
| P2-42 | `src/gui/web/src/store/history-store.ts:125-131`                                                                                                                 | `selectEvent` 无代次守卫且静默吞错（DB 降级时只显示空态）                                                                                       | `detailSeq` + 写入 `error`                                       |
| P2-43 | `src/gui/web/src/components/history/HistoryPanel.tsx:62-66`                                                                                                      | 撤销失败把真实错误（`permissionDenied` / `database`）覆盖成「没有可撤销的记录」                                                                 | 只在 `undoLast` 正常返回 `null` 时写该文案                       |
| P2-44 | `src/gui/web/src/services/backend/index.ts:100-104`                                                                                                              | `import_file` / `clean_path_entries` 直接 `invoke`，未走 `parsePathEntries`（违反 `data-and-security.md` 的「PathEntry 运行时形状校验」）       | 复用 `parsePathEntries`                                          |
| P2-45 | `src/gui/web/src/components/path-list/MergePreview.tsx:18-30`                                                                                                    | 去重时丢弃 `enabled` 语义：system 禁用 + user 启用会显示为「禁用」，与实际生效相反                                                              | 合并时 `enabled = existing.enabled \|\| entry.enabled`           |
| P2-46 | `src/gui/web/src/components/env-list/EnvVarTable.tsx:61-66`、`path-list/PathTable.tsx:101-105`                                                                   | 定位请求在目标尚未出现（`index === -1`）时就清空，快照加载后不再滚动                                                                            | 仅 `index >= 0` 时消费                                           |
| P2-47 | `src/gui/web/src/components/env/RefGraphDialog.tsx`                                                                                                              | 181 行组件 + 3 个后端命令 + 测试全部存在，但**无任何生产引用**：「删除前看影响范围」的产品语义缺失                                              | 接入删除确认，或删除组件与命令                                   |
| P2-48 | 7 处（`env-store.ts:28`、`history-store.ts:14`、`search-store.ts:17`、`sync-store.ts:29`、`RefGraphDialog.tsx:24`、`CompareDialog.tsx:22`、`StatsPanel.tsx:17`） | 各自维护一份 `isCoreError` / `errorMessage`，对 `code` 的特判各不相同                                                                           | 收敛到 `core/error.ts`                                           |
| P2-49 | `src/gui/web/src/components/path-list/PathTable.tsx:142-166`、`env-list/EnvVarTable.tsx:116-118`                                                                 | 行不可键盘选中（无 `role` / `tabIndex` / `onKeyDown`），勾选框无 `aria-label`                                                                   | 行级 roving tabindex + `aria-selected`；勾选框补名称             |
| P2-50 | `src/gui/web/src/components/toolbar/SearchInput.tsx:13-20`                                                                                                       | 模态框打开时 Ctrl+F 仍把焦点移到遮罩后的搜索框（绕过 Modal 焦点陷阱）                                                                           | handler 开头 `if (isAnyModalOpen()) return;`                     |

### 4.6 工程 / 测试 / 文档

| #     | 位置                                                                      | 问题                                                                                                                                                                              | 建议                                                                                           |
| ----- | ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| P2-51 | `src/gui/web/tests/setup.ts:5`                                            | 未注册 RTL 自动 cleanup（依赖全局 `afterEach`）；5 个测试文件 render 后不清理，`use-path-validation` 的「期望 3 实得 11」抖动由此而来                                             | setup.ts 加 `afterEach(() => cleanup())`，删掉 `testing.md:92-93` 的「随机抖动」免责说明       |
| P2-52 | `src/cli/src/commands/env/tests.rs:793-798`、`:537-547`                   | CLI 测试真实读写 `~/.patheditor` 的 DPAPI 密钥（`env backup` 走 `config_dir()`），并要求宿主 HKCU 至少 4 个用户变量                                                               | 改用 `write_env_backup_to_with_key(tempdir, ...)` 与显式注入的变量列表；删掉 CI 的预置变量步骤 |
| P2-53 | `src/gui/src-tauri/Cargo.toml:9`                                          | `rust-version = "1.77.2"`，但 `src/core/src/db/refs.rs:247` 用了 `Option::is_none_or`（Rust 1.82 稳定），README 又要求 1.95+                                                      | 统一真实下限（≥1.82），core/cli 用 `rust-version.workspace`                                    |
| P2-54 | `src/core/src/platform/registry/env_edit.rs:1355`、`registry/path.rs:368` | 两个 `#[ignore]` 均有理由（一为不可达分支，一需真实写权限），无隐形缺陷                                                                                                           | 维持；在 `testing.md` 列明                                                                     |
| P2-55 | `.github/workflows/release.yml:79-97`、`:209`                             | 版本校验只覆盖 3 处（README / CHANGELOG 不校验，找不到 CHANGELOG 段落时静默回退 `git log`）；预发布 tag 也 `--latest`                                                             | 补 README 徽章与 CHANGELOG 段落校验；`-` 后缀走 `--prerelease`                                 |
| P2-56 | `README.md:282`、`README.md:533`                                          | `cargo install --path cli` —— 仓库根无 `cli/` 目录                                                                                                                                | 改为 `--path src/cli`                                                                          |
| P2-57 | `vitest.config.ts:17-21`、`docs/dev/testing.md:32`                        | 「覆盖率门槛 80% 行」只统计 `core/store/hooks`；`services/`（含安全关键的 `backend/`）与 `components/` 完全不统计；`core/path-entry.ts` 0%、`store/sync-store.ts` 7.5% 被聚合掩盖 | 文档写明统计范围，或把 `services/**` 纳入并开 `perFile`                                        |
| P2-58 | `src/gui/web/src/store/sync-store.ts`（157 行）                           | 同步中心测试（`sync-center.test.tsx:38`）整体 mock 掉 store → 覆盖率 7.5%，冲突解决 / 口令流程零单测                                                                              | 为 store 补直接测试（尤其 `resolveConflict` 与错误映射）                                       |
| P2-59 | `.github/workflows/v6-sqlcipher-probe.yml:48-56`                          | 测试文件列表硬编码 8 个名字；无 `permissions:`                                                                                                                                    | 枚举 `tests/*.rs`；补权限；若已归档改 `workflow_dispatch`                                      |
| P2-60 | `.gitignore:59`、`docs/dev/testing.md:83`、`README.md:652`                | 文档说 `probes/v6-sqlcipher/perl5lib/` 已在仓库里，实际被 gitignore                                                                                                               | 提交最小模块树，或改文档为生成命令                                                             |
| P2-61 | `src/core/src/platform/backup/env.rs:275-284`                             | `rotate_env_backups` 只匹配 `env_backup_*` + `.json/.pebak`，会删掉用户自放文件（如 `env_backup_notes.json`）；字典序排序让 `env_backup_zzz.json` 挤掉合法备份                    | 与 `is_legacy_env_backup` 共用时间戳匹配                                                       |
| P2-62 | `src/core/src/platform/backup/env.rs:458-468`                             | 恢复校验「文件名以 `env_backup_` 开头即放行任意目录」；`RestoreChange` 不含值 → 预览无法核对内容                                                                                  | 限定必须在备份目录内；预览增加值摘要（敏感掩码）                                               |

---

## 5. P3（信息 / 卫生）

| #    | 位置                                     | 问题                                                                                      | 建议                                                          |
| ---- | ---------------------------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| P3-1 | `src/gui/web/index.html:2`               | `<html lang>` 固定 `zh-CN`，切英文后读屏仍按中文朗读                                      | 语言切换时同步 `document.documentElement.lang`                |
| P3-2 | `src/gui/web/src/styles/globals.css:139` | 深色背景 `#15151d/#0d0d12` 硬编码，未走 `--app-*` token                                   | 换成语义 token                                                |
| P3-3 | `src/gui/web/src/i18n/locales/*.json`    | 22 个全仓无引用的键（`dialog.exportTitle`、`status.dragFolderOnly` 等）                   | 回查是否漏接功能（尤其 `dragFolderOnly` 对应 P1-7），其余删除 |
| P3-4 | `src/gui/web/e2e/mocks/ipc.ts:144-247`   | 保留 `load_system_paths`、`backup_registry`、`stats_rebuild` 等**未注册**命令的 mock 分支 | 删除死分支                                                    |
| P3-5 | `.gitattributes:1-2`                     | 声明「统一 CRLF」，但 `Cargo.lock` 入库为纯 LF 且未匹配任何规则                           | 加 `Cargo.lock text eol=lf` 并改注释                          |
| P3-6 | `docs/screenshots/demo.gif`              | 5.28 MB 二进制直接入库，每次重录只增不减                                                  | 移到 Release 附件或压缩到 <2 MB                               |

---

## 6. 文档 ↔ 代码事实不符清单

| #   | 文档位置                                                                                                                                                                                                            | 文档所述                                                      | 实际                                                                             |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| 1   | `CLAUDE.md:13`、`AGENTS.md:13`                                                                                                                                                                                      | 当前版本 **6.1.1**                                            | `package.json:4` / `Cargo.toml:13` / `tauri.conf.json:4` 均为 **6.2.0**          |
| 2   | `CLAUDE.md:57`、`AGENTS.md:57`                                                                                                                                                                                      | `cargo test --workspace` → **324 passed**                     | 实测 327 passed / 2 ignored（同文件第 155 行已改对，第 57 行没改）               |
| 3   | `README.md:574`                                                                                                                                                                                                     | `cargo test (324 passed / 2 ignored)`                         | 327                                                                              |
| 4   | `README.md:13`                                                                                                                                                                                                      | 徽章 `tests-737 passed`                                       | 按同页口径应为 413+327=**740**                                                   |
| 5   | `README.md:114`                                                                                                                                                                                                     | 「**v6.1.1** 使用 Tauri 2.x …」                               | 6.2.0                                                                            |
| 6   | `ROADMAP.md:10-12`                                                                                                                                                                                                  | 「v6.1（当前开发线）」，最高 6.1.1                            | 6.2.0 已发布，ROADMAP 无该条目                                                   |
| 7   | `README.md:282`、`:533`                                                                                                                                                                                             | `cargo install --path cli`                                    | 实际 `src/cli`                                                                   |
| 8   | `CLAUDE.md:100/138/151`、`AGENTS.md` 同、`README.md:135`、`docs/dev/architecture.md:74`、`docs/dev/data-and-security.md:23/31`、`docs/dev/interfaces.md:36`、`.claude/skills/patheditor-review-window/SKILL.md:125` | IPC 入口 `src/gui/web/src/services/backend.ts`                | 实际是目录 `services/backend/`（`index.ts` + `parse-core.ts` + `parse-data.ts`） |
| 9   | `docs/dev/testing.md:83`、`README.md:652`                                                                                                                                                                           | 「仓库已把需要的模块树放在 `probes/v6-sqlcipher/perl5lib/`」  | `.gitignore:59` 忽略该目录，`git ls-files` 命中 0                                |
| 10  | `docs/dev/testing.md:32`、`CLAUDE.md:156`                                                                                                                                                                           | 「覆盖率门槛 80% 行」                                         | 仅统计 `core/store/hooks`；`services/`、`components/` 不统计                     |
| 11  | `docs/dev/release.md:17,30`                                                                                                                                                                                         | 发布前版本「四处一致（含 README）」+ CHANGELOG 必须有当前段落 | `release.yml:79-97` 只校验 3 处；CHANGELOG 缺失时静默回退 `git log`              |
| 12  | `docs/dev/testing.md:92-93`                                                                                                                                                                                         | `use-path-validation` 抖动「与本轮改动无关，重跑即可判定」    | 根因是 5 个测试文件缺 RTL cleanup（系统性配置缺陷）                              |
| 13  | `src/gui/src-tauri/Cargo.toml:9`                                                                                                                                                                                    | `rust-version = "1.77.2"`                                     | 代码用到 1.82 的 `Option::is_none_or`；README 要求 1.95+                         |
| 14  | `.gitattributes:1-2`                                                                                                                                                                                                | 「本仓库统一 CRLF」                                           | `Cargo.lock` 入库为 LF                                                           |
| 15  | `docs/dev/testing.md:50`                                                                                                                                                                                            | CI 第 5 步是「前端类型检查」                                  | `npx tsc --noEmit` 不检查任何文件（`--listFilesOnly` 0 行）                      |

---

## 7. 已验证无问题（避免重复审查）

1. **unsafe 使用**：`platform/system.rs` 3 处、`db/{dpapi,dacl,crypto}.rs` 25 处，全部有 `// SAFETY:` 且前提成立
   （`ExpandEnvironmentStringsW` 先查长度再分配；`CryptProtectData` 缓冲区 `LocalFree` 释放；token 句柄成对关闭）。
2. **保留名 / 保护名单点判定**：`is_reserved`（`eq_ignore_ascii_case`）覆盖 6 个闸门（列表过滤、reveal 拒绝、
   update/create/delete 拒绝、恢复通路跳过）；`Path` 无法经通用 env 通路改写。
3. **CAS 覆盖面**：所有非 force 的 env 写入口强制 `expected_revision`；`*_force_*` 只跳过 revision，
   保护名单/类型/名值校验仍走同一函数；PATH 的 CAS 唯一实现是 `service::check_expected`（多重集比较）。
4. **SQLCipher / DPAPI 边界**：`PRAGMA key` 前强制 64 位 hex；密钥只在进程内存；全仓 `log::*` 不含值/口令/密钥；
   `cipher_memory_security` 的关闭有显式文档取舍。
5. **同步包密码学构造**：AAD 覆盖 `magic..nonce`（含 header/salt/nonce）；每次 `seal` 新 salt+nonce；
   Argon2id 参数写死；错误口令与篡改统一返回 `Crypto` 且文案固定。
6. **FTS 不索引明文值**：`search.rs:87-101` 只取 `env_values.name`，`SearchHit.content` 明确不含值。
7. **前端安全面**：全仓无 `dangerouslySetInnerHTML` / `innerHTML` / `eval` / `new Function`；
   `localStorage` 仅存 `darkMode` 与语言；CSP 非空且 `connect-src` 仅 `ipc:`；无 `fs`/`shell`/`http` 插件权限；
   `parseEnvVarMeta` 显式拒绝 `value` 字段并有回归用例。
8. **错误序列化**：`CoreError` / `ApplyOutcome` / `HiveOutcome` / `SidecarOutcome` 与全部 DB 视图均为 camelCase，
   无 snake_case 混用；`message` 不泄漏路径全量或变量值。
9. **分层方向**：`domain` 不引用 `platform/app/db`；`platform → db` 只有 `.pebak` 密钥白名单；
   `src/gui/web/src/core` 零 React/Tauri 依赖；组件/Store 无直连 `invoke`。
10. **测试纪律**：`core/tests/*.rs` 全部单 `#[test]`（SQLCipher 一文件一测试）；CI 的 GUI lib 清单开关与
    `build.rs` 分支一致；`ipc-contract.test.ts` 双向差集校验 IPC 名；i18n 中英键集合 353/353 一致。
11. **密钥文件当前实际 ACL**：`icacls` 实测 `db.key.dpapi` 只有 `NT AUTHORITY\SYSTEM:(F)` 与 `LHY\LHY:(F)`，
    无 `(I)` 继承条目——P2-28 是「不具结构性保证」，不是当前已暴露。

---

## 8. 建议整改顺序

**Wave 1（发布阻塞，1–2 天）**

1. P0-1 `import --target` 空侧守卫（core 层拒绝空 PATH 写入）
2. P1-1 配置应用 / 历史恢复的失败传播 + 三处消费点
3. P1-4 `clean`（`--json` 语义 + 保留禁用项 + 与 GUI 统一）
4. P1-6 CI 门禁（`tsc -b`、lint、format、coverage）

**Wave 2（同步与检索的正确性）**

5. P1-2 冲突解决落实体（含 schema v4 的 `remote_content`）
6. P1-3 FTS 增量维护 profile 与导入
7. P2-22~P2-26（迁移并发、NotFound 降级、uid 判重、LWW 基线、水位）

**Wave 3（GUI 正确性）**

8. P1-7 拖放改 `onDragDropEvent`
9. P1-8 / P1-9 / P1-10 / P1-11（保存竞态、撤销按钮、草稿、刷新）
10. P1-5 IPC 异步化（先 `scan_paths` / 保存 / 同步四个）

**Wave 4（安全加固与工程卫生）**

11. P2-28 DACL `D:P` + 白名单校验；P2-34 `expand_env_vars` 收敛；P2-35 同步路径校验
12. P1-12 明文备份回落提示
13. P2-51 测试 cleanup；P2-52 CLI 测试隔离；P2-53 MSRV
14. 第 6 节文档同步（一次改完，避免第二套数字）

---

## 9. 未覆盖 / 需实测

- 真实注册表写入闭环（HKLM/HKCU 双向、并发冲突、UAC 拒绝）、真实 DPAPI 换用户/换机器行为——本轮只读，未执行。
- Tauri GUI 真机运行（拖放、主线程卡顿、关窗、双开）——静态证据充分，但建议在 Wave 2 前各做一次 3 分钟冒烟。
- E2E 未运行（CI 也不跑）；`npm run test:e2e` 54 条为文档基线，本轮未复跑。
- 同步双设备闭环（导出 → 改 → 导入 → 冲突 → 解决 → 再导出）——P1-2 的修复必须用这条链路验收。
- 依赖升级（`quick-xml` / `rkyv`）不属本轮范围，建议单独立项。

---

## 10. 追加（2026-11-05 用户反馈）：GUI「保存慢 2–3 秒」的根因与修复

**现象**：GUI 中修改 PATH 后点击保存，需等待 2–3 秒才返回。

**根因**：保存编排末尾的 `platform::system::broadcast_env_change()` 用
`SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, …, SMTO_ABORTIFHUNG, 5000)`
**逐顶层窗口同步发送**，总耗时是各窗口处理时间的累加（`SMTO_ABORTIFHUNG` 只在窗口真正挂起时才放弃）。
它在 `app/service/mod.rs:349` 被调用，此时 GUI 命令正持有 `DbState` 的互斥量，且命令本身是同步 IPC
（见 P1-5）——于是整个窗口冻结数秒。

**实测证据**（2026-11-05，本机）：

| 测量对象                                                                                        | 耗时                      |
| ----------------------------------------------------------------------------------------------- | ------------------------- |
| 直接调用 `SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, "Environment")`（ctypes，3 次） | **3.28s / 4.00s / 4.02s** |
| 同上，lParam 传 null（排除「刷新 Environment」的开销）                                          | 4.06s / 4.00s             |
| `cargo test -p path-editor-core --test db_profile_apply`（FakeIo + 临时库，唯一耗时项是该广播） | **4.35s**                 |

**修复**（`src/core/src/platform/system.rs:78-118`）：

- 广播改到独立线程执行，调用方立即返回；发送结果只写日志（通知本就是 best-effort）。
- lParam 改为进程级 `LazyLock<Vec<u16>>` 静态缓冲区 —— 必须比「发送」活得久，后台线程不再持有临时 `Vec`。
- 保留 5 秒超时：它现在只约束后台线程的生命周期，不再影响保存延迟。
- **短命进程不丢通知**：新增在途计数 + `flush_env_broadcasts()`，CLI 在 `main` 返回前等待在途广播发完。
  否则 `patheditor path add` 打印成功就退出，进程把广播线程一起带走，Explorer 收不到环境变更。

**修复后实测**：

| 测量对象                                          | 修复前 | 修复后                             |
| ------------------------------------------------- | ------ | ---------------------------------- |
| `db_profile_apply` 集成测试                       | 4.35s  | **0.07s**                          |
| core 单测套件                                     | 2.74s  | **0.21s**                          |
| 新增 `broadcast_env_change_does_not_block_caller` | —      | 通过（阈值 500ms；改回同步即失败） |

质量门：`cargo fmt --check` 通过、`cargo clippy --workspace --all-targets -D warnings` 通过。
**注意**：本文写成时 41 个 IPC 命令全部同步占用 UI 线程（当轮 P1 结论）；整改中已把 32 个重型命令
改为 `#[tauri::command(async)]`（见 §11），`validate_path` / `expand_env_vars` 等纯计算命令保持同步。

---

## 11. 整改状态（2026-11-05 按本报告实施）

**已修复**：P0-1；P1-1～P1-13；P2-1～P2-50、P2-53～P2-62；P3-1、P3-2、P3-4、P3-5。
主要落点：

| 主题                                  | 落点                                                                                                 |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| 同步冲突落实体 / schema v4            | `db/sync/conflict.rs`、`db/sync/import.rs`、`db/schema.rs`、`db/mod.rs`                              |
| FTS 增量                              | `db/search.rs`、`db/profiles.rs`、`db/sync/import.rs`                                                |
| 保留策略 / 水位 / 迁移并发 / NotFound | `db/maintenance.rs`、`db/derived.rs`、`db/mod.rs`、`db/history.rs`                                   |
| 密钥与 DACL / 同步包上限 / lamport    | `db/crypto.rs`、`db/dacl.rs`、`db/sync/import.rs`、`db/sync/change_log.rs`                           |
| PATH 类型/分号/系统目录/原子写/扫描   | `platform/registry/path.rs`、`platform/fs.rs`、`platform/scanner.rs`                                 |
| 备份轮换 / 恢复预览                   | `platform/backup/{env,restore}.rs`、`db/maintenance.rs`                                              |
| CLI 破坏性缺陷与契约                  | `cli/src/commands/{import_export,path,scan,maintenance,sync}.rs`、`cli.rs`、`runtime.rs`             |
| GUI 主线程 / 权限面 / 拖放 / 竞态     | `gui/src-tauri/src/{lib.rs,commands/*,capabilities/default.json,tauri.conf.json}`、前端 store 与组件 |
| 工程门禁                              | `.github/workflows/{ci,release,v6-sqlcipher-probe}.yml`、`tests/setup.ts`、`Cargo.toml`              |

**新增回归测试**：`db_sync_conflict_remote_applies`、`db_search_profile`、
`broadcast_env_change_does_not_block_caller`、`app-store-save-race`、Escape 清草稿、
降级 sidecar 与备份失败提示。

**刻意未做（有明确理由）**：

1. **P2-52 CLI 测试隔离**：夹具依赖真实 `~/.patheditor` 密钥，而 `config_dir()` 按既有决策
   不可被环境变量重定向；彻底隔离需要新增「显式密钥注入」的 CLI 入口，属接口设计变更，另行立项。
2. **P3-3 删除 22 个未使用 i18n 键**：其中部分可能对应尚未接线的功能（`status.dragFolderOnly`
   已随着拖放修复接回），逐个删除前需要产品取舍。
3. **P3-6 `demo.gif`（5.28 MB）**：迁移到 Release 附件或对象存储属于发布资产决策。
4. **P2-37 中的文件删除**：仅移除了 `bundle.resources` 条目；`src/gui/src-tauri/WebView2Loader.dll`
   与嵌套 `Cargo.lock` 仍保留在仓库——按用户规则「未经书面同意不得删除任何文件」，删除需单独确认。
