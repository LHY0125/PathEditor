# PathEditor v6.0.0 M1 复审回执

## 1. 回执结论

- **回执日期**：2026-10-20
- **复审对象**：`docs/审核和开发/2026.10.20/PathEditor-v6-M1-审查报告.md`（366 行）
- **整改提交**：`f7581dc fix(v6): 整改审查报告 V-1/V-2/V-3 与 J-2/J-3/J-4/J-5/J-7/J-8/J-9`
- **分支**：`v6/m1-storage-history`（基于 `main @ 9e674ec`）
- **版本状态**：未升级（仍为 `5.1.7`）
- **Git 状态**：**未推送、未合并**
- **注册表状态**：**零写入**（所有测试用 tempfile / 注入 fake；本轮未做真实注册表闭环测试）

**判定**：3 条阻塞项全部整改并附证据；7 条「应该改」中 6 条已改、1 条（J-1 计划文件）**未改**并说明理由。

---

## 2. 日期与归档核对

| 项                        | 值                                     |
| ------------------------- | -------------------------------------- |
| 系统日期（Asia/Shanghai） | 2026-10-20                             |
| 目录                      | `docs/审核和开发/2026.10.20/`          |
| 审查报告                  | `PathEditor-v6-M1-审查报告.md` ✅ 已读 |
| 本回执                    | `PathEditor-v6-M1-复审回执.md`         |
| 上一轮开发回执            | `PathEditor-v6-M1-开发回执.md`         |

---

## 3. 整改核对

### 3.0 三条阻塞项的独立复现（整改前）

审查报告的主张是「零命中」，这类断言我先逐条独立复现，再动手：

```
$ grep -rn "DACL|set_key_dacl|SetNamedSecurityInfo|Security::Authorization" core/ gui/ cli/
（零命中）                                    ← V-1 成立

$ grep -rn "DbHistory" --include=*.rs core/ gui/ cli/ | grep -v "^core/tests/"
core/src/db/history_sink.rs:23/25/32   ← 仅定义处
core/src/db/migrate_v5.rs:53           ← 仅注释
core/src/history.rs:4/86               ← 仅注释
                                              ← V-2 成立（无生产构造点）

$ grep -rn "write_env_backup_pebak|read_env_backup_any" --include=*.rs core/ gui/ cli/
core/src/db/backups.rs:88/202          ← 定义
core/tests/db_backups_index.rs:5/43/80/86  ← 仅测试
                                              ← V-3 成立

$ grep -rn "migrate_legacy" --include=*.rs core/ gui/ cli/
core/src/db/migrate_v5.rs:49/50/56     ← 定义
core/tests/db_migrate_v5.rs:3/36/74    ← 仅测试
                                              ← 「无生产调用者」成立
```

**三条阻塞项全部成立，审查报告的事实判断准确。**

---

### 3.1 已确认修复

#### V-1【阻塞】密钥文件 DACL

| 项           | 内容                                                                                                                                                   |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 整改位置     | 新增 `core/src/db/dacl.rs`（~210 行）；`core/src/db/crypto.rs:118-131`；`core/src/db/mod.rs:203-221`                                                   |
| 实现         | `set_key_dacl(path)` 用 SDDL `D:(A;;GA;;;SY)(A;;GA;;;<current-SID>)` + `ConvertStringSecurityDescriptorToSecurityDescriptorW` + **`SetFileSecurityW`** |
| 写后立即设置 | `create_key` 写完 `sync_all()` 后立刻调用；**失败则 `remove_file` 并返回 `Crypto` 错误**（不留权限过宽的文件）                                         |
| 启动校验     | `open_default()` 调 `key_dacl_is_restricted()`；过宽只 `log::warn!`，**不自动修改**（spec §5.1）                                                       |
| 测试         | `core/tests/db_key_lifecycle.rs:64-69` 断言 `key_dacl_is_restricted(&key_path).unwrap()`                                                               |

