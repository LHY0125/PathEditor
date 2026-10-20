# PathEditor v6.0.0 M1 — 缺口补齐轮复审结论

- **日期**：2026-10-20
- **被审对象**：D-1 / D-2 修复（工作区未提交）
- **被回复对象**：开发窗口对 `PathEditor-v6-M1-缺口补齐轮审查报告.md` 的修复说明
- **基准**：`v6/m1-storage-history`，全部改动仍在工作区，未提交

---

## 一、验证证据（审核窗口独立执行）

| #   | 命令                                                    | 结果                                               |
| --- | ------------------------------------------------------- | -------------------------------------------------- |
| 1   | `cargo fmt --all -- --check`                            | exit 0                                             |
| 2   | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0                                             |
| 3   | `cargo test --workspace`                                | **21 binaries，312 passed / 0 failed / 2 ignored** |
| 4   | `npm run verify`                                        | exit 0                                             |
| 5   | `npx vitest run`                                        | 27 files / **336** tests                           |
| 6   | `npm run test:e2e`                                      | **44 passed**                                      |

计数用显式字段拆分累加，与回执的 312 一致。

---

## 二、D-1 复核：修复成立

**读路径确实切到 DB 了**（`core/src/disabled.rs:289-303`）：

```rust
let state = if v5_writes_disabled() {
    match load_state_from_db() {
        Some(s) => s,
        None => { log::debug!("DB 侧无当前快照，回落读 disabled.json"); read_state()? }
    }
} else {
    read_state()?
};
```

**设计判断正确**：用 `set_current_snapshot_loader` 注入而非让 `disabled` 直接依赖 `db` 模块——注释说明了理由（`db::migrate_v5` 已经依赖 `disabled`，反向依赖会成环）。这是对的，Cargo 的模块循环依赖虽然允许，但语义上会形成僵化的耦合。

**回落逻辑符合 spec**：DB 取不到时读文件，对应 §7.8「降级模式 `disabled.json` 是唯一兜底」。

**CLI 不受影响**：`cli/src/` 里既没有 `set_v5_writes_disabled` 也没有 `set_current_snapshot_loader`，所以 `v5_writes_disabled()` 恒为 `false`，走 `else` 分支读文件——与改动前行为一致。这是正确的：CLI 不常驻，没有「启动时停写」的概念。

**回归测试质量高**：`core/tests/db_v5_read_from_db.rs` 用了注册表里真实存在的 `C:\WINDOWS` / `C:\WINDOWS\system32`，并在注释里写明「必须用真实路径，`merge_hive` 会过滤掉不存在的条目——审核窗口的探针正是在这里失败的」。这个注释把一次失败转化成了对未来维护者的提示。

---

## 三、D-2 复核：修复成立，但测试缺失

**SQL 已修正**（`migrate_v5.rs:90-97`）：

```sql
(SELECT id FROM snapshots
   WHERE kind = 'current'
   ORDER BY created_at DESC, id DESC LIMIT 1)
```

**但没有任何 `core/tests/` 测试 D-2。** `grep -rn "kind = 'current'" core/tests/*.rs` 零命中。

现有相关测试只覆盖 `app_state` 的 seed 与 CAS（`db_app_state.rs`），不涉及迁移后的 `current_snapshot_id` 指向正确性。

**建议补一条**：构造一个同时含 `disabled.json` 与 `path_backup_*.txt` 的迁移夹具，断言迁移后 `current_snapshot_id` 指向 `kind='current'` 的快照，而**不是** backup。这是 D-2 唯一的直接验收判据。

**严重性**：非阻塞（实现已对），但缺少守住这类回归的手段——而 D-2 恰恰是「看起来对、语义错」的典型，没有测试就守不住。

---

## 四、发现一处新缺陷（非阻塞，但与 D-2 同源）

### D-6【应改】迁移用 `kind='current'`，但迁移可能根本不产生 `kind='current'` 快照

**位置**：`core/src/db/migrate_v5.rs:118-160`（`import_disabled`）、`:90-92`（D-2 的 SQL）

**机制**：

`import_disabled` 只在 `disabled.json` **存在且非空**时才产生 `kind=Current` 快照。新装用户或 `disabled.json` 已被清理的用户，**没有 current 快照**。

此时 D-2 的 SQL 返回 `NULL` → `app_state.current_snapshot_id = NULL`。

**后果**：D-1 修好后，`load_path_snapshot` 在停写模式下的路径是：

```
load_state_from_db() → 读 app_state.current_snapshot_id → NULL → 返回 None
  → 回落 read_state() 读 disabled.json
```

但 `disabled.json` 已经停写、且可能根本不存在 → 读到 `DisabledState::default()`（空）→ `merge_hive` 把注册表 PATH 全部标记 `enabled: true`。

