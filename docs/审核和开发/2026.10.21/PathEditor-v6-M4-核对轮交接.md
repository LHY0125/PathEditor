# PathEditor v6.0.0 M4 计划核对 — 开发窗口提示词

> **给审核窗口**：本文件是开发窗口对 M4 计划与开工提示词的核对结果，
> 供你决定是否修订计划。我**未写任何产品代码**，未提交、未推送、未升级版本号、未碰注册表。
>
> 按 `patheditor-dev-window` 的流程，**打磨收敛前不开工**。

- **核对对象**：`docs/superpowers/plans/2026-10-21-v6-sqlite-m4-plan.md`（2306 行，11 个 Task）
- **开工指令**：`docs/审核和开发/2026.10.21/PathEditor-v6-M4-开工提示词.md`（278 行）
- **对照权威**：`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` §4.2 第 10 条 / §6.3 / §6.4 / §6.5 / §7.4 / §十二 / §十三
- **判定**：**方向正确，2 处会阻塞开工，另有 4 项建议同轮修。**
- **日期**：2026-10-21

---

## 0. 结论摘要

| 类别                                           | 数量 | 是否阻塞 |
| ---------------------------------------------- | ---- | -------- |
| **阻塞**（照写会红、或会在用户机器上毁掉兜底） | 2    | **是**   |
| 建议同轮修（不阻塞但口径会错）                 | 4    | 否       |
| 观察项（照做，写进回执）                       | 3    | 否       |
| 我核对通过的强项                               | 18   | —        |

**最严重的一条**：Task 7 把 `.v5.bak` 归档放在 `setup_database` 的 ③c，
**早于 ⑤「读路径切到 DB」**。任何在 ③c 之后、⑤ 之前的失败都会让
`disabled.json` 已经改名、而读路径仍依赖它 —— 用户禁用的 PATH 条目会全部显示成启用。
这与同一文件里 ⑤ 的注释所警告的形态**完全一致**。

---

## 1. 阻塞项

### S-1 `core/tests/db_sync_import_limits.rs` 会红，但计划要求它绿

**计划原文**（Task 1 Step 6）：

```text
Run: cargo test -p path_editor_core --test db_sync_import --test db_sync_import_limits \
       --test db_sync_conflict --test db_sync_merge --test db_sync_export
Expected: 全部 passed。
```

**实际情况**：`core/tests/db_sync_import_limits.rs` 断言的正是 **M3 的行为**：

| 行  | 断言                                                | Task 1 之后的结果                                              |
| --- | --------------------------------------------------- | -------------------------------------------------------------- |
| :61 | `report.applied == 1`（只有快照落库）               | 变 **2**（profile 也计入 applied）                             |
| :63 | `report.skipped_unsupported == 1`                   | 变 **0**                                                       |
| :77 | `changes` 里 `entity_type='profile'` 的行数为 **0** | 变 **1**（该包的 profile `content` 是 null，只记变更不落实体） |
| :91 | 第二次导入 `skipped_unsupported == 1`               | 变 **0**，且 `skipped` 变 **2**                                |

**为什么计划没发现**：这个文件是 M3 整改时**专门为守旧行为加的守卫测试**
（M3 复审回执 §3.4）。Task 1 恰好把被守的行为改掉了。
`grep -rn "skipped_unsupported" core/tests/` 只有它一个命中。

**建议修法**（需要你确认改写后的断言口径）：

1. **不删文件**（未经用户书面同意不删）。
2. 保留第二段「未知 `entityType = "backups"` → `SyncFormat` 且零写入」——它跨版本有效。
3. 第一段改成守 **M4 语义**：
   - 含 1 快照 + 1 配置（`content` 为 `null`）的包 → `applied == 2`、`skipped_unsupported == 0`、
     `changes` 里 profile 行数为 **1**、`profiles` 表行数为 **0**（`content` 为 null 时只记变更，不落实体）。
   - 重复导入 → `applied == 0`、`skipped == 2`、`skipped_unsupported == 0`。
4. 文件头注释改写：R-1 的「不能报不存在的成功」在 M4 由「已知类型全部可落库」覆盖，
   这条测试的新职责是**守「content=null 的已知类型只记变更、不造实体」**。

> **待裁断**：上面第 3 条的四个数字是否就是你要的口径？若你想保留「不支持的计数」语义，
> 需要先定义 M4 之后什么是「不支持的类型」——目前答案是「没有」。

---

### S-2 Task 7 的归档位置早于「读路径切到 DB」，会销毁降级兜底

