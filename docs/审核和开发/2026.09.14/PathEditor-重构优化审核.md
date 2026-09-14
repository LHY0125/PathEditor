# PathEditor 重构与优化审核

## 1. 审核范围与结论

- 审核日期：2026-09-14
- 仓库：`D:\Code\doing_exercises\programs\PathEditor`
- 代码基线：`main` 的当前工作树；本轮只读业务代码，并新增本文档
- 技术构成：React 19 + TypeScript + Zustand + Tauri 2 + Rust workspace（`core` / `gui` / `cli`）
- 项目现状：功能面已经比较完整，PATH 编辑、撤销重做、导入导出、配置、备份、冲突检测和 CLI 均有实现；主要短板已经不在“缺功能”，而在正确性边界、跨语言契约、质量门禁和模块边界。

建议的优化顺序：

1. 先恢复质量门禁，并把 P0 行为写成回归测试。
2. 修复权限、`enabled` 状态传播、长列表验证和清理语义。
3. 统一 Rust/TypeScript 的数据与导入导出契约，减少双实现漂移。
4. 再拆分 `app-store`、CLI 和大型弹窗组件，补强 IPC 类型与安全配置。

不建议现在优先做“大规模换框架”或“重写后端”。现有 `core` / `gui` / `cli` 的分层方向是正确的，问题集中在边界协议和状态一致性，局部重构的收益更高。

## 2. 当前构建与验证基线

本轮实际执行结果如下。

| 检查项                                                  | 结果         | 证据                                                        |
| ------------------------------------------------------- | ------------ | ----------------------------------------------------------- |
| `npm run build`                                         | 通过         | Vite 产物主 JS 约 319.08 kB，gzip 约 99.06 kB               |
| `npm run lint`                                          | 通过但有警告 | 0 error，2 个 TanStack Virtual 与 React Compiler 兼容性警告 |
| `npm run test:coverage`                                 | 失败         | 105 个前端测试全部通过，但行覆盖率 66.83%，低于阈值 80%     |
| `cargo test --workspace`                                | 通过         | 57 个 Rust 测试通过                                         |
| `cargo fmt --check`                                     | 失败         | `core/src/backup.rs`、`core/src/fs.rs` 存在格式差异         |
| `cargo clippy --workspace --all-targets -- -D warnings` | 失败         | `core/src/system.rs:132` 的布尔比较触发 `bool_comparison`   |
| `npm run test:e2e`                                      | 未完成       | 本机缺少 Playwright Chromium；CI 也没有 E2E job             |
| `npm run format:check`                                  | 通过         | 前端格式一致                                                |

环境版本：Rust 1.96.0、Cargo 1.96.0、Node.js 24.19.0、npm 11.17.0。

结论是：单元测试数量不少，但 CI 目前不能作为可靠的重构保护网。`vitest.config.ts` 的 80% 行覆盖率阈值和 `.github/workflows/ci.yml` 的覆盖率命令、Rust 格式检查、Clippy 命令在当前基线下都会失败。

## 3. 问题清单

### F-01 [P0] CI 门禁当前不是绿色

**证据**

- 覆盖率阈值定义在 `vitest.config.ts:13-20`，CI 在 `.github/workflows/ci.yml:36-45` 直接执行 `vitest run --coverage`。
- 实测覆盖率为 Statements 65.03%、Branches 56.23%、Functions 56.25%、Lines 66.83%。
- Rust CI 在 `.github/workflows/ci.yml:55-65` 执行格式、Clippy 和测试；当前 `cargo fmt --check` 与 `cargo clippy -- -D warnings` 均失败。
- E2E 没有出现在 CI job 中；本机单独执行时因缺少 Chromium 无法启动。

**影响**

任何重构都可能在“本地单测通过、CI 失败”或“没有真实交互测试”的状态下合入。当前门禁不能承担后续大改动的回归保护职责。

**建议**

1. 立即修正 Rust 格式和 Clippy 问题，不把这两个问题带入重构分支。
2. 覆盖率不要用“降低全局阈值”掩盖，而是补上 `use-path-validation`、`use-keyboard`、`theme-store` 和错误分支测试；如确需分阶段，可按目录设置阈值，但必须记录退出条件。
3. 在 CI 中加入 E2E job，并在 job 中安装 Playwright Chromium；E2E 至少覆盖启动、CRUD、保存、导入、配置应用和权限分支。
4. 把 `npm run build`、格式检查、覆盖率和 Rust 检查合并成一个统一的本机验收命令。

