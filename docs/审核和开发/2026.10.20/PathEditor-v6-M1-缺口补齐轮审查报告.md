# PathEditor v6.0.0 M1 — 缺口补齐轮审查报告

- **审查日期**：2026-10-20
- **被审对象**：工作区未提交改动（41 项）+ 8 项缺口整改
- **基准点**：`main @ 9e674ec`；branch `v6/m1-storage-history` @ `be88f35`（本轮回执所指提交尚不存在，改动全在工作区）
- **本轮依据**：`PathEditor-v6-M1-缺口清单.md`（下称清单）
- **规格来源**：`specs/2026-10-19-v6-sqlite-design.md`（下称 spec）

---

## 一、验证证据（审核窗口独立执行）

| #   | 命令                                                    | 结果                                               |
| --- | ------------------------------------------------------- | -------------------------------------------------- |
| 1   | `cargo fmt --all -- --check`                            | exit 0                                             |
| 2   | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0                                             |
| 3   | `cargo test --workspace`                                | **20 binaries，311 passed / 0 failed / 2 ignored** |
| 4   | `npm run verify`                                        | exit 0                                             |
| 5   | `npx vitest run`                                        | 27 files / **336** tests                           |
| 6   | `npm run test:e2e`                                      | **44 passed**                                      |
| 7   | `e2e/tests/history.spec.ts`                             | 3 条：空态 / 降级条幅 / 展开详情                   |

> 计数用显式字段拆分（`passed`/`failed`/`ignored` 分开累加），未手算。311 = 230 core lib + 15 个集成文件（各 1）+ 66 cli。

---

## 二、八项逐条复核

| #    | 项                         | 判定                          | 证据                                                                                                                                                                                                                                                                                                                                  |
| ---- | -------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0-1 | 配置方案接通 DB            | ✅ **完成**                   | `use-profiles.ts:19,33,49,88,101` 全部改调 `profileList`/`profileSave`/`profilePaths`/`profileDeleteByName`/`profileRename`；`commands/profiles.rs` 的 v5 命令**已从 `invoke_handler` 下线**（`grep` 确认零注册），并加了 `#![allow(dead_code)]` 与「不要重新注册」的醒目注释；CLI `profile_ops.rs:54` 走 `db_profiles::save_profile` |
| P0-2 | `snapshotUid` 回填         | ✅ **完成**                   | `DbHistory::with_snapshot`（`history_sink.rs:44`）+ `with_history_and_snapshot`（`service.rs:48-96`）在命令层建「操作前」快照并绑给事件                                                                                                                                                                                               |
| P0-3 | `history_restore_snapshot` | ✅ **完成**                   | `gui/src/commands/history.rs:290-323`，走既有 `save_path_with_sidecar`，append-only 由 `with_history_and_snapshot` 保证；已注册（`lib.rs:147`）                                                                                                                                                                                       |
| P1-1 | `EventFilter` 补 4 维      | ✅ **完成**                   | `hive`/`target`/`result`/`from_ms`/`to_ms` 五字段齐备（`history.rs` 的 `EventFilter`）                                                                                                                                                                                                                                                |
| P1-2 | 备份索引接线               | ✅ **完成**                   | 启动时（`lib.rs:42`）+ `backup_list` 每次调用前增量（`history.rs:332`）                                                                                                                                                                                                                                                               |
| P1-4 | v5 文件停写                | ⚠️ **部分完成 —— 见缺陷 D-1** | `set_v5_writes_disabled` + `gui/src/lib.rs:57` 已接；但**读路径未同步**                                                                                                                                                                                                                                                               |
| P1-5 | 修正错误引用               | ✅ **完成**                   | `migrate_v5.rs:7-11` 明确写「spec §7.4 是迁移规则本身，不是对 M4 的授权」，并把「实现取舍」与「规格要求」分开表述                                                                                                                                                                                                                     |
| P1-6 | 历史面板 E2E               | ✅ **完成**                   | `e2e/tests/history.spec.ts` 3 条，44 passed                                                                                                                                                                                                                                                                                           |
| P1-7 | 跨会话撤销                 | ✅ **完成**                   | `history-store.ts:124` 的 `undoLast`，UI 入口 `HistoryPanel.tsx:133`，4 条单测                                                                                                                                                                                                                                                        |

