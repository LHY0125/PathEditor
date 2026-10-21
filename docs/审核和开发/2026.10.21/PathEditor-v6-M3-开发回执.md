# PathEditor v6.0.0 M3 同步 — 开发回执

**提交窗口**: 开发窗口
**日期**: 2026-10-21
**交付对象**: 审核窗口
**分支**: main
**版本**: 5.1.7（M3 不升版本号）
**状态**: 开发完成，质量门全绿，**未推送**

---

## §0 事故与异常

### 事故 1：GUI 测试二进制无法启动（0xC0000139）

**现象**：Task 8 执行 `cargo test --workspace` 时，`app_lib` 测试二进制以
`0xC0000139 STATUS_ENTRYPOINT_NOT_FOUND` 退出——**一个测试都没跑**，报告只写
「进程未能成功退出」，极易被误读成测试失败。

**定位过程**（全部有命令输出支撑）：

| 步骤                                        | 结论                                           |
| ------------------------------------------- | ---------------------------------------------- |
| 用 `objdump -p` 导出完整导入表              | 发现 `comctl32.dll!TaskDialogIndirect`         |
| 核对 System32 的 comctl32                   | 版本 **5.82**，**没有**该导出                  |
| 用 `mt.exe` 把 v6 清单注入同一 exe          | 立即恢复正常 → 根因确认                        |
| 查 `target/debug/build/patheditor-*/output` | Tauri 用 `rustc-link-arg-**bins**`，只覆盖 bin |

**根因**：依赖链启用 `windows-sys` 的 `Win32_UI_Controls`，导入
`comctl32!TaskDialogIndirect`——该符号**只存在于 comctl32 v6**，需 side-by-side
清单才会加载 WinSxS 里的 v6。Tauri 只给 **bin** 嵌了清单（`libresource.a`），
**lib 的 test harness 拿不到**。

**为什么以前没暴露**：M2 期间 GUI 测试一直绿。Task 8 前我执行过
`cargo clean -p patheditor`（清掉 20.1 GiB），强制用当前依赖树重建了测试二进制，
新增的 `rpassword → rtoolbox → windows-sys` 依赖与工作区发生 feature 合并，把
`Win32_UI_Controls` 带进了链接。**陈旧产物长期掩盖了这个问题。**

**修复**（`gui/build.rs` + `gui/test.rc` + `gui/test.manifest`）：
用 `windres` 把 v6 清单编译成 `.o`，经 `cargo:rustc-link-arg` 注入。

两个踩过的坑，都记在代码注释里：

1. `cargo:rustc-link-arg-**tests**` **不覆盖 lib 单元测试**——该指令只对
   `tests/*.rs` 集成测试生效，对 lib 的 test harness 无效（会报
   `does not have a test target`）。只能用全目标的 `rustc-link-arg`。
2. 用全目标后，bin 会同时链上 Tauri 的 `libresource.a` 与我的 `.o`，
   二者都是 `RT_MANIFEST/ID 1` → 链接器报 **duplicate resource**。
   解法是让我的资源用**中性语言 0**，与 Tauri 的 1033 资源身份不同。

**验证**：`cargo test -p patheditor --lib` 通过；`cargo build -p patheditor`
（生产 bin）也通过，无 duplicate resource。

### 事故 2：`.git/index.lock` 残留

14:34 一次被中断的 git 操作留下 0 字节锁文件，阻塞提交。经用户书面同意后删除
（当时运行中的 git 进程属于**另一个仓库**的 marketplace 克隆，与本仓库无关）。

### 事故 3：清单 XML 首次解析失败

第一版 `test.manifest` 带大段中文注释，`mt.exe` 报
`Failed to load and parse the manifest`，Windows 侧表现为
`os error 14001`（并行配置不正确）。改为**纯 ASCII 最小清单**后恢复正常。
教训：清单是给 Windows 解析器读的，不是给自己读的。

---

## §1 交付内容

