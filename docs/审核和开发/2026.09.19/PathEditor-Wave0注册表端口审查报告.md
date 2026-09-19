# PathEditor Wave 0（注册表端口 + F-04）审查报告

- **审查日期**：2026-09-19
- **被审分支**：`worktree-wave0`（`af184b0`），检出于 `.claude/worktrees/wave0`
- **基准点**：`80e2ccd`（分支起点）；当前 `main` @ `40058ef`（未合并）
- **范围**：6 个提交，4 个文件（`core/src/lib.rs` +1、`core/src/reg_store.rs` 新增 300、`core/src/registry.rs` ±586、Wave 0 计划文档回填 12 行），+577 / −322
- **功能**：`EnvHiveStore` 存储端口（生产 `WinregHive` / 测试 `MemoryHive`）+ F-04 列表失败不再静默 + F-05 快照措辞 + 测试去真实 HKCU
- **规格来源**：`docs/superpowers/specs/2026-09-18-consistency-and-architecture-design.md`（§3/§4/§7、§10 基线）
- **实施计划**：`docs/superpowers/plans/2026-09-18-wave0-registry-port-implementation.md`（含开发窗口评审裁断 O-1~O-7 与 Execution Notes 7 行）
- **规范来源**：`AGENTS.md` / `CLAUDE.md`、`CONTRIBUTING.md`
- **复现命令**：`git diff 80e2ccd..worktree-wave0`；质量门在 worktree 内执行

## 提交清单

```
af184b0 fix(core): F-04 列表错误携带 hive 标识并恢复 warning 日志；回填 Execution Notes
e5177e2 chore(core): 清理复审 minor —— 修正休眠测试注释指针，移除过时的 dead_code 属性
65e1d13 refactor(core): 环境变量读写改走 EnvHiveStore 端口，列表失败不再静默
5c4054b test(core): 新增内存 hive 测试替身与故障注入能力
daba84d docs(core): 补 hive_location 文档注释
ff5b4f8 feat(core): 新增环境变量存储端口 EnvHiveStore 与 WinregHive
```

## 一、验证证据（审核窗口在 worktree 内独立复跑，未采信回执数字）

| 检查项                                                  | 结果                                              | 说明       |
| ------------------------------------------------------- | ------------------------------------------------- | ---------- |
| `cargo fmt --all -- --check`                            | ✅                                                |            |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ 零警告                                         |            |
| `cargo test --workspace`                                | ✅ **106 + 41 = 147 passed；2 ignored；0 failed** | 与回执一致 |
| `cargo test -p patheditor-cli --bins`                   | ✅ 41 passed                                      |            |
| `npm test`                                              | ✅ 213 passed（18 files）                         |            |

定点核查（逐项看到代码，非抽样）：

| 核查项                                                                               | 结果                                                                                                                                  |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| F-04：`enum_names` / `get_raw` / 解码三处失败均 `?` 传播，**不再 `warn + continue`** | ✅ `list_env_vars_in_store`（registry.rs:299 起），错误文案带 hive 标签（「读取系统/用户环境变量列表失败」）                          |
| F-04：`log::warn!` 恢复（spec 原文「warning 保留，但不再是唯一反馈」）               | ✅ 三处 map_err 内均有                                                                                                                |
| `enum_values().flatten()` 吞错残留                                                   | ✅ 无（仅 create 的 doc 注释里作为历史说明引用，评审 O-2 标注的语义变更）                                                             |
| `continue` 残留                                                                      | ✅ 仅两处且合法：`clean_path_entries` 的 PATH 去重（非本通路）、保留名 `Path` 过滤（正确行为）                                        |
| `broadcast_env_change` 只在公开包装                                                  | ✅ 恰好 3 处（update:405 / create:462 / delete:518），`*_in_store` 内无 → 测试路径零系统广播                                          |
| F-05：`list_all_env_vars` 注释                                                       | ✅ 已改为「两个接近时刻的快照，不是原子一致快照」                                                                                     |
| `env_key` 删除                                                                       | ✅ 已删（延后到 list 端口化之后，裁决 P-1；clippy -D warnings 全绿旁证无 dead_code）                                                  |
| HKCU 写入脚手架                                                                      | ✅ `env_var_tests` 的 `TempRegistryKey` 已全部移除；仅剩 `issue26_tests` 的既有 `#[ignore]`（registry.rs:681）                        |
| reg_store 冒烟测试                                                                   | ✅ 只读（open + `enum_names`），不写注册表                                                                                            |
| `pub fn` 文档注释                                                                    | ✅ awk 全量检查，registry.rs / reg_store.rs / env_var.rs 无缺失                                                                       |
| P-5 裁决核实                                                                         | ✅ winreg-0.52.0/src/types.rs:38 确为 `String::from_utf16_lossy`——解码失败分支对字符串类型不可达属实；休眠测试 `#[ignore]` 带根因注释 |
| Execution Notes                                                                      | ✅ 7 行回填，含 5 项裁决与偏离，与代码一致                                                                                            |
| 提交信息                                                                             | ✅ Conventional Commits 合规                                                                                                          |
| `AGENTS.md` / `CLAUDE.md`                                                            | ✅ 本分支未触碰                                                                                                                       |

## 二、Standards 轴（规范符合性）

### 硬性违反

无。

### 判断性意见（均为 Minor，不阻塞）

