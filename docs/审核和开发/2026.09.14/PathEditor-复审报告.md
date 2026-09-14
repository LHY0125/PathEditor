# PathEditor 复审报告

## 1. 复审结论

- 复审日期：2026-09-14
- 复审对象：开发窗口本轮未提交的权限、事务、导入导出、扫描、安全和模块拆分改动
- 复审结论：**请求修改（Request Changes）**
- 原因：实现方向正确，但存在一个会让真实 Tauri 运行时的权限模型失效的 P0 问题，并有异步验证竞态、禁用状态跨重启语义和失败提交状态等 P1 问题。当前单元测试和 mock E2E 通过，但不能覆盖这些真实跨层行为。

本轮没有修改业务代码，只新增复审报告。

## 2. 复审范围与验证

### 2.1 变更面

复审覆盖以下主要变更：

- `core`：新增 `PathCapabilities`、`PathEntry`、统一清理、单次扫描和导入导出。
- `gui`：新增能力查询、文件导入、条目清理和 `scan_paths` 命令。
- 前端：新增 `backend.ts`、`path-session.ts`、校验队列和权限判断。
- CLI：拆分 `runtime`、`import_export`、`profile_ops`、`scan_ops`。
- 工程：新增 `verify` / `verify:all`、CI E2E job、CSP 收紧和更多单元测试。

### 2.2 独立验证结果

| 检查项                                                  | 结果                                                                  |
| ------------------------------------------------------- | --------------------------------------------------------------------- |
| `npm run format:check`                                  | 通过                                                                  |
| `npm run build`                                         | 通过                                                                  |
| `npm run test:coverage`                                 | 124 tests passed，Lines 87.09%                                        |
| E2E（沙箱外启动 Chromium）                              | 13 passed                                                             |
| `cargo fmt --check`                                     | 通过                                                                  |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过                                                                  |
| `cargo test --workspace`                                | 58 passed                                                             |
| `npm run lint`                                          | 0 error，但 ESLint 扫描了 `coverage/lcov-report`，当前有 5 个 warning |

说明：E2E 在受限沙箱内会因 Chromium `spawn EPERM` 失败，增加权限后在 Windows 本机 13 条均通过；这不是代码失败。复审没有向真实系统 PATH 注册表写入数据，主要依赖源码审查、单元测试、E2E mock 和静态检查。

## 3. 阻塞性问题

### REV-01 [P0] `PathCapabilities` 的 Rust 字段名与前端不匹配，真实权限模型仍然失效

**证据**

- Rust 结构体 `core/src/capabilities.rs:6-12` 使用 `can_read_system`、`can_write_system`、`can_read_user`、`can_write_user`。
- Serde 默认按 Rust 字段名序列化，当前没有 `#[serde(rename_all = "camelCase")]`；搜索仓库没有发现对应的重命名配置。
- 前端接口 `src/core/path-capabilities.ts:5-9` 读取的是 `canReadSystem`、`canWriteSystem`、`canReadUser`、`canWriteUser`。
- `src/services/backend.ts:57` 直接把命令返回值断言为这个 camelCase 类型；`src/store/app-store.ts:370-375` 直接使用 `capabilities.canWriteSystem`。
- Unit/E2E mock 在 `tests/unit/app-store.test.ts:310-315` 和 `e2e/mocks/ipc.ts:11-16` 手工返回 camelCase，因此测试没有覆盖真实 Rust 序列化。

**实际行为**

Rust 实际会返回类似：

```json
{
  "can_read_system": true,
  "can_write_system": false,
  "can_read_user": true,
  "can_write_user": true
}
```

前端读取 `canWriteSystem` 会得到 `undefined`。`initialize` 随后把 `isAdmin` 设为假值；`canWriteTarget` 又因四个 camelCase 字段全部不可见而回退到 `isAdmin=false`。结果是无管理员权限判断、用户 PATH 可写能力和系统 PATH 状态在真实桌面应用中都可能错误。