**在「用户从未禁用任何条目」时这是正确的**（回执的事故定论也确认了这一点）。**但一旦用户禁用了条目**，而没有 current 快照，禁用标记会丢失。

**关键问题**：`update_current_snapshot`（`gui/src/commands/service.rs`）在**每次写操作后**都会建一个新快照并设为 current。所以：

- 用户第一天启动（无 current）→ 禁用某条目 → 写操作触发 `update_current_snapshot` → current 建立 ✅
- 但**在第一次写操作之前**，如果用户打开 GUI 看到的是「全部启用」，这与 `disabled.json` 里的旧禁用标记不一致。

也就是说：**迁移时没有 current 快照的用户，会在第一次写操作前丢失禁用状态**。

**验收判据**（二选一）：

- **(A)** 迁移结束时若 `kind='current'` 的快照不存在，用 `disabled.json`（或注册表当前状态）建一条。
- **(B)** 在 `setup_database` 里，迁移后立即调一次 `update_current_snapshot` 建立基线。

**严重性**：非阻塞。因为「`disabled.json` 存在且含禁用标记」的用户一定有 current 快照（`import_disabled` 会建）；只有「`disabled.json` 不存在」的用户才会遇到，而那时本来就没有禁用状态可丢。

**但这仍然是一个需要显式回答的问题**，否则下一个维护者会以为 current 一定有值。

---

## 五、关于 disabled.json 事故

回执的定论：**实际影响为零，未恢复任何文件**。

基于：

| 检查项                              | 结果                                      |
| ----------------------------------- | ----------------------------------------- |
| 注册表 system PATH                  | 43 条完好                                 |
| 注册表 user PATH                    | 12 条完好                                 |
| `load_path_snapshot()` 输出         | system 43 / user 12 / 零禁用 / 无 D1REPRO |
| 三份 `disabled.json` 变体的禁用标记 | 全是 0                                    |

**我核对后同意这个定论**，理由：

1. `disabled.json` 的 `systemSnapshot`/`userSnapshot` 只有在「存在被禁用的条目」时才影响 `merge_hive` 的结果——`merge_hive` 用快照的 `enabled` 标记覆盖注册表条目，而**全 `enabled: true` 的快照等价于「没有禁用信息」**。
2. 我写坏的缓存是 `systemSnapshot = [C:\D1REPRO enabled=true]` + `userSnapshot = 10 条全 true`。`C:\D1REPRO` 不在注册表里，被 `merge_hive` 过滤；其余 10 条全 true，不改变任何启用状态。
3. 因此：**覆盖前后的 `load_path_snapshot()` 结果相同**。

**但事故本身仍然成立**——它是「用未重定向的路径写真实用户数据」这个错误。定论是「碰巧没造成伤害」，不是「没有错误」。这一点回执没有混淆，值得肯定。

**遗留问题**：`disabled.json` 现在含 `C:\D1REPRO` 这条假数据。它当前无影响（被 merge 过滤），但它是一份**脏文件**。是否清理由用户决定；我不建议在未经确认时改动它。

---

## 六、判定

### 合并前

**无新增阻塞项。** D-1 已修复且测试覆盖，D-2 已修复。

### 建议合并前补（均为分钟级）

1. **D-2 的回归测试**（见 §三）——当前是唯一「实现了但没测试」的修复项。
2. **D-6 的显式回答**（见 §四）——实现二选一，或至少在代码里写明「无 current 快照时的预期行为」。

### 可以合并后做

- `disabled.json` 里的 `C:\D1REPRO` 脏数据清理（需用户确认）
- 上一轮报告中的 D-3（`#![allow(dead_code)]` 收紧）、D-4（CAS 失败日志）

### 合规亮点

- **D-1 的注入设计是对的**，而且注释写清了「为什么不能直接依赖 db 模块」——避免了循环依赖。
- **回归测试的注释把一次失败固化成了知识**：「必须用注册表里真实存在的路径，`merge_hive` 会过滤掉不存在的条目——审核窗口的探针正是在这里失败的」。这比测试本身更有价值。
- **事故定论做到了「结论」与「责任」分离**：影响为零是事实，操作错误也是事实，两者都写了。

---

## 七、回执怎么写

**可以写。** 建议在回执里明确列入以下三项（现在都还悬着）：

1. **D-2 无测试**：如实写「实现已改，未补回归测试」。
2. **D-6 未处理**：如实写「迁移后无 current 快照的路径未显式定义」。
3. **disabled.json 脏数据**：如实写「探针写入了 `C:\D1REPRO` 至真实文件，当前无影响，未清理，待用户决定」。

这三条都不是阻塞，但**回执的价值在于让下一轮的人知道哪里还没钉死**。前面几轮的经验是：没写进回执的项，下一轮就会消失。
