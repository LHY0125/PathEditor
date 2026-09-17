# PathEditor 全环境变量功能对抗性复审报告

**复审日期**：2026-09-17  
**复审对象**：`.claude/worktrees/all-env-vars`  
**分支**：`worktree-all-env-vars`  
**基线**：`main` / `1c13daf`（v5.1.2）  
**被审查 HEAD**：`37e7057`  
**复审方式**：只读代码审查、规格对照、反例推演、mock IPC E2E、非注册表写入质量门验证  
**重要边界**：未写真实 PATH，未执行任何会写 HKCU/HKLM 的 Rust 测试，未 commit、未 push、未升级版本。

## 一、结论

**结论：当前分支不建议直接合并或发布。**

整体架构方向正确，`Path` 专用通路与通用 `EnvVar` 通路已分离，敏感变量默认打码、类型保持、revision 回传、独立工具栏、mock IPC E2E 等主要骨架已经落地；前端 200 个单元测试与 22 个 E2E 均通过。

但是，对抗性复审确认存在 **1 个 P0 数据完整性缺陷、5 个 P1 高风险缺陷、3 个 P2/P3 收口问题**。其中 P0 会在用户编辑长变量时真实截断原值，测试全绿不能覆盖该风险。建议至少修复 F-01 至 F-06 后再进入合并验收。

## 二、发现清单

### F-01 [P0] 编辑框使用截断后的 `preview`，会破坏长值或被净化的原值

**证据**：

- `core/src/env_var.rs:179-193` 将普通变量预览截断到 256 字符，并删除 `\r`、`\n`、`\0`。
- `src/components/dialogs/EditEnvVarDialog.tsx:14-16` 对未 reveal 的变量直接使用 `meta.preview ?? ''` 作为编辑初值。
- `src/components/layout/AppShell.tsx:262-265` 把该编辑值作为完整值提交给 `update_env_var`。

**攻击/复现路径**：

1. 注册表中存在一个未命中敏感规则的 `REG_SZ`/`REG_EXPAND_SZ` 变量，值长度超过 256 字符。
2. 用户点击“编辑”，不做任何改动，直接点击“确定”。
3. 前端提交的是“前 256 字符 + `…`”，完整原值被永久覆盖。

含换行的值也会在编辑后被静默去掉换行。该问题属于真实数据损坏，必须阻断发布。

**修复建议**：编辑任何可写变量前都调用 `reveal_env_var` 获取完整原值；`preview` 只能展示，严禁作为编辑数据源。新增 300 字符值、含 CR/LF 值的回归测试，断言“打开后原样保存不改变值”。

### F-02 [P1] revision 冲突后弹窗保留，但重试永远使用旧 revision

**证据**：

- `src/components/layout/AppShell.tsx:58` 把完整 `EnvVarMeta` 固定存入 `editVar`。
- `src/store/env-store.ts:97-100` 冲突后刷新快照。
- `src/components/layout/AppShell.tsx:262-265` 重试仍调用 `store.save(editVar)`，使用刷新前的旧 `revision`。

**后果**：第一次冲突后，即使列表已刷新，用户在保留的弹窗里再次点击确定，仍会携带旧 revision，再次冲突，形成无法恢复的重试死路。当前测试只断言“弹窗保留”，未测试第二次提交。

**修复建议**：只保存 `editVarKey`，每次提交从最新 snapshot 派生 meta；冲突刷新后保留输入值，但 revision 必须切换到最新值。增加“首次冲突、刷新、第二次提交成功”的测试。

### F-03 [P1] reveal 与 refresh/load 存在乱序竞态，可把旧明文绑定到新 revision

**证据**：

- `src/store/env-store.ts:56-67` 的 `load()` 没有请求序号或取消机制。
- `src/store/env-store.ts:143-148` 的 `reveal()` 完成后无条件把明文写入 `revealed`。
- `revealed` 仅以 `${hive}:${name}` 为键，不携带 revision。

**攻击路径**：reveal 读取旧值尚未返回；外部进程修改变量；refresh 先返回新 snapshot/revision 并清空明文；旧 reveal 随后返回，又把旧值写入当前界面。此时用户看到旧值，但保存时可能使用新 revision，从而覆盖外部刚写入的新值，绕过设计意图中的并发保护。

**修复建议**：`revealed` 至少绑定 `{ revision, value }`；reveal 返回时仅在当前 snapshot 仍存在同名且 revision 未变化时提交。`load()` 增加递增 request id，旧请求不得覆盖新请求。

