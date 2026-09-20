# PathEditor Wave 2（架构收口 F-06/F-07/F-08/F-10/F-11）审查报告

- **审查日期**：2026-09-20
- **被审分支**：`worktree-wave2`（`aa9864c`），检出于 `.claude/worktrees/wave2`
- **基准点**：`a3bb7e9`（Wave 1 归档 + Wave 2 计划折入核对轮裁断后的 main）
- **范围**：12 个提交，68 个文件（+4609 / −1542）
- **功能**：F-06 `CoreError` 贯通四层；F-07 `registry.rs` 拆目录（模块根形态）；F-08 共享应用服务层（CLI 侧）；F-10 golden 基线 25 用例；F-11 持久化 schemaVersion/.bak/quarantine
- **规格来源**：`docs/superpowers/specs/2026-09-18-consistency-and-architecture-design.md` §4（Wave 2 各项）+ §7 验收标准
- **实施计划**：`docs/superpowers/plans/2026-09-18-wave2-architecture-consolidation-implementation.md`（含 2026-09-20 核对轮 W2-B1~B4/N1~N6 裁断）
- **规范来源**：`AGENTS.md` / `CLAUDE.md`、`CONTRIBUTING.md`
- **复现命令**：`git diff a3bb7e9..worktree-wave2`；质量门在 worktree 内独立执行
- **开发回执**：`docs/审核和开发/2026.09.20/PathEditor-Wave2架构收口开发回执.md`（**已落盘**——Wave 0/1 两次登记的流程缺口就此修复）

## 提交清单

```
aa9864c docs: Wave 2 终审修复——F-08 GUI 延后披露、冲突判定契约文档更正与兜底细节
49a0dea fix: CLAUDE.md 表格列宽与 AGENTS.md 对齐（lint-staged 单侧格式化分叉）
a0a14db chore: Wave 2 架构收口质量门与文档同步
69e2159 test(core): golden 备份用例补齐逐行与文件名断言
28680bd test(core): 新增 C→Rust 行为等价 golden 基线
eb995ae feat(core): 持久化文件增加 schemaVersion、.bak 轮换与损坏隔离，错误迁移 CoreError
d7d049b feat(core): 新增共享应用服务层，统一 PATH/profile 事务编排
05a966f refactor(core): registry.rs 拆分为 registry/ 目录模块（纯搬家零行为变化）
a321913 refactor: 四层错误判定改按 CoreError.code，CLI 退出码由 exit_code 驱动（F-06 Wave 2 Task 3）
99cf9e4 refactor(core): 环境变量通路错误迁移到 CoreError（F-06 Wave 2 Task 2）
65b30b2 feat(core): 新增结构化错误契约 CoreError/ErrorCode
```

（注：`git log` 列出 11 个提交，回执称 12——差异是回执把分支起点前的一个 docs 提交计入或计数口径不同，不影响合并判断。）

## 一、验证证据（审核窗口独立复跑，未采信回执与终审数字）

| 检查项                                                  | 结果                                                          | 说明                       |
| ------------------------------------------------------- | ------------------------------------------------------------- | -------------------------- |
| `cargo fmt --all -- --check`                            | ✅                                                            |                            |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ 零警告                                                     |                            |
| `cargo test --workspace`                                | ✅ **192 passed / 2 ignored / 0 failed**（core 151 + CLI 41） | 与回执一致                 |
| `cargo test -p patheditor-cli --bins`                   | ✅ 41 passed                                                  |                            |
| `cargo test -p path-editor-core golden`                 | ✅ **25 passed / 0 failed**                                   | golden 专项                |
| `npm test`（Vitest）                                    | ✅ **235 passed（18 files）**                                 | 回执写 234，见判断性意见 1 |
| `npm run test:e2e`（Playwright）                        | ✅ 24 passed                                                  |                            |
| `npx tsc --noEmit`                                      | ✅                                                            |                            |

