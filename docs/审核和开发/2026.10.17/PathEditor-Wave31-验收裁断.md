# Wave 3.1 验收裁断（审核窗口 → 开发窗口，第 5 轮）

- **日期**：2026-10-17
- **裁断窗口**：审核窗口
- **复审对象**：`docs/审核和开发/2026.10.17/PathEditor-Wave31-开发回执.md`
- **状态**：**Wave 3.1 验收通过**；行提交放行（直接 main）；P1 测试顺手修。

---

## 一、Wave 3.1 验收：通过

审核窗口**亲自复核**（非采信回执数字）：

| 复核项                                       | 证据                                                                                                                             |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| 核心修复为**多重集比较**                     | `core/src/service.rs` `check_expected`：`current_sorted.sort(); expected_sorted.sort(); if current_sorted != expected_sorted` ✅ |
| 3 条契约测试存在                             | `service.rs:760 reorder_only_does_not_conflict` / `:783 membership_change_still_conflicts` / `:806 duplicate_count_matters` ✅   |
| CLI 签名**去 `expected` 参数**               | `cli/src/runtime.rs:70 save_path_write(system: Option<Vec<PathEntry>>, user: ...)`，`PathHiveWrite{expected: None}` 内部构造 ✅  |
| `enabled_paths` 删净                         | `grep enabled_paths cli/src` → 零命中 ✅                                                                                         |
| `merge_hive` / `registry/path.rs` **零改动** | `git diff --stat` 仅 `golden/README.md`（= Wave 3.0 N10 符号锚定修复，**预期内**）✅                                             |
| 质量门                                       | 回执 §1：227 / 65 / 260 / 30；覆盖率 87.58%；`verify:all` 真实退出码 0；clippy 零警告 ✅                                         |
| 两方向变异测试                               | 回执 §2：改回顺序敏感 → `reorder_only` 红；改恒通过 → `membership_change` + `duplicate_count` 双红 ✅                            |

**判定：通过，无需返工。** 真机边界（§2 Task 4）说明**诚实且正确**——顺序已对齐，`add` 成功只证回归正常，分叉场景由单测覆盖；**不为验证重排真实 PATH** 是正确取舍。

---

## 二、逐项裁断

### ① CLI 签名简化 vs plan 的「传 `None`」→ **采纳开发窗口，且它更对**

plan 说「传 `expected: None`」，开发窗口**从签名里删掉参数**。**裁断：采纳**——理由：后者把「CLI 不校验」由**运行时约定**升级为**编译期不可表示**（后人想误传 `Some` 也没有参数可传）。这正是 `codebase-design` 的「让非法状态不可表示」；且在 plan 的「实施自由度」内。**doc 注释已写明恢复条件（CLI 若变批处理需加回）**——处理得当。

**连带删 `enabled_paths` 亦正确**：加 `#[allow(dead_code)]` 会把一次架构简化变成永久死重。

### ② 提交 → **授权：直接提交到 main**

用户 2026-10-17 授权（按本仓既有习惯，同 5.1.5）。**范围 = Wave 3.0 + 3.1 累计改动（约 34 项，含 7 个删除）+ 本轮 P1 修复。**

### ③ `backup_registry_default_dir_no_panic` → **现在顺手修**

用户 2026-10-17 裁定「现在顺手修」。**注意它比「P1」更严重**：`backup_registry(None)` 写**真实备份目录**并触发 **20 份轮换**——每次 `cargo test` 都可能**淘汰一份真实备份**（不只是污染，是备份被静默挤掉）。

**修法**（参照同文件 `backup_registry_with_custom_dir`（`backup.rs:1305`）与轮换接线测试的隔离手法）：

- 设 `PATHEDITOR_BACKUP_DIR` 到临时目录，**消除对真实目录的写入**；
- **不再 `let _ =` 丢弃返回值**——改为有意义的断言（在可读注册表时断言返回路径落在隔离目录内；不可读时至少断言「不 panic」并有注释说明为何可能失败）。

---

## 三、授权与放行

| 项                | 授权                                            |
| ----------------- | ----------------------------------------------- |
| Wave 3.1 验收     | ✅ 通过                                         |
| 改代码（P1 修复） | ✅                                              |
| **提交到 main**   | ✅（本轮新增）                                  |
| **推送 origin**   | ✅（随提交，本仓习惯：提 main 即推）            |
| 升版本号          | ❌ **仍不升**（5.1.6 保持「未发布」；发版另议） |

---

## 四、执行清单（开发窗口）

1. **P1 修复**（§二.③）→ 跑 `cargo test -p path-editor-core` + `clippy`。
2. **质量门**：`npm run verify:all` 退出码 0。
3. **提交**（Conventional Commits，建议**按逻辑分组**：F-08 收口 / 死码清理 / 顺序敏感修复 / 测试隔离修复；允许开发窗口按粒度自定）。commit body 每行 ≤ 100 字符。
4. **推送** `origin main`。
5. 落**收尾回执**（含：提交 SHA 清单、P1 修复证据、push 结果），并确认 `~/.patheditor/` 无本轮残留。

---

## 五、交接链状态

| 文档                                                     | 状态                    |
| -------------------------------------------------------- | ----------------------- |
| 报告 / 两轮核对 / 三轮裁断 / 两轮回执 / 终裁（Wave 3.0） | ✅ 全部在库             |
| Wave 3.1 spec + plan + 开发回执                          | ✅ 在库                 |
| **本验收裁断（第 5 轮）**                                | ✅ 本文件               |
| Wave 3.1 收尾回执                                        | ⏳ 待开发窗口（提交后） |

**Wave 3.1 代码工作已验收完毕；剩余为「P1 修复 + 提交 + 推送 + 收尾回执」。**
