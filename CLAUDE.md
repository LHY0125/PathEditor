# CLAUDE.md

## 项目概述

PathEditor v5.0 — Windows 系统环境变量 (PATH) 编辑器，Tauri 2.x + React 19 + TypeScript + Rust workspace 构建。GUI + CLI 双模式。

## 构建命令

```bash
# 安装前端依赖
npm install

# 开发模式（GUI 热更新）
npx tauri dev

# 仅前端（浏览器预览，无注册表功能）
npm run dev

# 前端测试
npm test
npm run test:watch

# Rust workspace 全部检查
cargo check

# 仅核心库检查
cargo check -p path-editor-core

# CLI 构建
cargo build --release -p patheditor-cli

# 生产构建（前端）
npm run build

# 完整构建（生成 NSIS 安装包）
npx tauri build
```

## 架构

Cargo workspace 三层，前后端分离，通过 Tauri IPC 通信。

```
PathEditor/
├── core/                       # Rust 库 crate（零 Tauri 依赖）
│   └── src/
│       ├── registry.rs         # 注册表读写 + clean_paths
│       ├── system.rs           # check_admin、validate_path、expand_env_vars、broadcast
│       ├── backup.rs           # backup_registry、get_appdata_dir
│       ├── disabled.rs         # save/load_disabled_state
│       ├── fs.rs               # read_text_file、import_paths、export_paths
│       ├── scanner.rs          # scan_conflicts、scan_tools
│       ├── profiles.rs         # list/save/load/delete/rename_profile
│       └── lib.rs
├── gui/                        # Tauri 桌面应用（依赖 core）
│   └── src/commands/           # 薄包装：#[tauri::command] → 调用 core
├── cli/                        # CLI 命令行（依赖 core + clap）
│   └── src/main.rs             # 18 条命令
├── src/                        # React 前端 (TypeScript strict 模式)
│   ├── core/                   # 纯逻辑 — 零 React/零 Tauri 依赖
│   ├── store/                  # Zustand 状态管理
│   ├── components/             # UI 组件
│   │   ├── layout/             # AppShell、TitleBar、StatusBar、ErrorBoundary
│   │   ├── path-list/          # PathTable、MergePreview
│   │   ├── toolbar/            # ToolBar、ActionButtons、UndoRedoButtons
│   │   ├── dialogs/            # PathEdit、Help、Import、Analyze、Profile
│   │   └── ui/                 # Modal、buttons
│   ├── hooks/                  # useAppActions、useKeyboard
│   ├── i18n/                   # zh-CN / en
│   └── config/                 # default.json
├── tests/unit/                 # Vitest 前端单元测试
├── e2e/                        # Playwright E2E 测试
└── Cargo.toml                  # Workspace 根 + [workspace.package]
```

## IPC 接口（Rust → Frontend）

| Command | 参数 | 返回值 | 功能 |
|---------|------|--------|------|
| `load_system_paths` | — | `Result<Vec<String>, String>` | 从 HKLM 读取系统 PATH |
| `load_user_paths` | — | `Result<Vec<String>, String>` | 从 HKCU 读取用户 PATH |
| `save_system_paths` | `paths: Vec<String>` | `Result<(), String>` | 保存系统 PATH（含 32767 字符上限） |
| `save_user_paths` | `paths: Vec<String>` | `Result<(), String>` | 保存用户 PATH |
| `check_admin` | — | `bool` | 检测管理员权限 |
| `validate_path` | `path: &str` | `bool` | 检查目录是否存在 |
| `expand_env_vars` | `path: &str` | `String` | 展开 `%VAR%` |
| `broadcast_env_change` | — | `()` | 广播 `WM_SETTINGCHANGE` |
| `backup_registry` | `custom_dir` | `Result<String, String>` | 备份注册表 |
| `get_appdata_dir` | — | `String` | 备份目录路径 |
| `save_disabled_state` | `system, user` | `Result<(), String>` | 持久化禁用状态 |
| `load_disabled_state` | — | `Result<(Vec, Vec), String>` | 加载禁用状态 |
| `read_text_file` | `path` | `Result<String, String>` | 读取文本文件 |
| `scan_conflicts` | `paths` | `Result<Vec<ConflictEntry>, String>` | 可执行文件冲突检测 |
| `scan_tools` | `paths, query` | `Result<Vec<ToolGroup>, String>` | 可执行文件清单 |
| `list_profiles` | — | `Result<Vec<ProfileMeta>, String>` | 列出配置 |
| `save_profile` | `name, sys, user` | `Result<(), String>` | 保存配置 |
| `load_profile` | `name` | `Result<ProfileData, String>` | 加载配置 |
| `delete_profile` | `name` | `Result<(), String>` | 删除配置 |
| `rename_profile` | `old, new` | `Result<(), String>` | 重命名配置 |