**验收**

- `npm run format:check`、`npm run build`、`npm run test:coverage`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 全部成功。
- E2E 在 CI 中可重复执行，不再依赖开发者机器已安装浏览器。

### F-02 [P0] 非管理员模式把用户 PATH 编辑也锁死了

**证据**

- `core/src/system.rs:14-20` 只通过尝试写 HKLM 系统 PATH 注册表来判断 `isAdmin`。
- `src/store/app-store.ts:463-471` 直接把这个结果写入全局 `isAdmin`。
- `src/components/toolbar/ActionButtons.tsx:25-49`、`src/components/toolbar/ToolBar.tsx:39-56`、`src/components/toolbar/UndoRedoButtons.tsx:18-30` 和 `src/hooks/use-keyboard.ts:39-58` 都用同一个 `isAdmin` 禁用所有编辑能力。

**影响**

用户 PATH 位于 HKCU，通常不需要管理员权限；普通启动时却无法新建、编辑、删除、导入或保存用户 PATH，迫使应用不必要地以管理员身份运行。权限模型与应用实际的注册表边界不一致。

**建议**

- 将二值 `isAdmin` 拆成能力集合：`canReadSystem`、`canWriteSystem`、`canReadUser`、`canWriteUser`。
- 读取和写入按 hive 独立判断；用户 PATH 的常规操作不依赖 HKLM 写权限。
- UI 按钮按当前 tab 的目标 hive 和对应能力禁用，而不是统一看 `isAdmin`。
- 保存时只提交有权限且确实修改的 hive，并分别报告结果。

**验收**

- 以普通用户启动时，用户 tab 可完整编辑并保存。
- 系统 tab 在无权限时清晰显示只读或权限提示，而不是把所有功能都标成只读。
- 增加非管理员权限分支的 Store、组件和 E2E 测试。

### F-03 [P0] 导入和应用配置会丢失 `enabled` 状态

**证据**

- `src/store/app-store.ts:216-250` 中 `replacePaths` 和 `replaceBothPaths` 接收的是 `string[]`，并统一构造 `{ path, enabled: true }`。
- `src/hooks/use-app-actions.ts:99-107` 和 `191-205` 调用替换接口前只保留 `e.path`，丢弃 `e.enabled`。
- `src/components/dialogs/ProfileDialog.tsx:67-75` 先把配置中的路径替换进 Store，再单独写 `disabled.json`，但 Store 中这些路径已经全部变成启用状态。
- `cli/src/main.rs:393-416` 的导入同样只提取路径字符串；带 `enabled: false` 的 JSON/CSV 条目会被写入注册表。

**影响**

这是用户可感知且可能造成环境变化的功能错误：从 JSON/CSV 导入或应用配置时，原本禁用的路径可能重新进入注册表。禁用状态文件与真实注册表状态可能互相矛盾。

**建议**

- 在领域层统一用 `PathEntry[]` 或 `PathSnapshot` 作为替换接口输入，禁止在 Store 边界再次降级成 `string[]`。
- `replacePaths`、`replaceBothPaths` 和 CLI 导入都必须保留每个条目的 `enabled` 值。
- 导入、应用配置、撤销/重做和保存使用同一个 `PathSnapshot`，由保存服务按 `enabled` 过滤后写注册表。
- 只有用户明确选择“全部启用”时，才允许把状态改为启用。

**验收**

- JSON、CSV、配置文件和 CLI 导入均保留禁用标记。
- 应用配置后，禁用路径不出现在注册表写入列表。
- 增加“带 false 的导入/配置/撤销重做”回归测试。

### F-04 [P1] PATH 超过 20 条后，验证和环境变量展开会停止

**证据**

- `src/hooks/use-path-validation.ts:56-85` 只对 `toValidate.slice(0, 20)` 发起一次验证。
- 同一文件 `88-116` 的展开逻辑也只处理前 20 条。
- 两个 effect 都只依赖 `paths`；异步结果写入 state 后不会改变 `paths` 引用，因此 effect 不会继续处理下一批。
- `src/components/path-list/PathTable.tsx:47-55` 对没有缓存的路径默认返回 `'valid'`。