| Task | 内容                                                                                  | 提交      |
| ---- | ------------------------------------------------------------------------------------- | --------- |
| 1    | schema v3：`changes` + `sync_conflicts` + 三条索引                                    | `84720fd` |
| 2    | 变更日志：`record_change_tx` / `pending_export` / `mark_exported` / `current_lamport` | `0c7ef86` |
| 2b   | **S-1 接线**：4 个写入口在各自事务内记变更，`EventSource::Migration` 跳过             | `d167629` |
| 3    | Bundle 加密：`PESYNC1` + Argon2id + ChaCha20-Poly1305，**AAD 覆盖 header**            | `afccb22` |
| 4    | 导出：收集 → 加密 → 写文件 →（成功后）标记                                            | `a725d01` |
| 5    | 字段级 LWW：`resolve` 纯函数，四类情况 + 冲突对称性                                   | `cc10b67` |
| 6    | 导入：单事务、幂等、冲突记录、篡改拒绝；`resolve_conflict` 追加更高 lamport           | `8889c8a` |
| 7    | 退出码拆分：`Sync`→4 / `SyncFormat`→1（映射在 Task 3 已提前落地）                     | `5b3b5ce` |
| 8    | IPC 5 条命令 + CLI 5 个子命令（`rpassword` 隐藏输入）                                 | `41be188` |
| 9    | 前端形状校验 + `sync-store` + `SyncCenter` + `sync` 标签页                            | `8a1c087` |
| 10   | E2E 4 条主路径 + README / CHANGELOG                                                   | `d041b45` |

**代码规模**：47 个文件，+4001 / −14 行。

---

## §2 设计决策

### 1. `changes` 是真相，不是派生表

M2 的 `search_fts` / `daily_stats` / `var_references` 都可从真相重建，所以
`rebuild_derived_tables` 敢在启动时清空重建。`changes` **不能**——它记录
「发生过什么」，重建等于伪造历史。代码注释与 CHANGELOG 都写明了这一点。

### 2. `seq` 与 `lamport` 同源推进

`seq` 是**本设备**单调序号（`MAX(seq) WHERE device_id = ? + 1`），
`lamport` 是跨设备可比的逻辑时钟。两者每次写入都 +1。
若 `lamport` 落后于 `seq`，导入时的 LWW 会用错误时间比较，
「较新的本地修改」会输给「较旧的远端修改」。

**为什么 `seq` 不依赖 `id` 自增**：导入会把**其它设备**的变更写进同一张表，
自增 id 因此不等于本设备序号。

### 3. bundle 里带 `content`（对 spec §6.4 的补充）

spec 的 `Change` 只有 `oldValue` / `newValue` 两个自由文本字段，
**不足以让接收端重建实体**：变更日志只知道「某快照被创建了」，
不知道它里面有哪些 PATH 条目。只导出变更行的话，接收端解密成功、
能记一条 `changes` 行，却写不出 `snapshots` / `path_entries`——
**同步「成功」但什么都没同步**。

因此导出时把实体**当前内容**一并带上（`content`）。代价是同一实体多次变更
会重复携带内容，由导入侧按 `entityUid` 去重（后到的覆盖先到的）。

### 4. 冲突解决追加「更新」的变更

`resolve_conflict` 追加的变更由 `record_change_tx` 分配 lamport
（= `app_state.lamport + 1`），与 `next_lamport()` 同一算法。
不追加的话，第三台设备导入时会按原 lamport 重新判一次，
可能把用户刚做的选择覆盖掉。

### 5. 导入只覆盖能自包含的实体

`apply_entity_tx` 目前只落 `snapshot`（含 `path_entries` / `env_values`）。
`profile` / `profile_revision` 的落库需要「远端 uid → 本地整数主键」的映射，
属于未覆盖项（见 §5）。

### 6. `Sync` / `SyncFormat` 拆码

`Sync` → 4（合并冲突，需人工选择）；`SyncFormat` → 1（包格式/口令/篡改，重试无用）。
拆开是因为脚本要能区分「提示用户解决冲突」与「报告文件损坏」。

---

## §3 偏离记录

