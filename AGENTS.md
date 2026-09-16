# PathEditor 开发指南

> 本文件是开发窗口的仓库说明；更新后同步复制为根目录 `AGENTS.md`，两份文件应保持一致。

## 项目概述

PathEditor v5.1.2 是 Windows 系统环境变量（PATH）编辑器，采用 Tauri 2.x + React 19 + TypeScript strict + Rust workspace，提供 GUI 和 CLI 两种入口。

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
cargo test --workspace
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
│   ├── path_entry.rs            # PathEntry / PathSnapshot 契约
│   ├── scanner.rs               # scan_paths 单次枚举、冲突和工具清单
│   ├── profiles.rs              # 配置保存/加载/重命名/删除
│   ├── fs.rs                    # 受限文件读取、导入导出
│   └── backup.rs                # 注册表备份
├── gui/src/commands/            # #[tauri::command] 薄包装，调用 core
├── cli/src/                     # Clap 定义 + 命令分派
│   ├── runtime.rs               # 注册表安全保存、错误退出、快照提交
│   ├── import_export.rs         # 导入导出命令
│   ├── profile_ops.rs           # profile 子命令
│   └── scan_ops.rs              # 冲突、扫描、权限检查
├── src/                         # React 前端
│   ├── core/                    # 纯逻辑，零 React/Tauri 依赖
│   ├── services/backend.ts      # 唯一 IPC 适配入口 + 运行时形状校验
│   ├── services/path-session.ts # 加载、保存计划、部分成功、禁用快照
│   ├── store/                   # Zustand 状态、CRUD、撤销重做、action 编排
│   ├── components/              # layout / path-list / toolbar / env-list / dialogs / ui
│   ├── hooks/                   # useAppActions、useKeyboard、usePathValidation
│   └── i18n/                    # zh-CN / en
├── tests/unit/                  # Vitest 测试
├── tests/fixtures/              # 跨 Rust/TS 契约夹具
├── e2e/tests/                   # Playwright 测试（mock IPC）
└── docs/审核和开发/YYYY.MM.DD/  # 审核与开发记录
```

关键约束：

- `src/core/` 保持纯函数和零框架依赖；组件和 Store 不得直接调用 `invoke`，统一走 `src/services/backend.ts`。
- `gui` 和 `cli` 只做参数转换、命令分派和错误呈现，业务规则放在 `core`。
- `PathCapabilities` 使用 camelCase 序列化；共享契约见 `tests/fixtures/path-capabilities.json`。
- 正式 PATH 解析以 Rust `registry::split_path` 为准；TS 的 `split_path` 仅保留为兼容/测试夹具。
- `PathEntry { path, enabled }` 是跨层契约；导入、配置、注册表和撤销重做都必须保留 `enabled`。
- `.codegraph/` 存在时，优先使用 CodeGraph 理解符号和调用路径，再决定是否读取文件。

## Tauri IPC 接口

| Command                                                                                 | 参数 / 返回值                              | 说明                                   |
| --------------------------------------------------------------------------------------- | ------------------------------------------ | -------------------------------------- |
| `load_system_paths` / `load_user_paths`                                                 | `() -> Result<Vec<String>, String>`        | 读取注册表原始路径                     |
| `save_system_paths` / `save_user_paths`                                                 | `(paths, original?) -> Result<(), String>` | 可选乐观并发校验；外部修改时拒绝覆盖   |
| `clean_path_entries`                                                                    | `Vec<PathEntry> -> (kept, removed)`        | 统一清理语义，展开变量后检查目录       |
| `get_path_capabilities`                                                                 | `() -> PathCapabilities`                   | 按 hive 返回读写能力                   |
| `check_admin` / `validate_path` / `expand_env_vars` / `broadcast_env_change`            | 系统工具接口                               | 权限、目录验证、变量展开、环境通知     |
| `load_path_snapshot`                                                                    | `() -> Result<PathSnapshot, String>`       | 合并注册表、禁用状态和有序快照         |
| `save_path_snapshot`                                                                    | `(system?, user?) -> Result<(), String>`   | 保存完整有序快照，`None` 保留对应 hive |
| `load_disabled_state` / `save_disabled_state`                                           | 旧版字符串接口                             | 仅用于兼容                             |
| `import_file` / `export_path_entries`                                                   | 文件路径 / `PathEntry` + 格式              | Rust 统一导入导出                      |
| `scan_paths`                                                                            | `(paths, query) -> ScanResult`             | 一次枚举生成冲突和工具清单             |
| `scan_conflicts` / `scan_tools`                                                         | 兼容包装                                   | 分别返回冲突或工具清单                 |
| `backup_registry` / `get_appdata_dir`                                                   | 备份和目录查询                             | 备份当前注册表值                       |
| `list_profiles` / `save_profile` / `load_profile` / `delete_profile` / `rename_profile` | 配置 CRUD                                  | 配置保存 `PathEntry[]`                 |

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
```

`remove`、`edit`、`move-up`、`move-down` 默认操作用户 PATH，传入 `--system` 才操作系统 PATH。CLI 的 `list`、`import/export`、`profile`、`enable/disable` 都使用完整快照，避免丢失 `enabled=false` 条目和顺序。

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
- E2E 使用 mock IPC，**不得写真实注册表**。真实 Tauri/注册表闭环测试必须显式授权，并记录备份、操作前后快照、重启结果和回滚结果。

## 测试与质量门

- TypeScript 使用 Vitest + jsdom，测试放在 `tests/unit/*.test.ts` 或 `*.test.tsx`；行为变化必须有回归测试。
- Playwright 测试放在 `e2e/tests/*.spec.ts`，运行 `npm run test:e2e`。它验证前端流程，不等同于真实 Tauri 集成验证。
- Rust 测试放在模块内或 workspace test target，运行 `cargo test --workspace`。
- 覆盖率门槛为 80% 行覆盖；`npm run verify` 依次执行 Prettier、ESLint、构建、覆盖率、`cargo fmt`、Clippy 和 Rust 测试。提交前优先运行 `npm run verify:all`。
- 代码风格：UTF-8、CRLF；TS 2 空格，Rust/TOML 4 空格；Prettier 使用单引号、尾逗号和 100 列。

## 版本号升级清单

当前版本为 `5.1.2`。升级时至少检查：

| 文件                  | 字段                          |
| --------------------- | ----------------------------- |
| `package.json`        | `version`                     |
| `Cargo.toml`          | `[workspace.package] version` |
| `gui/tauri.conf.json` | `version`、窗口 `title`       |
| `README.md`           | 版本徽章和安装包说明          |

其他位置通常从 `package.json` 或 `env!("CARGO_PKG_VERSION")` 动态读取，不要手工制造第二套版本源。

## 提交与协作

使用 Conventional Commits：`<type>: <description>`，允许 `feat`、`fix`、`refactor`、`docs`、`test`、`chore`、`perf`、`ci`、`style`、`revert`。Husky + lint-staged 会在提交前运行 Prettier/ESLint；不要绕过失败的质量门。

审查或开发大改动前先看 `docs/审核和开发/` 的历史记录，说明风险、验证证据和未覆盖项。未经明确要求不要提交、推送、升级版本或执行真实注册表写入。