真实注册表闭环的本地证据核对（审核窗口在本机验证）：

| 核对项                                          | 结果                    |
| ----------------------------------------------- | ----------------------- |
| 备份 `path_backup_20260920_152446_043.txt` 存在 | ✅ 2.3KB，时间戳吻合    |
| `PATHEDITOR_W2_TEST` 已清除                     | ✅ `env get` os error 2 |
| pending 文件无残留                              | ✅                      |
| system hive 无提权如实记 skipped                | ✅（与闭环记录一致）    |

定点核查（逐处看到代码）：

| 核查项                                                                                                                                                                                              | 结果                                                                                                                            |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| F-06：`error.rs` 12 变体 + `exit_code()`（Conflict→3）+ serde camelCase；`conflict_maps_to_exit_code_3` 契约测试                                                                                    | ✅                                                                                                                              |
| F-06：判定全面 code 化——前端 `err.code === 'conflict'`（env-store.ts:23）、CLI `apply_concurrency(Result<(), CoreError>)`；`[E_CONFLICT]` 前缀仅存于契约测试断言与展示文本                          | ✅                                                                                                                              |
| F-06：`read_env_var` 三分类落地（Io/UnsupportedType/Parse），env_var.rs 54 处 CoreError 构造                                                                                                        | ✅                                                                                                                              |
| F-06：`WinregHive::open` 按 `ErrorKind::PermissionDenied` 诚实分类（W2-N2 裁断落地）                                                                                                                | ✅                                                                                                                              |
| F-07：`registry.rs` 保留为模块根（35 行，纯 mod 声明 + 重导出），六子模块就位，**零文件删除**（W2-N6 首选形态落地）                                                                                 | ✅                                                                                                                              |
| F-07：外部路径兼容——`pub use` 重导出与拆分前签名一致；43 个既有测试随迁移全绿                                                                                                                       | ✅                                                                                                                              |
| F-08：`service.rs` 四个 pub 服务函数 + `ApplyOutcome{system,user,sidecar}` 按 hive 分字段；`SidecarOutcome::Pending(CoreError)` 携带错误细节（偏离披露，回执 §3 已登记）                            | ✅                                                                                                                              |
| F-08：CLI pending 编排下沉——`flush_pending_snapshot` 转调 `core::service::retry_pending_path_state`，5 入口 flush 全保留；**服务层报错 / CLI 警告不阻断**（W2-N1 分层裁断落地，runtime.rs:192-193） | ✅                                                                                                                              |
| F-08：GUI 侧 4 个 service IPC 命令已在 `gui/src/lib.rs` 注册，`path-session.ts` 未接线——**与回执 §5 未覆盖项、CLAUDE.md 注记、spec 三处披露一致**                                                   | ✅                                                                                                                              |
| F-10：golden 25 用例 + 真实函数 runner；`system_lines`/`filename_pattern` else-panic 守卫（Task 8 修复落地）；备份用例逐行断言 + 文件名契约                                                         | ✅                                                                                                                              |
| F-11：`Versioned` 信封 + `rotate_backup`（写入前轮换 .bak，失败中止写入）+ `parse_error_quarantined`；disabled/profiles/pending 三文件全覆盖（disabled.rs:4-99、289 均见调用）                      | ✅                                                                                                                              |
| 广播纪律：env_var.rs 恰 5 处公开包装 + service.rs 1 处（`sys_applied \|\| usr_applied` 才广播）+ CLI toggle 保留——测试路径零广播边界未破                                                            | ✅                                                                                                                              |
| `pub fn` 文档注释                                                                                                                                                                                   | ✅ core/cli 全量 awk 检查无缺失；gui/commands 两文件为 main 固有缺口（见判断性意见 4）                                          |
| Conventional Commits                                                                                                                                                                                | ✅ 11/11 合规                                                                                                                   |
| `AGENTS.md` ↔ `CLAUDE.md`                                                                                                                                                                           | ✅ 字节级一致（含 `49a0dea` 修复后的表格）                                                                                      |
| lockfile                                                                                                                                                                                            | 分支上 package.json 与 package-lock.json 均为 5.1.3（main 固有的 lockfile 漂移就此顺带修复；混入 `99cf9e4` 的偏离已披露并追认） |

