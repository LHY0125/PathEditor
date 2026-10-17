# PATH 并发校验「顺序敏感死锁」修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修掉 PATH 保存的乐观并发校验「顺序敏感」——它使注册表与快照顺序一旦分歧即**永久拒绝保存**（死锁）。修法：`check_expected` 由**顺序比较**改为**内容比较（多重集，排序后比较）**。

**Architecture:** 单点改动，落在 `core/src/service.rs` 的私有函数 `check_expected`。**不动** `merge_hive`（顺序语义有意，golden `merge_snapshot_order_wins` 保持）、**不动** `registry/path.rs`。可选 Task 3 让 CLI 传 `expected: None`（见 spec 附二待裁断）。

**Tech Stack:** Rust（core / gui / cli workspace，edition 2021）、serde / serde_json。

**Spec:** `docs/superpowers/specs/2026-10-17-path-conflict-order-sensitivity-design.md`

## Global Constraints

- **提交与版本**：**不授权提交、不推送、不升版本号**（沿用）。
- **架构边界**：判定在 core；CLI/GUI 零判定。
- **错误契约**：判定只认 `CoreError.code`（camelCase）。
- **不放宽真实保护**：仅放宽「因**顺序差异**而拒绝」；**外部增删仍拒绝**（spec K1）。保护名单 / 类型 / 权限判定**零改动**。
- **不碰** `merge_hive` / golden / `registry/path.rs`（spec K3）。
- **文档注释**：所有 `pub fn` / `pub(crate) fn` 有 `///`；代码注释中文。
- **代码风格**：UTF-8 / CRLF；Rust 4 空格；无新增 `unsafe`。
- **质量门**：`npm run verify:all`。
- **真实注册表**：Task 1–3 不写真实注册表（经 fake `PathHiveIo`）；Task 4 真机仅**读 + 既有 `add` 流程**。
- **每任务结束**：`cargo fmt` + `cargo clippy --workspace --all-targets -- -D warnings` 零警告。

---

## 文件结构

| 文件                                                  | 职责                                                       |
| ----------------------------------------------------- | ---------------------------------------------------------- |
| `core/src/service.rs`                                 | Task 1 加契约测试；Task 2 改 `check_expected` 比较         |
| `cli/src/{runtime,main,import_export,profile_ops}.rs` | Task 3（可选）：6 入口传 `expected: None`                  |
| `CHANGELOG.md`                                        | Task 4：`### 修复`（死锁）+ `### 变更`（存盘覆盖外部顺序） |

---

### Task 1: 契约测试（先 RED）

**Files:** Modify/Test `core/src/service.rs`（`#[cfg(test)] mod tests`）

**Interfaces:**

- Consumes: 既有 fake `PathHiveIo`（`service.rs` 测试内）、`check_expected`

- [ ] **Step 1:** 写三条契约测试（经 fake `PathHiveIo`）：
  - `reorder_only_does_not_conflict`——`expected` 与 `current` **同集合、不同顺序** → `check_expected` **通过**（**现状 RED**）。
  - `membership_change_still_conflicts`——`.current` 与 `expected` **成员不同**（增/删）→ **仍拒绝**（**现状 GREEN，作为反例守护**）。
  - `duplicate_count_matters`——同集合但**重复计数不同** → **拒绝**（**现状 GREEN，守护多重集语义**）。
- [ ] **Step 2:** `cargo test -p path-editor-core service`：确认第 1 条 **RED**、第 2/3 条 GREEN。
- [ ] **Step 3:** `cargo fmt` + `clippy -D warnings` 零警告。

---

### Task 2: `check_expected` 改多重集比较

**Files:** Modify `core/src/service.rs`（`check_expected`，`:183-205`）

- [ ] **Step 1:** 把 `if &current != expected`（**顺序敏感**）改为**排序后比较**：

  ```rust
  // 内容比较（多重集）：忽略顺序、保留重复计数。
  // 理由：merge_hive 的成员忠实于注册表、唯独顺序取自快照；
  // 顺序分量不是 lost-update（PathEditor 拥有顺序），比较它只会误报（死锁）。
  let mut a = current;
  let mut b = expected.clone();
  a.sort();
  b.sort();
  if a != b { /* Conflict */ }
  ```

- [ ] **Step 2:** `cargo test -p path-editor-core service`：`reorder_only_does_not_conflict` 转 **GREEN**；第 2/3 条仍 GREEN。
- [ ] **Step 3:** **变异测试**（两条）：
  - 改回顺序敏感（`current != *expected`）→ 第 1 条**转红**；
  - 改为「恒通过」（删 `if`）→ 第 2/3 条**转红**。
    两条都证明可证伪；恢复。
- [ ] **Step 4:** `cargo fmt` + `clippy -D warnings` 零警告。

---

### Task 3（待裁断，建议采纳）: CLI 传 `expected: None`

**Files:** Modify `cli/src/{runtime,main,import_export,profile_ops}.rs`（6 入口）

> **依据**：spec 附二——CLI 每次命令重载快照，`expected == current`，校验**惰性**（真实并发无窗口、顺序误报已由 Task 2 消除）。用户已授权改代码。

- [ ] **Step 1:** 6 个保存入口（`save_path_write` 的调用点）由 `expected: Some(enabled_paths(..))` 改为 `expected: None`，并**注释写明理由**（CLI 单次进程、无并发窗口，校验归 GUI）。
- [ ] **Step 2:** `cargo test -p patheditor-cli --bins` 全绿。
- [ ] **Step 3:** **若开发窗口判定应保留** → 改为保留 `Some(..)` 并在注释注明「CLI 侧惰性，仅对称保留」；在回执写明该选择。

---

### Task 4: 文档 + 质量门 + 回执

**Files:** `CHANGELOG.md`；真机验证；回执

- [ ] **Step 1:** `CHANGELOG.md` 顶部（5.1.6 段落内）补 **`### 修复`**（外部重排 PATH 导致无法保存的死锁）+ **`### 变更`**（PathEditor 存盘会覆盖外部对 PATH 顺序的改动）。
- [ ] **Step 2:** **真机验证**：本机（数据已对齐）跑一次 `patheditor add <唯一探针>` → **成功**（回归既有流程）+ 逐条清除探针。**注**：分叉场景（`reorder_only`）由 Task 1 单测覆盖，**不**为此重排真实注册表。
- [ ] **Step 3:** `npm run verify:all` 退出码 0。
- [ ] **Step 4:** 落**开发回执**（`docs/审核和开发/2026.10.17/PathEditor-Wave31-开发回执.md`），含变异测试证据与真机记录。

---

## 验收（对应 spec §验收标准）

1. 三条契约测试存在且真实执行；**两方向变异测试**均可证伪（Task 2 Step 3）。
2. `reorder_only_does_not_conflict` GREEN；`membership_change_still_conflicts` / `duplicate_count_matters` GREEN（守护 K1 不被误实现为「恒通过」）。
3. `cargo test --workspace` + `npm run verify:all` 全绿；clippy 零警告。
4. 本机 `patheditor add` 成功（既有流程回归）。
5. `merge_hive` / golden / `registry/path.rs` 零改动。

## 范围外

`backup_registry_default_dir_no_panic`（另开，P1）；GUI 真机保存（E2E mock 覆盖）；`merge_hive` 顺序语义变更。