**影响**

条目数超过 20 时，后续路径不会被验证或展开，但界面可能仍显示为正常状态。用户会误以为这些路径有效，`一键清理` 也无法利用这些验证结果。这是“长列表优化”真正需要优先解决的部分。

**建议**

- 把验证/展开改成有界并发队列，持续处理到没有待处理 key，或在单个 effect 中完整循环分批。
- 使用 `Promise.allSettled` 保证单条失败不阻塞后续条目。
- 给缓存增加明确的 `pending` 状态，避免未验证项默认为 `valid`。
- 并发度可配置，建议 8-16，避免一次发起几十个 IPC。

**验收**

- 25、200、1000 条路径时，所有唯一的 `%VAR%` 路径都被展开，所有非环境变量路径都得到 `valid`/`invalid`/`unknown` 结果。
- 删除或编辑路径后缓存不会污染新列表。

### F-05 [P1] GUI 的“一键清理”并不检查路径是否真实存在

**证据**

- `src/hooks/use-app-actions.ts:79-83` 把 `is_valid_path_format` 传给 `cleanPaths`。
- `src/core/validation.ts:6-22` 只判断字符串形态，不检查目录是否存在；`C:\不存在的目录` 会被判为有效格式。
- Rust `core/src/registry.rs:122-140` 的 `clean_paths` 对非 `%` 路径使用 `Path::is_dir()`，因此 CLI 与 GUI 的清理语义不同。

**影响**

GUI 能标红不存在的路径，但“一键清理”不一定删除它，用户会认为清理没有生效。GUI 与 CLI 对同一命令的理解不一致。

**建议**

- 把“格式合法”和“真实存在”拆成两个独立概念：`isValidFormat` 与 `exists`。
- GUI 清理通过 backend 的统一清理服务执行，或调用一个批量 `inspect_paths` IPC，而不是在纯前端只做字符串判断。
- 环境变量路径需要使用展开后的结果判断，展开失败时保留并标记为 `unknown`，不能直接删除。

**验收**

- 格式正确但目录不存在的路径可被 GUI 一键清理。
- 环境变量路径、UNC 路径、权限不足路径分别有明确行为。
- GUI 与 CLI 对相同输入返回一致的 kept/removed 集合。

### F-06 [P1] Rust 与 TypeScript 的导入导出是两套实现，已经开始漂移

**证据**

- 两份实现都明确写着“修改时需同步”：`src/core/import-export.ts:1-6`、`core/src/fs.rs:1-2`。
- TXT 分类不一致：TS `src/core/import-export.ts:217-227` 把 TXT 放入 system，Rust `core/src/fs.rs:187-202` 把 TXT 放入 user。
- CSV 写出不一致：TS `src/core/import-export.ts:39-58` 会转义逗号和引号；Rust `core/src/fs.rs:223-230` 直接拼接路径，包含逗号时会破坏 CSV。
- `src/core/validation.ts:29-35` 和 `core/src/registry.rs:83-99` 也各自维护了 PATH 分割/拼接逻辑。

**影响**

GUI 与 CLI 对同一格式可能产生不同结果；某个端修复后，另一端很容易继续使用旧语义。后续新增格式或加入新字段时，漂移概率会继续增大。

**建议**

- 把序列化、反序列化和 PATH 分割/拼接收敛到 `core`，通过一个批处理 IPC 暴露给 GUI。
- GUI 只负责选择文件、显示预览和确认目标，不再保留第二套解析实现。
- TS 侧类型从 Rust 契约生成，至少用共享 JSON schema/样例做契约测试。
- CSV 写出必须经过统一的 RFC 4180 子集转义，TXT 的目标 hive 必须写成明确规则并测试。

**验收**

- 同一组 JSON/CSV/TXT 在 GUI 和 CLI 中产生相同 `PathSnapshot`。
- GUI 导出的 CSV 可被 Rust 导入且路径、`enabled`、顺序完全一致。
- 删除重复实现后，旧测试改为契约测试而不是两端各自测试。

### F-07 [P1] 禁用状态在注册表保存前就持久化，缺少一致的事务边界

**证据**

