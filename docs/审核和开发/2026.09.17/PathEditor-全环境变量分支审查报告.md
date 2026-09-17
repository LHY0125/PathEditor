# PathEditor 全环境变量分支审查报告

- **审查日期**：2026-09-17
- **被审分支**：`worktree-all-env-vars`（37e7057）
- **基准点**：`main`（1c13daf），合并基与 main HEAD 相同
- **范围**：13 个提交，30 个文件，+3299 / −73 行
- **功能**：「全部变量」Tab —— 通用环境变量管理（Rust core 读写 + Tauri IPC + React UI + 单测/E2E）
- **规格来源**：`docs/superpowers/specs/2026-09-16-all-env-vars-design.md`（570 行）、`docs/superpowers/plans/2026-09-16-all-env-vars-implementation.md`（2835 行）
- **规范来源**：`AGENTS.md` / `CLAUDE.md`（两文件字节级一致）、`CONTRIBUTING.md`
- **复现命令**：`git diff main...worktree-all-env-vars`

## 提交清单

```
37e7057 docs: 同步全环境变量管理到开发指南与 README
57b3133 test(e2e): 覆盖「全部变量」Tab、打码、只读锁定与筛选
e12652e fix(ui): 修复环境变量编辑流与 Task 6/7 联合复审发现
f5b5a46 feat(ui): 接入「全部变量」Tab 与独立工具栏
bd84314 feat(ui): 新增环境变量表格与专用工具栏
7a4f867 test(env-store): 用 resetAllMocks 消除 mock 实现跨用例残留
0ee4db8 feat(frontend): 新增环境变量 store（revision 校验 + 敏感值按需 reveal）
64f0b3f fix(frontend): filterEnvVars 空查询分支返回副本，避免污染共享快照
17dd7e5 feat(frontend): 新增环境变量纯逻辑与 IPC 边界契约
d71910c feat(gui): 注册通用环境变量读写 command
b68e499 feat(core): 新增通用环境变量读写函数（含 revision 并发校验）
e366e1a feat(core): 新增通用环境变量契约与安全判定函数
```

## 一、验证证据（worktree 内实测）

| 检查项                     | 结果                                            |
| -------------------------- | ----------------------------------------------- |
| vitest 单测                | ✅ 200/200 通过（17 个文件）                    |
| cargo test --workspace     | ✅ 88 通过 / 0 失败 / 1 ignored                 |
| cargo clippy `-D warnings` | ✅ 通过                                         |
| cargo fmt --check          | ✅ 通过                                         |
| tsc -b                     | ✅ 通过                                         |
| eslint                     | ✅ 0 error，3 warning（均为既有虚拟列表库提示） |
| prettier format:check      | ✅ 通过                                         |
| Playwright e2e             | ✅ 22/22 通过（含 6 个新增 env-vars 用例）      |
| 覆盖率                     | env-var.ts 91.7%、env-store.ts 97.2%（行覆盖）  |

`npm run verify` 全链路绿灯。**注意**：绿灯 ≠ 规格场景全覆盖，见 Spec 轴 (a)1。

## 二、Standards 轴（规范符合性）

### 硬性违反

1. **gui/src/commands/env_var.rs:5-37** —— 5 个 `pub fn`（list_all_env_vars 等）均无 `///` 文档注释，违反 CONTRIBUTING.md「所有 `pub fn` 必须有 `///` 文档注释」。注：既有 gui/src/commands/registry.rs 同样缺失，属既有缺口，但新代码不应延续。

### 判断性意见