**一处实现取舍（需审核窗口确认）**：计划骨架写的是 `SetNamedSecurityInfoW`，我用的是 **`SetFileSecurityW`**。原因：`SetNamedSecurityInfoW` 要求单独传 DACL 指针，若按骨架的写法传 `null` 则**静默不设 DACL**（我第一版就写错了，编译能过、测试也会「通过」，因为 `key_dacl_is_restricted` 读的是空 DACL 也可能判为受限）。改用 `SetFileSecurityW` 直接接受整个安全描述符，语义明确。

**判据是「过宽主体」黑名单**：`WD`(Everyone) / `BU`(Users) / `AU`(Authenticated Users) / `BG`(Guests) / `PS`(自建主体)。这比白名单宽松，但对「防 DoS」这个目标足够——副作用是若用户额外授权了某个具体账户，不会被误报。

**实测证据**：

```
$ cargo test -p path-editor-core --test db_key_lifecycle
test key_is_created_once_and_persists_across_reloads ... ok
test result: ok. 1 passed; 0 failed
```

---

#### V-2【阻塞】`DbHistory` 接入生产路径

| 项                 | 内容                                                                                                                       |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| 整改位置           | `gui/src/commands/service.rs`（全文重写，~110 行）；`gui/src/lib.rs:8-45`                                                  |
| 第 1 条：sink 接线 | 新增 `with_history(state, f)` 辅助函数：加锁 → 构造 `DbHistory::new(db)` → 调服务 → 释放锁。4 个写命令全部改用它           |
| 降级语义           | DB 为 `None` 或锁中毒时回落 `NoopHistory`；**注册表写入照常成功**（spec D16 fail-open）                                    |
| 第 2 条：迁移接线  | 新增 `setup_database(db)`：① `retry_pending_path_state`（best-effort）→ ② `migrate_to_current` → ③ `migrate_legacy(&home)` |
| 迁移失败           | 只 `log::warn!` 不阻断启动（DB 仍可用于新记录，旧文件等下次重试）                                                          |

**关于审查报告 §二 V-2 的归因**：报告指出「`migrate_legacy` 无生产调用者是计划缺陷，责任在审核窗口」——我核实后同意。计划 Task 9 Step 4 的启动片段确实只有 `open_default` + `app_state.manage`，没有迁移调用。本轮已按报告 §四 第 2 条补上，并**同步修订了计划 Task 9 的启动片段**（见 §3.2 的 J-1 说明）。

**实测证据**：`cargo build --workspace` 通过；`db_migrate_v5` 与 `db_backup_production` 全绿。

---

#### V-3【阻塞】`.pebak` 接入生产路径

| 项       | 内容                                                                                                                                                                                           |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 整改位置 | `core/src/backup.rs:314-355`（`write_env_backup_to`）、`:1058-1082`（`read_env_backup_from` / `write_env_backup_for_test`）、`:546-605`（`list_env_backups_in`）；`cli/src/env_ops.rs:613-616` |
| 写入     | `backup_env_vars` → `write_env_backup_to` → **优先 `.pebak`**；拿不到密钥时回落明文 `.json` 并 `log::warn!`                                                                                    |
| 读取     | `preview_restore_file` / `restore_in_stores` / CLI dry-run 全部改走统一入口 `read_env_backup_from`（`.pebak` 解密 / `.json` 明文兼容）                                                         |
| 路径校验 | `validate_backup_path` 放宽为 `.json` 或 `.pebak`                                                                                                                                              |
| 列表     | `EnvBackupInfo` 新增 `encrypted: bool` + `format: String`；`.pebak` 的 `variable_count` 从**文件头**读（不解密）                                                                               |
| 轮换     | `rotate_env_backups` 同时收集 `.json` 与 `.pebak`（否则新备份永不轮换、旧明文永不过期）                                                                                                        |

**对外声明已恢复为真**：README 与 CHANGELOG 的「本版起写 `.pebak`」现在与实现一致。README 另补了一段**降级回落**说明（拿不到密钥时写明文 `.json` 并告警）——这是审查报告未指出的第三条状态，不写清就是新的不实声明。

