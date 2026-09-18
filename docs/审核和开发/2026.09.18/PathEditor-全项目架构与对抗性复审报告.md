# PathEditor 全项目架构与对抗性复审报告

- **复审日期**：2026-09-18
- **复审对象**：当前 `main` 发布候选代码
- **当前 HEAD**：`2c8117c`（`main`，位于 `v5.1.3` tag 之后 8 个文档/CI 提交）
- **技术范围**：Rust workspace、Tauri 2 GUI、React 19/TypeScript 前端、CLI、Windows 注册表、配置文件、测试与发布工作流
- **复审性质**：只读架构审查 + 对抗性代码审查 + 质量门复验
- **边界**：未修改业务代码，未写真实 PATH 或真实环境变量，未 commit/push，未执行会写入 HKCU 隔离键的完整 Rust 测试

## 一、最终结论

**结论：Changes Requested。当前实现具备继续演进的基础，但不应被视为“架构已经收口”，也不建议在未处理 P1 问题前继续扩展环境变量写入能力。**

项目已经完成从 C/IUP 单体桌面程序到 Rust/Tauri/React/CLI 的主体迁移，核心方向正确：Windows 操作集中在 Rust，GUI 和 CLI 共享 `core`，PATH 与通用环境变量分为两条业务通路，前端 IPC 也已集中到 `backend.ts`。这比在 C 代码上继续堆叠 UI 回调和注册表操作有明显改善。

但从成熟桌面系统的标准看，目前仍存在三类问题：

1. **数据一致性问题**：编辑窗口的异步完整值读取、CLI 的 `--force` 语义、CLI 的注册表与 `disabled.json` 双写边界，仍可能在真实并发或失败场景下产生错误结果。
2. **可观测性与完整性问题**：注册表枚举/解码错误被静默跳过，用户可能看到一个“成功返回但不完整”的变量列表。
3. **架构收口问题**：错误契约仍是自由文本，PATH/EnvVar 注册表逻辑仍集中在 1101 行的 `registry.rs`，GUI 与 CLI 的事务策略不一致，测试仍依赖 Windows 用户注册表隔离键。

本轮确认：**0 个新增 P0、4 个 P1、7 个 P2、0 个 P3**。P1 项建议作为下一次修复批次的硬门槛。

## 二、审查范围与架构判断

### 2.1 当前分层

```text
React UI
  -> Zustand Store / pure core logic
  -> services/backend.ts
  -> Tauri commands
  -> path-editor-core
  -> Windows Registry / filesystem / Win32 broadcast

CLI
  -> clap command dispatch
  -> cli runtime helpers
  -> path-editor-core
  -> Windows Registry / filesystem / Win32 broadcast
```

### 2.2 已经做对的事情

- `core` 不依赖 Tauri，GUI 与 CLI 共享注册表、PATH、扫描、配置和环境变量规则。
- `Path` 被保留在专用 PATH 通路，通用 EnvVar 通路不展示、不允许写入，避免绕过 `disabled.json` 和 PATH 快照事务。
- `PathCapabilities` 按 HKLM/HKCU 分开探测，修复了“非管理员用户连用户 PATH 也不能编辑”的旧架构问题。
- `EnvVarMeta` 不包含 `value`；敏感变量通过显式 reveal 读取，列表契约有 Rust 与 TypeScript 两层校验。
- GUI 的环境变量编辑已经使用完整值，不再把截断的 `preview` 当作写入源；revision 冲突后也能从最新快照派生 meta。
- Rust 写入入口保留现有注册表类型，写成功后广播 `WM_SETTINGCHANGE`。
- 前端 213 个单元测试、23 个 mock IPC E2E、构建、fmt、clippy 均通过。

这些优点说明迁移并非简单“换语言重写”，已经形成了可维护的领域边界；但事务、错误和集成验证还没有达到同一成熟度。

## 三、必须整改的问题

### F-01 [P1] 编辑弹窗的完整值请求没有绑定 revision，刷新期间可能覆盖外部新值

**证据**：