**八项中七项完整落地，一项有缺陷。**

---

## 三、缺陷（新发现，均未在回执中登记）

### D-1【阻塞】`v5_writes_disabled` 只关了写，没关读 —— 禁用状态会永久冻结

**位置**：`core/src/disabled.rs:102-108`（写路径）、`:132-155`（读路径 `read_state`）、`:236-262`（`load_path_snapshot`）

**机制**

`write_state` 在停写时直接返回 `Ok(())`（`:105-108`）：

```rust
fn write_state(state: &DisabledState) -> Result<(), CoreError> {
    if v5_writes_disabled() {
        log::debug!("v5 文件停写已启用，跳过 disabled.json 写入");
        return Ok(());
    }
    ...
}
```

但 `read_state()`（`:132`）**完全不检查这个标志**，仍从 `disabled.json` 读：

```rust
fn read_state() -> Result<DisabledState, CoreError> {
    let path = disabled_file_path();
    ...
}
```

而 `load_path_snapshot()`（`:236`）是**所有 PATH 读取的主入口**：

```rust
pub fn load_path_snapshot() -> Result<PathSnapshot, CoreError> {
    let registry_system = registry::load_system_paths()?;
    let registry_user = registry::load_user_paths()?;
    let state = read_state()?;            // ← 读的是冻结的旧文件
    Ok(PathSnapshot {
        system: merge_hive(registry_system, state.system, state.system_snapshot),
        ...
    })
}
```

**后果链**

1. 迁移完成 → `set_v5_writes_disabled(true)`。
2. 用户禁用一个 PATH 条目（`enabled = false`）→ `service.rs:252` 调 `disabled::save_path_snapshot(...)` → 内存里算好了新状态 → `write_state` **静默丢弃**。
3. 用户重新加载 PATH → `load_path_snapshot` → `read_state` 读**迁移那一刻的旧快照** → `merge_hive` 按旧快照的 `enabled` 值重建。
4. 用户看到的界面：**禁用操作没有生效**（或更糟——被旧快照复活成"启用"）。

**为什么这会稳定复现**：`merge_hive` 的输入是「注册表当前 PATH」+「快照里的 enabled 标记」。注册表里 PATH 是字符串，**没有启用/禁用位**——禁用完全是 sidecar 概念。sidecar 冻结 = 禁用信息永久丢失。

**P1-4 的前置条件没有真正满足**。回执 §2 说「按你的决定先补了『当前快照进 DB』，再做停写」——`update_current_snapshot` 确实写了 `app_state.current_snapshot_id`，但**没有任何代码从 DB 读回来**。`load_path_snapshot` 仍然是 v5 文件的读者。

**验收判据**

二选一，必须明确：

- **(A) 补读路径**：`load_path_snapshot` 在 `v5_writes_disabled()` 为真时改从 `app_state.current_snapshot_id` 对应的 `path_entries` 读。这需要给 `disabled` 模块注入 `&Db`（或把 `load_path_snapshot` 上移到 service 层），是一次**结构性改动**。
- **(B) 回退 P1-4**：撤掉 `set_v5_writes_disabled(true)` 的调用，恢复双写。清单里 P1-4 的前提「DB 里有当前 PATH 的落点」在没有读路径时**不成立**。

**我倾向 (B) 本轮回退、M2 做 (A)**：(A) 涉及 `disabled` 模块与 `Db` 的耦合，不该在缺口补齐轮里仓促做；而 (B) 只是删一行，风险为零。带着 D-1 合并会让「禁用 PATH」这个**核心功能**在 v6 上静默失效。

---

### D-2【应改】迁移把「最新快照」当作当前状态，可能是 backups 快照

**位置**：`core/src/db/migrate_v5.rs:79-82`

```sql
INSERT INTO app_state (id, current_snapshot_id, version, migrated_v5, device_id, lamport)
VALUES (1, (SELECT id FROM snapshots ORDER BY created_at DESC, id DESC LIMIT 1), ...)
```

**机制**：导入顺序是 `disabled` → `profiles` → `env_backups` → `path_backups`（`:72-75`）。`disabled` 产生 `kind=Current`（`:147`），后三者产生的都是 `kind=Profile`（`:214`）或 `kind=Backup`（`:330`、`:395`）。