### F-04 [P1] “未保存草稿关窗确认”没有覆盖真实草稿，也没有覆盖原生窗口关闭

**证据**：

- `EditEnvVarDialog.tsx:27` 的输入值只存在组件本地 state。
- `AppShell.tsx:263-271` 仅在点击确定时瞬间写入 store，失败后又清除。因此用户正在输入但尚未提交时，`env-store.draft` 仍为空。
- `AppShell.tsx:152-157` 的确认逻辑只挂在 PATH `ToolBar` 的“取消”按钮上。
- `allVars` 分支在 `AppShell.tsx:124-168` 不渲染 PATH `ToolBar`，且项目没有 Tauri `onCloseRequested`/`beforeunload` 处理。
- `tests/unit/app-shell-env-vars.test.tsx:207-229` 通过人工向 store 注入 draft，再在 PATH Tab 点击“取消”验证，未覆盖真实编辑输入和原生窗口 X。

**后果**：用户在编辑弹窗中输入内容后点击窗口右上角 X，可直接丢失输入而没有确认；规格第 407 行要求未落地。

**修复建议**：编辑输入变化时同步 dirty/draft 状态；在 Tauri 原生 `onCloseRequested` 中统一检查 PATH 修改与 EnvVar 草稿并 `preventDefault()`；测试真实输入后触发关闭处理器，而不是手工造 store 状态。

### F-05 [P1] IPC 运行时校验没有真正执行“敏感明文不得随列表进入前端”的契约

**证据**：

- `src/services/backend.ts:98-110` 只检查必需字段，不拒绝额外的 `value` 字段。
- `src/services/backend.ts:112-116` 校验后原样返回上游对象，而不是白名单复制 8 个字段。
- 规格 `2026-09-16-all-env-vars-design.md:339-347` 明确要求运行时校验与类型系统双重保证，且 `revision` 必须是非空字符串。

**后果**：一旦 Rust 序列化回归、mock 配错或未来命令误带 `value`，完整对象会进入 Zustand 和 React DevTools；TS 类型不会删除运行时字段。空 `name`/`revision` 也会被接受。

**修复建议**：白名单构造新的 `EnvVarMeta` 对象；显式拒绝 `value` 自有属性；要求 `name.trim()` 与 `revision` 非空。增加恶意 fixture 测试。

### F-06 [P1] “同一 Rust 调用消除 TOCTOU/原子创建”的表述不成立

**证据**：

- `registry.rs:389-408` 是独立的“读取 → 比较 → 写入”注册表 API 调用。
- `registry.rs:440-453` 是独立的“枚举是否存在 → 写入”调用。
- `registry.rs:474-485` 删除同样是“读取 → 比较 → 删除”。

把步骤放进同一个 Rust 函数只能缩小窗口，不能形成 Windows 注册表的 compare-and-swap。外部进程仍可在比较后、写入前修改值；并发创建也可能在存在性检查后覆盖同名值。

**修复建议**：不要再宣称“消除 TOCTOU/原子”。至少使用进程间命名互斥量保护 PathEditor 多实例，并明确不能约束 regedit/其他程序；评估注册表事务或写后校验与冲突报告。规格、注释、README 同步改为准确保证。

### F-07 [P2] Unsupported 类型的公开接口错误分支实际不可达，专项测试未覆盖公开流程

`read_env_var()` 在 `registry.rs:229-235` 先把原始值解码成 String。`reveal_env_var`、`update_env_var`、`delete_env_var` 都在判定 `Unsupported` 之前调用它。因此 `REG_DWORD` 等类型会先返回“无法解码”，不会到达设计规定的“不支持类型”错误分支。

现有 `registry.rs:795-813` 测试还明确绕开了 `read_env_var`，只测类型映射；没有验证 5 个公开函数的隔离键端到端行为。建议先读 raw type，确认类型后再解码，并把核心逻辑重构为可注入 `RegKey`/测试路径的内部函数，覆盖 list/reveal/update/create/delete。

### F-08 [P2] 新建弹窗只门禁系统 hive，没有处理用户 hive 不可写

`NewEnvVarDialog` 只接收 `canWriteSystem`，用户来源始终可选且是默认值。若 `canWriteUser=false`，UI 仍允许提交，最终依赖 Rust 打开注册表失败。建议同时传入 `canWriteUser`，没有可写 hive 时禁用“新建变量”，并在来源选项上显示准确原因。

### F-09 [P3] 交付与文档仍未完全收口