- `src/store/app-store.ts:295-301` 在 `togglePath` 中立即调用 `save_disabled_state`，不等待用户点击保存。
- `src/components/dialogs/ProfileDialog.tsx:71-76` 先写禁用状态，再调用 `savePaths()`。
- 保存失败或用户取消保存时，`disabled.json` 已经改变，但注册表仍是旧状态。
- `core/src/disabled.rs:27-43` 将其定义为“即时持久化”，但它和注册表是两个独立存储。

**影响**

下次启动时应用会把磁盘中的禁用状态重新映射到旧注册表内容，导致界面状态与实际 PATH 不一致。配置应用失败时尤其容易发生。

**建议**

- 把“编辑态”和“已提交态”分开：`draftPathSnapshot` 与 `committedPathSnapshot`。
- 只有注册表保存成功后才提交 `disabled.json`，或者把禁用状态写入同一份应用状态文件并在启动时做一致性校验。
- 保存失败时保留 draft 并让用户重试，不要产生“半保存”的禁用状态。
- 对 system/user 双 hive 保存给出明确的 partial-success 恢复方案。

**验收**

- 关闭应用或保存失败后重新打开，界面状态与注册表一致。
- 应用配置失败时不会改变持久化禁用状态。

### F-08 [P2] 分析界面重复扫描 PATH 目录

**证据**

- `src/components/dialogs/AnalyzeDialog.tsx:47-57` 同时调用 `scan_conflicts` 和 `scan_tools`。
- `core/src/scanner.rs:52-118` 两个扫描函数都独立遍历 PATH 目录，`list_exes` 也在每个路径中重复执行。
- 扫描线程按目录无上限创建，PATH 数量异常大时线程数不可控。

**影响**

分析操作会对文件系统做约两倍的工作；在磁盘慢、PATH 目录大或网络路径不可用时，弹窗延迟明显。线程数量完全由输入 PATH 数量决定，缺少有界并发。

**建议**

- 新增一个 `scan_paths(paths)` 命令，一次读取每个目录的 executable 列表，再分别派生出冲突和工具清单结果。
- 使用固定的有界线程池或 `rayon`，限制并发目录数。
- 对扫描结果增加短期缓存；只在 PATH 快照或显式刷新时失效。

**验收**

- 同一路径列表只发生一次目录枚举。
- 结果保持现有排序、遮蔽优先级和查询过滤语义。
- 大 PATH 列表下并发数有上限，不因线程创建失败而失效。

### F-09 [P1] IPC 调用散落在组件和 Store 中，类型契约靠手工复制

**证据**

- `rg` 检索到约 20 处 `invoke(...)` 调用，分布在 `src/store/app-store.ts`、`src/hooks/use-app-actions.ts`、`src/components/dialogs/AnalyzeDialog.tsx` 和 `ProfileDialog.tsx`。
- `ProfileMeta`、`ProfileData`、`ConflictEntry`、`ToolGroup` 等类型在 TS 文件中手工声明；Rust 端另有同名结构体。
- 组件直接处理 `invoke` 错误、按钮状态和部分业务规则；`AnalyzeDialog` 的失败分支只写 `console.error`。

**影响**

命令名、参数名或返回结构变化时，TypeScript 编译器不能自动发现 Rust 端不兼容；组件测试也需要大量 mock。错误处理容易遗漏，且错误信息难以统一呈现给用户。

**建议**

- 建立 `src/services/backend.ts`，集中定义所有 IPC 命令的参数、返回值和错误映射。
- Rust 端使用 `specta`、`ts-rs` 或等价的类型导出方案，让 `PathEntry`、`ProfileData`、`ConflictEntry` 等模型单点定义。
- 对保存、加载、导入、配置应用建立领域服务，组件只消费服务结果和用户可见错误。
- 统一错误模型，例如 `AppError { code, message, context }`，避免每个组件自己 `String(error)`。

**验收**

- 组件和 Store 不再直接出现裸命令字符串。
- Rust 命令参数或返回结构变化时，TypeScript 构建能立刻失败。
- IPC 失败在 UI 中有稳定、可测试的状态，不只出现在控制台。

### F-10 [P1] 测试覆盖结构不平衡，E2E 没有覆盖真实闭环

**证据**

