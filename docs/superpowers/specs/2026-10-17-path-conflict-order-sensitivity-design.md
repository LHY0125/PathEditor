# PATH 并发校验「顺序敏感死锁」修复 — 设计文档

- **日期**：2026-10-17
- **版本目标**：独立波次（**不属 Wave 3.0**；建议 **Wave 3.1**）
- **作者**：审核窗口
- **状态**：待开发窗口逐行核对 → plan
- **来源**：`PathEditor-F08收口复审裁断.md` §二（开发回执复审发现）
- **性质**：**既有 bug**（HEAD `9f9c0d8` 复现，非 Wave 3.0 引入）

## 概述

保存 PATH 的乐观并发校验（`core::service::check_expected`，`service.rs:183-205`）用 `if &current != expected` 比较**有序**列表。但 `expected` 来自 `load_path_snapshot` 的 `merge_hive` 输出，其**顺序取自快照**（`disabled.rs:136` 注释 + golden 测试 `merge_snapshot_order_wins`），而 `current` 是**注册表顺序**。二者在**任何外部重排**后即不相等 → 保存**永久被拒** → **死锁**（存不了 → 快照永不更新 → 一直拒绝），且「请重新加载后重试」是**假解药**（reload 走同一合并，产出同一顺序）。

**本机实证**：注册表 user PATH（10 条）与快照合并（10 条）**集合相同、顺序不同** → 目前用 PathEditor **存不了 PATH**（GUI/CLI 同一校验）。

## 背景与现状（已核实代码事实）

| 事实                                                                          | 位置                                                                |
| ----------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| `check_expected` 顺序敏感比较：`if &current != expected`                      | `core/src/service.rs:195`                                           |
| `merge_hive` **保快照顺序**：先推快照中「在注册表内」的条目，再追加注册表独有 | `core/src/disabled.rs:137-163`                                      |
| `merge_hive` **丢弃**「启用但不在注册表」的条目（不复活外部删除）             | `core/src/disabled.rs:148`（`else if !entry.enabled`）              |
| 快照顺序是**有意**的真相源                                                    | golden `merge_snapshot_order_wins.json`                             |
| 旧 CLI `verify_and_save` 同样顺序敏感（`reload != original`）                 | `cli/src/runtime.rs:59`（Wave 3.0 已删，逻辑并入 `check_expected`） |
| 本机快照 `disabled.json` 日期 **Sep 20**；注册表在其后被 Scoop 重排           | 实测（`~/.patheditor/disabled.json` vs `HKCU:\Environment\Path`）   |

## 根因：两条「顺序权威」打架

- **合并层**：`merge_hive` 用**快照**决定顺序（设计如此）。
- **校验层**：`check_expected` 拿合并结果跟**注册表**顺序比。

但 `merge_hive` 对**成员**是忠实于注册表的（只保留注册表内条目 + 快照禁用项），**唯独顺序取自快照**。因此：

> **校验集合（成员）是等价于「加载时注册表内容」的；校验顺序却是错的（拿快照顺序比注册表顺序）。**

**推论（关键）**：校验的**顺序分量**只能在「快照顺序 ≠ 注册表顺序」时触发——**它检出的 100% 是顺序差异，而非任何真实的成员变化**。而成员分量**才是**「加载→保存之间外部增删」的有效检测。

## 关键设计决策

### K1 —— 校验改为**内容比较（多重集）**，去掉顺序分量

`check_expected` 比较改为**排序后比较**（或等价的多重集比较），**忽略顺序、保留重复计数**：

```rust
// 现状（错）：顺序敏感
if &current != expected { ... }

// 修复：内容敏感（多重集）
let mut a = current.clone(); a.sort();
let mut b = expected.clone(); b.sort();
if a != b { ... }
```

**理由**：

1. **成员分量**才是校验的本职（检测「加载→保存之间外部增删」）；`merge_hive` 保证 `expected` 的成员 == 加载时注册表成员，故成员比较**有效**。
2. **顺序分量**是误报源：PathEditor **拥有 PATH 顺序**（快照是顺序真相源、golden `merge_snapshot_order_wins` 明确「快照顺序胜」）。外部**纯重排**不是「丢失更新」——PathEditor 存盘时本就会写入自己的顺序。故**不应**因顺序拒绝。

### K2 —— 「重新加载后重试」由此**变成真解药**

修复后：

- **纯重排**（本机情形）→ 内容相同 → **不再拒绝** → 死锁消失。
- **真实成员冲突**（外部增删）→ 内容不同 → 仍拒绝（**正确**）；此时 reload 重新合并（成员 = 当前注册表）→ 再存即通过。**reload 成为有效恢复路径**（现状是死循环）。