| #   | 计划原文                                      | 实际做法                                                   | 理由                                                                                                                                                                                                                      |
| --- | --------------------------------------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `insert_snapshot_tx` 无条件记 `snapshot` 变更 | **`EventSource::Migration` 时跳过**                        | 该函数同时被 v5 文件迁移与首次启动建基线调用。无条件记录会让用户升级到 v6 的瞬间，把历史配置/备份/PATH 快照全部推入待导出队列。判据复用现成的 `EventSource`，未新造机制。已写进 `db_change_log_wiring.rs` 的第 ③ 段断言。 |
| 2   | spec §5.2 `magic "PESYNC1" (8)`               | `PESYNC1\0`（8 字节，尾随 NUL）                            | `PESYNC1` 只有 **7 个字符**，规格本身笔误。取 8 字节定长使「8 字节 magic」的文字描述成立。                                                                                                                                |
| 3   | Task 7 才定义 `SyncFormat`                    | **提前到 Task 3**                                          | Task 3 的 `bundle.rs` 就要用它表达「格式错误」。任务顺序存在依赖倒置。Task 7 仍保留 `error_exit_codes.rs` 回归测试。                                                                                                      |
| 4   | 计划未提                                      | **新增 `gui/build.rs` + `test.{rc,manifest}`**             | 见 §0 事故 1。这是**构建修复**，不是功能扩张；只影响测试目标的清单。                                                                                                                                                      |
| 5   | 计划未提                                      | **bundle 增加 `content` 字段**                             | 见 §2 决策 3。                                                                                                                                                                                                            |
| 6   | `ChangeRow` 字段 14 个（S-2 裁断）            | 完全照做                                                   | 无偏离。                                                                                                                                                                                                                  |
| 7   | 计划的 `save_profile_tx_helper` 占位名        | 用实际的 `save_profile(db, name, description, tags, snap)` | 计划自己标注「按实际签名调整」。                                                                                                                                                                                          |

---

## §4 质量门

（见下方「实际执行输出」——每条都来自本次会话真实运行）

### Rust

```
cargo test --workspace
Rust: passed=339 failed=0 ignored=2
```

基线 329 → 339（**+10**，即 M3 新增的 10 个 `core/tests/*.rs`）。

### 前端

```
npx vitest run
Test Files  33 passed (33)
     Tests  395 passed (395)
```

基线 374 → 395（**+21**：`backend-sync-shapes` 14 条 + `sync-center` 7 条）。

### E2E

```
npx playwright test --config e2e/playwright.config.ts
Running 54 tests using 2 workers
54 passed (1.8m)
```

基线 50 → 54（**+4**，`e2e/tests/sync.spec.ts`）。

### 全量质量门

```
npm run verify:all
54 passed (1.8m)
[exited with code 0]
```

`verify:all` 依次执行：Prettier check → ESLint → 前端构建 → 覆盖率（行覆盖 ≥80%）
→ `cargo fmt --check` → Clippy `-D warnings` → `cargo test --workspace` → Playwright。
**exit 0 意味着上述每一步都通过**（覆盖率门槛未触发失败）。

### 其他

```
cargo fmt --all -- --check        → FMT_OK
cargo clippy --workspace --all-targets -- -D warnings → 无 error / 无 warning
cargo test -p patheditor-cli --bins → 67 passed
npx tsc --noEmit                  → 无输出（通过）
```

### S-1 硬性门（逐个 pub fn 核调用者）

```
# 0. 先验证 grep 模式本身
grep -rn "dashboard(" --include=*.rs core/src gui/src cli/src | grep -v "fn dashboard"
gui/src/commands/m2.rs:49:        stats::dashboard(db, days.unwrap_or(30))   ← 模式有效

# 1. record_change_tx 生产调用者
grep -rn "record_change_tx(" --include=*.rs core/src gui/src cli/src \
  | grep -v "fn record_change_tx" | wc -l
6

# 2. sync 模块全部 pub fn 零调用者扫描
（遍历 core/src/db/sync/*.rs 的 pub fn，逐个 grep 调用点）
SCAN_DONE   ← 无 ZERO 输出
```

