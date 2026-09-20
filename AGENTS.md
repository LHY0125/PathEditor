# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# PathEditor 开发指南

> 本文件是开发窗口的仓库说明；更新后同步复制为根目录 `AGENTS.md`，两份文件应保持一致。

## 项目概述

PathEditor v5.1.3 是 Windows 系统环境变量（PATH）编辑器，采用 Tauri 2.x + React 19 + TypeScript strict + Rust workspace，提供 GUI 和 CLI 两种入口。

系统 PATH（HKLM）写入需要管理员权限；用户 PATH（HKCU）由 `PathCapabilities` 按 hive 独立判断权限。不能再用一个全局 `isAdmin` 字段推断两个 PATH 是否可写。

## 快速命令

```powershell
npm install                         # 安装前端依赖
npx tauri dev                       # GUI 开发模式（热更新）
npm run dev                         # 仅运行 Vite 前端
npm run build                       # 前端类型检查 + Vite 构建
cargo check                         # Rust workspace 检查
cargo build --release -p patheditor-cli
npx tauri build                     # 生成 NSIS 安装包

npm test                            # Vitest 单元测试
npm run test:watch                  # Vitest 监听模式
npm run test:coverage               # 单元测试 + 覆盖率门槛
npm run test:e2e                    # Playwright（生产构建 + mock IPC）
npm run lint                        # ESLint
npm run format:check                # Prettier 检查
cargo fmt --check                   # Rust 格式检查
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace              # Rust 全量测试
cargo test -p patheditor-cli --bins # CLI 单测（该 crate 无 lib target）
npm run verify                      # 前端 + Rust 全量质量门
npm run verify:all                  # verify + Playwright E2E
```

## 架构与模块边界

Cargo workspace 分为 `core`、`gui`、`cli` 三个 crate，前端通过 Tauri IPC 调用 GUI command。

```text
PathEditor/
├── core/src/                    # Rust 核心库（零 Tauri 依赖）
│   ├── registry.rs              # HKLM/HKCU PATH 读写、清理、长度校验
│   ├── capabilities.rs          # 按 hive 探测 PathCapabilities
│   ├── system.rs                # 权限、路径验证、变量展开、环境广播
│   ├── disabled.rs              # 完整快照、禁用状态、旧格式兼容
│   ├── env_var.rs               # 通用环境变量契约与保留/保护/敏感判定
│   ├── path_entry.rs            # PathEntry / PathSnapshot 契约
│   ├── scanner.rs               # scan_paths 单次枚举、冲突和工具清单
│   ├── profiles.rs              # 配置保存/加载/重命名/删除
│   ├── fs.rs                    # 受限文件读取、导入导出
│   └── backup.rs                # 注册表备份
├── gui/src/commands/            # #[tauri::command] 薄包装，调用 core
├── cli/src/                     # Clap 定义 + 命令分派（bin-only crate，无 lib target）
│   ├── runtime.rs               # 注册表安全保存、错误退出、快照提交
│   ├── env_ops.rs               # env 子命令：值通道/并发选项/表格与 JSON 渲染
│   ├── import_export.rs         # 导入导出命令
│   ├── profile_ops.rs           # profile 子命令
│   └── scan_ops.rs              # 冲突、扫描、权限检查
├── src/                         # React 前端
│   ├── core/                    # 纯逻辑，零 React/Tauri 依赖（path-manager、env-var、undo-redo…）
│   ├── services/backend.ts      # 唯一 IPC 适配入口 + 运行时形状校验
│   ├── services/path-session.ts # 加载、保存计划、部分成功、禁用快照
│   ├── store/                   # Zustand：app-store / env-store / theme-store
│   ├── components/              # layout / path-list / env-list / toolbar / dialogs / ui
│   ├── hooks/                   # useAppActions、useKeyboard、usePathValidation
│   └── i18n/                    # zh-CN / en
├── tests/unit/                  # Vitest 测试
├── tests/fixtures/              # 跨 Rust/TS 契约夹具
├── e2e/tests/                   # Playwright 测试（mock IPC）
├── docs/superpowers/specs/      # 特性设计文档（YYYY-MM-DD-*-design.md）
├── docs/superpowers/plans/      # 实施计划（YYYY-MM-DD-*-implementation.md）
└── docs/审核和开发/YYYY.MM.DD/  # 审核与开发记录
```

关键约束：

