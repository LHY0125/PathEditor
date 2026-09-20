# PathEditor Wave 1（数据一致性 F-01/F-02/F-03/F-05）审查报告

- **审查日期**：2026-09-19
- **被审分支**：`worktree-wave1`（`367df44`），检出于 `.claude/worktrees/wave1`
- **基准点**：`fa5c842`（Wave 0 归档后的 main）
- **范围**：12 个提交，30 个文件（+914 / −169；其中 Rust +601/−116，前端 +TS 若干，文档 4 处 + 闭环测试记录 1 份新增）
- **功能**：F-02 真 force API + CLI 接线；F-01 `RevealedValue` + 前端双层陈旧防护；F-03 CLI pending 最小闭环（5 入口 flush）；F-05 `capturedAt` + 措辞
- **规格来源**：`docs/superpowers/specs/2026-09-18-consistency-and-architecture-design.md` §4 F-01/F-02/F-03(Wave 1)/F-05
- **实施计划**：`docs/superpowers/plans/2026-09-18-wave1-consistency-fixes-implementation.md`（Execution Notes 已回填 7 行）
- **规范来源**：`AGENTS.md` / `CLAUDE.md`、`CONTRIBUTING.md`
- **复现命令**：`git diff fa5c842..worktree-wave1`；质量门在 worktree 内独立执行

## 提交清单

```
367df44 fix: toggle 入口补 flush、persist 成功后清 pending、弹窗重取容错
3b35cfd docs: 闭环测试记录标注临时工件易失性
d79fafa docs: 归档 Wave 1 闭环测试记录并回填 Execution Notes
346e86d fix(cli): import 与 profile apply 入口补写 pending，防陈旧覆盖
bc8bf5a fix(cli): sidecar 写失败落 pending 待补写状态，PATH 命令启动时自动补写
fdf51e9 feat(core): 新增 PATH 快照待补写原语（pending）
fc7c206 feat(core): 环境变量快照增加 capturedAt 采集时刻字段
58c2a0a fix(ui): 编辑弹窗绑定读值 revision，陈旧值不得覆盖外部更新
acdc63c feat(core): reveal_env_var 返回明文与读取时 revision
d04ec05 docs: 统一 --force 为最后写入者胜语义，修正退出码说明
1a671a7 fix(cli): --force 改调 core force API，删除重读 revision 的模拟实现
d59d880 feat(core): 新增 update/delete_env_var_force，显式豁免 revision 校验
```

## 一、验证证据（审核窗口独立复跑，未采信回执数字）

| 检查项                                                  | 结果                                              | 说明                                                                          |
| ------------------------------------------------------- | ------------------------------------------------- | ----------------------------------------------------------------------------- |
| `cargo fmt --all -- --check`                            | ✅                                                |                                                                               |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ 零警告                                         |                                                                               |
| `cargo test --workspace`                                | ✅ **113 + 41 = 154 passed；2 ignored；0 failed** | 回执写 152，见判断性意见 1                                                    |
| `cargo test -p patheditor-cli --bins`                   | ✅ 41 passed                                      |                                                                               |
| `npm test`（Vitest）                                    | ✅ 216 passed（18 files）                         | 与回执一致                                                                    |
| `npm run test:e2e`（Playwright）                        | ✅ **24 passed**（含 F-01 陈旧拦截用例）          | 与回执一致；首次裸 `npx playwright test` 因环境差异输出混乱，改用项目脚本复跑 |
| `npx tsc --noEmit`                                      | ✅                                                |                                                                               |

真实注册表闭环的本地证据核对（审核窗口在本机验证，非仅读文档）：

| 核对项                                                      | 结果                         |
| ----------------------------------------------------------- | ---------------------------- |
| 备份文件 `path_backup_20260919_180215_566.txt` 存在         | ✅ 2.3KB，时间戳吻合         |
| `~/.patheditor/pending_path_snapshot.json` 不存在（无残留） | ✅                           |
| `PATHEDITOR_W1_TEST` / `PATHEDITOR_W1_RV` 已清除            | ✅ `env get` 双双 os error 2 |
| 闭环测试记录已归档为文档且标注临时工件易失性                | ✅ `3b35cfd`                 |

定点核查（逐处看到代码）：