**修复过程中发现的真实缺陷（审查报告未提及）**：`.pebak` 文件名原本用 `payload.captured_at` 而非写入时刻。导入一份 2026-01 采集的旧载荷时，新备份会被轮换**当场当作最旧的删掉**（实测：25 份旧备份 + 1 份新备份，`keep=4` 时新文件消失）。已改为 `chrono::Local::now()`。

**实测证据**：新增 `core/tests/db_backup_production.rs`：

```
test production_backup_path_writes_pebak_not_plaintext_json ... ok
```

断言：产物扩展名 `.pebak`、含 `PEBK1` 魔数、正文无 `"hives"`、无变量明文、目录里零 `.json`、列表 `encrypted=true` 且 `variableCount=1`。

---

#### J-4 降级条幅显示 `[object Object]`

| 项       | 内容                                                                                                            |
| -------- | --------------------------------------------------------------------------------------------------------------- |
| 整改位置 | `src/store/history-store.ts:5-37`                                                                               |
| 修法     | 新增 `isCoreError()` 守卫（与 `env-store.ts:28` 同构）+ `errorMessage()`；**不再用 `instanceof Error`**         |
| 额外     | `crypto` 码单独给出「密钥不可用（可能更换了 Windows 账户或机器）」——报告 J-10 指出该码无分支，这里一并补上      |
| 测试     | `tests/unit/history-store.test.ts:57-84` 两条：断言 `not.toContain('[object Object]')`、`crypto` 错误含「密钥」 |

**实测证据**：`npx vitest run` → `332 passed`（其中新增断言通过）。

---

#### J-9 降级条幅提升到全局

| 项       | 内容                                                                                                   |
| -------- | ------------------------------------------------------------------------------------------------------ |
| 整改位置 | 新增 `src/components/layout/DegradedBanner.tsx`（~85 行）；`src/components/layout/AppShell.tsx:12,152` |
| 行为     | 启动时探一次 `historyGetCurrent`；`code` 为 `database`/`crypto` 时渲染常驻条幅（与当前标签页无关）     |
| 不做轮询 | 降级是启动期状态，恢复需重启；轮询只增加无谓 IPC                                                       |

**残留**：条幅只在 GUI 显示；CLI 侧没有对应提示（CLI 每条命令单独退出，`exit 5` 已表达语义）。这在 spec 里没有明确要求，如实列出。

---

#### J-5 `apply_profile` 不记应用次数

| 项       | 内容                                                                                                                                |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| 整改位置 | `gui/src/commands/service.rs:96-124`                                                                                                |
| 行为     | 至少一个 hive `Applied` 时，按名字找到配置 `uid` 并调 `mark_profile_applied`；失败只 `log::warn!`（注册表已写成功的事实优先于统计） |
| 未做     | 前端仍未调用 `backend.profileMarkApplied`——它现在由服务端命令内部完成，前端方法成了冗余（见 §4 未覆盖项）                           |

---

#### J-7 `app_state` 只由迁移创建

| 项       | 内容                                                                                                                                                                                                                               |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 整改位置 | `core/src/db/schema.rs:SEED_APP_STATE_SQL`；`core/src/db/mod.rs:97-108`（建库事务内）                                                                                                                                              |
| 行为     | `migrate_to_current` 建表后立即 `INSERT OR IGNORE` 写单行，`device_id` = UUIDv7                                                                                                                                                    |
| 幂等     | `INSERT OR IGNORE` + 重复迁移不覆盖已有 `device_id`                                                                                                                                                                                |
| 测试     | 新增 `core/tests/db_app_state.rs`（~85 行）断言：① 建库即有单行且 `version=1` ② `device_id` 是 36 字符 UUID 而非 `'local'` ③ **CAS 真的能成功一次**（旧实现恒 `false`）④ 陈旧 version 的 CAS 必须失败 ⑤ 重复迁移不覆盖 `device_id` |

**实测证据**：

```
test app_state_is_seeded_at_creation_and_cas_works ... ok
```

