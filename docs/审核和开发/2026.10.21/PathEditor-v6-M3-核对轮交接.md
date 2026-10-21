# PathEditor v6.0.0 M3 计划核对 — 开发窗口提示词

> **给审核窗口**：本文件是开发窗口对 M3 计划的核对结果，供你决定是否修订计划。
> 我**未写任何代码**，未提交、未推送、未升级版本号、未碰注册表。

- **核对对象**：`docs/superpowers/plans/2026-10-21-v6-sqlite-m3-plan.md`（684 行，11 个 Task）
- **对照权威**：`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` §4.1 / §4.2 / §5.2 / §6.3 / §6.4 / §8
- **判定**：**方向正确，但有 2 处会阻塞开工，建议先修。**
- **日期**：2026-10-21

---

## 0. 结论摘要

| 类别                                       | 数量 | 是否阻塞   |
| ------------------------------------------ | ---- | ---------- |
| **阻塞**（照写必然无法实现或产出错误结果） | 2    | **是**     |
| 规格缺口（你已标记，我的答复见 §3）        | 3    | 需你裁断   |
| 内部矛盾 / 事实错误                        | 4    | 建议同轮修 |
| 我核对通过的强项                           | 6    | —          |

**最严重的一条**：Task 2 的 `record_change_tx` **零调用者**——计划里没有任何 Task 把「写变更」接进生产写路径。
这是 M1 的 V-3、M2 的 `index_snapshot_tx` 之后的**第三次**同一形态缺陷，
而这次它出现在**计划阶段**，不是实现阶段。

---

## 1. 阻塞项

### S-1 `record_change_tx` 没有生产调用者（第三次重演）

**事实**：

```bash
$ grep -n "record_change" docs/superpowers/plans/2026-10-21-v6-sqlite-m3-plan.md
243:  - `pub fn record_change_tx(tx: &Transaction, c: NewChange) -> Result<String, CoreError>` — 事务内写入
255:    current_lamport, mark_exported, pending_export, record_change_tx, NewChange,
269:            record_change_tx(          ← Task 2 的测试里
283:            record_change_tx(          ← Task 2 的测试里
316:- **`record_change_tx` 在事务内调用**，不自开事务（避免嵌套）。
```

**全部命中都在 Task 2 自己的测试里，没有任何生产代码调用它。**

**后果（比前两次更严重）**：`changes` 表永远是空的 → `pending_export()` 永远返回空 →
`export_bundle` 导出 0 条变更 → 同步功能**整条链路空转**，而所有单测都会通过
（因为测试自己调 `record_change_tx` 造数据）。

**根因**：计划的 11 个 Task 里，**没有一个负责「把写操作接进变更日志」**。
Task 4（导出）假设 `changes` 里已经有数据，但没人负责产生它。

**修法建议**：新增一个 Task（或并入 Task 2），明确列出**每个写入口**要记哪些变更：

| 写入口                                                 | 位置                          | 要记的变更                            |
| ------------------------------------------------------ | ----------------------------- | ------------------------------------- |
| `save_path_with_sidecar`                               | `core/src/service.rs`         | snapshot + path_entries               |
| `save_profile`                                         | `core/src/db/profiles.rs`     | profile + profile_revision + snapshot |
| `delete_profile` / `rename_profile`                    | 同上                          | profile（含墓碑）                     |
| `update_env_var` / `create_env_var` / `delete_env_var` | `core/src/registry/`          | env_value                             |
| `history_restore_snapshot`                             | `gui/src/commands/history.rs` | 经 `save_path_with_sidecar`，不重复记 |

**建议的检查步骤**（按 M3 计划 §Global Constraints 的方法，但**先验证工具本身**）：

```bash
# 0. 先用已知有调用者的函数验证 grep 模式有效
grep -rn "rebuild_daily(" --include=*.rs core/src gui/src cli/src | grep -v "fn rebuild_daily"
#    必须 ≥1 命中，否则模式本身有问题

# 1. 再查目标函数
grep -rn "record_change_tx(" --include=*.rs core/src gui/src cli/src | grep -v "fn record_change_tx"
#    结果里必须出现非测试文件
```

