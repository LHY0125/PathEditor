# PathEditor 全环境变量分支复审回执

- **复审日期**：2026-09-17
- **复审对象**：`worktree-all-env-vars` 整改提交 `4e2e765`（Rust 侧）+ `000d3a5`（前端侧，19 文件 +565/−171），分支共 14 个提交
- **复审性质**：对「对抗性复审报告（F-01～F-09）」与「分支审查报告（S/a/c 18 项）」整改声明的逐项核验
- **前置报告**：`docs/审核和开发/2026.09.17/PathEditor-全环境变量分支审查报告.md`

## 一、验证证据（worktree 内实测）

| 检查项                     | 整改前                | 本次实测                    | 结果 |
| -------------------------- | --------------------- | --------------------------- | ---- |
| cargo test --workspace     | 88 passed             | **97 passed / 0 failed**    | ✅   |
| cargo clippy `-D warnings` | 绿                    | 绿                          | ✅   |
| cargo fmt --check          | 绿                    | 绿                          | ✅   |
| vitest 单测                | 200 passed（17 文件） | **213 passed（18 文件）**   | ✅   |
| tsc -b                     | 0 错                  | 0 错                        | ✅   |
| eslint                     | 0 error               | 0 error（3 条既有 warning） | ✅   |
| prettier                   | 干净                  | 干净                        | ✅   |
| Playwright e2e             | 22 passed             | **23 passed**               | ✅   |

测试计数与整改声明完全一致（97 / 213 / 23）。

## 二、整改项逐条核验

### 对抗性复审（F-01～F-09）

| 项                             | 声明                                                   | 核验证据                                                                                                                                   | 结论    |
| ------------------------------ | ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------ | ------- |
| F-01 P0 编辑截断损坏原值       | 编辑弹窗经 fetchFullValue 取完整原值；300 字符回归测试 | EditEnvVarDialog.tsx:50 调用 `fetchFullValue`；tests/unit/app-shell-env-vars.test.tsx:339,354 `'x'.repeat(300)` 断言编辑框数据源非 preview | ✅ 属实 |
| F-02 P1 冲突重试死路           | 编辑只存稳定键，提交前 findMetaByKey 派生 meta         | src/core/env-var.ts:42 `findMetaByKey` 新增并被 store 引用                                                                                 | ✅ 属实 |
| F-03 P1 reveal/load 竞态       | 请求代次 + revision 绑定                               | env-store.ts:113 fetchFullValue 实现；store 测试 213 中含 F-03 回归                                                                        | ✅ 属实 |
| F-04 P1 关窗确认缺失           | 接入 Tauri onCloseRequested                            | AppShell.tsx:85 `getCurrentWindow().onCloseRequested`，检查 `isModified \|\| hasDrafts()`                                                  | ✅ 属实 |
| F-05 P1 IPC 契约漏洞           | 白名单构造、显式拒绝 value 自有属性                    | backend.ts:106 `if ('value' in value)` 拒绝逻辑                                                                                            | ✅ 属实 |
| F-06 P2「原子」表述不实        | 注释与 IPC 表改如实措辞                                | CLAUDE.md IPC 表 create_env_var 行：「检查与写入是两步操作，存在竞态窗口」；registry.rs:491 同步                                           | ✅ 属实 |
| F-07 P2 Unsupported 分支不可达 | 先读 vtype 再解码                                      | registry.rs:234「先判定类型再做字符串解码」                                                                                                | ✅ 属实 |
| F-08 P2 用户 hive 未门禁       | NewEnvVarDialog 补 canWriteUser                        | NewEnvVarDialog.tsx:11,25,60 props、默认来源选择、option disabled                                                                          | ✅ 属实 |
| F-09 P3 收口                   | README 计数、worktree 清理                             | README 更新；worktree 仅剩 2 个未跟踪规格文档                                                                                              | ✅ 属实 |

### 分支审查报告（S/a/c）

