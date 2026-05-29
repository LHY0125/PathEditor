<p align="center">
  <h1>PathEditor</h1>
  <p>Windows 系统环境变量 (PATH) 编辑器</p>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-5.0.0-blue" alt="version">
  <img src="https://img.shields.io/badge/tauri-2.x-ffa03a" alt="tauri">
  <img src="https://img.shields.io/badge/react-19-61dafb" alt="react">
  <img src="https://img.shields.io/badge/rust-1.95-000000" alt="rust">
  <img src="https://img.shields.io/badge/typescript-strict-blue" alt="typescript">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="license">
  <img src="https://img.shields.io/badge/tests-72%20passed-brightgreen" alt="tests">
  <img src="https://github.com/LHY0125/PathEditor/actions/workflows/ci.yml/badge.svg" alt="CI">
</p>

---

## 简介

PathEditor 是 Windows PATH 环境变量的可视化管理工具。支持系统变量和用户变量的增删改查、拖拽排序、一键清理无效路径、导入导出以及完整的撤销/重做。

v5.0 使用 **Tauri 2.x + React 19 + TypeScript + Rust** 完全重写，替代了原有的 C + IUP GUI。

## 架构

```mermaid
graph TB
    subgraph 前端["React 前端"]
        UI[UI 组件层<br/>AppShell / PathTable / Dialogs]
        Store[状态管理<br/>Zustand Store]
        Core[纯逻辑层<br/>undo-redo / path-manager / validation]
        UI --> Store
        UI --> Core
        Store --> Core
    end

    subgraph CLI["CLI 命令行"]
        Clap[clap 参数解析<br/>18 条命令]
        Atomic[原子性保护<br/>verify_and_save]
    end

    subgraph IPC["Tauri IPC 桥接"]
        invoke[invoke / plugin-dialog]
    end

    subgraph 后端["Rust core 库"]
        Registry[注册表读写<br/>HKLM / HKCU]
        System[系统操作<br/>权限检测 / 路径验证 / 环境变量展开]
        Files[文件操作<br/>备份 / 配置 / 导入导出]
        Scanner[分析引擎<br/>冲突检测 / 工具清单]
        Profiles[配置管理<br/>save/load/apply/rename]
    end

    subgraph Windows["Windows 系统"]
        Reg[(注册表<br/>SYSTEM / USER PATH)]
        FS[(文件系统<br/>目录验证 / exe 扫描)]
    end

    UI --> invoke
    invoke --> Registry
    invoke --> System
    invoke --> Files
    invoke --> Scanner
    invoke --> Profiles
    Clap --> Atomic
    Atomic --> Registry
    Atomic --> System
    Atomic --> Files
    Atomic --> Scanner
    Atomic --> Profiles
    Registry --> Reg
    System --> FS
    Scanner --> FS
    Files --> FS
    Profiles --> FS
```

### 组件树

```mermaid
graph TD
    App["App.tsx<br/>ErrorBoundary"]
    Shell["AppShell<br/>布局编排 + 弹窗管理"]
    TitleBar["TitleBar<br/>拖拽区域"]
    ToolBar["ToolBar<br/>搜索 / 操作 / 分析 / 配置"]
    PathTable["PathTable<br/>路径列表 + 验证 + 复选框"]
    MergePreview["MergePreview<br/>系统+用户合并视图"]
    StatusBar["StatusBar<br/>状态 / 权限 / 重试"]
    Dialogs["弹窗层<br/>PathEdit / Import / Help / Analyze / Profile"]

    App --> Shell
    Shell --> TitleBar
    Shell --> ToolBar
    Shell --> PathTable
    Shell --> MergePreview
    Shell --> StatusBar
    Shell --> Dialogs
```

### 操作流程

```mermaid
sequenceDiagram
    actor U as 用户
    participant UI as React UI
    participant Z as Zustand Store
    participant IPC as Tauri IPC
    participant R as Rust 后端
    participant Win as Windows

    U->>UI: 点击「保存」
    UI->>Z: savePaths()
    Z->>IPC: invoke('backup_registry')
    IPC->>R: backup_registry()
    R->>Win: 读取注册表 → 写入备份文件
    Z->>IPC: Promise.allSettled([save_system, save_user])
    IPC->>R: save_system_paths() / save_user_paths()
    R->>Win: RegSetValueEx()
    Z->>IPC: invoke('broadcast_env_change')
    IPC->>R: SendMessageTimeout(WM_SETTINGCHANGE)
    R->>Win: 通知所有进程
    Z->>UI: isModified → false, statusMessage → '保存成功'
```

### CLI 操作流程

```mermaid
sequenceDiagram
    actor U as 用户
    participant CLI as patheditor
    participant Core as Rust core 库
    participant Win as Windows

    U->>CLI: patheditor add "D:\Tools" --system
    CLI->>Core: load_system_paths() → 旧列表
    CLI->>CLI: 执行操作 (push / splice / clean)
    CLI->>Core: load_system_paths() → 重新读取
    alt 注册表未修改
        CLI->>Core: save_system_paths(new_list)
        Core->>Win: RegSetValueEx()
        CLI->>Core: broadcast_env_change()
        Core->>Win: SendMessageTimeout(WM_SETTINGCHANGE)
        CLI-->>U: 已添加到系统 PATH
    else 注册表已被其他进程修改
        CLI-->>U: 错误: 注册表已被其他进程修改
    end
```