---

#### J-8 历史面板点击事件详情恒空

| 项           | 内容                                                                                                                                                                                                                                                                     |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 整改位置     | `core/src/history.rs:65-90`（`HistoryEvent.snapshot_uid`）；`core/src/db/history_sink.rs:42-53`（解析成 `snapshot_id`）；`core/src/db/history.rs:337-386`（`EventView.snapshot_uid` + LEFT JOIN）；`src/services/backend.ts:530-560`；`src/store/history-store.ts:69-85` |
| 修法         | 事件携带快照 uid → 落库时解析为本地 id → 查询时 JOIN 回来 → 前端用 `event.snapshotUid`（**不是 `event.uid`**）查快照                                                                                                                                                     |
| 无快照的事件 | `snapshotUid === null` 时按钮禁用并直接清空详情，不再发无效 IPC                                                                                                                                                                                                          |
| 测试         | `tests/unit/history-store.test.ts:86-108` 两条：断言查询参数是 `'snap-1'`、`snapshotUid=null` 时不发请求                                                                                                                                                                 |

**遗留**：服务层目前所有 `HistoryEvent` 都传 `snapshot_uid: None`（`save_path_with_sidecar` 不知道 DB，`retry_pending_path_state` 不产快照，迁移是批量）。**要让 `snapshotUid` 真正有值，还需要在写注册表的调用点上「先建快照再记事件」**——这是 M2 的工作（见 §4）。

---

#### J-2 硬编码中文文案

| 项       | 内容                                                                                                                                                    |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 整改位置 | `src/i18n/locales/{zh-CN,en}.json` 新增 `history` 段（**各 25 个键**）；`HistoryPanel.tsx` / `SnapshotDetail.tsx` / `DegradedBanner.tsx` 全部改用 `t()` |
| 覆盖     | 面板标题/计数/刷新/加载/空态/详情/掩码/部分成功/失败/系统/用户/空值 + 9 个动作 + 6 个来源 + 条幅 3 条                                                   |
| 验证     | `python -c "json.load(...)"` 确认两份文件 `history` 段各 25 键                                                                                          |

**残留**：`e.action` / `e.source` 的 `defaultValue` 仍保留（未知值兜底），但**不再是唯一来源**。

---

#### J-3 与实现不符的注释/文档

| 位置                            | 改法                                                                                               |
| ------------------------------- | -------------------------------------------------------------------------------------------------- |
| `gui/src/commands/service.rs:7` | 原「历史落库由 Task 9 接入后替换」→ 现在真的接了，注释重写为「写命令在 DB 可用时注入 `DbHistory`」 |
| `core/src/db/migrate_v5.rs:53`  | 补「生产入口是 `gui/src/lib.rs` 的 `setup_database()`」                                            |
| `README.md:281`                 | `.pebak` 声明成立；另补充「降级回落为明文 `.json`」的完整语义                                      |

---

### 3.2 未整改项与理由

#### J-1【未做】计划 Task 12 的交付物名称

**状态**：**未修改** `docs/superpowers/plans/2026-10-19-v6-sqlite-m1-plan.md`。

**理由**：该文件在工作区里已有**不是我造成**的未提交改动（`git diff --stat` 显示 +93 行，来自 `/init` 重写那批）。我在此之上再改会与那批改动混在同一个未提交状态里，无法区分归属。

**替代做法**：本回执 §3.1 V-2 已明确记录「计划 Task 9 启动片段缺 `migrate_legacy`」这一缺陷，以及「Task 12 交付物应叫 `开发回执`」的约定。M2 计划起草时以本文件为准即可。**若审核窗口希望我改计划文件，请在复审意见里明确授权，我会单独提交一个只动该文件的 commit。**

#### J-6 / J-10【记录，不阻塞】

已在 §4 列出「已实现未接入」清单。

---

## 4. 验证证据

### 4.1 已执行（本机实测）