| 核查项                                                                                                                                                                          | 结果                                                                                                   |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| F-02：`update_env_var_force` / `delete_env_var_force` 不做 revision 比对，安全链（validate/is_reserved/is_protected/Unsupported/写权限）逐项在位                                | ✅ registry.rs:568-625                                                                                 |
| F-02：CLI `Concurrency::Force` 直接调 force API，「重读 revision」模拟实现（`current_revision`/`find_revision`/`expected_revision`）已删除                                      | ✅ env_ops.rs（连带删除 2 个过时单测）                                                                 |
| F-02：文档四处同步（AGENTS/CLAUDE/README/2026-09-17 spec）措辞一致「最后写入者胜 + 不产生退出码 3」                                                                             | ✅                                                                                                     |
| F-01：`reveal_env_var` 全链返回 `RevealedValue{value,revision}`，revision 用与列表一致的 `revision_of(name, vtype, value)` 计算                                                 | ✅ registry.rs:395-400                                                                                 |
| F-01：`backend.ts` `parseRevealedValue` 白名单构造 + 空 revision 拒绝；`parseEnvVarSnapshot` capturedAt 缺失回退 0                                                              | ✅                                                                                                     |
| F-01：弹窗提交前断言 `readRevision !== current.revision` → 重取 + 提示 + **不调用** `update_env_var`；重取失败有容错（不会永久禁用按钮）                                        | ✅ EditEnvVarDialog.tsx:74-95                                                                          |
| F-01：`env-store.save(meta, readRevision)` 二次校验（store 层兜底）+ core CAS 兜底，三层防护                                                                                    | ✅                                                                                                     |
| F-03：pending 四原语落 `disabled.rs`，覆盖式整份落盘；`persist_snapshot` 失败先填 None 侧再落 pending（None→空数组陷阱已正确处理）                                              | ✅                                                                                                     |
| F-03：flush 入口 5 处（`load_and_save`/`load_operate_save`/`cmd_import`/`profile_apply`/`cmd_toggle`），即**全部 persist 路径**；`clean` 本就不 persist（基线一致，非本波遗漏） | ✅                                                                                                     |
| F-05：`captured_at` Unix 毫秒、clock 异常回退 0；注释「两个接近时刻的快照，不是原子一致快照」                                                                                   | ✅                                                                                                     |
| 广播纪律：`broadcast_env_change` 恰 5 处公开包装（update/create/delete + 两个 force），`*_in_store` 内无                                                                        | ✅ registry.rs:412/469/525/572/611                                                                     |
| `pub fn` / `pub(crate) fn` 文档注释                                                                                                                                             | ✅ awk 全量检查无缺失                                                                                  |
| Conventional Commits                                                                                                                                                            | ✅ 12/12 合规                                                                                          |
| `AGENTS.md` ↔ `CLAUDE.md`                                                                                                                                                       | ✅ 字节级一致（`git show` 逐字节 diff）                                                                |
| 前端无绕过 `backend.ts` 的 `invoke`                                                                                                                                             | ✅                                                                                                     |
| `package-lock.json` 版本                                                                                                                                                        | 分支 HEAD 仍为 5.1.2（main 固有，未纳入分支——处理正确）；worktree 内有未提交的 lock 同步改动，不入合并 |

## 二、Standards 轴（规范符合性）

### 硬性违反

无。

### 判断性意见（均为 Minor，不阻塞合并）

1. **回执测试数字自相矛盾**：回执与闭环测试记录均写「152 passed / 2 ignored」，但同处给出的口径是「core 113 + CLI 41」= **154**。审核窗口独立复跑实测 **154 passed / 2 ignored**（分支 tip `367df44`）。数字偏小无害，但连续两波出现回执数与自证数据对不上（Wave 0 是 `#[ignore]` 计数），建议归档时修正为 154。
2. **开发回执再次未落文件**：本轮交付仍以聊天摘要给出，仅闭环测试记录落了文档（`PathEditor-Wave1数据一致性闭环测试记录.md`）。Wave 0 已登记过同一问题，**复发**。建议 Wave 2 起把回执落盘固化为开发窗口的交付步骤。
3. **`parseRevealedValue` 拒绝路径无直接单测**：`tests/unit/backend-env-contract.test.ts` 只覆盖 `EnvVarMeta` 契约（8 个用例），新增的 `RevealedValue` 校验（非 record 拒绝、空 revision 拒绝）与 capturedAt 回退 0 都没有直接用例，只被 env-store 测试间接经过。防护本身实现了��且 F-06 会重构错误契约——判为可延后，建议并入 Wave 2 的收口文档/测试任务。
4. **pending 文件无 schemaVersion**：`pending_path_snapshot.json` 是新持久化文件，与 `disabled.json` 一样无版本头——这是 F-11（Wave 2）的既定范围，登记 F-11 覆盖清单时须把 pending 文件列入，避免漏掉新成员。

### 合规亮点

- **Task 8 的 None→空数组陷阱是本轮最有价值的自主发现**：`save_pending_path_snapshot` 的 `None` 语义若被原样使用，补写时会把未操作的 hive 清成空数组。开发窗口在实现时识别、填充 None 侧后再落盘，并在 Execution Notes 留痕。这是真正理解语义后才写得出的代码。
- **真实注册表闭环的收尾纪律**：备份落盘、双 hive 前后快照对比零差异、临时变量清除、pending 预检/终检、临时工件清理前先给归档记录加易失性标注（`3b35cfd`）——CLAUDE.md 对真实写入的全部记录要求逐条满足，且「快照重排插曲」如实记录而没有美化。
- **广播纪律延续**：force API 的广播也只放在公开包装层，测试路径零 `WM_SETTINGCHANGE`，Wave 0 建立的边界没有因新增 API 而破口。
- **文档同步四处一致**：`--force` 语义变更同步了 AGENTS/CLAUDE/README/2026-09-17 spec，且退出码表述精确到「仅 `--revision` 模式产生退出码 3」。
- **Execution Notes 如实**：R1~R5 全部记录，包括「测试结构与 brief 不同」「修复面比预估大」这类不利偏离。
- **不可达检查带注释保留**（R5）：与 Wave 0 的 O-3 先例一致，宁可留防御性代码也不留无解释的静默。