- `src/components/dialogs/EditEnvVarDialog.tsx:43-63` 打开弹窗后读取完整值，响应只写入本地 `value`。
- `src/store/env-store.ts:113-117` 的 `fetchFullValue()` 直接返回 `revealEnvVar()` 结果，没有携带请求时 revision。
- `src/components/layout/AppShell.tsx:291-300` 保存时使用当前快照的 `meta.revision`，但使用的是弹窗里可能已经过时的 `value`。

**反例路径**：

1. 快照为值 A、revision A，用户打开编辑窗口。
2. `reveal_env_var` 读取 A 尚未返回；另一个进程把变量改成 B。
3. 列表刷新为值 B、revision B，弹窗异步请求随后才返回 A。
4. 用户未改动输入，点击保存；前端携带 revision B 和 value A。
5. Rust 认为 revision B 正确，于是把 A 写回，覆盖了外部刚写入的 B。

此前修复的 F-03 只保护了表格 reveal 写入 `revealed` 的路径，没有覆盖编辑弹窗专用的 `fetchFullValue` 路径。这是“测试通过但真实交互会损坏数据”的典型遗漏。

**修复要求**：

- `fetchFullValue` 返回 `{ value, revision }`，或在调用方保存请求 revision 并在响应返回时比较当前 snapshot。
- 快照 revision 变化时，编辑框必须提示重新加载/重新取值，不能静默保留旧值。
- 保存前还应校验“当前编辑值对应的读取 revision”和“当前提交 revision”一致；若不一致，强制重新读取完整值。
- 增加回归测试：`fetch` 延迟期间刷新快照，旧值返回后不得提交覆盖新值。

### F-02 [P1] CLI `--force` 没有实现设计文档承诺的“跳过校验直接覆盖”

**证据**：

- 设计文档 `docs/superpowers/specs/2026-09-17-cli-env-vars-design.md:62-64` 明确写的是：`--revision` 做 CAS，`--force` 跳过校验直接覆盖。
- `cli/src/env_ops.rs:319-348` 中，`Concurrency::Force` 仍调用 `current_revision()`，读取列表 revision 后再传给 `update_env_var`/`delete_env_var`。
- `core/src/registry.rs:431-450` 始终执行 revision 比较，core 没有 force API。

当前实现实际上是“重新读取一个 revision，再按 CAS 写入”，不是 force。若变量在 CLI 的列表读取与 core 再读取之间发生变化，`--force` 仍会以退出码 3 失败。这个行为与帮助文案、README、设计文档不一致，会误导脚本作者以为该命令具有覆盖语义。

**修复要求**：

- 选择一种并贯彻到底：
  - 增加显式的 `update_env_var_force` / `delete_env_var_force` core API，在文档中明确这是“最后写入者胜”；或
  - 删除 `--force`，改名为 `--current-revision`/`--refresh-revision`，承认它仍是 CAS；或
  - 保留命令但把语义改为“以当前读取的 revision 尝试写入”，并修改所有文档、退出码和测试。
- 不要用空字符串或魔法 revision 模拟 force，这会把并发语义隐藏在错误的参数契约中。
- 增加并发变更测试，明确 force 模式的成功/冲突行为。

### F-03 [P1] CLI 的注册表与 `disabled.json` 双写没有失败恢复，和 GUI 的事务策略不一致

**证据**：

- GUI `src/services/path-session.ts:162-212` 已有 `_pendingSys/_pendingUser`，注册表成功但侧车文件失败时会保留待补写状态。
- CLI `cli/src/runtime.rs:56-105` 在注册表写入后才调用 `save_path_snapshot`，失败时直接 `exit_err`。
- `cli/src/runtime.rs:125-130` 的 `persist_snapshot` 同样是失败即退出，没有 journal、pending 文件或补偿动作。
- `cli/src/profile_ops.rs:45-67` 先写系统 PATH、再写用户 PATH、最后保存快照；任一步失败都可能留下部分应用状态。

**反例路径**：

- CLI 写注册表成功后磁盘满、权限变化或 `disabled.json.tmp` 被占用；进程报错退出，但 PATH 已经改变，快照仍是旧的。
- `profile apply` 系统 PATH 成功、用户 PATH 失败时直接退出，用户面对的是半应用 profile，且没有结构化恢复信息。