**建议**

- 在 Rust 返回结构上增加 `#[serde(rename_all = "camelCase")]`，或统一改成 snake_case 并同步所有 TS/测试代码。
- 增加真实序列化契约测试，例如断言 `serde_json::to_value(PathCapabilities{...})` 的键名。
- 在 `backend.getPathCapabilities` 边界增加运行时形状校验，不能只依赖 TypeScript 类型断言。
- E2E mock 应从共享 fixture/真实序列化结果生成，避免再次手工构造出错误契约。

### REV-02 [P1] 验证队列在列表变化时可能永久留下 `pending`

**证据**

- `src/hooks/use-path-validation.ts:91-119` 让一次 effect 生成顺序执行所有批次，并把正在处理的路径写入 `inFlightRef`。
- effect 清理函数在 `123-125` 只设置 `cancelled=true`。
- 新 effect 在 `94-100` 会过滤仍然处于 `inFlightRef` 的路径；如果这些路径仍在新的列表中，它们不会被重新验证。
- 旧批次完成后再在 `105-107` 清除 in-flight 并因 `cancelled` 直接返回，不写入结果，也不会触发新的 effect。

**触发场景**

用户在一批验证尚未返回时重排、编辑或替换列表，且受影响路径仍存在于新列表。旧 effect 被取消，新 effect 跳过这些路径，最终它们可能一直显示为 `pending`，直到用户再次改变列表。

**测试缺口**

`tests/unit/use-path-validation.test.tsx:45-53` 只验证静态列表的 19/20/21/100 条；`107-120` 只验证删除后的缓存清理，没有覆盖“请求未完成时 rerender”的竞态。

**建议**

- 用 generation id 隔离每一轮验证；取消时不要清掉新一轮仍需要的 in-flight 状态。
- 在 `finally` 中清理 in-flight，并让当前列表对 pending 项重新调度一次。
- 增加 deferred promise 测试：第一批请求未完成时 rerender，随后 resolve 旧请求，断言所有当前路径最终离开 `pending`。

### REV-03 [P1] 禁用路径跨重启仍然会丢失，`disabled.json` 会变成孤儿数据

**证据**

- 保存时 `src/services/path-session.ts:102-103` 只把 `enabled=true` 的条目写入注册表，因此禁用条目被从注册表移除。
- `src/services/path-session.ts:151-157` 又把禁用路径写入 `disabled.json`。
- 启动时 `src/services/path-session.ts:53-69` 只遍历从注册表读到的 `sysArr/userArr`，用禁用列表给这些已有条目打标；没有把 `disabled.json` 中不在注册表里的条目重新放回列表。
- `core/src/registry.rs:54-65` 也只从注册表返回当前 PATH 字符串。

**影响**

如果用户保存一个禁用路径，当前会话还能看到它；重新启动后它不在注册表中，也不会被禁用列表补回，因此会从 UI 消失。`disabled.json` 中仍保留该路径但没有任何消费者，之后再次重启用也缺少可靠的顺序信息。CLI 的禁用命令仍只写 sidecar、不修改注册表，导致 CLI 和 GUI 对“禁用”的语义不一致。

**建议**

- 明确产品语义并统一实现：如果禁用需要跨会话，应在启动时把 `disabled.json` 中的条目合并回 `PathSnapshot`，并保存其原始顺序；如果禁用只在当前会话生效，则不要持久化孤儿路径。
- 将禁用状态建模为完整的有序 `PathSnapshot`，而不是只存字符串集合。
- 修正 CLI `disable/enable`，使其与 GUI 的“是否写入注册表”约定一致；当前 `cli/src/main.rs:306-334` 只更新 sidecar，PATH 对子进程仍然有效。

### REV-04 [P1] 禁用状态写入失败后，注册表成功但前端认为已保存，无法重试

**证据**