- `src/core/` 保持纯函数和零框架依赖；组件和 Store 不得直接调用 `invoke`，统一走 `src/services/backend.ts`。
- `gui` 和 `cli` 只做参数转换、命令分派和错误呈现，业务规则放在 `core`。CLI 侧**零安全判定逻辑**：保留名、保护名单、`Unsupported` 类型、hive 写权限、revision 校验全部由 core 判定，CLI 仅透传错误文本。新增 CLI 命令时不要复制任何判定规则。
- `patheditor-cli` 是 **bin-only crate**（`[[bin]] name = "patheditor"`，无 `[lib]`）。对 CLI 跑单测必须用 `cargo test -p patheditor-cli --bins`，`--lib` 会报 `no library targets found`。
- `PathCapabilities` 使用 camelCase 序列化；共享契约见 `tests/fixtures/path-capabilities.json`。
- 正式 PATH 解析以 Rust `registry::split_path` 为准；TS 的 `split_path` 仅保留为兼容/测试夹具。
- `PathEntry { path, enabled }` 是跨层契约；导入、配置、注册表和撤销重做都必须保留 `enabled`。
- 通用环境变量通路（`EnvVar`）与 PATH 通路（`PathEntry`）并存：`Path` 列入 `RESERVED_NAMES`，`list_all_env_vars` 过滤掉它，Rust 写入口拒绝 —— **PATH 只能经专用通路编辑**，否则会绕过 `disabled.json` 与快照事务。`EnvVarMeta` 契约上不含 `value` 字段，命中敏感规则的变量明文只能经 `reveal_env_var` 获取。
- 环境变量读写的并发契约只有一套：core 的 `update_env_var` / `delete_env_var` 恒要求 `expected_revision`（FNV-1a 散列 `name+type+value`），在**同一 core 调用内**完成「读→算→比对→校验→写」。GUI 与 CLI 共用该契约，前端与 CLI 都**不得**自行实现并发校验。冲突判定**只认 `CoreError.code`（serde camelCase）**：前端判 `err.code === 'conflict'`，CLI 经 `CoreError::exit_code()` 判定；`[E_CONFLICT]` 消息前缀（`core::registry::conflict_message()`）仅为过渡期展示文本（保留给旧文本路径），不是判定机制。
- Windows 注册表无 CAS，读-比-写是两次独立注册表调用，仍有毫秒级竞态窗口。revision 校验缩小影响，不能完全消除 TOCTOU —— 文档与注释须如实措辞，不要宣称原子性。
- `.codegraph/` 存在时，优先使用 CodeGraph 理解符号和调用路径，再决定是否读取文件。

## Tauri IPC 接口

| Command                                                                                 | 参数 / 返回值                                      | 说明                                                                                         |
| --------------------------------------------------------------------------------------- | -------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `load_system_paths` / `load_user_paths`                                                 | `() -> Result<Vec<String>, String>`                | 读取注册表原始路径                                                                           |
| `save_system_paths` / `save_user_paths`                                                 | `(paths, original?) -> Result<(), String>`         | 可选乐观并发校验；外部修改时拒绝覆盖                                                         |
| `clean_path_entries`                                                                    | `Vec<PathEntry> -> (kept, removed)`                | 统一清理语义，展开变量后检查目录                                                             |
| `get_path_capabilities`                                                                 | `() -> PathCapabilities`                           | 按 hive 返回读写能力                                                                         |
| `check_admin` / `validate_path` / `expand_env_vars` / `broadcast_env_change`            | 系统工具接口                                       | 权限、目录验证、变量展开、环境通知                                                           |
| `load_path_snapshot`                                                                    | `() -> Result<PathSnapshot, String>`               | 合并注册表、禁用状态和有序快照                                                               |
| `save_path_snapshot`                                                                    | `(system?, user?) -> Result<(), String>`           | 保存完整有序快照，`None` 保留对应 hive                                                       |
| `load_disabled_state` / `save_disabled_state`                                           | 旧版字符串接口                                     | 仅用于兼容                                                                                   |
| `import_file` / `export_path_entries`                                                   | 文件路径 / `PathEntry` + 格式                      | Rust 统一导入导出                                                                            |
| `scan_paths`                                                                            | `(paths, query) -> ScanResult`                     | 一次枚举生成冲突和工具清单                                                                   |
| `scan_conflicts` / `scan_tools`                                                         | 兼容包装                                           | 分别返回冲突或工具清单                                                                       |
| `backup_registry` / `get_appdata_dir`                                                   | 备份和目录查询                                     | 备份当前注册表值                                                                             |
| `list_profiles` / `save_profile` / `load_profile` / `delete_profile` / `rename_profile` | 配置 CRUD                                          | 配置保存 `PathEntry[]`                                                                       |
| `list_all_env_vars`                                                                     | `() -> Result<EnvVarSnapshot, CoreError>`          | 一次读取两个 hive 的全部环境变量元数据（不含敏感明文）                                       |
| `reveal_env_var`                                                                        | `(hive, name) -> Result<RevealedValue, CoreError>` | 按需读取单个变量明文及读取时的 revision；`Unsupported` 类型返回 `CoreError`                  |
| `update_env_var`                                                                        | `(hive, name, value, expectedRevision)`            | 写入已有变量；类型从注册表读取，revision 不匹配则拒绝（返回 `[E_CONFLICT]` 前缀错误）        |
| `create_env_var`                                                                        | `(hive, name, value, kind)`                        | 新建变量；写入前检查名称是否存在（检查与写入是两步操作，存在竞态窗口；重复创建由 Rust 拒绝） |
| `delete_env_var`                                                                        | `(hive, name, expectedRevision)`                   | 删除变量；revision 不匹配则拒绝                                                              |