- worktree 仍有 `gui/Cargo.toml` 的换行状态脏标记，以及未跟踪的设计/实施计划；不能称为 clean worktree。
- README 徽章仍写“195 passed”，本次实测为前端 200 tests + E2E 22 tests，统计口径已漂移。
- 设计文档状态仍为“待确认”，但实现已经完成；应更新为已实现/待复审整改，并记录已知限制。

## 三、已确认正确的部分

1. `Path` 在 Rust 保留名单中按大小写不敏感过滤，写入口也二次拒绝，未混入通用变量通路。
2. `EnvVarMeta` 的 Rust 结构本身不含 `value` 字段；命中敏感规则时 preview 为 `None`。
3. 已有变量写入时从注册表读取真实 `RegType` 并原样写回，未从前端接收类型，Issue #26 的类型降级风险在主路径上得到规避。
4. 三个写操作成功后均调用 `broadcast_env_change()`。
5. `allVars` 使用独立工具栏；PATH 上移、下移、清理、导入、导出在该 Tab 不可见，拖放也早退。
6. revision 参数已经贯穿 Rust → Tauri → backend → store → E2E，E2E 能断言 `expectedRevision`。
7. 敏感变量默认固定长度打码，显示后明文只保存在内存 Map；刷新会清除已显示明文。
8. `CLAUDE.md` 与 `AGENTS.md` 本次提交内容一致，Tauri CSP 未被放松。

## 四、验证证据

| 检查项                                                  | 结果                                                                                          |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `npm run format:check`                                  | 通过                                                                                          |
| `npm run lint`                                          | 0 error，3 个 TanStack Virtual 既有/同类 warning                                              |
| `npm run build`                                         | 通过；仅 Tailwind `PLUGIN_TIMINGS` 提示                                                       |
| `npm run test:coverage`                                 | 17 files / **200 tests passed**；Lines **87.79%**                                             |
| `npm run test:e2e`                                      | **22 passed**；全部为生产构建 + mock IPC                                                      |
| `cargo fmt --check`                                     | 通过                                                                                          |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过                                                                                          |
| `cargo test --workspace --no-run`                       | 通过，全部测试目标可编译                                                                      |
| `cargo test -p path-editor-core env_var::tests`         | **10 passed**，纯函数测试，无注册表写入                                                       |
| 完整 `cargo test --workspace`                           | **未执行**：其中 `registry::env_var_tests` 会写 HKCU 隔离测试键，受本次“不得写真实注册表”约束 |
| 真实 Tauri/注册表闭环                                   | **未执行**，仍需显式授权                                                                      |

说明：首次并行执行 build 与 coverage 时，Vitest 的 `coverage/.tmp` 出现文件竞争导致 `ENOENT`；待 build 完成后单独重跑，200 tests 与覆盖率均通过。该次失败属于工具并发冲突，不是测试断言失败。

## 五、建议整改顺序

1. **立即修复 F-01**：编辑必须加载完整值，禁止以 preview 保存。
2. 修复 F-02/F-03：revision 与 revealed 都绑定最新 snapshot，加入请求代次控制。
3. 修复 F-04：把真实输入纳入 dirty 状态，并接入 Tauri 原生关闭事件。
4. 修复 F-05：IPC 白名单复制、拒绝 `value`、校验非空 revision。
5. 对 F-06 调整并发模型或至少修正承诺；补可复现并发测试。
6. 重构 registry 可测试边界，补 F-07 的公开函数测试。
7. 收口 F-08/F-09，更新 README、设计状态并清理 worktree。

## 六、重新验收门槛

- 长度 300+ 的普通变量打开编辑后原样保存，字节/UTF-16 内容不变。
- 含 `\r`/`\n` 的普通变量编辑不丢字符。
- revision 冲突后刷新，保留用户输入，第二次提交携带新 revision 并可成功。
- reveal 与 refresh 乱序时，旧明文不得写入新 snapshot。
- 原生窗口关闭可拦截真实编辑中的未提交内容。
- `list_all_env_vars` 恶意返回 `{ value: 'secret' }` 时前端拒绝或剥离字段。
- Unsupported 的 reveal/update/delete 返回稳定的类型错误，并确认值未变。
- `npm run verify:all` 全绿；Rust 隔离键测试若需执行，必须另行取得用户书面授权。

## 七、合并建议

当前状态建议标记为 **Changes Requested**。F-01 是明确的数据损坏路径；F-02、F-03、F-04 会让并发保护和草稿保护在真实交互中失效。修复这些问题并按第六节复验后，再进行最终合并复审。