**计划原文**（Task 7 Step 4）：

> 在 `ensure_current_snapshot_baseline(db);` 之后：

```rust
    // ③c v5 原件归档（spec §7.4）：只在迁移零失败时执行；失败只告警。
    if migration_clean {
        match path_editor_core::db::archive_v5::archive_v5_sources(&home, false) { … }
    }
```

**仓库事实**：

| 位置                       | 事实                                                                                                                    |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `core/src/disabled.rs:292` | `load_path_snapshot()`：`v5_writes_disabled()` 为 **false** 时**只读 `disabled.json`**                                  |
| `core/src/disabled.rs:297` | 为 **true** 但 DB 取不到当前快照时，**仍然回落读 `disabled.json`**                                                      |
| `gui/src/lib.rs:153`       | `ensure_current_snapshot_baseline` 在读注册表失败或 `insert_snapshot` 失败时**只 `log::warn!` 然后 `return`**，不建基线 |
| `gui/src/lib.rs:70`        | `set_v5_writes_disabled(true)` 在 **⑤**，晚于计划要求的 ③c                                                              |

**失败场景（可复现的推理链，不是猜测）**：

```text
启动 → ③ 迁移成功（migration_clean = true）
     → ③b ensure_current_snapshot_baseline 读注册表失败 → 只 warn，current_snapshot_id 仍为 NULL
     → ③c 归档：disabled.json 改名为 disabled.json.v5.bak
     → 若此后任一步失败（④ 备份索引 panic / ⑤ 前 return / GUI 进入降级模式）
     → load_path_snapshot()：
          v5_writes_disabled == false → read_state() → 文件不存在 → 空状态
          或 v5_writes_disabled == true 且 DB 无当前快照 → 回落 read_state() → 空状态
     → merge_hive 拿不到 system_snapshot/user_snapshot
     → **用户禁用过的 PATH 条目全部显示为启用**
```

这正是 `gui/src/lib.rs:40-49` 那段注释写明要避免的形态：
「文件已停写且不存在 → 空状态 → 注册表里被禁用的条目会显示成启用」。

**建议修法**：把归档调用**挪到 `setup_database` 末尾**（⑤ 注册 `set_current_snapshot_loader` 与
`set_v5_writes_disabled(true)` 之后，⑥ 派生表重建之后）。理由：

- spec §7.4 只要求「迁移成功后归档」，没有要求「迁移后立刻归档」；
- 排到最后意味着**所有可能失败的步骤都跑完了**才动用户文件，任何中途失败都保留兜底；
- 与 Task 6 的 `prune`（同样在启动序列里、同样只应影响已稳定的状态）位置一致。

**连带建议**：归档前再判一次 `load_state_from_db()` 是否可用，不可用则跳过并 warn。
这一条可选——挪到末尾已经消除了主要风险。

> **待裁断**：同意挪到 ⑤/⑥ 之后吗？若你坚持 ③c，我需要你回答「③c 之后、⑤ 之前的失败
> 如何保证禁用状态不丢」——我目前只能想到「把 `disabled.json` 复制而不是改名」，那会违反
> 「不删除/不覆盖用户文件」的边界。

---

## 2. 建议同轮修（不阻塞）

### C-1 `maintenance prune --dry-run` 并非纯读

**计划原文**（Task 6 Step 5）：`maintenance_ops::open_db()` 用 `core::db::open_and_migrate()`。

**仓库事实**：`core/src/db/mod.rs:254` 的 `open_and_migrate()` 除了建库 + `migrate_to_current()`，
**还会执行 `migrate_v5::migrate_legacy(&mut db, &home)`**。因此在没有库的机器上，
`patheditor maintenance prune --dry-run` 会：

1. 创建 `~/.patheditor/…db`；
2. 建表；
3. **把 v5 文件导入 DB**。

dry-run 于是产生了真实写入。

**建议**：`maintenance prune` 改用 `open_default()? + migrate_to_current()`，dry-run 与实跑同一入口。
（实跑也不需要 v5 导入——库里没有变更时 `prune` 是空操作。）

### C-2 `cleanup-legacy-backups --dry-run` 会创建密钥文件

**计划原文**（Task 8 Step 4）：先 `load_or_create_key(config_dir())`，再判 `dry_run`。

**事实**：`load_or_create_key` 在密钥不存在时会**新建** `~/.patheditor/keys/db.key.dpapi`；
而 dry-run 分支（`cleanup_legacy_env_backups` 的 `dry_run == true`）**根本不用 `material`**。