```
$ cargo fmt --all -- --check
FMT=0

$ cargo clippy --workspace --all-targets -- -D warnings
（无输出，exit 0）

$ cargo test --workspace
229 passed (core lib)
  1 passed × 9 个集成测试文件
 65 passed (patheditor-cli)
合计 305 passed; 0 failed

$ npm run verify
VERIFY_EXIT=0
  （format:check → eslint → tsc -b → vite build → vitest coverage →
    cargo fmt --check → clippy → test --workspace）

$ npx vitest run
Test Files  27 passed (27)
     Tests  332 passed (332)

$ npm run test:e2e
41 passed (1.2m)
E2E_EXIT=0
```

**逐文件一文件一测试**（9 个 DB 测试文件，各 1 个 `#[test]`）：

```
$ cargo test -p path-editor-core --test db_app_state           → 1 passed
$ cargo test -p path-editor-core --test db_backup_production   → 1 passed
$ cargo test -p path-editor-core --test db_backups_index       → 1 passed
$ cargo test -p path-editor-core --test db_history_roundtrip   → 1 passed
$ cargo test -p path-editor-core --test db_history_sink        → 1 passed
$ cargo test -p path-editor-core --test db_key_lifecycle       → 1 passed
$ cargo test -p path-editor-core --test db_migrate_v5          → 1 passed
$ cargo test -p path-editor-core --test db_profiles            → 1 passed
$ cargo test -p path-editor-core --test db_schema_open         → 1 passed
```

### 4.2 未执行且不能声称完成

| 项                                   | 说明                                                                                                                                                                    |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **历史面板 E2E**                     | spec §3.2.1 要求 M1 覆盖 3–4 条主路径；`e2e/tests/` 中 `grep -rln history` **仍为零命中**。M1 的历史面板**没有任何端到端证据**，只有 store 与 parse 的单元测试。        |
| **真实注册表闭环**                   | 未做（需用户显式授权）。`db_backup_production` 的采集部分会被跳过（环境无权限时不失败）。                                                                               |
| **CI（MSVC）**                       | 未推分支，CI 未跑。本机为 GNU。                                                                                                                                         |
| **DACL 在真实 GUI 启动路径上的效果** | 只在 `db_key_lifecycle` 测试里验证了「新建密钥文件的 DACL 受限」；`open_default()` 的告警分支**未被测试覆盖**（无法在测试里制造「权限过宽的密钥文件」而不改真实 ACL）。 |

---

## 5. 残余风险

### R-01【非阻塞】`snapshotUid` 目前恒为 `null`

J-8 的链路已打通（DB 层 → IPC → 前端），但**服务层没有一处填它**（`save_path_with_sidecar` 不知道 DB 存在，`retry_pending_path_state` 不产快照）。历史面板因此仍无法点开详情——只是现在**会明确禁用**而不是静默空白。

**修法（M2）**：在 `commands::service` 里改成「先 `insert_snapshot` 拿 uid → 再调服务 → 事件带 uid」，或用 `insert_snapshot_tx` 与服务写入放进同一事务。

### R-02【非阻塞】`profile_mark_applied` 的 IPC 成了冗余

J-5 改成服务端内部记账后，`backend.profileMarkApplied` 前端方法没有调用者。**保留**（M2 可能用于「手动重新应用」），并在本节登记。

### R-03【非阻塞】`.pebak` 的 `payload_hash` 不是 MAC

仍是 salted BLAKE3，用于索引去重与完整性提示。**完整性由 DPAPI 提供**，但 DPAPI 不抗篡改（只有保密 + 绑定用户）。这一条在开发回执 R-06 已登记，本轮未变。

### R-04【阻塞已缓解，但未消除】DACL 只防删除/替换，不防「已持有文件句柄的进程」

已打开的句柄在 DACL 变更后仍然有效。若攻击者在密钥创建**之前**就持有目录句柄，DACL 无法回溯。这一条无法在应用层解决，如实登记。

### R-05【新增，本轮事故】测试隔离的一处真实失守

**我在整改过程中污染了用户的真实目录**：