1. **src/store/env-store.ts:15,97,134** —— 用中文文案子串 `CONFLICT_MARKER = '已被其他进程修改'` 做控制流分支；e2e/mocks/ipc.ts:128 复刻同句。AGENTS.md 自认「自由文本 Result 是待完成项」，但新代码进一步加深文案依赖——将来改 Rust 错误文案会**静默破坏冲突检测路径**。
2. **core/src/registry.rs:317** —— 循环内逐变量调用 `capabilities_for(hive,…)`，每个变量重复执行 `check_admin()`/`can_write_user()` 并打开真实注册表键（N 变量 N 次键操作）。模块专为此提供的 `capabilities_for_with(writable,…)`（env_var.rs:208）未被使用，应在循环外探测一次复用。性能问题。
3. **core/src/registry.rs:415,440** —— 注释称「原子检查名称不存在」，实现是 `enum_values` 遍历后 `set_raw_value`，存在 TOCTOU 竞态窗口，注释言过其实。
4. **src/components/env-list/EnvVarTable.tsx:60-63** —— 前端再滤 `path`，与 env-var.ts 模块声明「安全判定单一实现在 Rust，不重复实现」自相矛盾。防御性可接受，但应有注释界定「判定 vs 防御」。
5. **src/core/env-var.ts:42,101,105** —— `maskValue`、`isHiveFilter`、`hiveOf` 无任何产品调用方（仅测试引用），属死导出。
6. **tests/unit/env-var-table.test.tsx** —— 与同分支另两个新测试文件策略不一致：手写 TRANSLATIONS 文案副本（app-shell-env-vars.test.tsx 用真实 zh-CN.json，无漂移风险）；且缺 `afterEach(cleanup)`——另两个文件均显式注明「未开启 vitest globals，必须手动 cleanup」，本文件靠查询恰好不会命中多值才未爆，存在脆弱性。
7. **src/components/layout/AppShell.tsx:271 vs src/store/env-store.ts** —— 保存失败后 AppShell `clearDraft`，而 store 冲突分支测试断言「冲突后草稿保留避免输入丢失」。两场景语义不同（弹窗本地留值），但同一动作两个相反草稿策略，宜统一说明。

### 合规亮点

- core 全部 pub fn 均有 `///` 文档；`src/core/env-var.ts` 零框架依赖、纯函数。
- 组件/Store 未直调 `invoke`，统一走 src/services/backend.ts；backend.ts 对 EnvVarMeta/Snapshot 做运行时形状校验（backend.ts:92-127）。
- E2E 全程 mock IPC，未写真实注册表。
- i18n 双语 key 完全同步（28 个 envVar 键两端一致）；tab 改名同步更新了 startup.spec。
- Conventional Commits 全部合规。
- AGENTS.md 与 CLAUDE.md 修改后字节级一致；新增 5 个 IPC 命令已补录 IPC 表格，文档修改合理且与实现相符。

## 三、Spec 轴（规格符合性）

### (a) 规格要求但缺失或只做一半

1. **Rust 测试大量缺席公开 API 场景**（最重发现）。设计文档 L477-486 要求覆盖：「list_all_env_vars 不含 path/Path」「敏感变量 preview=None」「revision 冲突拒绝」「REG_DWORD 拒绝写入且值未被修改」「windir 等保护变量拒绝写入」「非管理员系统 hive 只读」「原始名大小写保持」。实际 `core/src/registry.rs` 的 env_var_tests 新增段只测了内部辅助函数 `read_env_var`/`write_env_var`、`validate_env_name`/`validate_env_value`、类型映射与序列化；`update_env_var`/`delete_env_var`/`create_env_var`/`list_all_env_vars`/`reveal_env_var` **五个公开函数零直接调用测试**——Path 过滤（P0 安全项）、保护名单拒绝写入等核心点在 Rust 侧无验证。
2. **E2E 缺「allVars 下拖放无效」用例**（设计文档 L499 明确列出）。e2e/tests/env-vars.spec.ts 无拖放用例，仅单测 tests/unit/app-shell-env-vars.test.tsx:189 间接覆盖。
3. **关窗确认只做一半**（设计文档 L407 决策 4：「关闭确认需同时检查 env-store.draft.size > 0」）。AppShell.tsx:155 仅在工具栏 onCancel 路径检查 `hasDrafts()`；真实窗口关闭（X 按钮/Alt+F4）无 `onCloseRequested` 监听，未提交草稿会静默丢失。

### (b) 规格没要求但做了的行为（scope creep）

