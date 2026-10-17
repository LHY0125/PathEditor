# PathEditor Wave 3.1（顺序敏感死锁修复）— 开发回执

- **日期**：2026-10-17
- **提交窗口**：开发窗口
- **交付对象**：审核窗口
- **依据**：`docs/superpowers/specs/2026-10-17-path-conflict-order-sensitivity-design.md` + `docs/superpowers/plans/2026-10-17-path-conflict-order-sensitivity-implementation.md`
- **状态**：**Task 1–4 全部完成**；质量门全绿。未提交、未推送、未升版本号。

---

## 1. 交付摘要

| 指标                 | 值                                                                       |
| -------------------- | ------------------------------------------------------------------------ |
| core 测试            | **227 passed / 0 failed / 2 ignored**（Wave 3.0 基线 224 → +3 契约测试） |
| CLI 测试             | **65 passed / 0 failed**                                                 |
| 前端 / E2E           | 260 passed / 30 passed                                                   |
| 覆盖率               | **87.58% 行**                                                            |
| `npm run verify:all` | **真实退出码 0**                                                         |
| clippy `-D warnings` | 零警告（整个 workspace）                                                 |
| 改动位置             | `core/src/service.rs`（`check_expected` 一处）+ CLI 11 个调用点          |

**核心成果**：`check_expected` 由**顺序敏感**改为**多重集比较** —— 一行逻辑，修掉一个使 PATH **永久无法保存**的既有死锁。

---

## 2. 逐 Task 证据

### Task 1：契约测试（先 RED）

**新增 3 条**（`core/src/service.rs` 测试模块，经 fake `PathHiveIo`）：

| 测试                                | 角色                       | Task 1 时状态 |
| ----------------------------------- | -------------------------- | ------------- |
| `reorder_only_does_not_conflict`    | **驱动修复**               | **RED** ✅    |
| `membership_change_still_conflicts` | **守护**（防「恒通过」）   | GREEN ✅      |
| `duplicate_count_matters`           | **守护**（防集合化吞重复） | GREEN ✅      |

`cargo test -p path-editor-core service`：**13 passed / 1 failed**（正是 `reorder_only_does_not_conflict`）。

### Task 2：`check_expected` 改多重集比较

```rust
// 现状（错）：顺序敏感
let current = io.read_enabled(hive)?;
if &current != expected { /* Conflict */ }

// 修复：内容比较（多重集）——忽略顺序、保留重复计数
let mut current_sorted = current;
let mut expected_sorted = expected.clone();
current_sorted.sort();
expected_sorted.sort();
if current_sorted != expected_sorted { /* Conflict */ }
```

**三条全部 GREEN**（14 passed）。

#### 两方向变异测试（**核心可证伪性证据**）

| 变异                | 操作                             | 结果                                                                                    |
| ------------------- | -------------------------------- | --------------------------------------------------------------------------------------- |
| **A：改回顺序敏感** | 替换为 `if current != *expected` | `reorder_only_does_not_conflict` **转红** ✅                                            |
| **B：改成恒通过**   | 条件改为 `if false`              | `membership_change_still_conflicts` **转红** ✅ + `duplicate_count_matters` **转红** ✅ |

**两方向都证伪** —— 这正是「不放宽成员冲突」这一约束的可执行证据：单独证明「顺序放宽」不够，还必须证明「成员变化仍被拒」。

> **实施细节（CRLF 教训）**：变异脚本首次因源文件是 **CRLF** 而未匹配（错误信息仅 "pattern not found"，无线索）。改为**逐行 `contains` 预检 + `\r\n` 拼接**后成功。**在 Windows 仓库里做多行文本替换，必须先确认行尾**。

### Task 3：CLI 传 `expected: None`

**选择：采纳「CLI 不做并发校验」，且以类型消除而非传 `None`。**

**偏离 plan 字面（在实现自由度内，更严格）**：plan 说「6 入口传 `expected: None`」。开发窗口**改为简化 `save_path_write` 签名**——去掉 `Option<Vec<String>>` 第二元，只保留 `Option<Vec<PathEntry>>`：

```rust
// 旧（Wave 3.0）：expected 可传
pub(crate) fn save_path_write(
    system: Option<(Vec<Core::PathEntry>, Option<Vec<String>>)>,
    user: Option<(Vec<Core::PathEntry>, Option<Vec<String>>)>,
)

// 新（Wave 3.1）：签名内没有 expected —— 不可能误传
pub(crate) fn save_path_write(
    system: Option<Vec<core::PathEntry>>,
    user: Option<Vec<core::PathEntry>>,
)
```

**理由**：用**类型系统**表达「CLI 不校验」，比「传 `None` + 注释」更不可绕过（无法被后来者误传 `Some`）。doc 注释写明**何时应改回**（CLI 若变为批处理模式）。

**连带改动（计划外但不可避免）**：参数简化后 `enabled_paths`（`runtime.rs:99`）**零引用**。处置 = **删除**（非加 `#[allow(dead_code)]`）——加豁免会把「一次架构简化」变成「永久死重」。已在 §3 记录。

| 验证                                          | 结果                            |
| --------------------------------------------- | ------------------------------- |
| `cargo check -p patheditor-cli --all-targets` | 零警告（删 `enabled_paths` 后） |
| `cargo test -p patheditor-cli --bins`         | **65 passed / 0 failed**        |

**共改 11 个调用点**（6 个入口；双 hive 入口各 2 处）：`runtime.rs`×4、`main.rs`×3、`import_export.rs`×3、`profile_ops.rs`×2（其中一处是多行调用）。

### Task 4：文档 + 质量门 + 真机 + 回执