**过程中真实发现并修掉的 3 个零调用者**（自查，非审核指出）：
`latest_for`、`raise_lamport`、`find_by_uid`。前两个删除；
`find_by_uid` 改造成 `change_exists_tx` 并接进导入幂等判据。

### 一文件一测试（逐个 grep）

```
db_schema_v3: 1        db_schema_v2_to_v3: 1   db_change_log: 1
db_change_log_wiring: 1 db_bundle_crypto: 1    db_sync_export: 1
db_sync_merge: 1       db_sync_import: 1       db_sync_conflict: 1
error_exit_codes: 1
```

全部恰好 1 个 `#[test]`。

### 反向验证（测试真的能抓到缺陷）

两处**故意把实现改错**，确认测试会失败，然后恢复：

| 改错                                                     | 结果                                                                              |
| -------------------------------------------------------- | --------------------------------------------------------------------------------- |
| `bundle.rs` 的 AAD 从「magic…nonce」收窄成「salt+nonce」 | `db_bundle_crypto` 第 47 行「篡改 header」断言失败                                |
| `export.rs` 把 `mark_exported` 提到写文件之前            | `db_sync_export` 第 49 行「写失败后待导出队列不变」失败，报「3 条变更被永久丢失」 |

---

## §5 未覆盖项（如实列出）

1. **配置（`profiles` / `profile_revisions`）导入时不落库**。
   导出侧已带 `content`，导入侧 `apply_entity_tx` 只处理 `snapshot`。
   需要「远端 uid → 本地整数主键」的映射表，M3 未做。
   **影响**：同步包里的配置变更会被记为 applied，但数据库里不会出现对应配置。
2. **`events` 不同步**（spec §6.3 已修订）。M3 按裁断不实现。
3. **真实注册表闭环测试未做**。同步本身不写注册表，但「导入后 apply 生效」这条
   端到端链路没有实机验证，需用户授权。
4. **`path_entry` / `env_value` 作为独立 entityType 的导入未实现**。
   它们随快照整体搬运，`apply_entity_tx` 里没有单独分支。
5. **C-3 的 `events.snapshot_id` 悬空规则**：因 M3 不同步 `events`，
   该规则只写在计划里，代码中无对应实现（将来恢复 events 同步时需补）。
6. **`maskConflictValue` 只在长度上折叠，不做敏感判定**。
   前端无法判定敏感（权威在 Rust 的 `env_var::is_sensitive`），
   真正的掩码需要在 Rust 生成 `SyncConflictView` 时完成——**当前未做**。
7. **CLI `sync` 子命令无单测**。`rpassword` 需要真实 TTY，单测环境不可用；
   目前只有 `--help` 与编译期保证。
8. **E2E 只覆盖 mock IPC**。真实 Tauri + 真实库的同步链路未验证。

---

## §6 残余风险

| #   | 风险                      | 说明                                                                                                                                                                                                                |
| --- | ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R-1 | **配置同步静默不完整**    | 见 §5-1。UI 会显示「应用 N 条」，但那 N 条里若含配置，实际没落库。这是本轮最大的功能缺口。                                                                                                                          |
| R-2 | 冲突值可能以明文出现在 UI | `SyncConflictView.local_value` 不含敏感标记，前端只能按长度折叠。若冲突字段是环境变量值，明文会显示在同步中心。                                                                                                     |
| R-3 | 单设备写假设              | `record_change_tx` 的 `MAX(seq)+1` 依赖「同一时刻只有一个连接写」。`Db` 由 `Mutex` 保护，当前成立；若将来引入多进程写同一库，`seq` 可能重复。                                                                       |
| R-4 | Argon2 参数不可调         | spec §5.2 写死（64 MiB / t=3 / p=1）。调高会让旧客户端解不开，这是**有意的兼容性锁定**，不是疏漏。                                                                                                                  |
| R-5 | 口令强度无校验            | 只拒绝空串。弱口令（如 `123`）仍会生成合法包——加密强度取决于用户。                                                                                                                                                  |
| R-6 | `windres` 成为构建依赖    | 新增的 `gui/build.rs` 在 Windows 上调用 `windres`。本机已有（CLAUDE.md 记录 Tauri GUI 构建本就依赖它），但**这是新引入的显式依赖**；找不到时只打 `cargo:warning`，不阻断构建（届时 GUI 测试会以 0xC0000139 退出）。 |