- `src/services/path-session.ts:163-191` 在注册表成功后尝试保存禁用状态；失败时返回 `kind: 'partial'`。
- 同段仍把 `nextSavedSys`、`nextSavedUser` 设为最新快照。
- `src/store/app-store.ts:355-364` 无条件用 `outcome.savedSys/savedUser` 更新 `_savedSys/_savedUser`，随后 `isModified` 会变为 `false`。
- `tests/unit/app-store.test.ts` 没有覆盖 `saveDisabledState` 失败，只覆盖了注册表 hive 的部分成功。

**影响**

用户看到“注册表已保存，但禁用状态写入失败”，但下次点击保存时不会知道禁用状态仍需重试，因为 UI 已把它当成干净状态。下次启动可能再次出现启用/禁用不一致。

**建议**

- 将“注册表已提交”和“禁用状态待提交”拆成两个状态。
- 禁用状态写入失败时保留 `isModified=true` 或增加显式的 `pendingMetadata` 状态，并让重试只补写 sidecar。
- 增加对应的失败恢复测试，断言 UI 不会错误清除 dirty 标记。

## 4. 非阻塞但应尽快处理的问题

### REV-05 [P2] 导入目标权限检查不完整，可能先改草稿再在保存时失败

**证据**

- `src/hooks/use-app-actions.ts:104-120` 只在导入开始检查当前 tab 是否可写。
- 当导入内容是 system-only 且当前 tab 是 user 时，代码会直接 `replacePaths(SYSTEM, system)`，没有检查 `canWrite(TargetType.SYSTEM)`。
- `src/hooks/use-app-actions.ts:204-219` 在用户选择不可写 hive 时会静默关闭对话框；`ImportDialog` 仍会展示所有目标选项。

**影响**

用户可能在没有权限的 hive 上修改草稿，界面却只显示为已修改，直到保存时才失败；导入对话框也可能出现选择了目标但没有执行任何操作的现象。

**建议**

- 根据导入内容与 capabilities 过滤/禁用 ImportDialog 的目标选项。
- 在只操作一个 hive 的分支也显式检查该 hive 的写权限。
- 对不可写目标给出明确提示，而不是静默关闭。

### REV-06 [P2] 旧的 TypeScript 导入导出实现仍然存在，且注释已经与现实不符

**证据**

- `src/core/import-export.ts:1-6` 仍写着“前端使用此模块，CLI 使用 Rust 版，修改时需同步”。
- 运行时代码已改为 `backend.importFile` / `backend.exportPathEntries`；生产源码没有导入 `src/core/import-export.ts`。
- `tests/unit/import-export.test.ts` 和 `tests/unit/import-parity.test.ts` 仍在测试这套未使用实现。
- `src/core/validation.ts:29-35` 的 `split_path` 同样只有测试引用，生产已使用 Rust 边界。

**影响**

覆盖率会因为测试未使用代码而虚高，重构者也会误以为仍有两套运行时实现需要同步。它与本轮“统一契约”的目标相反。

**建议**

- 删除不再使用的 TS 实现和仅围绕它的测试，或将其明确改名为 fixture/规范样例。
- 将测试迁移到 Rust 契约：同一输入经过真实 Rust 命令、序列化和前端后端边界，验证结果一致。

### REV-07 [P2] CLI 保存配置时不能保证包含已经从注册表移除的禁用项

**证据**

- `cli/src/profile_ops.rs:17-35` 读取注册表路径，再用 `disabled_sys/disabled_usr` 标记 `enabled`。
- 已禁用的路径在 GUI 保存后会被从注册表移除；此时它不在 `registry::load_system_paths()` / `load_user_paths()` 的结果中，因此不会进入配置。
- `cli/src/registry.rs:128-160` 的清理/保存语义也以注册表中的字符串列表为输入，没有读取 sidecar 中的孤儿禁用项。

**影响**

CLI 保存的配置可能无法表达完整的 UI 状态：禁用路径仍不在配置中，应用配置后无法恢复用户之前的禁用列表。GUI 使用当前 Store 时通常可以避开这个问题，但 CLI 与 GUI 的配置语义不一致。