| 项                        | 证据                                                                                                                                                         |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `CHANGELOG.md` 5.1.6 段落 | 补 `### 修复`（顺序敏感死锁）+ `### 变更`（存盘覆盖外部顺序）+ `### 说明`（CLI 不做并发校验及理由）✅                                                        |
| 质量门                    | `npm run verify:all` **真实退出码 0**（227 / 65 / 260 / 30；覆盖率 87.58%）✅                                                                                |
| 真机验证                  | `patheditor add D:\__W31_VERIFY__` → **exit 0**，注册表 10→11 条 ✅；**备份 mtime 变化**（`03:37:54` → `03:39:32`）✅；探针已清除、注册表回 10 条、零残留 ✅ |

> **真机验证的边界（如实说明）**：本机**顺序已对齐**（Wave 3.0 Task 8 的数据对齐），故本次 `add` 成功**只证明既有流程回归正常**，**不证明**顺序敏感修复生效。**分叉场景由 Task 1 的 `reorder_only_does_not_conflict` 单测覆盖**（plan Task 4 Step 2 明确要求**不**为验证重排真实注册表——重排真实 PATH 有风险且低价值）。

---

## 3. plan 偏离记录

| #   | 计划原文                                                | 实际                                                                      | 处理                                                            |
| --- | ------------------------------------------------------- | ------------------------------------------------------------------------- | --------------------------------------------------------------- |
| 1   | Task 3「6 入口传 `expected: None`」                     | **简化签名**去掉 expected 参数（11 处的 `Some(..)` 简化为直接传 entries） | 实现自由度内，更严格（类型消除）；签名 doc 写明恢复条件         |
| 2   | 未提 `enabled_paths`                                    | 参数简化后成死码                                                          | **删除**（`runtime.rs:99`）；连带但不可避免（否则 clippy 失败） |
| 3   | Task 3 说「6 个保存入口（`save_path_write` 的调用点）」 | 入口 6 个，但**调用点 11 处**（双 hive 入口各 2 处）                      | 措辞精度问题，非缺陷；本回执记准                                |

---

## 4. 与验收标准（spec 5 条）的对照

| #   | 标准                                                                           | 落点                             | 状态 |
| --- | ------------------------------------------------------------------------------ | -------------------------------- | ---- |
| 1   | 三条契约测试存在且真实执行；两方向变异可证伪                                   | §2 Task 2 变异表                 | ✅   |
| 2   | `reorder_only` GREEN；`membership_change` / `duplicate_count` GREEN（守护 K1） | §2 Task 2                        | ✅   |
| 3   | `cargo test --workspace` + `verify:all` 全绿；clippy 零警告                    | §1 摘要                          | ✅   |
| 4   | 本机 `patheditor add` 成功                                                     | §2 Task 4                        | ✅   |
| 5   | `merge_hive` / golden / `registry/path.rs` **零改动**                          | `git diff --stat` 未见这三个文件 | ✅   |

---

## 5. 未覆盖项 / 残余风险

| 项                                         | 说明                                                                    |
| ------------------------------------------ | ----------------------------------------------------------------------- |
| **顺序敏感修复的真机验证**                 | 见 §2 Task 4 的边界说明——真机是「已对齐」状态，**分叉场景仅单测覆盖**。 |
| **GUI 真机保存**                           | 仍未验（Wave 3.0 遗留）；E2E mock 覆盖。                                |
| **CLI 并发校验完全移除**                   | 若未来 CLI 变为批处理模式，需重新引入（签名 doc 已写明条件）。          |
| **`backup_registry_default_dir_no_panic`** | 既有测试写用户真实备份目录；裁断判「另开，P1」，**未动**。              |
| **未提交**                                 | 工作树含 Wave 3.0 + Wave 3.1 全部改动；**未提交、未推送、未升版本号**。 |

---

## 6. 交付物位置索引

| 交付物        | 路径                                                                                  |
| ------------- | ------------------------------------------------------------------------------------- |
| 本开发回执    | `docs/审核和开发/2026.10.17/PathEditor-Wave31-开发回执.md`                            |
| Wave 3.1 spec | `docs/superpowers/specs/2026-10-17-path-conflict-order-sensitivity-design.md`         |
| Wave 3.1 plan | `docs/superpowers/plans/2026-10-17-path-conflict-order-sensitivity-implementation.md` |
| Wave 3.0 终裁 | `docs/审核和开发/2026.10.17/PathEditor-F08收口终裁.md`                                |

---

## 7. Wave 3.0 收尾（本轮一并完成）

| 项                                                                                                                       | 状态                                                                  |
| ------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------- |
| `README.md` 两张 Mermaid 图同步为收口后流程（GUI：单次 `save_path_with_sidecar`；CLI：经 `save_path_with_sidecar` 编排） | ✅ 零残留（`grep save_system_paths README.md` 无命中）；Prettier 通过 |
| 删 `~/.patheditor/pending_path_snapshot.json.bak`                                                                        | ✅（授权内）                                                          |

> 开发窗口接受「过度保守」的批评：**「是否越界」的准据不是改动大小，而是「是否属本次变更的语义范围」**——那张 Mermaid 图画的就是本波替换掉的流程，它属于本次变更。

---

## 8. 待裁断

1. **CLI 签名的简化形态**（去掉 `expected` 参数 vs plan 的「传 `None`」）：开发窗口选前者，若审核窗口认为应保持 plan 字面，请裁断。
2. **是否现在提交**（累计 Wave 3.0 + 3.1 改动）。
3. **`backup_registry_default_dir_no_panic`**（P1 另开）何时处理。

---

**开发窗口签章**：本回执所有结论均来自本次实际执行的命令输出（`cargo test` / `npm run verify:all` / 两方向变异测试 / `patheditor` 真机操作 / PowerShell 注册表读写）。**未跑过的未写为通过**；未覆盖项已如实列出。