`created_at` 是**插入时刻**（`history.rs:249` 的 `Utc::now().timestamp_millis()`），不是数据本身的时刻。所以「最后插入的那条」= `path_backups` 的最后一份备份快照，**不是当前状态**。

**后果**：迁移后的 `app_state.current_snapshot_id` 指向一份历史备份，而不是「现在的 PATH」。在 D-1 修复（读路径切到 DB）之后，这会让用户一启动就看到几周前的 PATH 状态。

**当前是否已造成伤害**：`current_snapshot_id` 目前**没有任何读路径**（D-1），所以是潜伏缺陷。但 D-1 一旦按 (A) 修复，D-2 立即变成阻塞。

**验收判据**：改为按 `kind = 'current'` 选取，且必须限定是 `import_disabled` 产生的那一条：

```sql
(SELECT id FROM snapshots WHERE kind = 'current' ORDER BY created_at DESC, id DESC LIMIT 1)
```

若无 current 快照（如没有 `disabled.json`），应留 `NULL` 并在启动时按需创建，而不是随便挑一条。

---

### D-3【应改】`hidden` 的 v5 配置命令与 `#![allow(dead_code)]`

**位置**：`gui/src/commands/profiles.rs:13`

`#![allow(dead_code)]` 是**文件级**的 `allow`，会把这个文件里**将来新增的**真死代码也一并豁免。当前这个文件确实全部是保留用途，但更精确的写法是对每个函数加 `#[allow(dead_code)]`，或加 `#[cfg_attr(not(test), allow(dead_code))]`。

**严重性**：低。文件已用 12 行注释说明「为什么保留、不要重新注册」，判读成本可控。**不阻塞**，但清单要求「查是否有死导出」——这是本轮唯一一处主动豁免的。

---

### D-4【判断性】`update_current_snapshot` 的 CAS 失败静默放弃

**位置**：`gui/src/commands/service.rs:106+`

回执 §「两个取舍」第 2 条说：「CAS 失败时放弃而非重试。理由：历史仍然正确，只是当前指针没跟上，下次操作会重新对齐。」

**我同意这个取舍**，但补一条：既然 `current_snapshot_id` 是 D-1 修复后「PATH 当前状态的唯一真相」，**放弃更新 = 真相停滞**。当前的「下次操作会重新对齐」在没有读路径时无法验证。

建议：CAS 失败时至少 `log::warn!`（当前是否记录需确认），并在 M2 做 D-1(A) 时重新评估——那时「指针落后」的代价从「无影响」变成「用户看到旧 PATH」。

---

### D-5【记录】`.claude/skills/` 两份准则文件被修改

**位置**：`.claude/skills/patheditor-review-window/SKILL.md:128`、`.claude/skills/patheditor-two-window-flow/SKILL.md:125`

改动内容是把「必须字节级一致」改为「不要求逐字节一致；若选择同步，判据必须是字节级比较」，并补了 `cmp` 假阳性、`prettier --check` 假阴性的说明。

**内容质量**：改得比我原本的建议更好——它保留「若要声称同步，判据必须可靠」这个实质约束，只放开了「必须同步」这条形式约束。`cmp` 会忽略 CRLF、`prettier --check` 会归一化比较，这两条正是我这轮踩过的坑。

**但**：这两份文件是**审核窗口的行为准则**，按「未经明确指令不得修改」的精神应由用户拍板。回执未在「未提交」清单里单独标注，我是在 `git status` 里发现的。

**待用户确认**：这两份改动保留还是回退？

---

## 四、三处「超出清单的判断」复核

回执列了三处，我逐条判断：

### 1. P0-1 必须同时改 CLI —— ✅ **判断正确且必要**

清单只写了 UI。但 `cli/src/profile_ops.rs` 有完整的 profile 子命令，只改 GUI 会把双真相源从「GUI 文件 vs DB」变成「CLI 文件 vs DB」。回执主动扩大范围是对的，且 `profile_save` / `profile_load` / `delete` / `rename` 在 CLI 侧已全部切到 `db_profiles::*`（`profile_ops.rs:54,115,129`）。

**这是本轮最值得记的一个判断**：清单的措辞是「UI 改调 DB 版」，如果严格照字面执行，会留下同样的缺陷。开发窗口读了清单的**意图**而不只是字面。