**建议**：把密钥加载移到 `if !dry_run` 分支内，或先 `if dry_run { … return }`。
与开工提示词 §三.6「所有验证只用 TempDir」的精神一致：dry-run 不该有磁盘副作用。

### C-3 `unique_name_tx` 的可见性自相矛盾

Interfaces 写 `pub(crate)`，Step 3 的代码块写 `fn`。它只在 `profiles.rs` 内部被调用。
**建议保持私有**（同时满足「每个新增 `pub fn` 必须有生产调用者」的严格口径）。

### C-4 Task 4 需要反向验证，否则可能是假绿

Task 4 的测试在 Task 1 之后写，**天然不会经历 RED**。它对「实体级 LWW」的守卫能力
只有在「把 `latest_for_entity_tx` 换回 `latest_for_tx`」时才能证明。

**建议**：Task 4 完成后做一次反向验证并记录输出——
临时把 `"snapshot" | "profile"` 分支改回 `latest_for_tx(...)`，确认
`db_sync_profile_resurrect` 在 `assert_eq!(report.skipped, 1)` 处失败，然后还原。
这与 M3 的两条反向验证（AAD 收窄、`mark_exported` 提前）同一口径。

### C-5（小）`db_sync_conflict.rs:26` 的注释会过期

注释写「`profile` 属于『格式合法但当前版本不落库』的类型」。M4 起不成立。
建议 Task 1 顺手更正注释，**不动断言**（该测试本就用 `snapshot`）。

---

## 3. 观察项（照做，但请写进回执）

### 3.1 改名后缀是**持久化的中文**

spec §6.5.1 规定 `"{name} (来自 {deviceId 前 8 位})"`，计划照做。但这条字符串会
**落进 `profiles.name`**——英文界面下用户的配置名里会出现中文「来自」。
这是 spec 的明确选择，我不改；建议在回执「残余风险」里记一条：本地化字符串进入持久数据，
将来要处理需要数据迁移。

### 3.2 `sync.noticeImportedUnsupported` 在 M4 后不可达

`APPLICABLE_ENTITY_TYPES` 覆盖全部已知类型后，`skippedUnsupported > 0` 只会在
「未来新增类型」时出现。保留 i18n 键无害，**不删**（删除需要用户同意，且删了将来还得加回来）。
回执记一句即可。

### 3.3 `prune` 没有 `--yes`

spec §6.5.3 的接口表只给 `cleanup-legacy-backups` 配了 `--yes`，但同节正文写
「清理类命令在未给 `--yes` 时打印计划并以 1 退出」，§6.5.4 又写「删除类操作必须显式确认」。

我按**接口表**理解：`prune` 是保留策略（自动、只删已导出且到期的行），不需要 `--yes`；
需要 `--yes` 的只有 `cleanup-legacy-backups`（它删的是用户的明文文件）。
计划与我的理解一致。**若你认为 `prune` 也要 `--yes`，请在这一轮说**——它会影响 GUI 启动序列
（GUI 不交互，就只能自动执行，那 `--yes` 只对 CLI 有意义）。

---

## 4. 我核对通过的强项

以下每一条都对照过仓库实际代码，**不是通读计划的印象**。

