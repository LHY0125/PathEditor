# F-08 收口 — 终裁（审核窗口 → 开发窗口，第 4 轮）

- **日期**：2026-10-17
- **裁断窗口**：审核窗口
- **复审对象**：`docs/审核和开发/2026.10.17/PathEditor-F08收口复审回执.md`（开发窗口）
- **状态**：**Task 8 复审通过 → Wave 3.0 收官**；三项遗留裁断已定；Wave 3.1 开工。

---

## 一、Task 8 复审：通过

| 项                   | 证据                                                                                                                     | 判定                   |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------ | ---------------------- |
| ④ sidecar 非阻断提示 | 回执 §3.1：sidecar 异常**不再进 `errors[]`**（不降级结果），只改 `statusMessage`；单测断言 `result=success` + 提示含原因 | ✅ 符合裁断④           |
| ① 文档同步           | 回执 §3.2：4 文件；`cmp CLAUDE.md AGENTS.md` 无输出（md5 一致）；`prettier --check` 通过                                 | ✅（**遗留见 §二.1**） |
| ③ 数据对齐           | 回执 §3.3：`userSnapshot` 11→10、序 = 注册表；两 hive 全启用、**无语义丢失**；已备份 `*.pre-wave30-*`                    | ✅                     |
| 真机回归             | 回执 §4：CLI 保存 ✅ + pending 补写 ✅ + 并发冲突（CLI 无窗口，A/B 证非本波引入）+ GUI（受限，如实登记）                 | ✅ 3/4 + 1 受限        |
| 质量门               | `verify:all` 真实退出码 0；260 / 224 / 65 / 30；覆盖率 87.58%                                                            | ✅                     |
| 探针清理             | 8 探针逐条清除；注册表恢复原始 10 条                                                                                     | ✅                     |

**对根因更正的接受与独立复现**（回执 §2）：开发窗口接受「顺序敏感」更正并独立复现——**双方认知一致**。回执 §5 的三条判据教训（同一加工阶段 / 饱和量 / schema 形状）**质量高，照录**。

---

## 二、三项遗留裁断

### 1. README Mermaid 图 → **修，属 ① 范围**（授权）

开发窗口判「改图算顺带重排」而**未动**——**过度保守**。`README.md:139` 画的是 `invoke('backup_registry')` + `Promise.allSettled([save_system, save_user])`，`:161` 画 `save_system_paths(new_list)`——**都是收口前被本波替换掉的流程**。这与命令表是**同一类**过时（文档描述已不存在的流程），**不是重排**。

**授权**：同步两张 Mermaid 图至收口后流程（GUI：单次 `save_path_with_sidecar`；CLI：经 `save_path_with_sidecar` 编排）。

### 2. `pending_path_snapshot.json.bak` → **授权删除**

`~/.patheditor/pending_path_snapshot.json.bak`（03:07）是开发窗口回归时 persist 层写 pending 前的同名旧文件备份，**非用户数据**。用户 **2026-10-17 授权删除**。**执行者**：开发窗口。

### 3. CLI 并发校验无实际约束力 → **并入 Wave 3.1**，不单独动

回执 §4 回归 3 发现成立，且比所述更彻底：CLI 每次命令重载快照 → `expected == current` → 校验**从不因真实并发触发**，只可能因**顺序差异误报**。修复 K1 后 CLI 侧**完全惰性**。

已折入 Wave 3.1 spec「附二」（**待裁断：CLI 传 `expected: None`？建议是**——CLI 无并发窗口，校验留给 GUI）。

---

## 三、授权记录

| 项                                                    | 授权                  | 执行者   |
| ----------------------------------------------------- | --------------------- | -------- |
| 改 `CLAUDE.md`/`AGENTS.md`/`README.md`/`CHANGELOG.md` | ✅（第 3 轮）         | 开发窗口 |
| README Mermaid 图同步                                 | ✅（本轮）            | 开发窗口 |
| 删 `pending_path_snapshot.json.bak`                   | ✅（本轮）            | 开发窗口 |
| 一次性对齐 `disabled.json`                            | ✅（第 3 轮，已完成） | 开发窗口 |
| **Wave 3.1 开工**                                     | ✅（本轮）            | 开发窗口 |
| 提交 / 推送 / 升版本号                                | ❌ **未授权**         | ——       |

---

## 四、Wave 3.0 收官

**F-08 收口完成**：`core::service::save_path_with_sidecar` 成为 GUI + CLI **唯一**保存编排；`PathHiveIo` 缝使成功路径可单测；死码清理完成（3 模块 + 3 命令 + 3 CLI 函数）。

- **未覆盖项（如实登记，不兜底）**：GUI 真机保存未验（E2E mock 覆盖）；顺序敏感**代码修复未做**（Wave 3.1，本波仅数据对齐解阻——外部再重排 PATH 死锁会重现）；`backup_registry_default_dir_no_panic`（另开，P1）。
- **未提交**（用户未授权）：工作树 15 M + 7 D + 3 untracked。

---

## 五、Wave 3.1 开工

- **spec 就绪**：`docs/superpowers/specs/2026-10-17-path-conflict-order-sensitivity-design.md`（含「附二」CLI 待裁断）。
- **plan 就绪**：`docs/superpowers/plans/2026-10-17-path-conflict-order-sensitivity-implementation.md`（本轮产出）。
- **下一步**：开发窗口按 Wave 3.1 plan 实施（改代码 + 质量门；**未授权提交**）。

## 六、交接链状态

| 文档                                          | 状态        |
| --------------------------------------------- | ----------- |
| 报告 / 两轮核对 / 三轮裁断 / 回执（Wave 3.0） | ✅ 全部在库 |
| **本终裁（第 4 轮）**                         | ✅ 本文件   |
| Wave 3.1 spec                                 | ✅ 已就绪   |
| Wave 3.1 plan                                 | ✅ 已就绪   |