**建议**

- 配置保存应接收完整 `PathSnapshot`，而不是只接收注册表字符串。
- 在 CLI 中先合并注册表路径与 `disabled.json`，再生成 `ProfilePathEntry[]`，并保持原始顺序。
- 增加“禁用后保存配置、应用配置、重启后仍保留禁用状态”的 CLI 测试。

### REV-08 [P3] `loadDisabledState` 把契约错误静默伪装成空状态

**证据**

- `src/services/backend.ts:46-49` 对非数组结果直接返回 `[[], []]`。
- 这会让 Rust/前端字段变化、旧二进制缓存或 mock 漂移表现为“没有任何禁用项”，而不是显式失败。

**影响**

这会掩盖发布不匹配和序列化回归，甚至让后续保存覆盖已有的 `disabled.json` 状态。它正是本次 `PathCapabilities` 问题没有被测试捕获的同类风险。

**建议**

- 对返回结构做严格校验，不符合契约时抛出明确错误。
- 只有“文件不存在”才将状态解释为空，不要对任意错误都回退为空数组。

### REV-09 [P3] `verify:all` 没有包含 ESLint，和 CI 的验证范围不完全一致

**证据**

- `package.json:28-29` 的 `verify` 串联了格式、构建、覆盖率、Rust 格式、Clippy 和测试，但没有 `npm run lint`。
- CI 的 frontend job 仍单独执行 `npx eslint src/ tests/ e2e/`。
- 本机执行 `npm run lint` 会扫描 `coverage/lcov-report`，当前产生 3 个生成文件 warning；CI 使用显式路径所以没有这个问题。

**影响**

开发者运行 `npm run verify:all` 会得到一个与 CI 不等的“全量通过”，lint 回归只能在 CI 暴露；本地 lint 又会混入生成目录噪声。

**建议**

- 将 lint 加入 `verify`，并让 lint 命令只扫描 `src`、`tests`、`e2e`，或明确忽略 `coverage`。

## 5. 已确认的改进

以下方向已通过源码审查和现有测试确认：

- `TargetType` 的权限判断不再使用真值判断，而是使用显式 hive/capability 分支。
- Store 的替换接口已接收 `PathEntry[]`，GUI 导入和配置应用不再主动丢弃 `enabled`。
- 注册表写入成功后集中提交禁用状态，比旧的分散即时写入更接近正确事务边界。
- GUI 清理已改为 Rust 统一实现，存在性检查与环境变量路径处理方向正确。
- 分析扫描改成单次目录枚举，并限制为 8 个工作线程。
- Tauri CSP 已从 `null` 收紧，GUI 的 `read_text_file` 改为 scoped 版本。
- 导入导出 CSV 引号/逗号转义已在 Rust 端统一，模块拆分后构建和测试均通过。

## 6. 合并前必须完成

1. 修复 `PathCapabilities` 的序列化键名，并加入真实 Rust→TS 契约测试。
2. 修复验证队列在 rerender/取消时的 in-flight 竞态，加入 deferred promise 回归测试。
3. 明确并实现禁用状态跨重启语义；至少消除 `disabled.json` 孤儿记录和 CLI/GUI 行为分叉。
4. 修复禁用状态写入失败后错误清除 dirty 状态的问题。
5. 根据本轮结论重新运行完整验收，并确认真实 Tauri 环境至少执行一次“普通用户编辑用户 PATH”和“禁用后重启”的集成验证。

## 7. 复审边界

- 本报告只审查当前工作树，不把尚未提交的改动视为已合并或已发布。
- 本次没有写真实注册表；因此真实桌面闭环仍需开发窗口在 Windows 上补一次有记录的手工或集成验证。
- `npm run verify:all` 的核心检查通过；E2E 需要通过非沙箱浏览器启动环境执行，避免把 `spawn EPERM` 误判为测试失败。