| #   | 计划的断言                                                                                       | 仓库事实                                                                                                       |
| --- | ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------- |
| 1   | 测试 helper：`load_or_create_key` / `key_hex`                                                    | `core/src/db/crypto.rs:59` / `:30`，签名一致                                                                   |
| 2   | `Db::open_encrypted(path, &key_hex)` / `migrate_to_current`                                      | `core/src/db/mod.rs:44` / 存在                                                                                 |
| 3   | `query_row_with` / `with_transaction`                                                            | `core/src/db/mod.rs:169` / `:155`，闭包签名 `FnOnce(&Transaction) -> Result<T, CoreError>` 一致                |
| 4   | `seal(passphrase, &header, payload)` / `BundleHeader` 五字段                                     | `core/src/db/sync/bundle.rs:86` / `:53`，字段名与顺序全对                                                      |
| 5   | `ChangeRow` 13 个字段、`r.get(0..12)` 的顺序                                                     | `core/src/db/sync/change_log.rs:37-64`，与 `latest_for_tx` 现有 mapper 逐字段一致                              |
| 6   | `insert_snapshot_with_uid_tx(tx, snap, uid, false)`                                              | `core/src/db/history.rs:257/259` 的调用形态，签名 4 参                                                         |
| 7   | `NewSnapshot { kind, source, system, user, env, parent_uid }`                                    | `core/src/db/history.rs:133-145`，六字段全对                                                                   |
| 8   | `EventSource::{Gui, Import, Sync}`                                                               | `core/src/history.rs:12-26`                                                                                    |
| 9   | `PathEntry { path, enabled }` 可反序列化                                                         | `core/tests/db_sync_import.rs` 的现有夹具已在用同一形状                                                        |
| 10  | `profiles` 表有 `uid/name/description/tags/created_at/updated_at/apply_count/version/deleted_at` | `core/src/db/schema.rs:80-93`，且 `uq_profiles_name_active` 确实存在（部分唯一索引）                           |
| 11  | `profile_revisions` 有 `UNIQUE(profile_id, version)`                                             | `core/src/db/schema.rs:95-103`，`ON CONFLICT(profile_id, version)` 可用                                        |
| 12  | `snapshots` 有 `deleted_at` 与 `updated_at NOT NULL`                                             | `core/src/db/schema.rs:13-25`，Task 3 的墓碑 SQL 能跑                                                          |
| 13  | `changes` 没有 `(device_id, seq)` 唯一约束                                                       | `core/src/db/schema.rs:192` 只有**普通索引**，Task 6 测试里两条同 seq 的行可以共存                             |
| 14  | 导出侧已为 `profile` 提供 `content`                                                              | `core/src/db/sync/export.rs:106-114`，且 `ProfileView` 的 camelCase 字段覆盖 `ProfileSyncContent` 所需全部字段 |
| 15  | 已删除配置导出时 `content == None`（墓碑形状）                                                   | `export.rs:107` 的 `get_profile` 对已删除行返回 `None`                                                         |
| 16  | `write_env_backup_pebak(dir, payload, material)` / `read_env_backup_any(path, material)`         | `core/src/db/backups.rs:88` / `:206`；`.pebak` 文件名用**写入时刻**，不会与 `.json` 撞名                       |
| 17  | Task 8 的手工解析与 `read_env_backup` 同口径                                                     | `core/src/backup.rs:799-802` 就是 `read_versioned_file + migrate`，只多了损坏隔离                              |
| 18  | `Versioned` / `PERSIST_SCHEMA_VERSION` / `migrate` 为 `pub(crate)`                               | `core/src/persist.rs:30/38/83`；Task 8 的测试在 crate 内，可用                                                 |

补充核对（同样成立）：

- `EnvBackupPayload` / `EnvBackupHives` / `EnvBackupVar` 均派生 `PartialEq` → Task 8 的 `assert_eq!` 成立
  （`core/src/backup.rs:170-205`）。
- `chrono` 已在 `cli/Cargo.toml` → Task 6/7 的 CLI **不需要新增依赖**。
- `open_and_migrate()`（`core/src/db/mod.rs:254`）、`runtime::exit_core_error`（`cli/src/runtime.rs:22`）、
  `profile_ops::open_db`（`cli/src/profile_ops.rs:16`）、`db_profiles` 导入均存在。
- 前端 `renderNotice` 的 `switch` 形状、`getByRole('status')`、i18n `sync.*` 命名空间
  与 Task 2 的改法一致（`src/components/sync/SyncCenter.tsx:15-41`、`sync-center.test.tsx:159`）。
- 四处版本号当前均为 `5.1.7`；README 有 2 处（徽章 + 安装包名），与 Task 11 的清单一致。
- `core/src` 内联 `#[cfg(test)]` 已有 211 个 `#[test]` 且**没有** SQLCipher 测试，
  Task 8 把 DPAPI 测试放进 `maintenance.rs` 的内联模块**不会**触发「一进程两个 SQLCipher」。

---

## 5. 依赖与基线核实

| 项          | 开工提示词声明       | 我的实测                                                                        |
| ----------- | -------------------- | ------------------------------------------------------------------------------- |
| 分支 / HEAD | `main` / `a65f650`   | `main` / `435bc76`（开工提示词提交后前移一位）                                  |
| 版本号      | 5.1.7                | `package.json` / `gui/tauri.conf.json` / `Cargo.toml` 均为 5.1.7                |
| 工作区      | 干净                 | 干净（`git status --short` 无输出）                                             |
| 前端        | 33 files / 397 tests | **33 / 397，全绿**                                                              |
| Rust        | 340 passed           | 见 §5.1                                                                         |
| E2E         | 54 passed            | 未在本轮重跑（避免为核对跑一次 40 分钟的构建）；以 Task 11 的 `verify:all` 为准 |