1. **回执措辞与自身数据矛盾**：回执称「仅剩 1 个 `#[ignore]` 保留项」，但同一回执的质量门数字是 **2 ignored**，实际也是 2 个（`issue26`@681 写 HKCU 的既有保留项 + 休眠解码测试@1020，后者用 `MemoryHive` 不写 HKCU）。回执括号里限定了「写 HKCU 的脚手架」尚可辩解，但读者按字面核对测试输出会对不上。建议回执归档时改写为「2 个 `#[ignore]`：1 个既有 HKCU 保留项 + 1 个休眠用例」。
2. **Execution Notes 表含乱码残留**：`docs/superpowers/plans/2026-09-18-wave0-registry-port-implementation.md:1240` 处理列出现「（`new_notebook.py` 不可满足…）」——与内容无关的模板损坏残留，建议随下次文档提交清理。
3. **开发回执未落文件**：本次交付以聊天摘要给出，未按流程写 `docs/审核和开发/2026.09.19/PathEditor-Wave0开发回执.md`。内容本身完整（摘要 + 裁决 + 遗留），但两窗口协作依赖文档留痕，建议归档（可由开发窗口按聊天摘要落文件，或由审核窗口代归并注明来源）。

### 合规亮点

- **质量门独立复现**：审核窗口在 worktree 内全量重跑，与回执数字逐项一致。
- **F-04 语义实现与 spec 原文逐字对齐**：传播 + hive 标签 + warn 保留，无一处静默吞错。
- **`broadcast_env_change` 上移**：测试路径不再向系统发送 `WM_SETTINGCHANGE`，这是计划里刻意的设计，落实到位。
- **P-5 裁决有源码级证据**：lossy 解码结论直接引到 winreg 源码行号，休眠测试保留根因注释而非删除——这正是「如实措辞」文化该有的样子。
- **Execution Notes 如实**：连「brief 代码不能原样编译」这类对自己不利的偏离也记录了（`let hive`→`let mut hive`、import 位置、`#[allow(dead_code)]` 的去留）。
- **分支纪律**：只动 core 与计划文档，gui/cli 零改动——CLI/GUI 零安全判定约束平凡成立；worktree 内开发、未推送、未升级版本号。

## 三、Spec 轴（规格符合性）

### (a) 规格要求但缺失或只做一半

无。F-04 / F-09 / F-05（措辞部分）全部落地；F-05 的 `capturedAt` 字段按 spec §3 波次表本就划归 Wave 1。

### (b) 规格没要求但做了的行为（scope creep）

无实质越界。错误文案带 hive 标签与 `warn` 恢复属最终复审修复，spec F-04 原文支持。

### (c) 实现与规格/计划不符

1. **spec 自身命名不一致（P-4，规格缺陷而非实现缺陷）**：spec §3 用 `EnvHiveStore`，§4 F-09 草稿用 `RegistryStore`/`WinregStore`/`MemoryStore` 且方法名不同（`enum_values`/`get_raw_value`/`view` 参数）。开发窗口取更具体且与计划一致的 `EnvHiveStore`，正确。**审核中已由规格所有者修正 spec**（§4 trait 草图与 §7 验收标准改为实现命名，并留勘误注记）；spec §4 中「`get_raw_value` 带 `RegistryView` 参数、错误类型 `CoreError`」的两处超前设计（属于 Wave 1 F-06/F-09 后续范围）一并去除，避免再次误导。
2. **解码失败分支不可达（P-5）**：winreg 0.52 对字符串类型 lossy 解码，损坏 UTF-16 静默变 U+FFFD。这是 **Wave 0 之前就有的行为，无回归**；是否在 core 自行做严格 UTF-16 校验属产品决策，登记为 F-06/Wave 2 候选项，不判为缺陷。

### (d) 额外发现（非本分支引入，登记在案）

**package-lock.json 版本不同步是 main 固有问题**：基线 `80e2ccd` 上 `package.json` = 5.1.3 而 `package-lock.json` = 5.1.2（两处 version 字段均为 5.1.2）。开发窗口如实披露且未纳入分支，处理正确。根因是 CLAUDE.md「版本号升级清单」**不含 package-lock.json**——v5.1.3 升级时漏跑了 lock 同步。建议：下次发版前 `npm install` 刷新 lock，并把 `package-lock.json` 加入升级清单（属发布流程修正，另立处理）。

## 四、总结

- **Standards 轴**：0 硬性违反，3 项 Minor（回执措辞、文档乱码、回执未落文件），多项合规亮点。
- **Spec 轴**：F-04/F-09/F-05 全部落地且与 spec 对齐；唯一不符项是 spec 自身的命名不一致（已在审核中修正 spec 本体）。
- **质量门**：审核窗口独立复现全绿。
- **裁决复核**：开发窗口 5 项裁决（P-1~P-5）逐条核实，全部成立且记录完整。

### 审核结论

**通过（Ready to Merge）。** 建议以 `git merge --no-ff worktree-wave0` 合入 main（分支基于 80e2ccd，main 侧 `40058ef` 只新增 `.agents/` 文档，无冲突面）；合并后 `worktree-wave0` 分支与 `.claude/worktrees/wave0` 可删。合并属影响 main 的操作，**待用户发话**。

### 后续登记（不阻塞合并）

1. 开发回执归档为文件（见判断性意见 3）。
2. `read_env_var` 单值读取路径的解码错误不带 hive 标签（开发窗口自报遗留）→ Wave 1 顺带处理。
3. `MemoryHive` 在 `#[cfg(test)]` 下，Wave 1 如需在 core 外做端口测试，按 F-07 迁至 `test_adapter`。
4. 严格 UTF-16 校验的取舍 → 随 F-06/Wave 2 决策。
5. package-lock 同步 + 版本升级清单补条目 → 发布流程修正，独立处理。

本报告基于审核窗口实际执行的命令输出与逐处代码核查，未采信任何未经复现的结论。