1. `src/core/env-var.ts:81-89` 的 `hiveOf` 被导出但全库无调用（计划 Task 4 草稿带入的死代码）；`isHiveFilter`（env-var.ts:99-101）同样无人使用。
2. 计划外新增组件：`EditEnvVarDialog.tsx`（计划 Task 7 只有 NewEnvVarDialog 的最小实现）、`StatusBar.tsx:16-24` 的 env 状态分流与重试逻辑、`NewEnvVarDialog` 的 `canWriteSystem` 禁用系统来源选项。属合理 UX 补全，但规格与计划均未要求。

### (c) 实现与规格/计划不符

1. **前端重复实现 hive 写权限判定**：EnvVarTable.tsx:75 用 `canWriteTarget(isAdmin, pathCapabilities,…)` 推导 tooltip 文案，违反设计文档 L171「判定规则单一实现处（Rust）」与计划 L2168「不区分保护与无权限，统一 protectedHint 文案」的明确取舍。
2. **revision 冲突后清草稿，偏离计划验收标准**：计划 L1541 要求冲突时「草稿保留，避免用户输入丢失」；AppShell.tsx:255-259 的 onConfirm 失败即 `clearDraft`，输入仅存于弹窗本地状态，用户取消弹窗即丢失。
3. **capabilities_for 每行重复探测注册表**：同 Standards 轴发现 2，两轴独立命中，可信度高。
4. **revision 用 FNV-1a 散列而非计划的明文拼接——正向偏差**：计划 L367 的 `format!("{}|{}|{}",…)` 拼接会把敏感值经 IPC 下发（违背规格 L147「摘要」语义），实现改为 FNV-1a 散列（env_var.rs:127-140）更安全。但与计划代码不一致，且无测试覆盖其对 `|` 分隔符歧义的免疫性。
5. **mock 一致性总体合格**：e2e/mocks/ipc.ts fixture 五类齐备（普通/敏感/保护/Unsupported/canEdit=false），Path 不在返回值中，冲突注入文案与 store 的 CONFLICT_MARKER 一致，`__capturedCalls` 校验 expectedRevision 按计划落实。文档提交 37e7057 如实反映实现。但 (a)1、(a)2 的测试缺口意味着「verify 全绿」并不能证明规格声明的关键场景被覆盖。

## 四、总结

- **Standards 轴**：8 项发现（1 硬性 + 7 判断性）。最重的硬性违反：gui 命令层 pub fn 缺文档注释；实际风险最高：CONFLICT_MARKER 中文文案耦合（错误文案一改、冲突检测静默失效）。
- **Spec 轴**：10 项发现（缺失 3 + 超范围 2 + 不符 5）。最重：**Rust 侧 5 个公开 API 零直接测试**，Path 过滤、保护名单拒写等 P0 安全场景目前仅由前端单测与 e2e mock 间接保障。

### 合并前建议整改（按优先级）

1. **补 Rust 公开 API 测试**：`list_all_env_vars`（Path 过滤、大小写保持）、`update_env_var`（revision 冲突、REG_DWORD 拒写且值未变）、`create_env_var`（保护名单拒绝、重名拒绝）、`delete_env_var`、`reveal_env_var`（敏感值返回明文、preview=None）。
2. **补 `onCloseRequested` 关窗确认**：检查 `env-store.draft.size > 0`，与工具栏 onCancel 路径对齐。
3. **复用 `capabilities_for_with`**：`list_hive_env_vars` 循环外探测一次 writable，消除逐行注册表键打开。
4. **统一草稿策略**：冲突失败后保留用户输入（对齐计划 L1541 验收标准），或明确注释两场景差异。
5. **对齐「单一判定实现」取舍**：EnvVarTable 的 hiveWritable 仅用于 tooltip 文案，与计划 L2168 不符，按计划统一 protectedHint 或在 AGENTS.md 更新设计决策。
6. **降低 CONFLICT_MARKER 脆弱性**：Rust 错误加结构化错误码前缀（如 `[E_CONFLICT]`），前端匹配前缀而非中文文案。
7. 清理 `maskValue`/`isHiveFilter`/`hiveOf` 死导出；为 env-var-table.test.tsx 补 `afterEach(cleanup)` 并改用真实 zh-CN.json。
8. 修正 registry.rs:415,440 的「原子」注释措辞（改为「写入前检查」）；为 gui/src/commands/env_var.rs 补 `///` 文档。