⚠️ 不要用 `\b` 词边界——它匹配不到 `change_log::record_change_tx` 这类带模块前缀的调用。

### S-2 Task 5 的测试代码无法编译：`ChangeRow` 未定义

Task 5 的测试：

```rust
use path_editor_core::db::sync::merge::{resolve, MergeDecision};

fn change(lamport: i64, device: &str, value: &str) -> ChangeRow { /* 构造辅助 */ }
```

**`ChangeRow` 在整个计划里没有任何定义。** Task 2 的 Interfaces 只列了：

```text
pub fn record_change_tx(tx, c: NewChange) -> Result<String, CoreError>
pub fn pending_export(db) -> Result<Vec<ChangeRow>, CoreError>   ← 只在这里出现，无定义
pub fn mark_exported(db, uids, at) -> Result<usize, CoreError>
pub fn current_lamport(db) -> Result<i64, CoreError>
pub struct NewChange { ... }                                      ← 只有 NewChange 有定义
```

`ChangeRow` 被 `pending_export` 的返回类型、Task 5 的 `resolve` 参数、Task 6 的导入逻辑引用，
但**从未定义**。Task 5 的测试里那句 `/* 构造辅助 */` 是注释，不是实现。

**修法**：在 Task 2 的 Interfaces 里补 `ChangeRow` 的完整定义（字段、类型、是否 `Serialize`）。
建议与 spec §6.4 的 `Change` JSON 形状对齐：

```rust
pub struct ChangeRow {
    pub uid: String,
    pub device_id: String,
    pub seq: i64,
    pub lamport: i64,
    pub origin: String,
    pub entity_type: String,
    pub entity_uid: String,
    pub field: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub created_at: i64,
    pub applied_at: i64,
    pub exported_at: Option<i64>,
}
```

---

## 2. 内部矛盾与事实错误

### C-1 Task 3 的测试 `use` 路径与 File Structure 不一致

```rust
// Task 3 的测试
use path_editor_core::db::bundle::*;   // 路径按实际调整
```

但 File Structure（第 54 行）写的是 `core/src/db/sync/bundle.rs`，即模块路径应为
`path_editor_core::db::sync::bundle`。

「路径按实际调整」这种占位写法在计划里可以接受，但**同一份计划里两处写了不同的路径**，
实施者会先试错一次。建议统一成 `db::sync::bundle`。

### C-2 Task 7 与 spec §8 的退出码语义有偏差

Task 7 写「`Sync → exit 4`」，但 spec §8 的原文是：

> CLI 退出码：0 成功；1 一般错误；2 clap；3 revision 冲突（既有）；**4 同步/合并冲突**（新增）；**5 加密/密钥错误**（新增）。

spec 把 4 定义为「**同步/合并冲突**」，而 `ErrorCode::Sync` 在 spec §8 的另一处被描述为
「`Sync`（包/冲突）」——**即「包格式错误」与「合并冲突」共用同一个码**。

这会导致一个实际问题：**导入一个格式损坏的包（不是冲突，是文件坏了）也会返回 exit 4**，
脚本无法区分「需要人工解决冲突」与「文件传坏了」。

**建议**：明确 4 的语义边界。两个选项：

- **A**：4 只用于「合并冲突」（`sync_conflicts` 有未解决项），包格式错误用 1；
- **B**：4 用于两者，但在 CLI 的 stderr 上明确区分文案。

我倾向 A，因为脚本最需要区分的就是这两者。**这需要你裁断，因为它涉及 spec §8 的措辞。**

### C-3 Task 6 的「导入顺序」与 `events` 的依赖关系未说明

Task 6 写：

> 按依赖序应用：`snapshots → path_entries/env_values → profiles → profile_revisions → events`

但 `events` 表有 `snapshot_id INTEGER REFERENCES snapshots(id)`——
`events` 依赖 `snapshots`，顺序上放最后是对的。