## 二、Standards 轴（规范符合性）

### 硬性违反

无。

### 判断性意见（均为 Minor，不阻塞合并）

1. **回执 Vitest 数字又一次 ±1**：回执写「234 passed」，审核实测 **235**。口径：`npm test` 全量 18 文件（回执 6 节的 234 是覆盖率模式下的分项）。连续第三波回执数字与自证数据有出入（Wave 0 ignored 计数、Wave 1 少 2、本轮差 1），模式已稳定：**回执数字来自部分完成态的中间值**。合并判断不受影响，但建议 Wave 起回执数字以收口轮终跑为准并注明口径。
2. **提交计数 11 vs 回执称 12**：见提交清单注，疑为计数口径（是否含收口轮合并前 docs 提交）。建议回执提交表逐行列 SHA（本轮已做到，仅总数口径未对齐）。
3. **`exit_persist_error` 与 `exit_core_error` 约 10 行重复**：开发窗口豁免理由（bin-only crate 无 lib、跨模块耦合代价）成立，豁免接受。若未来 CLI 引入公共模块可顺手合并。
4. **`gui/src/commands/{disabled,profiles}.rs` 的 `pub fn` 无 `///` 文档注释**：**main 固有缺口**（基线 a3bb7e9 对比确认，非本波引入），本波只加了 `.map_err` 透传。与 CONTRIBUTING.md 硬性要求不符，登记为独立小修（与「GUI 服务层接线」小波次同批处理合适）。
5. **README 测试徽章 234 vs 实测 235**：同判断性意见 1，README 数字随最后一次全量跑漂移。属文档-数字弱一致性问题，发版前统一刷新即可。

### 合规亮点

- **回执落盘固化**：Wave 0/1 两次登记的流程缺口在本轮修复，回执文档完整覆盖 Minor 处置三分类（已修/登记/豁免），**无任何一项被静默丢弃**——这正是双窗口文档化流程想要的样子。
- **核对轮裁断全部落地**：W2-B1（三分类）、W2-B2（注入改输入校验）、W2-N1（flush 分层）、W2-N2（ErrorKind 诚实分类）、W2-N6（模块根零删除）逐一在代码中找到对应实现，与裁断文字一致。
- **golden 守卫升级**：`system_lines`/`filename_pattern` 从布尔门改为 else-panic，测试失败会大声死而不是静默走错——这是对复审意见的正确回应方式。
- **GUI 未接线的诚实披露**：`path-session.ts` 未切到 service 层在三处文档（回执 §5、CLAUDE.md IPC 表注记、spec）同步披露，没有声称完成。
- **lockfile 偏离处理规范**：混入提交即披露、复核窗口追认、发版流程记忆更新——偏离处理本身成了流程样本。

## 三、Spec 轴（规格符合性）

### (a) 规格要求但缺失或只做一半

1. **F-08 GUI 接线未完成**：spec 要求「GUI/CLI 只负责输入转换…编排逻辑在同一处 Rust service」。CLI 侧完成；GUI `path-session.ts` 仍走旧编排（`_pendingSys/_pendingUser` 逻辑未删除）。**判可接受**：4 个 service IPC 命令已注册，接口先行、接线延后的保守路径合理，三处文档如实披露。但须明确登记为**唯一未闭环的 spec 条目**，不得在 spec 状态「已实现（Wave 2）」中消失——建议 spec 状态行注明「F-08 GUI 接线延后」。
2. **F-10 广播时机用例延后**：spec 六类行为中「WM_SETTINGCHANGE 广播时机」以「延后」标注（golden_tests.rs 无广播类用例）。回执如实登记。可接受——广播纪律已由代码审查确认（见定点核查），golden 补用例需广播端口注入（spec 604 行预留的方案），随 GUI 接线小波次一并处理合适。
3. **F-11 pending 文件的 quarantine 语义**：`load_pending_path_snapshot` 失败走 `parse_error_quarantined`（disabled.rs:289）——已覆盖，无缺口。此项核查通过，列出仅为说明「三文件全覆盖」的说法经得起核对。