### 5.1 Rust 基线

```text
$ cargo test --workspace
test result 行数（即 test binaries）= 48
passed = 340   failed = 0   ignored = 2
exit code 0
```

与开工提示词 §二 声明的 **48 binaries / 340 passed / 0 failed / 2 ignored** 逐项一致，
**基线确认通过**。前端同为 33 files / 397 passed。可以按该基线开工。

---

## 6. 建议的修订顺序

1. **你裁断 S-1 的四个数字**（改写 `db_sync_import_limits.rs` 的口径）。
2. **你裁断 S-2 的归档位置**（我建议挪到 `setup_database` 末尾）。
3. 计划文本落上 C-1 / C-2 / C-3 / C-4 / C-5。
4. 我在同一份文件里回一封「核对轮答复」，然后开工 Task 1。

若你同意 S-1 / S-2 且不需要改计划文本（只让我在 Execution Notes 里记偏离），
直接告诉我「按你的建议执行」即可，我会：

- 把 S-1 写进 Task 1 的提交（测试改写 + 注释更正）；
- 把 S-2 写进 Task 7 的提交（位置改到末尾），并在 Execution Notes 的偏离表里写明
  「计划原文 / 实际 / 处理」。

---

## 6.1 核对轮答复（2026-10-21，用户转达）

用户就两条阻塞项作出裁断，**均按开发窗口的建议执行**：

| 编号 | 裁断                                                                                                                    | 落地位置                               |
| ---- | ----------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| S-1  | **按 §1.S-1 建议的四个断言改写 `db_sync_import_limits.rs`**；保留「未知类型整包拒绝」那半；不删文件；同时更正文件头注释 | Task 1 的提交                          |
| S-2  | **把 `archive_v5_sources` 调用挪到 `setup_database` 末尾**（⑤ `set_v5_writes_disabled(true)` 与 ⑥ 派生表重建之后）      | Task 7 的提交 + Execution Notes 偏离表 |

其余 4 项建议（C-1 ~ C-4）与 1 项小更正（C-5）**一并按建议执行**，
每项在对应 Task 的提交里体现，并在 Execution Notes 里逐条登记：

| 编号 | 处理                                                                           | 落在哪个 Task |
| ---- | ------------------------------------------------------------------------------ | ------------- |
| C-1  | `maintenance prune` 改用 `open_default() + migrate_to_current()`，不做 v5 导入 | Task 6        |
| C-2  | `cleanup-legacy-backups --dry-run` 不加载密钥（密钥只在实跑分支需要）          | Task 8        |
| C-3  | `unique_name_tx` 保持私有（`fn`），不写 `pub(crate)`                           | Task 2        |
| C-4  | Task 4 完成后做反向验证：临时换回 `latest_for_tx`，确认测试转红再还原          | Task 4        |
| C-5  | 更正 `db_sync_conflict.rs` 第 26 行「profile 不落库」的过期注释                | Task 1        |

观察项 3.1 / 3.2 / 3.3 **不改代码**，原样写进开发回执的「残余风险」与「未覆盖项」。

---

## 7. 本次核对**没有**做的事

- 没有改任何 `docs/superpowers/plans/` 或 `docs/superpowers/specs/` 下的文件——那是你的产物。
- 没有写产品代码、没有提交、没有推送、没有升级版本号。
- 没有碰注册表，没有碰真实 `~/.patheditor`。
- 没有跑 E2E（理由见 §5）。
- 没有验证 MSVC 工具链（本机是 GNU，CI 不跑测试）。

---

## 8. 一条方法论观察

M3 的核对轮抓到的是「**计划漏了接线**」（`record_change_tx` 零调用者）。
M4 这次抓到的是另一种形态：**计划漏了「被本次变更淘汰的守卫测试」**。

两者的共同点：**计划只描述增量，不描述增量的反作用力**。
`db_sync_import_limits.rs` 不是写错了，它当年完全正确——它是 M3 为 R-1 专门加的守卫。
Task 1 把 `profile` 变成可落库后，这个守卫就从「保护正确行为」变成「固化错误行为」。

可复用的检查动作只有一条：**任何改变既有行为的 Task，落笔前先
`grep` 一遍被改符号在 `core/tests/` 里的全部命中**。
本轮 `grep -rn "skipped_unsupported" core/tests/` 一次就命中了。

---

**签章**：本文件中每条「仓库事实」都注明了 `file:line`，可直接复核。
标为「建议」的都是我未执行的改动；标为「待裁断」的需要你回答。
没有推测性结论。