这不是“极端情况下忽略错误”的小问题，因为 `disabled.json` 保存了禁用项和顺序，是 PATH 业务状态的一部分。GUI 和 CLI 对同一个持久化模型采用了两种不同的可靠性策略，说明事务边界还没有真正下沉到共享层。

**修复要求**：

- 将 PATH 应用流程下沉到 core 的 application service，返回每个 hive、注册表、侧车文件的结构化结果。
- CLI 也使用 pending/journal 机制；至少要在侧车写失败时生成可重试状态，而不是只打印错误后退出。
- 多 hive apply 必须明确 atomic、best-effort 或 partial 三种语义，并在输出和退出码中表达。
- 增加磁盘写失败、系统成功/用户失败、快照成功/注册表失败的故障注入测试。

### F-04 [P1] 注册表枚举和单项读取错误被静默跳过，`list_all_env_vars` 可能返回“成功但不完整”的列表

**证据**：

- `core/src/registry.rs:311` 使用 `key.enum_values().flatten()`，枚举错误被直接丢弃。
- `core/src/registry.rs:317-322` 单项读取失败只记录 warning 并 `continue`。
- `core/src/registry.rs:327-334` 字符串解码失败同样只记录 warning 并 `continue`。

对“查看全部环境变量”的命令而言，静默丢项是错误的成功语义。用户无法区分“变量不存在”和“枚举/读取失败”，可能在列表不完整的情况下做删除、迁移或脚本同步。日志也没有回传到 GUI/CLI，普通用户看不到 warning。

**修复要求**：

- 枚举失败、读取失败、解码失败至少应使整个 hive 返回错误；或返回带 `complete=false` 和 diagnostics 的结构化快照。
- 不要用 `Iterator::flatten()` 吞掉系统 API 错误。
- 增加可注入注册表适配器，测试枚举中途失败、单值读取失败和不支持类型的行为。

## 四、架构级整改项

### F-05 [P2] “两个 hive 的一致快照”只是顺序读取，不是可证明的一致快照

`core/src/registry.rs:361-369` 先读取 HKLM，再读取 HKCU。两个 hive 没有 Windows 级跨键事务，注释却写成“保证快照一致”。外部进程可以在两次读取之间修改任意一侧。

建议把契约改成“单次请求返回两个接近时刻的快照”，并增加 `capturedAt` 或每个 hive 的 generation/revision。文档应明确这是 best-effort snapshot，而不是 atomic snapshot。若业务需要强一致，应提供基于单 hive revision 的提交检查，而不是在列表层声称跨 hive 一致。

### F-06 [P2] 错误仍以 `String` 跨 Rust、Tauri、TypeScript、CLI 传播

当前 GUI command 和 core 大量使用 `Result<T, String>`，前端通过字符串包含关系识别 `[E_CONFLICT]`，CLI 也通过文本前缀映射退出码。这样会造成：

- 文案变化会破坏程序分支；
- GUI 无法稳定地按错误码本地化；
- CLI 无法为脚本提供稳定机器可读错误；
- 权限、保护变量、类型不支持、并发冲突和磁盘失败没有统一 taxonomy。

下一阶段应定义 `CoreError`：至少包含 `code`、`operation`、`hive`、`name`、`retryable` 和安全展示消息。Tauri 返回序列化错误对象，CLI 将 code 映射到退出码，前端将 code 映射到 i18n。`String` 只保留在日志和最终 fallback 展示。

### F-07 [P2] `registry.rs` 仍是 PATH 与 EnvVar 的大模块，迁移后的领域边界还不够稳定

当前 `core/src/registry.rs` 约 1101 行，同时承载：PATH 读写、PATH 清理、注册表类型、通用环境变量读写、revision、错误、测试隔离键。C/IUP 迁移虽然把 UI 与系统操作分开了，但注册表领域仍然是一个高耦合模块。

建议拆成：

```text
core/src/registry/
  mod.rs              # 统一入口与类型导出
  path.rs             # PATH value 读写、长度、分割、清理
  env_var.rs          # 通用环境变量 CRUD、revision
  access.rs           # HKLM/HKCU 权限与 hive 定位
  error.rs            # 结构化错误
  test_adapter.rs     # 测试用注册表端口
```