| 项                          | 声明                                       | 核验证据                                                                                                                     | 结论            |
| --------------------------- | ------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- | --------------- |
| S1 gui pub fn 缺 ///        | 5 个补齐                                   | gui/src/commands/env_var.rs 全部 5 个 pub fn 均有完整 `///`（含 # Returns 段）                                               | ✅ 硬性违反清零 |
| S2 CONFLICT_MARKER 文案耦合 | 改匹配 `[E_CONFLICT]` 前缀，三端对齐       | registry.rs:18 `ERR_CONFLICT` 常量 + :1092-1093 前缀契约测试；env-store.ts:18 `CONFLICT_PREFIX`；e2e/mocks/ipc.ts:127 同前缀 | ✅ 属实         |
| S3 逐行注册表探测           | 循环外探测一次，复用 capabilities_for_with | registry.rs:339 `capabilities_for_with(writable, …)`                                                                         | ✅ 属实         |
| S4「原子」注释              | 全部修正                                   | 同 F-06                                                                                                                      | ✅ 属实         |
| S5 死导出                   | 移除 maskValue/isHiveFilter/hiveOf         | src/core/env-var.ts 中三者已无踪迹                                                                                           | ✅ 属实         |
| S6 表格测试策略             | 真实 zh-CN.json + afterEach(cleanup)       | env-var-table.test.tsx:19,23,106                                                                                             | ✅ 属实         |
| a1 Rust 公开 API 零测试     | 补 7 个隔离键测试                          | registry.rs 测试段：Path 过滤与大小写保持（:897,915）、revision 冲突拒写且值不变（:927）、DWORD 拒写（:864-877,958）等       | ✅ 属实         |
| a2 E2E 缺拖放用例           | 补「allVars 下拖放无效」                   | env-vars.spec.ts:74（page.evaluate 构造 DragEvent）                                                                          | ✅ 属实         |
| c1 前端重复权限判定         | 移除，统一 protectedHint                   | EnvVarTable.tsx 已无 canWriteTarget 权限推导（提交 000d3a5 -30 行）                                                          | ✅ 属实         |
| c2 草稿策略不统一           | 统一「镜像弹窗输入」                       | 提交信息与 store/AppShell 实现一致                                                                                           | ✅ 属实         |
| c4 revision 分隔符免疫      | 补测试                                     | 已纳入 213 单测                                                                                                              | ✅ 属实         |

## 三、遗留事项

1. **AGENTS.md 与 CLAUDE.md 表格分隔行宽度不一致**（本次复审新发现，唯一新问题）：`000d3a5` 中 AGENTS.md 的 IPC 表因 `create_env_var` 行说明变长而整表加宽，但分隔行连字符宽度未同步，CLAUDE.md 保持旧列宽。经忽略表格装饰空白比对，**语义内容完全一致**，仅 Markdown 表格渲染宽度不同。属格式瑕疵，建议合并前用同一份内容覆盖两个文件。
2. **注册表无 compare-and-swap**：读-比-写理论竞态窗口仍在，文档已如实表述；进程间命名互斥量可作为后续 issue。
3. **真实 Tauri/注册表闭环测试未执行**：按仓库约定需显式授权并记录备份/快照/回滚。
4. **原生窗口关闭拦截（F-04）无法在 jsdom 端到端验证**：需真实 GUI 冒烟确认。

## 四、复审结论

**整改通过。** 两份报告全部发现均已处理且核验属实，无虚报；测试证据（97/213/23）与声明一致，全链路质量门绿灯。分支处于「待合并」状态，合并前建议：

1. 修复 AGENTS.md / CLAUDE.md 表格分隔行宽度（一行格式修正）；
2. 真实环境 GUI 冒烟两项：编辑长变量（300+ 字符）原样保存、X/Alt+F4 关窗确认弹窗生效；
3. 冒烟通过后合并回 main，合并时将本回执与前置审查报告一并纳入 docs/审核和开发/2026.09.17/ 提交。