- 前端单测对 `src/core` 覆盖较高：import-export 89.62% lines、undo-redo 95.23% lines。
- `src/hooks/use-path-validation.ts` 为 0%，`use-keyboard.ts` 为 0%，`theme-store.ts` 为 0%。
- `src/store/app-store.ts` 约 81% lines，但权限、长列表、失败恢复和部分保存分支仍未覆盖。
- E2E mock 在 `e2e/mocks/ipc.ts` 中完全替代 IPC；它证明前端交互，不证明真实 Rust/Tauri/注册表闭环。

**影响**

当前测试数量看起来不少，但最容易出错的异步、权限和持久化边界恰好覆盖不足。重构时会出现“单元测试都过、真实应用状态仍不一致”的情况。

**建议**

1. 为 `use-path-validation` 增加 0、1、19、20、21、100+ 条目的队列测试。
2. 为权限模型增加“仅用户可写”和“仅系统可写”的 Store/组件测试。
3. 为导入/配置增加 `enabled=false`、保存失败、部分成功、回滚测试。
4. 增加真实 Tauri 集成测试或受控的 Windows 注册表适配器测试，验证保存前后的注册表快照。
5. E2E 在 CI 中安装浏览器并执行，覆盖主流程和错误提示。

**验收**

- 覆盖率阈值通过，且不是靠排除关键 hook 实现。
- P0/P1 问题都有至少一个会失败的回归测试。
- E2E 至少有一条路径经过真实 IPC 适配层，而不是全量 mock。

### F-11 [P2] 几个核心模块已经过长，下一步拆分应围绕职责而不是文件行数

**证据**

- `src/store/app-store.ts` 约 421 行，同时承担 CRUD、撤销重做、导入替换、注册表加载、保存协调、权限状态和 UI 状态。
- `cli/src/main.rs` 约 605 行，包含命令定义、参数校验、注册表读写、备份、导入导出、扫描和配置应用。
- `AnalyzeDialog.tsx` 约 216 行、`ProfileDialog.tsx` 约 277 行，混合了数据加载、持久化、错误处理和展示。

**建议**

- `core`：抽出 `PathSnapshot`、`PathEntry`、`PathCapabilities`、`SavePlan`、`PathStore`。
- `app-store`：只保留 UI 状态和 action 编排；加载、保存、导入、配置应用移到 service。
- `cli`：至少拆成 `commands/`、`runtime.rs`、`output.rs`、`profile.rs`、`registry_ops.rs`。
- `ProfileDialog`：拆出 profile list、profile detail、apply workflow；`AnalyzeDialog` 拆出 data hook 和两个 tab。

**原则**

- 先建立清楚的领域边界，再移动文件，避免只为了缩短行数而增加间接层。
- 每个拆分都必须由现有测试或新增契约测试保护。

### F-12 [P2] Tauri 安全配置和文件读取接口应做防御性收紧

**证据**

- `gui/tauri.conf.json:24-26` 的 CSP 为 `null`。
- `core/src/fs.rs:28-40` 的 `read_text_file` 只按扩展名白名单读取路径；`read_text_file` 被注册为可调用 IPC 命令。
- 当前应用没有远程页面，但 WebView 内容一旦被注入，CSP 缺失会放大影响。

**建议**

- 设置明确的最小 CSP，至少限制脚本、样式、连接源和 `img-src`。
- 继续保留扩展名校验，同时增加规范化路径和允许根目录约束；若要读取用户选择的任意路径，优先使用 Tauri 的 dialog 返回 capabilities，而不是扩大命令能力。
- 在安全回归测试中验证不合法扩展名和越界路径被拒绝。

**验收**

- 生产配置不再使用 `csp: null`。
- `read_text_file` 的允许范围、错误信息和失败测试都明确。

### F-13 [P3] 文档、国际化和错误提示已经出现漂移

**证据**

- `README.md` 的测试数量、版本说明和分支 badge 已与当前代码/CI 状态不完全一致。
- `src/components/layout/AppShell.tsx:100-108`、`src/components/dialogs/ProfileDialog.tsx:94-96`、`src/components/layout/ErrorBoundary.tsx:30-39` 存在硬编码中文。
- Rust 层错误字符串直接返回中文，英文界面无法完整国际化。