### K3 —— **不动** `merge_hive`、不动 golden

`merge_snapshot_order_wins` 是**有意**设计（顺序归快照），**保持**。本修复只改**校验**的比较方式，不改顺序语义。→ `registry/path.rs`、`disabled.rs` 零改动，仅 `service.rs` 一处。

### K4 —— 顺带收益来自 Wave 3.0 收口

旧代码里顺序敏感比较**散在** `gui/src/commands/registry.rs:16/26` 与 `cli/src/runtime.rs:59` **两处**（外加各自实现）。Wave 3.0 已把它收敛为**唯一**的 `check_expected`——**故本修复是单点一行**。这正是收口「一处判定」的回报。

## 接口规格

无公开接口变化。仅 `core/src/service.rs` 内 `check_expected` 的比较逻辑变更（私有函数）。

## 安全边界

- **无注册表写入语义变化**：仅放宽「因顺序差异而拒绝」，**不放宽**任何保护名单 / 类型 / 权限 / 真实成员冲突的判定。
- **不放宽成员冲突**：外部增删仍拒绝（K1）。
- ⚠️ **一次行为放宽**：外部**纯重排**从「拒绝」变为「存盘覆盖顺序」。须在 CHANGELOG 声明（`### 修复`：修复外部重排导致无法保存的死锁；`### 变更`：PathEditor 存盘会覆盖外部对 PATH 顺序的改动）。

## 验收标准

1. **新增回归测试**（`service.rs`，经 fake `PathHiveIo`）：
   - `reorder_only_does_not_conflict`——`expected` 与 `current` **同集合、不同顺序** → `check_expected` **通过**（现状为失败）。
   - `membership_change_still_conflicts`——`expected` 与 `current` **成员不同**（增/删）→ **仍拒绝**（防止 K1 被误实现成「恒通过」）。
   - `duplicate_count_matters`——同集合但**重复计数不同** → **拒绝**（多重集而非集合）。
2. **变异测试**：把比较改回顺序敏感 → 第 1 条转红；改为「恒通过」→ 第 2/3 条转红。两条都证明可证伪。
3. `cargo test --workspace` + `npm run verify:all` 全绿；clippy 零警告。
4. **真机验证**：本机（顺序已分叉）执行 `patheditor add` **成功**且注册表生效（**依赖 Task 8 的数据对齐或本修复二者之一**；修复落地后即便不对齐数据也应成功）。

## 范围外

- 不改 `merge_hive` 的顺序语义（K3）。
- 不引入「PathEditor 拥有的顺序 vs 外部顺序」的更复杂模型——`merge_snapshot_order_wins` 维持。
- 不处理`backup_registry_default_dir_no_panic`（另开，见复审裁断 §三.②）。

## 附：与 Wave 3.0 的关系

- **独立**：本 bug 早于 Wave 3.0 存在；Wave 3.0 只是把它收敛到单处。
- **解阻**：Wave 3.0 的 Task 8 真机回归被本 bug 阻塞；用户授权**一次性对齐 `disabled.json`** 解阻（见复审裁断 §四），**与本 spec 的代码修复并行**——数据对齐让本波能跑真机回归，代码修复根治。

## 附二：CLI 侧并发校验**无实际约束力**（复审回执 §4 回归 3 发现，并入本 spec 待裁断）

真机回归发现：CLI 每次命令**重新 `load_path_snapshot()`**，`expected` 即当前注册表 → **CLI 本就没有「加载→保存」时间窗口** → `check_expected` 在 CLI 侧**从不因真实并发触发**，只可能因**顺序差异误报**（即本 spec 的死锁）。A/B 对照证明 HEAD 同构（非 Wave 3.0 引入）。

**推论**：K1 修复后，CLI 侧该校验**完全惰性**（真实并发无窗口、顺序误报已消除）——纯死重。

**待裁断（并入本 spec 实施）**：CLI 是否仍传 `expected: Some(...)`？

- **建议：CLI 传 `None`**（明确跳过）——CLI 是单次进程，无并发窗口；把校验留给**有真实用户思考窗口的 GUI**。此改动**只移除 CLI 侧的误报**，不放宽任何真实保护。
- 若保留：须在注释写明「CLI 侧该校验为惰性，仅为对称保留」，避免后人误以为它在保护 CLI。

**真正的并发保护在 GUI**（`expected` 来自界面加载时的 `_savedSys`）。