**问题在于**：`events` 的 `uid` 是 `TEXT NOT NULL UNIQUE`，而 `changes.entity_uid` 指向它。
若远端事件的 `snapshot_id` 指向的快照**在本机不存在**（例如对方导出的包不含那个快照），
按 spec §6.4 的「悬空 uid → 整体回滚」，**整个导入会失败**。

而 `events` 是「本机操作历史」，它引用的快照很可能不在同步范围内（取决于 S-1 的修法）。

**建议**：明确 `events.snapshot_id` 在导入时找不到父快照的处理方式——是回滚，还是置 NULL。
spec 没写这一条。

### C-4 计划声明的测试文件数与 Global Constraints 的预估不符

Global Constraints 写：

> M2 结束时 `core/tests/` 有 31 个文件，M3 预计再加 **12–15 个**。

实际计划里只声明了 **9 个** `core/tests/*.rs`：

```
db_bundle_crypto / db_change_log / db_schema_v2_to_v3 / db_schema_v3 /
db_sync_conflict / db_sync_export / db_sync_import / db_sync_merge / error_exit_codes
```

差 3–6 个。这不是缺陷（少写测试比多写安全），但**预估与计划不符**会让回执的
「实际测试数」对不上预期。建议要么补足测试文件，要么把预估改成「9 个」。

---

## 3. 对你标记的三条规格缺口的答复

### 3.1 墓碑的表示方式（你的选择：只对有 `deleted_at` 的实体生效）

**我认同**，并补充两条事实：

1. 我核实过表结构：`snapshots`（`deleted_at INTEGER`）与 `profiles`（`deleted_at INTEGER`）
   有软删除列；`path_entries` 与 `env_values` **没有**，它们的主键是
   `(snapshot_id, hive, position)` / `(snapshot_id, hive, name)`，靠 `ON DELETE CASCADE` 随父快照消失。
2. **`profile_revisions` 也没有 `deleted_at`**，且它有 `uid`——计划的范围表里列了它，
   但墓碑规则没覆盖它。建议明确：`profile_revisions` 随 `profiles` 的软删除走，
   不做独立墓碑。

### 3.2 `events` 是否参与同步

**我建议不跟随 spec，改 spec。** 理由：

1. **`events` 是「本机操作日志」，不是「用户数据」。** 同步它的语义与同步 profile/PATH 完全不同——
   后者是「让两台机器的配置一致」，前者是「让两台机器的操作记录一致」。
2. **UI 目前不展示 `device_id`**（我核实过 `EventView` 有 `device_id` 字段但前端未渲染），
   所以设备 B 的用户会看到一堆「不是我做的」操作，且无法分辨来源。
3. **`events` 会无限增长**，同步它会让 bundle 体积随时间线性膨胀，而它同步过去**没有实际用途**
   （历史面板是只读的，不能「应用」一个事件）。

**但这是规格变更，需要你落笔改 spec §6.3。** 若你决定保留同步 events，
我建议至少在 Task 9 的 UI 里加上 `device_id` 的展示（否则用户无法理解看到的东西）。

**这一条我建议你在核对轮明确表态**——它影响用户对历史面板的信任。

### 3.3 冲突解决后是否追加新 change

**我认同追加**，理由：不追加的话，第三台设备永远不知道这次冲突被解决过，
下次同步会重新报同一个冲突。

**但需要明确一个细节**：追加的 change 的 `lamport` 是多少？

- 若用 `current_lamport() + 1`，那么它会**覆盖**冲突双方的值（LWW 语义）；
- 若用原冲突的 lamport，则它不会覆盖，其它设备仍会看到冲突。

我倾向前者（`current_lamport() + 1`），因为「用户已经做了选择」应当是一个**新的、更晚的决定**。
计划没写这一点，建议补。

---

## 4. 我核对通过的强项

记下来是为了让你知道我逐条核过，不是只挑刺：

1. **Global Constraints 的「先验证 grep 模式本身」**——这条写得好，
   而且它直接来自 M2 审查时你差点误判 `find_cycles` 的教训。**方法论的自我修正被固化了。**