### 2. P1-4 被前置依赖阻塞 —— ⚠️ **前置条件只满足了一半**

回执说：「DB 里没有『当前 PATH 状态』的落点……按你的决定先补了『当前快照进 DB』，再做停写。」

**写**的落点补上了（`update_current_snapshot`），**读**的落点没有（D-1）。所以「前置依赖已解除」这个判断**不成立**。

这不是判断错误，是**验证范围不足**：补了写就认为状态有了，没有验证「谁来读」。这与 R-05 事故是同一类型——识别了「需要落点」，没核对到「每个方向」。

### 3. P1-2 不只在启动时索引 —— ✅ **判断正确**

只在启动时索引会漏掉「启动后新做的备份」。改成每次 `backup_list` 前增量索引（幂等，成本是目录 stat）是对的，且注释写清了为什么。

---

## 五、两处取舍复核

### `AtomicBool` 而非 `OnceLock` —— ✅ 合理

「生产路径只设一次，但测试要验证开/关两态」是正当理由。`Ordering::Relaxed` 对这个用途足够（只做布尔标志，无跨变量顺序依赖）。

### `update_current_snapshot` CAS 失败放弃 —— ✅ 合理，但见 D-4

---

## 六、总结

### 判定

**8 项中 7 项完整落地，P1-4 有阻塞缺陷（D-1），另有 1 项潜伏缺陷（D-2）会在 D-1 修复后立即变为阻塞。**

**建议：D-1 修复后可合并，其余可延后。**

### 合并前必须处理

1. **D-1**：`v5_writes_disabled` 只关写没关读。二选一：
   - **(B) 回退 `lib.rs:57` 的 `set_v5_writes_disabled(true)` 调用**（一行，零风险，我推荐本轮这么做）；
   - **(A) 补读路径**（结构性改动，建议 M2）。

   **带着 D-1 合并会导致「禁用 PATH」静默失效。**

2. **D-2**：迁移的 `current_snapshot_id` 选取改为 `kind = 'current'`。若本轮做 (B)，此条降为「应改」；若做 (A)，此条与 D-1 同为阻塞。

3. **D-5**：`.claude/skills/` 的两份改动需要用户确认保留或回退。

### 可以合并后做

- D-3（`#![allow(dead_code)]` 收紧）
- D-4（CAS 失败日志 + M2 重评）
- 清单里的 P2 五项（已在清单登记，M2 处理）

### 合规亮点

- **`commands/profiles.rs` 的处理方式值得记**：不是简单删除，而是 `#![allow(dead_code)]` + 12 行注释说明「为什么保留、不要重新注册、which future feature needs it」。这把「死代码」变成了「有明确用途的保留代码」，比直接删更稳妥，比留着不管更清晰。
- **`migrate_v5.rs:7-11` 的引用更正是教科书级的**：不只改对了章节号，还把「spec 要求什么」与「我为什么推迟」分开写。这防止了下一轮再被误读。
- **P0-1 主动扩到 CLI**：读懂了清单的意图而不只是字面。
- **P1-2 的增量索引**：注释里写清了「只在启动时索引会漏什么」，是一个可被证伪的设计说明。

### 本轮暴露的模式

**D-1 与 R-05（上一轮的测试污染事故）是同一个失效模式**：识别了「需要做什么」，没核对到「每个方向都做了」。R-05 是「识别了风险，没把防范核对到每个路径来源」；D-1 是「识别了需要落点，只补了写没补读」。

建议在开发窗口的自查清单里加一条：**任何「写路径改为 X」的改动，必须同时回答「读路径改了吗」**。这类改动是成对的，写一个查一个不够。

---

## 七、本轮未执行（如实列出）

- **真实 GUI 端到端手工验证**：未做。历史面板的「保存 → 出现事件 → 展开详情 → 恢复」完整链路只有 E2E mock 证据。
- **真实注册表写入**：未做（需用户显式授权）。
- **CI（MSVC）**：未推分支，未跑。
- **D-1 的运行时复现**：未做。我的判定基于代码阅读（`read_state` 不检查 `v5_writes_disabled`），未实际构造「禁用条目 → 重启 → 观察丢失」的场景。**若要作为阻塞依据，建议开发窗口先用一个测试复现**。