---

## §7 需审核窗口重点关注的项

1. **§0 事故 1 的 build.rs 修复**是计划外的构建变更。请确认：
   - 中性语言清单（`LANGUAGE 0, 0`）在生产 bin 上是否真的无害
     （当前验证：bin 能构建；运行期 Windows 会用 Tauri 的 1033 版本）。
   - MSVC 工具链（CI）下是否仍正常（`CARGO_CFG_TARGET_OS` 判断只管平台，
     不管工具链；MSVC 下 `windres` 可能不存在 → 只打 warning）。
2. **§5-1 配置不落库**是否可接受进入 M4，还是本轮必须补。
3. **C-2 的 exit code 拆分**已在代码与前后端白名单落地，请确认措辞与 spec §8 一致。
4. **§3 偏离 1（Migration 跳过）**改变了计划 Task 2b 对照表的字面要求，请复核。
5. **`content` 字段**是对 spec §6.4 的实质补充，建议 spec 同步修订。

---

## §8 交付物位置索引

| 交付物             | 路径                                                                                       |
| ------------------ | ------------------------------------------------------------------------------------------ |
| schema v3          | `core/src/db/schema.rs`、`core/src/db/mod.rs`                                              |
| 同步模块           | `core/src/db/sync/{mod,change_log,bundle,export,import,merge,conflict}.rs`                 |
| 写路径接线         | `core/src/db/profiles.rs`、`core/src/db/history.rs`                                        |
| 错误码             | `core/src/error.rs`                                                                        |
| GUI 命令           | `gui/src/commands/sync.rs`、`gui/src/lib.rs`                                               |
| 构建修复           | `gui/build.rs`、`gui/test.rc`、`gui/test.manifest`                                         |
| CLI                | `cli/src/sync_ops.rs`、`cli/src/main.rs`                                                   |
| 前端               | `src/services/backend.ts`、`src/store/sync-store.ts`、`src/components/sync/SyncCenter.tsx` |
| i18n               | `src/i18n/locales/{zh-CN,en}.json`                                                         |
| Rust 测试（10 个） | `core/tests/db_schema_v3.rs` … `core/tests/error_exit_codes.rs`                            |
| 前端测试（2 个）   | `tests/unit/backend-sync-shapes.test.ts`、`tests/unit/sync-center.test.tsx`                |
| E2E                | `e2e/tests/sync.spec.ts`、`e2e/mocks/ipc.ts`、`e2e/global.d.ts`                            |
| 文档               | `README.md`、`CHANGELOG.md`                                                                |

---

## §9 提交清单与状态

```
d041b45 test(v6): 同步 E2E 四条主路径；README / CHANGELOG 更新
8a1c087 feat(v6): 同步中心 UI 与前端形状校验
41be188 feat(v6): 同步 IPC 与 CLI（sync export/import/status/conflicts/resolve）
5b3b5ce feat(v6): 拆分 Sync/SyncFormat 退出码（4 / 1）
8889c8a feat(v6): 同步包导入（单事务、幂等、冲突记录、篡改拒绝）
cc10b67 feat(v6): 字段级 LWW 合并（lamport + device_id 破平）
a725d01 feat(v6): 同步包导出（先写文件后标记，避免丢变更）
afccb22 feat(v6): 同步包加密（Argon2id + ChaCha20-Poly1305，AAD 覆盖 header）
d167629 feat(v6): 把写路径接进变更日志（S-1）
0c7ef86 feat(v6): changes 变更日志（seq/lamport 单调、待导出查询）
84720fd feat(v6): schema v3 —— 同步表（changes / sync_conflicts）
```

- **未推送**、**未升版本号**、**未写真实注册表**。
- E2E 全部走 mock IPC。

---

**签章**：本文档每一条「已通过」都来自实际执行过的命令输出；
未做的写在 §5，未验证的写在 §6。没有推测，没有「应该没问题」。