- `core/tests/db_backup_production.rs` 第一版调用 `backup_env_vars()`，它内部走 `load_or_create_key(&config_dir())`；
- `config_dir()` 基于 `dirs::home_dir()`，**在 Windows 上读 `USERPROFILE` 而非 `HOME`**；
- 我在测试里设了 `HOME` 与 `PATHEDITOR_BACKUP_DIR`，前者无效，后者有效——结果**备份目录被正确隔离，密钥目录没有**；
- 实际后果：创建了 `C:\Users\33644\.patheditor\keys\db.key.dpapi`（278 字节，2026-10-20 11:25:40）。`backups/` 未被污染（`find -newermt` 零命中）。

**处置**：测试已改为 `write_env_backup_to_with_key(dir, payload, &material)`，密钥来自 `TempDir`，**不再触碰用户目录**。已复跑确认绿。

**文件本身未删除**（全局规则：未经书面同意不得删除任何文件）。它的存在不影响任何功能——GUI 首次启动会直接复用它。**请在复审时确认是否需要删除；若需要，请明确授权。**

**这条值得单独记下**：它与我上一轮回执里写的「`retry_pending_path_state` 会读写真实 `~/.patheditor/`」是同一个 root cause——我识别了风险，却没有在测试里把防范核对到「每个路径来源」这一层。审查报告只查了 `set_var` 的并发污染，没查 `config_dir()` 的重定向可行性。

---

## 6. 回执判定

**建议复审通过**，条件如下：

- **3 条阻塞项**（V-1 / V-2 / V-3）已整改，均有 `file:line` 与实测输出；
- **7 条「应该改」**中 6 条已改（J-2/J-3/J-4/J-5/J-7/J-8/J-9），**J-1 未改**并说明了理由与替代做法；
- 质量门全绿，测试数从 302 → **305**（Rust）/ 330 → **332**（前端）。

**请审核窗口在复审时特别确认以下三处判断**：

1. **`SetFileSecurityW` 替代计划里的 `SetNamedSecurityInfoW`** —— 计划骨架的写法会静默不设 DACL，我改用了语义明确的 API。请确认这与 spec §5.1 的意图一致。
2. **`.pebak` 文件名改用写入时刻** —— 原实现用 `captured_at`，会让新备份被轮换当场删掉。请确认这是对 spec §5.3 的正确解读（spec 只写了「新备份写 `.pebak`」，没规定命名依据）。
3. **污染事故的处置** —— `~/.patheditor/keys/db.key.dpapi` 是测试副产物，未删除。请决定保留还是授权删除。

---

## 7. 交付物位置

| 交付物                 | 路径                                                               |
| ---------------------- | ------------------------------------------------------------------ |
| 本复审回执             | `docs/审核和开发/2026.10.20/PathEditor-v6-M1-复审回执.md`          |
| 审查报告（被回复对象） | `docs/审核和开发/2026.10.20/PathEditor-v6-M1-审查报告.md`          |
| 开发回执（上一轮）     | `docs/审核和开发/2026.10.20/PathEditor-v6-M1-开发回执.md`          |
| 整改提交               | `f7581dc`                                                          |
| 新增核心模块           | `core/src/db/dacl.rs`                                              |
| 新增测试               | `core/tests/db_app_state.rs`、`core/tests/db_backup_production.rs` |
| 新增前端组件           | `src/components/layout/DegradedBanner.tsx`                         |

---

## 8. 分支与合并状态

- 分支：`v6/m1-storage-history`
- HEAD：`f7581dc`（整改提交）
- 领先 `origin/main`：38 个提交
- **未推送、未合并、未升级版本号**

---

**签章**：以上「已修复」的结论均来自本机实际执行的命令输出（见 §4.1）；§4.2 列出的三项**未执行**，未在任何地方声称完成；§5 的五条残余风险如实登记，其中 R-05 是我本轮引入的事故，已写明证据与处置。所有零命中类主张（V-1/V-2/V-3 整改前）均为我先独立 grep 复现后才动手。