## 三、Spec 轴（规格符合性）

### (a) 规格要求但缺失或只做一半

1. **F-03 的故障注入测试只做了一半**：spec 要求「磁盘写失败、系统成功/用户失败、快照成功/注册表失败」三类注入测试。实际落地：pending 生命周期单测 + 错误文案单测 + 正常路径不误留 pending 的行为验证（真实注册表闭环）；**真正的故障注入**（模拟 `save_path_snapshot` 失败后 flush 重放）没有测试，闭环记录 §5 已如实登记。属 Wave 1 最小闭环内的已知让步，判可延后，但须转入 Wave 2 F-08 的测试清单，不得静默蒸发。
2. **Wave 0 遗留项未顺带处理**：`read_env_var` 解码/Unsupported 错误不带 hive 标签（Wave 0 报告后续登记第 2 条，「Wave 1 顺带处理」）。Wave 1 计划与交付均未包含，开发窗口也未声称——**责任在审核窗口**：登记时没有把它写进 Wave 1 计划。转入 Wave 2 登记，随 F-06 错误重构一并处理。

### (b) 规格没要求但做了的行为（scope creep）

1. **persist 成功后防御性 clear pending**：spec 未要求；属合理强化（「成功 persist ⇒ 无 pending」不变量），实现带警告不阻断，且代价分析正确（失败时最坏幂等重放已被取代的旧状态）。合理但未在规格内，特此登记。
2. **`cmd_toggle` 补 flush（第五入口）**：spec 的「任一 PATH 写命令」本就覆盖，是把计划 brief 的两入口清单纠回 spec 口径，不算越界。

### (c) 实现与规格/计划不符

1. **F-01「快照刷新时」提示的时机**：spec 原文要求弹窗打开期间快照刷新即提示并刷新输入框；实现为**提交时**断言并重取+提示。防护不变量（旧值绝不静默提交、绝不覆盖外部新值）完整成立，E2E 直接验证「不调用 update_env_var」；差异仅在用户体验时机（保存动作触发 vs 刷新触发）。判为等效实现，登记措辞差异；若后续要改为推送式提示，属增强不属于修复。
2. **弹窗 catch 分支用未本地化原始错误串**、**`readRevision === null` 旁路**（fetch 失败时弹窗降级，由 core CAS 兜底）：开发窗口已主动登记为遗留，审核确认旁路场景下 core 侧 CAS 仍然生效，无静默覆盖风险。可延后。

## 四、总结

- **Standards 轴**：0 硬性违反，4 项 Minor（回执计数矛盾、回执未落文件复发、RevealedValue 契约测试缺口、pending 无 schemaVersion），多项合规亮点。
- **Spec 轴**：F-01/F-02/F-05 全部落地且逐点对齐；F-03 最小闭环达成，故障注入测试让步并如实登记；两处登记项转移（hive 标签 → Wave 2，pending schemaVersion → F-11 清单）。
- **质量门**：审核窗口独立复现全绿；真实注册表闭环的本地证据（备份文件存在、零残留、临时变量已清）逐项核实。
- **裁决复核**：开发窗口 9 项裁决（R1~R5、Task 8 None 语义、flush 覆盖面、防御性 clear、易失性标注）逐条核实，全部成立且代价分析如实。

### 审核结论

**通过（Ready to Merge）。** 建议以 `git merge --no-ff worktree-wave1` 合入 main（基准 `fa5c842` 之上 main 无新增提交，无冲突面）。合并后 `worktree-wave1` 分支与 `.claude/worktrees/wave1` 可清理（worktree 内有未提交的 package-lock 同步改动，可再生，清理时直接丢弃即可）。

### 后续登记（不阻塞合并）

1. 回执落盘固化为开发窗口交付步骤（Minor 2，复发项）。
2. `parseRevealedValue` / capturedAt 回退的直接契约测试 → Wave 2 收口文档任务。
3. `read_env_var` 错误 hive 标签 → Wave 2，随 F-06。
4. pending 文件纳入 F-11 schemaVersion 覆盖清单。
5. F-03 故障注入测试 → Wave 2 F-08 测试清单。
6. IPC 文档表 `RevealedValue` 签名 + clap 帮助文本（开发窗口自报 Minor #4/#5）→ Wave 2 收口文档任务。

本报告基于审核窗口实际执行的命令输出与逐处代码核查，未采信任何未经复现的结论。