这样可以使 PATH 事务和 EnvVar 事务分别演进，减少后续修改通用变量时回归 PATH 的概率。

### F-08 [P2] GUI 与 CLI 没有共享“应用服务”层，导致相同业务规则出现两套事务编排

目前 GUI 在 `src/services/path-session.ts` 编排保存、pending、部分成功；CLI 在 `cli/src/runtime.rs` 和 `cli/src/profile_ops.rs` 重新编排读、比、写、快照、广播。两者共享底层函数，却不共享完整用例服务，因此出现 F-03 的策略分叉。

建议 core 暴露以用例为单位的服务接口，例如：

- `apply_path_snapshot`
- `apply_profile`
- `save_path_with_sidecar`
- `retry_pending_path_state`

GUI/CLI 只负责输入转换、权限展示和输出渲染。任何会改变注册表与 sidecar 的顺序、补偿、广播，都应在同一个 Rust service 中实现。

### F-09 [P2] 测试隔离仍依赖真实 HKCU，完整 Rust 质量门无法在本审查边界内安全运行

`core/src/registry.rs:737-775` 的测试通过 `HKEY_CURRENT_USER\Software\PathEditor\Tests\...` 建立隔离键，再在 Drop 时删除。它没有触碰真实环境变量键，但依然写入当前用户真实注册表。对于开发机安全策略和并行测试来说，这不是理想的隔离方式；同时完整 `cargo test --workspace` 因此不能在本窗口执行。

成熟方案应提供注册表端口/trait：

```rust
trait RegistryStore {
    fn enum_values(&self) -> Result<...>;
    fn get_raw_value(&self, name: &str) -> Result<...>;
    fn set_raw_value(&self, name: &str, value: &RegValue) -> Result<()>;
}
```

生产使用 Winreg adapter，测试使用内存 adapter。这样能覆盖失败注入、并发、权限和原子性语义，也不需要在测试中写 HKCU。

### F-10 [P2] 从 C 迁移到 Rust 缺少行为等价性基线，当前测试更多是新实现自证

历史首版是 C/IUP：`src/registry.c`、`src/callbacks.c`、`src/main.c`。当前 Rust 版本测试证明“Rust 实现符合当前测试”，但没有一份可执行的迁移兼容矩阵证明以下行为与旧版本一致或已被有意改变：

- PATH 分割、空项、空白和重复项处理；
- REG_SZ / REG_EXPAND_SZ 写回类型；
- 用户/系统 PATH 权限失败时的 UI 行为；
- 备份格式与恢复可用性；
- profile、导入导出和禁用项在升级后的兼容性；
- `WM_SETTINGCHANGE` 广播时机。

建议把迁移契约写成 golden tests：输入注册表快照 + 操作 + 期望注册表/文件/广播结果。旧 C 行为不必全部保留，但每一个改变都应在迁移记录中标注原因。

### F-11 [P2] 持久化文件缺少版本化 schema 和损坏恢复策略

`disabled.json` 通过字段默认值兼容旧格式，但没有显式 `schemaVersion`、校验和、备份恢复或损坏隔离。`profiles/*.json` 也直接反序列化为当前结构。若文件被截断、手工修改或未来字段语义变化，启动/应用可能失败，且用户没有恢复路径。

建议：

- 文件顶层增加 `schemaVersion`；
- 写入时保留上一份 `.bak` 或使用可轮换 journal；
- 读取失败时移动到 quarantine 文件并给出恢复提示，不要只返回通用 JSON 解析错误；
- 对 profile/disabled 快照做 schema migration 测试。

## 五、已知风险与接受条件

### 5.1 Windows 注册表没有真正 CAS

`update_env_var`、`delete_env_var` 的读、revision 比较、写入仍是多次独立 Winreg 调用。当前代码和文档已经如实说明存在竞态窗口，这是正确的；但不能把它称为原子更新。若产品需要强并发保证，应引入命名互斥量、写后校验或更强的应用级协调，而不是继续增加字符串 revision 规则。

### 5.2 敏感变量识别是名称启发式