## CLI 命令参考

```
patheditor list          [--system|--user] [--json]
patheditor add           <PATH> [--system|--user]
patheditor remove        <INDEX> [--system|--user]
patheditor edit          <INDEX> <NEW> [--system|--user]
patheditor move-up       <INDEX> [--steps N] [--system|--user]
patheditor move-down     <INDEX> [--steps N] [--system|--user]
patheditor clean         [--system|--user] [--dry-run] [--json]
patheditor enable        <INDEX> [--system|--user]
patheditor disable       <INDEX> [--system|--user]
patheditor import        <FILE> [--target system|user|both]
patheditor export        [--format json|csv|txt] [--output <FILE>]
patheditor backup
patheditor conflicts     [--json]
patheditor scan          [--query <NAME>] [--json]
patheditor check-admin   [--json]
patheditor profile       {list|save|load|apply|delete|rename}
```

所有修改操作在保存前重新读取注册表验证，防止覆盖其他进程的修改。

## 错误处理

### 前端

| 场景 | 处理 |
|------|------|
| IPC 调用失败 | `Promise.allSettled` 精确报告哪个 hive 保存失败 |
| JSON 文件损坏 | `importFromJson` try/catch，返回空结果 |
| 保存并发双击 | `isSaving` 守卫，第二次调用直接 return |
| 备份创建失败 | `.catch()` 显示 warning_backup，保存继续 |
| 渲染异常 | ErrorBoundary 捕获 + console.error + 重试按钮 |

### Rust / CLI

- 注册表操作：全部返回 `Result<T, String>`，中文错误消息
- FFI 调用：失败时 `log::warn` + 返回安全回退值
- PATH 长度：写入前检查 32767 字符上限
- SAFETY 注释：所有 `unsafe` 块均有文档
- CLI 原子性：保存前重新读取注册表与加载值对比，不一致报错退出

## 关键约束

- **TypeScript**：`strict: true`，零编译错误
- **Rust 工具链**：`stable-x86_64-pc-windows-gnu`（项目已设 override）
- **MinGW 兼容**：`.cargo/config.toml` 添加 `-lmcfgthread`（GCC 15.2.0 运行时）
- **运行权限**：需要管理员权限才能编辑系统 PATH，非管理员自动进入只读模式
- **构建产物**：NSIS 安装包，约 8MB

## 版本号升级清单

版本号需在 **4 个地方** 手动修改：

| 文件 | 字段 | 说明 |
|------|------|------|
| `Cargo.toml` | `[workspace.package] version` | Rust 全量自动继承（3 crate + `env!("CARGO_PKG_VERSION")`） |
| `package.json` | `version` | 前端动态 import（TitleBar、import-export.ts 自动读取） |
| `gui/tauri.conf.json` | `version` | 打包版本号 |
| `gui/tauri.conf.json` | 窗口 `title` | 窗口标题栏显示 |
| `README.md` | badges | 文档徽章 |

其他位置均从上述两源头动态读取，无需单独修改:
- `core/src/fs.rs` / `cli/src/main.rs` → `env!("CARGO_PKG_VERSION")`
- `src/components/layout/TitleBar.tsx` → `import { version } from '../../../package.json'`
- `src/core/import-export.ts` → `import { version } from '../../package.json'`

Release 操作：
- `gh release create vX.Y.Z` 创建新 Release
- 安装包上传到新 Release，**不要**覆盖旧版本
