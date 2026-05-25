# CLAUDE.md

## 项目概述

PathEditor v4.0 — Windows 系统环境变量 (PATH) 编辑器，使用 Tauri 2.x + React 19 + TypeScript + Rust 构建。

## 构建命令

```bash
# 安装前端依赖
npm install

# 开发模式（热更新）
npx tauri dev

# 仅前端
npm run dev

# 前端测试
npm test
npm run test:watch

# 构建生产版本
npm run build

# Rust 后端检查
cd src-tauri && cargo check

# Rust 后端测试
cd src-tauri && cargo test

# 完整构建（安装包）
npx tauri build
```

## 架构

前后端分离，通过 Tauri IPC 通信：

```
src/                      # React 前端 (TypeScript)
├── core/                 # 纯逻辑 — 零 React 依赖
│   ├── string-list.ts    # StringList 数据结构
│   ├── undo-redo.ts      # 撤销/重做管理器（8 种操作类型）
│   ├── path-manager.ts   # 路径增删移清理
│   ├── import-export.ts  # JSON/CSV/TXT 导入导出
│   └── validation.ts     # 路径格式验证
├── store/                # Zustand 状态管理
│   ├── app-store.ts      # 主状态（路径、撤销、CRUD、加载/保存）
│   └── theme-store.ts    # 深色/浅色模式
├── components/           # React UI 组件
│   ├── layout/           # AppShell、TitleBar、StatusBar
│   ├── path-list/        # PathTable、MergePreview
│   ├── toolbar/          # ToolBar、ActionButtons、UndoRedoButtons、SearchInput
│   └── dialogs/          # PathEditDialog、HelpDialog、ImportDialog
├── hooks/                # use-keyboard、use-path-validation
├── i18n/                 # i18next 中英文翻译
└── config/               # default.json UI 参数配置

src-tauri/                # Tauri Rust 后端
├── src/
│   ├── commands/
│   │   ├── registry.rs   # 注册表读写（load/save system & user paths）
│   │   ├── system.rs     # check_admin、validate_path、expand_env_vars、broadcast
│   │   └── backup.rs     # backup_registry、get_appdata_dir
│   ├── error.rs
│   └── lib.rs            # 注册所有 IPC commands
├── Cargo.toml
└── tauri.conf.json

tests/unit/               # Vitest 前端单元测试
```

## IPC 接口（Rust → Frontend）

| Command | 功能 |
|---------|------|
| `load_system_paths` | 从 HKLM 注册表读取系统 PATH |
| `load_user_paths` | 从 HKCU 注册表读取用户 PATH |
| `save_system_paths` | 保存系统 PATH 到注册表 |
| `save_user_paths` | 保存用户 PATH 到注册表 |
| `check_admin` | 检测管理员权限 |
| `validate_path` | 验证路径目录是否存在 |
| `expand_env_vars` | 展开 %VAR% 环境变量 |
| `broadcast_env_change` | 广播 WM_SETTINGCHANGE |
| `backup_registry` | 备份注册表 PATH 到文件 |
| `get_appdata_dir` | 获取备份目录路径 |

## 关键约束

- Rust 工具链：`stable-x86_64-pc-windows-gnu`（项目已设 override）
- `.cargo/config.toml` 添加了 `-lmcfgthread` 兼容 GCC 15.2.0 MinGW
- 移除 `cdylib` crate-type 避免 DLL 导出序数溢出
- 运行需要管理员权限才能编辑系统 PATH
- `cargo test` 需要 MinGW bin 在 PATH 中（GCC 15.2.0 运行时依赖 `libmcfgthread-2.dll`），开发模式下可用 `npx tauri dev` 替代