**建议**

- 把用户可见文案集中到 i18n key；Rust 返回错误码和参数，由前端本地化。
- 修改版本号、测试命令、覆盖率状态时同步 README、ROADMAP 和帮助弹窗。
- 对文档增加链接检查和状态描述检查，避免把计划项写成已交付。

## 4. 推荐的目标结构

```text
core/
  domain/
    path_entry.rs          # PathEntry / PathSnapshot / Target
    capabilities.rs        # 按 hive 描述读写能力
    save_plan.rs           # 保存意图和结果
  adapters/
    registry.rs            # 只负责 Windows 注册表
    app_state.rs           # disabled/profiles/backup 的统一持久化
  services/
    import_export.rs       # 唯一解析/序列化实现
    path_cleaner.rs        # 格式、存在性、重复、环境变量规则
    scanner.rs             # 单次枚举 + 有界并发

gui/src/commands/
  snapshot.rs              # load/save 的粗粒度命令
  capabilities.rs          # 权限查询
  import_export.rs         # 文件和预览
  profiles.rs
  scanner.rs

src/
  services/backend.ts      # 唯一的 Tauri IPC 边界
  features/path-editor/    # 编辑器 domain model 和 service
  store/                   # 只负责 UI state 与 action 编排
  components/              # 展示和用户交互
```

关键约束是：同一个领域对象只在 Rust 和 TypeScript 各定义一次；导入导出、清理、权限判断和保存计划不再分别写在 GUI、CLI 和前端。

## 5. 分阶段实施计划

### 阶段 0：恢复质量门禁（建议 1-2 天）

- 修复 `cargo fmt` 和 Clippy 基线问题。
- 确认覆盖率阈值与 CI 命令一致，补关键 hook 测试或先按目录设置可退出阈值。
- 在 CI 中加入 Playwright 浏览器安装和 E2E job。
- 把当前本机验证命令整理成一个脚本或 Make 等价入口。

### 阶段 1：修复 P0/P1 状态语义（建议 3-5 天）

- 按 hive 拆分权限能力，普通用户可编辑用户 PATH。
- 所有替换/导入/配置路径统一保留 `PathEntry.enabled`。
- 完成验证与展开的分批队列。
- 统一 GUI/CLI 清理语义。
- 把禁用状态纳入注册表保存提交边界。

### 阶段 2：统一契约与 IPC（建议 5-8 天）

- 把导入导出、PATH 分割拼接、清理规则收敛到 `core`。
- 建立 `services/backend.ts` 和生成的 TS 类型。
- 将分析改为单次目录枚举。
- 用契约测试覆盖 JSON/CSV/TXT、顺序、启用状态和错误输入。

### 阶段 3：控制模块复杂度（建议 5-10 天）

- 拆分 `app-store` 的 service 与 UI state。
- 拆分 CLI 命令模块。
- 拆分 `AnalyzeDialog` / `ProfileDialog` 的数据工作流。
- 收紧 CSP 和文件读取能力。
- 清理 i18n、README、ROADMAP 的漂移。

## 6. 总体验收标准

- 本机与 CI 的全部质量命令通过，且没有未解释的 warning。
- 普通用户无需管理员权限即可编辑并保存用户 PATH；系统 PATH 的权限提示准确。
- 导入、配置、撤销重做、保存、启动回载之间保持同一份 `PathSnapshot` 语义。
- 1000 条 PATH 条目全部完成验证/展开，不因批处理上限漏检。
- GUI 与 CLI 对 CSV、JSON、TXT 和清理任务的 kept/removed 结果一致。
- 所有关键 IPC 类型由单一来源生成，组件不再直接拼命令字符串。
- 分析扫描只枚举目录一次，并有明确并发上限。

## 7. 审核边界与未执行项

- 本轮没有写入真实系统/用户 PATH 注册表；验证以源码审查、单元测试、构建和静态检查为主。
- E2E 未完成，原因是本机缺少 Playwright Chromium；这本身说明当前项目不能把 E2E 视为稳定的本地/CI 门禁。
- 审核期间工作树存在其他未提交的 `ROADMAP.md` 变更和旧审查文档变更；本报告没有修改或回滚这些内容。
- 本文档只提出重构与优化方案，没有修改业务代码。