`core/src/env_var.rs:134-140` 只根据变量名关键词判断敏感性。没有命中 `TOKEN/KEY/SECRET/PASSWORD/API` 的密钥仍可能进入 `preview` 并流经 IPC。文档已经声明这是启发式限制，因此本轮不把它判为实现偏差，但发布说明必须明确：**这是降低误暴露概率，不是秘密检测保证**。更强方案需要用户显式标记、敏感变量 allow/deny 配置或统一 secret provider，不能只继续堆关键词。

### 5.3 E2E 不是 Tauri/注册表集成测试

当前 23 个 E2E 运行在生产前端 + mock IPC，能验证 UI 流程和 IPC 参数，但不能证明 Tauri command 注册、serde 字段、Winreg 权限、广播和真实窗口关闭事件全部闭环。发布前需要单独的受控 Windows 集成作业，使用专用测试账户/快照/回滚，不应在普通开发机上直接执行。

## 六、验证结果

| 检查项                                                  | 本轮结果 | 说明                                               |
| ------------------------------------------------------- | -------: | -------------------------------------------------- |
| `npm run format:check`                                  |     通过 | Prettier 无差异                                    |
| `npm run lint`                                          |     通过 | 0 error，3 个 TanStack Virtual warning             |
| `npm run build`                                         |     通过 | 仅 Tailwind`PLUGIN_TIMINGS` 非阻断提示             |
| `npm run test:coverage`                                 |     通过 | 18 files，213 tests；Lines 87.90%，Branches 75.16% |
| `npm run test:e2e`                                      |     通过 | 23 tests，生产构建 + mock IPC                      |
| `cargo fmt --check`                                     |     通过 |                                                    |
| `cargo clippy --workspace --all-targets -- -D warnings` |     通过 |                                                    |
| `cargo test --workspace --no-run`                       |     通过 | 测试目标可编译                                     |
| `cargo test -p path-editor-core env_var::tests`         |     通过 | 11 项纯逻辑测试                                    |
| `cargo test --workspace`                                |   未执行 | 会写 HKCU 隔离测试键，违反本次边界                 |
| 真实 Tauri/Winreg 闭环                                  |   未执行 | 需要专用测试环境与授权                             |

## 七、整改优先级与验收门槛

### 第一批：合并前必须处理

1. 修复 F-01：编辑完整值必须绑定读取 revision，补充刷新乱序回归测试。
2. 统一 F-02：重新定义 `--force`，使实现、core API、帮助、README、spec 和退出码一致。
3. 修复 F-03：CLI PATH/profile 应用必须有 sidecar 失败恢复或明确的 pending journal，禁止无记录退出。
4. 修复 F-04：注册表枚举/读取错误不得静默变成成功的部分列表。

### 第二批：架构收口

1. 先引入结构化 `CoreError`，再拆 GUI/CLI 的错误映射和 i18n。
2. 把 PATH 应用事务下沉到 Rust application service，GUI/CLI 共用。
3. 拆分 `registry.rs`，建立生产 Winreg adapter 与内存测试 adapter。
4. 建立 C→Rust golden behavior matrix 和持久化 schema migration 测试。

### 合并验收条件

- 能复现并证明 F-01 的旧请求不会覆盖新 revision。
- `--force` 在并发修改场景下行为与文档完全一致。
- sidecar 写失败后有可观测、可重试、可恢复状态。
- 列表出现任何注册表枚举/读取异常时，调用方能知道结果不完整或直接失败。
- 不写真实 HKCU 的 Rust 集成测试可以覆盖上述故障注入路径。
- 真实 Tauri 集成测试至少覆盖：GUI command 注册、camelCase serde、用户 PATH 写入、系统 PATH 权限失败、环境变量 CRUD 和窗口关闭确认。

## 八、交付判断

当前版本可以作为**功能演示和受控测试候选**，不能作为“全环境变量管理架构已经成熟”的结论。建议在修复第一批四项后重新做一次只读复审，再决定是否把 CLI 环境变量能力作为稳定自动化接口对外承诺。

本报告只记录审查和验证，不代表已经修改代码、写入注册表、提交或推送。