## CLI 命令行

```bash
# 安装
cargo install --path cli

# 安装后可直接使用:
patheditor --help

# 查看 PATH
patheditor list --system --json

# 冲突检测
patheditor conflicts

# 配置切换
patheditor profile save "Python开发"
patheditor profile apply "Python开发"
```

完整 18 条命令：`patheditor --help`

## 功能

### 路径管理

- 查看和编辑 **系统 PATH**（HKLM）和 **用户 PATH**（HKCU）
- 新建、编辑、删除、上移、下移路径条目
- 多选批量删除
- 实时搜索过滤
- 合并预览（系统 + 用户路径并列显示）
- 文件夹拖拽添加

### 路径验证

- **红色**标记：路径在文件系统中不存在
- **橙色**标记：路径在列表中重复出现
- 环境变量路径（含 `%VAR%`）悬浮展开预览

### 撤销/重做

- 支持 9 种操作类型，最多 50 步历史
- 新增、删除、编辑、移动、清理、清空、导入均可撤销

### 导入/导出

- **JSON**：结构化导出，含版本和时间戳
- **CSV**：UTF-8 BOM 编码，兼容 Excel
- **TXT**：纯文本，每行一个路径

### 安全

- 保存前自动备份注册表到 `%APPDATA%/PathEditor/backups/`
- PATH 长度检查（Windows 单变量上限 32767 字符）
- 非管理员自动进入**只读模式**
- 保存中途失败精确提示哪个注册表 hive 出错

### 界面

- 深色模式 / 浅色模式
- 中文 / English 界面切换
- 全局键盘快捷键
- 修改状态指示（黄点）+ 未保存退出确认

## 安装

从 [Releases](https://github.com/LHY0125/PathEditor/releases) 下载最新版 `PathEditor_5.0.0_x64-setup.exe` 安装。

或从源码构建：

```bash
# 安装依赖
npm install

# 构建安装包
npx tauri build
```

> **要求**：Windows 10+（自带 WebView2），管理员权限才能编辑系统 PATH。

## 开发

```bash
# 开发模式 GUI（热更新）
npx tauri dev

# 仅前端
npm run dev

# 前端测试
npm test

# Rust workspace 检查
cargo check

# CLI 构建
cargo build --release -p patheditor-cli

# 完整构建
npx tauri build
```

### 技术栈

| 层 | 技术 |
|---|---|
| 前端框架 | React 19 + TypeScript (strict) |
| UI 样式 | Tailwind CSS 4 |
| 状态管理 | Zustand |
| 国际化 | i18next |
| 桌面框架 | Tauri 2.x |
| 核心库 | Rust workspace (core + gui + cli) |
| 前端测试 | Vitest (72 个测试) |
| Rust 测试 | cargo test (10 个测试) |
| 构建 | Vite + Cargo |
| 打包 | NSIS |

### 项目结构

```
core/                         # Rust 核心库（零 Tauri 依赖）
├── registry.rs               # 注册表读写 + 路径清理
├── system.rs                 # 权限检测、路径验证、环境变量展开
├── scanner.rs                # 冲突检测、工具清单
├── profiles.rs               # 配置文件管理
├── backup.rs / disabled.rs   # 备份、禁用状态
└── fs.rs                     # 文件读写、导入导出解析
gui/                          # Tauri 桌面应用
└── src/commands/             # 薄包装 → 调用 core
cli/                          # 命令行工具
└── src/main.rs               # 18 条命令
src/                          # React 前端
├── core/                     # 纯逻辑 — 零框架依赖
├── store/                    # Zustand 状态管理
├── components/               # UI 组件
├── hooks/                    # useAppActions、useKeyboard
├── i18n/                     # zh-CN / en
└── config/                   # default.json
tests/unit/                   # 前端单元测试
```

## 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+N` | 新建路径 |
| `Ctrl+S` | 保存 |
| `Ctrl+Z` | 撤销 |
| `Ctrl+Y` | 重做 |
| `Ctrl+F` | 搜索 |
| `Delete` | 删除选中 |
| `F1` | 帮助 |

## 贡献

欢迎提交 Issue 和 Pull Request。在开始大改动前，建议先开 Issue 讨论。

### 本地开发环境

- Node.js 22+
- Rust 1.95+ (stable-x86_64-pc-windows-gnu)
- MinGW-w64 (GCC 15.x 需配置 `-lmcfgthread` 链接标志)

### 代码规范

- TypeScript `strict: true`，零编译错误
- 所有 Rust `unsafe` 块必须有 `// SAFETY:` 注释
- 前端核心逻辑在 `src/core/`，纯函数，零依赖，可独立测试

## 许可证

MIT License

## 作者

[刘航宇](https://github.com/LHY0125) — 河南理工大学人工智能协会