> 服务层命令 `apply_path_snapshot` / `save_path_with_sidecar` / `retry_pending_path_state` / `apply_profile` 已在 `gui/src/commands/service.rs` 注册，供 CLI/服务层事务编排使用，**GUI 前端暂未接线**（`path-session.ts` 仍走旧编排，见开发回执未覆盖项）。

## CLI 命令

```text
patheditor list          [--system|--user] [--json]
patheditor add           <PATH> [--system|--user]
patheditor remove        <INDEX> [--system]
patheditor edit          <INDEX> <NEW> [--system]
patheditor move-up       <INDEX> [--steps N] [--system]
patheditor move-down     <INDEX> [--steps N] [--system]
patheditor clean         [--system|--user] [--dry-run] [--json]
patheditor enable        <INDEX> [--system|--user]
patheditor disable       <INDEX> [--system|--user]
patheditor import        <FILE> [--target system|user|both]
patheditor export        [--format json|csv|txt] [--output <FILE>]
patheditor backup
patheditor conflicts     [--json]
patheditor scan          [--query <NAME>] [--json]
patheditor check-admin   [--json]
patheditor profile       {list [--json]|save <NAME>|load <NAME>|apply <NAME>|delete <NAME>|rename --old <OLD> --new <NEW>}
patheditor env list      [--system|--user] [--json]
patheditor env get       <NAME> [--system]
patheditor env set       <NAME> [--value <V>|--stdin|--value-file <F>] (--revision <R>|--force)
patheditor env add       <NAME> [<VALUE>] [--kind string|expand] [--system]
patheditor env remove    <NAME> (--revision <R>|--force)
```

`remove`、`edit`、`move-up`、`move-down` 默认操作用户 PATH，传入 `--system` 才操作系统 PATH。CLI 的 `list`、`import/export`、`profile`、`enable/disable` 都使用完整快照，避免丢失 `enabled=false` 条目和顺序。

`env` 子命令默认操作用户 hive，加 `--system` 操作系统 hive；`Path` 不在通用通路内。`set`/`remove` 必须显式给出 `--revision` 或 `--force`（互斥，缺一报错）；`--force` 跳过 revision 校验直接覆盖（最后写入者胜，仍受保护名单/类型/权限约束），不会因并发冲突产生退出码 3；退出码 3 仅在 `--revision` 不匹配时出现，其余错误为 1。`get` 是 CLI 侧唯一明文出口，stdout 只打印裸值 + 换行（管道友好）；`env add` 的 `--kind` 默认 `string`，可选 `string`（`REG_SZ`）/ `expand`（`REG_EXPAND_SZ`）；敏感值建议用 `--stdin` 或 `--value-file` 传入，避免明文进入 shell 历史与进程列表。

## 数据、保存与事务

- `disabled.json` 同时保存 `systemSnapshot`、`userSnapshot` 和旧版禁用字符串。启动时以注册表为启用路径真相来源，用快照恢复禁用项和顺序；旧格式仍可读取。
- GUI 保存时先写注册表，再提交侧车快照。注册表成功但快照写入失败时，保留 `_pendingSys/_pendingUser` 和 `isModified=true`，下次保存只补写快照；两个 hive 分别报告成功或失败。
- 保存前使用原始值做外部修改检测；不得用旧草稿覆盖其他进程刚写入的内容。
- 清理、禁用和启用操作必须保留 `PathEntry`，不能只传路径字符串。