### (b) 规格没要求但做了的行为（scope creep）

1. **`SidecarOutcome::Pending(CoreError)` 携带 payload**：简报为纯标记枚举。扩展有依据（不丢错误细节），两项 Minor 互相成就，披露完整。合理但未在规格内，特此登记。
2. **`exit_persist_error` 新函数**：属 F-11 错误迁移的必然产物，非越界。
3. **README 测试徽章更新**：Task 8 Step 1c 只要求「核对」，更新数字属合理顺手（虽引入了意见 5 的弱一致性问题）。

### (c) 实现与规格/计划不符

1. **`test_adapter.rs` 未创建**：计划 File Structure 列有此文件。开发窗口裁定「W2-B4 裁断后简报该项作废」——核对轮裁断原文只统一了 golden 路径，未明确作废 test_adapter；但 golden 测试直接放 `registry/golden_tests.rs` 后确无装配需求，**结果合理、引用链有瑕疵**。登记为计划-实现差异，不追溯。
2. **golden 目录落点**：计划写 `core/src/registry/golden_tests.rs` + `golden/`，实际 `core/src/registry/path/golden_tests.rs`（回执 Task 4 条目披露）。golden 测试覆盖 split/join/clean 均属 path 模块，落点合理。
3. **migrate 接受 `schemaVersion: 0` 按 v1**：与「旧格式仍可读取」契约一致，豁免理由成立。

## 四、总结

- **Standards 轴**：0 硬性违反，5 项 Minor（回执数字口径 ×2、exit_persist 重复豁免、gui commands 文档注释固有缺口、README 数字漂移），多项合规亮点。
- **Spec 轴**：F-06/F-07/F-10/F-11 全部落地且与裁断一致；F-08 CLI 侧完成、GUI 接线延后并三处披露——**这是本轮唯一未闭环的 spec 条目**，须在 spec 状态中显式注明。
- **质量门**：审核窗口独立复现全绿；真实注册表闭环本地证据（备份、零残留、临时变量清除）逐项核实。
- **偏离复核**：3 项显式偏离（lockfile、Pending payload、golden 守卫）全部披露完整、追认合理。

### 审核结论

**通过（Ready to Merge）。** 建议 `git merge --no-ff worktree-wave2` 合入 main。合并后 `worktree-wave2` 分支与 `.claude/worktrees/wave2` 可清理（worktree 内 2 个 review diff 临时文件随清理丢弃；main 工作区另有一处未提交的 README 格式化残迹，与本分支无关，合并时注意不要裹挟提交）。

### 后续登记（不阻塞合并）

1. **F-08 GUI 接线**：`path-session.ts` 切到 service 层 + 删除旧 pending 编排 + golden 广播用例（需广播端口注入）→ 独立小波次。
2. `gui/src/commands/{disabled,profiles}.rs` 补 `///` 文档注释（main 固有）→ 与上项同批。
3. spec 状态行显式注明「F-08 GUI 接线延后」→ 下次文档提交顺手处理。
4. README 测试徽章与回执数字统一以收口终跑为准 → 发版前刷新。
5. 三波回执数字口径漂移（模式化）→ 开发窗口回执数字注明统计口径与时间点。

本报告基于审核窗口实际执行的命令输出与逐处代码核查，未采信任何未经复现的结论（含开发窗口会话内的「终审」——其结论由本报告独立验证替代）。