2. **`PRAGMA user_version` 必须在所有分支之后**——写进了约束，且 Task 1 有专门的
   `db_schema_v2_to_v3.rs` 守它。**M2 的教训被正确继承。**
3. **Task 3 的 AAD 覆盖 header**——这是正确的设计。若 AAD 不含 header，
   header 里的 `change_count` / `device_id` 可被篡改而不被发现。
4. **Task 4 的「先写文件后标记」**——顺序正确，且计划明确写了「顺序不能反」及理由。
5. **Task 3 的测试覆盖了三种拒绝**（错误口令、篡改密文、篡改 header）——
   比只测「能往返」强得多。
6. **Task 5 的纯函数测试不建库**——计划注意到「不碰 SQLCipher 因此不受栈溢出约束」，
   这是对 M0 约束的正确理解。

---

## 5. 依赖核实

我核实了计划声明的两个新依赖：

| 依赖               | 计划声明       | 我的核实                                              |
| ------------------ | -------------- | ----------------------------------------------------- |
| `argon2`           | 南大镜像可获取 | ✅ 你说已实测；`Cargo.lock` 里目前**没有**（M3 新增） |
| `chacha20poly1305` | 同上           | ✅ 同上                                               |

**补充一条计划未提的**：Task 8 的 CLI 要求「口令用隐藏输入（不落盘、不进 shell history、
不出现在 `ps`）」。这需要一个新依赖（`rpassword` 或等价物），而 `cli/Cargo.toml` 目前**没有**它。
计划没有把 `rpassword` 列进「新增依赖」。**建议补上**，否则 Task 8 会临时发现缺依赖。

（`Cargo.lock` 里 `rpassword` 零命中，确认它不在现有依赖树里。）

---

## 6. 建议的修订顺序

1. **S-1**（`record_change_tx` 的调用者）——**最重要**，它决定整条链路是否空转
2. **S-2**（`ChangeRow` 的定义）
3. **C-2**（exit 4 的语义边界，需你裁断）
4. **C-3**（`events.snapshot_id` 悬空时的处理）
5. **C-1 / C-4**（机械修正）
6. **§3.2 的 `events` 是否同步**（需你裁断，可能改 spec）
7. **§3.3 的追加 change 的 lamport**（补一句）
8. **§5 的 `rpassword`**（补进依赖清单）

收敛后我按 `superpowers:subagent-driven-development` 逐 Task 开工。

---

## 7. 本次核对**没有**做的事

- **没有写任何代码**，没有建分支，没有提交。
- **没有实测 `argon2` / `chacha20poly1305` 的镜像可用性**——你已实测，我采信。
- **没有核对 spec 的 M4 部分**（§5.3 清理策略）——M3 计划不涉及。
- **没有检查 Task 9 的 UI 细节**（`--app-*` token、i18n 键名）——实现时再对。

---

## 8. 一条方法论观察

S-1 是 M1 的 V-3、M2 的 `index_snapshot_tx` 之后的**第三次**同一形态缺陷：

| 轮次 | 缺陷                             | 发现时机                          |
| ---- | -------------------------------- | --------------------------------- |
| M1   | `.pebak` 写入器零生产调用者      | 审查阶段                          |
| M2   | `index_snapshot_tx` 零生产调用者 | **实现完成后**（写 CHANGELOG 时） |
| M3   | `record_change_tx` 零生产调用者  | **计划阶段**（本次核对）          |

**发现时机在提前**，这是进步。但它连续出现三次，说明「新增函数要有调用者」
不是靠提醒能解决的——它需要在**计划里就写明调用点**。

**建议**：M3 计划修订时，把 S-1 那张「写入口 → 要记的变更」表**直接写进计划正文**，
而不是只在核对文档里提一次。这样实施者照着表做，就不会漏。

---

**签章**：本文档每一条「事实」都来自实际执行的 `grep` / `sed` / `Read`，行号已标注。
本轮未产生任何代码改动，未提交、未推送、未升级版本号、未写注册表。