路径验证和环境变量展开使用有界并发队列，不按前 20 条截断；扫描通过 `scan_paths` 共享枚举结果，最多使用 8 个扫描线程。

## 错误处理与安全

- `backend.ts` 对能力对象、快照、禁用状态和 `PathEntry` 做运行时形状校验，不能只依赖 TypeScript 断言。
- Rust/IPC 仍主要返回自由文本 `Result<T, String>`；后续统一错误码 + 前端本地化尚未完成。
- 导入文件读取限制在用户目录、临时目录或当前工作目录，并继续校验扩展名。
- Tauri CSP 不允许设置为 `null`；不要放松 `gui/tauri.conf.json` 的安全配置。
- 所有 `unsafe` 块必须有 `// SAFETY:` 注释。PATH 写入前检查 null 字节和 32767 字符上限。
- 所有 `pub fn` 必须有 `///` 文档注释；`pub(crate)` 同样补注释（CONTRIBUTING.md 硬性要求）。
- 环境变量输入在 core 侧统一校验；CLI/GUI 不做二次判定，但 `backend.ts` 仍要拒绝来路不明的 `EnvVarMeta`（含 `value` 字段者一律拒绝，白名单构造字段）。
- E2E 使用 mock IPC，**不得写真实注册表**。真实 Tauri/注册表闭环测试必须显式授权，并记录备份、操作前后快照、重启结果和回滚结果（样本见 `docs/审核和开发/2026.09.18/`）。
- CLI 退出码：0 成功、1 一般错误、2 clap 参数解析失败、3 revision 冲突（仅 `env set`/`env remove` 且使用 `--revision` 时；`--force` 不会因并发冲突产生退出码 3；PATH 命令恒为 1）。冲突判定按 `CoreError.code`（`CoreError::exit_code()`），`[E_CONFLICT]` 前缀仅为过渡期展示文本。

## 测试与质量门

- TypeScript 使用 Vitest + jsdom，测试放在 `tests/unit/*.test.ts` 或 `*.test.tsx`；行为变化必须有回归测试。跑单个文件：`npx vitest run tests/unit/<file>.test.ts`；按名字过滤：`npx vitest run -t "<name>"`。
- Playwright 测试放在 `e2e/tests/*.spec.ts`，运行 `npm run test:e2e`。它验证前端流程，不等同于真实 Tauri 集成验证。
- Rust 测试放在模块内或 workspace test target，运行 `cargo test --workspace`。CLI 单测需 `--bins`（见上），跑单个测试：`cargo test -p <crate> <name>`。
- 覆盖率门槛为 80% 行覆盖；`npm run verify` 依次执行 Prettier、ESLint、构建、覆盖率、`cargo fmt`、Clippy 和 Rust 测试。提交前优先运行 `npm run verify:all`。
- 代码风格：UTF-8、CRLF；TS 2 空格，Rust/TOML 4 空格；Prettier 使用单引号、尾逗号和 100 列。
- 从仓库根目录跑 `npm test` 时，`vitest.config.ts` 的 `exclude` 必须包含 `.claude/**`——否则会扫到 `.claude/worktrees/` 下嵌套 worktree 的 e2e 文件并大面积假失败。

## 版本号升级清单

当前版本为 `5.1.3`。升级时至少检查：

| 文件                  | 字段                          |
| --------------------- | ----------------------------- |
| `package.json`        | `version`                     |
| `Cargo.toml`          | `[workspace.package] version` |
| `gui/tauri.conf.json` | `version`、窗口 `title`       |
| `README.md`           | 版本徽章和安装包说明          |

其他位置通常从 `package.json` 或 `env!("CARGO_PKG_VERSION")` 动态读取，不要手工制造第二套版本源。

## 发布流程

**发布由 CI 自动完成，不要在本地手工构建或创建 Release。** `.github/workflows/release.yml` 在推送 `v*` tag 时触发，自动执行：校验版本号一致性 → `npx tauri build` → `cargo build --release -p patheditor-cli` → 整理产物 → 生成发布日志 → 创建 GitHub Release。

### 唯一正确的发布步骤

```powershell
# 1. 确认版本号四处一致（package.json / Cargo.toml / gui/tauri.conf.json / README）
# 2. 在 CHANGELOG.md 顶部写入当前版本的段落（见下）
# 3. 提交并推送 main
git push origin main

# 4. 打 annotated tag 并推送 —— 这一步触发 CI
git tag -a v5.1.3 -m "PathEditor v5.1.3" -m "- 变更要点..."
git push origin v5.1.3

# 5. 等 CI 跑完，用 gh 确认结果
gh run list --limit 3
gh release view v5.1.3
```

**推送 tag 后不要做任何事，等 CI 完成。** 本地构建、手动创建 Release 都是重复劳动，且会与 CI 冲突。

### 禁止事项（2026-09-18 事故教训）

| 禁止                                                              | 原因                                                                                                                                         |
| ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| **推送 tag 后用 `gh release create` 手动建 Release**              | 会与 CI 抢同一个 Release。CI 走到最后一步会以 `a release with the same tag name already exists` 失败，留下红色的 CI 历史，且无法通过重跑消除 |
| **推送 tag 前手动跑 `npx tauri build` / `cargo build --release`** | CI 会做同样的事。本地构建只用于**验证能否构建通过**，产物不要用于发布                                                                        |
| **在没读 `.github/workflows/` 的情况下规划发布**                  | 仓库有 tag 触发的自动发布。不了解它就会重复构建、或与它冲突                                                                                  |

**若确实需要手动发布**（CI 不可用时）：先确认该 tag 的 Release 不存在，再执行 `gh release create`，然后**不要**推送 tag（或推 tag 后接受 CI 会跳过）。两者只能选其一。

### CHANGELOG.md 是发布日志的来源

`release.yml` 的「生成发布日志」步骤**优先**用正则从 `CHANGELOG.md` 抽取当前版本段落：

```powershell
$pattern = "(?ms)^##\s+v?$([regex]::Escape($version))(?:\s|\(|$).*?(?=^##\s+|\z)"
```

匹配 `## 5.1.3` 或 `## 5.1.3 (2026-09-18)` 开头的段落。**若找不到，会回退到 `git log <上一 tag>..<本 tag>` 逐条列 commit 标题**——日志质量明显下降。

所以发版前**必须在 CHANGELOG.md 顶部写好当前版本段落**，小节标题沿用最近几版的中文风格（`### 新增` / `### 变更` / `### 修复` / `### 说明`）。

### CI 各步骤的前置依赖

| 步骤         | 依赖                                                                                                    |
| ------------ | ------------------------------------------------------------------------------------------------------- |
| 校验项目版本 | `package.json`、`gui/tauri.conf.json`、`Cargo.toml` 三处版本必须与 tag 一致，否则整条流水线失败         |
| Tauri Build  | 需要 `npm ci` 与 Node 20                                                                                |
| 整理发布产物 | 需要 `target\release\bundle\nsis\PathEditor_<VERSION>_x64-setup.exe` 与 `target\release\patheditor.exe` |
| 生成发布日志 | 读 `CHANGELOG.md`（缺失则回退 git log）                                                                 |

工作流使用 MSVC 工具链（覆盖 `rust-toolchain.toml` 的 GNU 设置），因为 GitHub Windows runner 上 MSVC 更稳定。这是刻意的，不要"修正"它。

### 产物命名

| 文件                                 | 说明                                                                                           |
| ------------------------------------ | ---------------------------------------------------------------------------------------------- |
| `PathEditor_<VERSION>_x64-setup.exe` | NSIS 安装包（GUI），产物目录中原始名                                                           |
| `patheditor-cli_<VERSION>_x64.exe`   | CLI 二进制，CI 在整理产物时重命名——**本地产物名为 `patheditor.exe`，不要据此判断发布包的名称** |

### Release 已存在时的行为

工作流有两道检查：开头的「检查 Release 是否已存在」（`exists` 输出）与末尾「创建 GitHub Release」步骤内的幂等再确认。已存在时后续步骤全部跳过、CI 正常变绿。

但**开头检查与末尾创建之间隔着数分钟的构建**，期间状态可能变化——这正是 2026-09-18 事故的成因。末尾的二次确认是为此加的防护。

## 提交与协作

使用 Conventional Commits：`<type>: <description>`，允许 `feat`、`fix`、`refactor`、`docs`、`test`、`chore`、`perf`、`ci`、`style`、`revert`。Husky + lint-staged 会在提交前运行 Prettier/ESLint；不要绕过失败的质量门。

审查或开发大改动前先看 `docs/审核和开发/` 的历史记录，说明风险、验证证据和未覆盖项。特性开发先写 `docs/superpowers/specs/` 下的设计文档、再写 `docs/superpowers/plans/` 下的实施计划，然后按任务逐步实施。未经明确要求不要提交、推送、升级版本或执行真实注册表写入。
