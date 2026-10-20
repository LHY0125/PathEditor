<p align="center">
  <h1>PathEditor</h1>
  <p>Windows 系统环境变量 (PATH) 编辑器</p>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-5.1.7-blue" alt="version">
  <img src="https://img.shields.io/badge/tauri-2.x-ffa03a" alt="tauri">
  <img src="https://img.shields.io/badge/react-19-61dafb" alt="react">
  <img src="https://img.shields.io/badge/rust-1.95-000000" alt="rust">
  <img src="https://img.shields.io/badge/typescript-strict-blue" alt="typescript">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="license">
  <img src="https://img.shields.io/badge/tests-582%20passed-brightgreen" alt="tests">
  <a href="https://codecov.io/gh/LHY0125/PathEditor"><img src="https://codecov.io/gh/LHY0125/PathEditor/branch/v5.1/graph/badge.svg" alt="coverage"></a>
  <img src="https://img.shields.io/badge/platform-Windows%2010%2B-0078D6" alt="platform">
</p>

---

## 截图

### 主界面

![主界面](docs/screenshots/main-window.png)

### 路径编辑

![路径编辑](docs/screenshots/path-edit.png)

### 冲突检测

![冲突检测](docs/screenshots/conflict-analysis.png)

### CLI 命令行

![CLI](docs/screenshots/cli-demo.png)

---

## 简介

PathEditor 是 Windows PATH 环境变量的可视化管理工具。支持系统变量和用户变量的增删改查、拖拽排序、一键清理无效路径、导入导出以及完整的撤销/重做。

v5.1 使用 **Tauri 2.x + React 19 + TypeScript + Rust**，替代了原有的 C + IUP GUI。

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
        Clap[clap 参数解析<br/>17 条顶层命令]
        Atomic[原子性保护<br/>verify_and_save]
    end

    subgraph IPC["Tauri IPC 桥接"]
        invoke[services/backend.ts<br/>统一 IPC 边界]
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
    Note over Z: 权限预检 + 组合长度软提醒（GUI 侧）
    Z->>IPC: invoke('save_path_with_sidecar', {system, user})
    Note over IPC,R: 唯一保存编排（core::service::save_path_with_sidecar）
    R->>Win: 写前备份（backup_registry）
    R->>Win: RegSetValueEx()（顺序 system → user，含 expected 并发校验）
    R->>Win: SendMessageTimeout(WM_SETTINGCHANGE)
    R->>Win: 提交 sidecar 快照（失败则落 pending，下次自动补写）
    R-->>Z: ApplyOutcome {system, user, sidecar}
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
    CLI->>Core: load_path_snapshot() → 当前条目 + 期望值
    CLI->>CLI: 执行操作 (push / splice / clean)
    CLI->>Core: save_path_with_sidecar(PathHiveWrite { entries, expected })
    Note over Core: 写前备份 + 并发校验 + 顺序写
    alt 注册表未修改（内容一致）
        Core->>Win: RegSetValueEx()
        Core->>Win: SendMessageTimeout(WM_SETTINGCHANGE)
        Core->>Win: 提交 sidecar 快照
        CLI-->>U: 已添加到系统 PATH
    else 注册表已被其他进程修改（成员变化）
        CLI-->>U: 错误: 注册表已被其他进程修改
    end
```

## CLI 命令行

### 安装

```powershell
scoop bucket add lhy https://github.com/LHY0125/scoop-bucket
scoop install lhy/patheditor-cli
```

或从源码安装：

```bash
cargo install --path cli
```

```bash
# 安装后可直接使用:
patheditor --help

# 查看 PATH
patheditor list --system --json

# 冲突检测
patheditor conflicts

# 配置切换
patheditor profile save "Python开发"
patheditor profile apply "Python开发"

# 通用环境变量（Path 除外）
patheditor env list     [--system|--user] [--json]
patheditor env get      <NAME> [--system]
patheditor env set      <NAME> [--value <V>|--stdin|--value-file <F>] (--revision <R>|--force) [--dry-run] [--json]
patheditor env add      <NAME> [<VALUE>] [--kind string|expand] [--system] [--dry-run] [--json]
patheditor env remove   <NAME> (--revision <R>|--force) [--dry-run] [--json]
patheditor env backup   [--json]
patheditor env backups  [--json]
patheditor env restore  <FILE> [--dry-run] [--force] [--json]
```

`remove`、`edit`、`move-up`、`move-down` 默认操作用户 PATH，传入 `--system` 才操作系统 PATH。CLI 的 `list`、`import/export`、`profile`、`enable/disable` 都使用完整快照，避免丢失 `enabled=false` 条目和顺序。

`env` 子命令管理通用环境变量（`Path` 除外，请用 PATH 专用命令）。默认操作用户 hive，加 `--system` 操作系统 hive。

`set` 与 `remove` 必须显式选择并发模式：`--revision <R>`（取自 `env list --json`，外部修改时拒绝写入，退出码 3）或 `--force`（跳过 revision 校验直接覆盖，最后写入者胜，仍受保护名单/类型/权限约束）。敏感值建议用 `--stdin` 或 `--value-file` 传入，避免明文进入 shell 历史与进程列表。

`set` / `add` / `remove` 支持 `--dry-run`：只显示将要发生的变更，**不写注册表、不产生备份、不广播**。预览不要求 `--revision` / `--force`；变量不存在时照常报错（与真正执行时的判定一致）。加 `--json` 可输出脚本友好的摘要（需与 `--dry-run` 同用，单独用会以退出码 2 被拒绝）：

```bash
patheditor env set JAVA_HOME --value "C:\Java\jdk-21" --dry-run --json
# {"operation":"set","hive":"user","name":"JAVA_HOME","oldValue":"C:\\Java\\jdk-17","newValue":"C:\\Java\\jdk-21","kind":"string","writable":true,"sensitive":false}
```

命中敏感规则的变量（如 `*_TOKEN` / `*_KEY`）在预览里不回传明文：`oldValue` / `newValue` 为 `null`、`sensitive` 为 `true`，文本预览显示 `(敏感)`。这与 `env list` 的打码一致 —— 预览不新开明文出口。

CLI 退出码约定：`0` 成功、`1` 一般错误、`3` revision 冲突（仅 `env set` / `env remove` 且使用 `--revision` 时；PATH 命令恒为 `1`）。`--force` 不携带 revision，不会因并发冲突产生退出码 3。

完整 17 条顶层命令（另有 `env` / `profile` 子命令组）：`patheditor --help`

### 环境变量备份与恢复

```bash
# 立即备份两个 hive 的全部环境变量
patheditor env backup

# 查看已有备份（按时间倒序；只列文件，不解析内容）
patheditor env backups

# 先看差异，不写注册表
patheditor env restore ~/.patheditor/backups/env_backup_20260921_200437_062.json --dry-run

# 执行恢复（默认模式：任一变量被外部改动则整批中止，退出码 3）
patheditor env restore ~/.patheditor/backups/env_backup_20260921_200437_062.json
```

此外，**每一次环境变量写入前都会自动备份**：CLI 的 `env set` / `env add` / `env remove` 与 GUI 的编辑 / 新建 / 删除都会先落一份备份，备份失败**不阻断写入**，只在 stderr 或状态栏提示。默认保留最近 20 份（可在 `~/.patheditor/config.ini` 用 `env_backup_keep = N` 覆盖），轮换只删除本工具生成的 `env_backup_*.json`。

> ⚠️ `~/.patheditor/backups/` 下的 `env_backup_*.json` 含环境变量的**明文值**，可能包括 API key、token 等敏感信息。请勿将该目录同步到云端或提交到版本库。

`env restore --dry-run` 的「修改 N」恒显示 0 —— 备份的 revision 与注册表不一致时一律判为「冲突」而非「修改」，两者在实现中是同一个条件。`--force` 下被冲突覆盖的变量会被改写却计入「冲突」，所以 dry-run 会**低报** force 模式的实际改动量。另需注意：**未提权时恢复恒失败**（无法以写权限打开系统 hive），请以管理员身份运行。

## 功能

### 路径管理

- 查看和编辑 **系统 PATH**（HKLM）和 **用户 PATH**（HKCU）
- 新建、编辑、删除、上移、下移路径条目
- 多选批量删除
- 实时搜索过滤
- 合并预览（系统 + 用户路径并列显示）
- 文件夹拖拽添加

### 全环境变量管理

- 查看和管理系统 / 用户环境变量项下的**所有变量**（不止 PATH）
- 显示变量的真实注册表类型（`REG_SZ` / `REG_EXPAND_SZ`），不支持的类型只读展示
- 敏感变量（名称含 TOKEN / KEY / SECRET / 密码 / API）默认打码，需显式点击才显示明文
- 系统内置关键变量（`windir`、`ComSpec`、`PATHEXT` 等）硬锁定为只读，防止改坏系统
- `Path` 仍由专用 PATH 视图管理，保证启用/禁用状态与顺序不被绕过

### 路径验证

- **红色**标记：路径在文件系统中不存在
- **橙色**标记：路径在列表中重复出现
- 环境变量路径（含 `%VAR%`）悬浮展开预览

### 撤销/重做

- 支持 9 种操作类型，最多 50 步历史
- 新增、删除、编辑、移动、清理、清空、导入均可撤销

### 历史记录（v6 M1）

- PATH 快照与操作事件持久化到 **SQLCipher 加密数据库**（`~/.patheditor/patheditor.db`）
- 「历史记录」标签页按时间倒序浏览事件
- **敏感变量的值默认掩码**（`••••`），掩码在 Rust 侧完成 —— 前端从不接收未要求的明文
- **数据库是 sidecar，注册表仍是唯一真相源**：DB 打不开时 GUI 照常工作，只把历史/备份降级，
  并在窗口顶部显示常驻降级条幅

> **M1 的已知限制**：事件列表可以浏览，但**点击事件暂时看不到快照详情** ——
> 事件与快照的关联（`snapshotUid`）要到 M2 才接通，当前按钮为禁用状态。
> 不要在 M2 之前把它当成「可预览完整快照」。

> **密钥与备份**：数据库密钥由 Windows DPAPI 保护，存放在 `~/.patheditor/keys/db.key.dpapi`，
> 文件权限限定为当前用户 + SYSTEM。
> **该密钥与当前 Windows 用户绑定**，换用户或换机器后历史无法解密；PathEditor 不会静默重建空库。
> 环境变量备份从本版起写 `.pebak`（DPAPI 加密），`env backups` 会带 `encrypted` 标志。
> 遗留的 `env_backup_*.json` 仍可一次性恢复，**读取时会在 stderr 打印明文警告**（GUI 记入日志）。
> **降级回落**：若密钥文件不可用（拿不到 DPAPI 密钥），备份会回落到明文 `.json` 并**在 stderr 打印警告**——
> 这是为了让备份功能在密钥故障时依然可用，但意味着此时磁盘上会新增明文敏感文件。
>
> **v6.0.0 尚未交付**：FTS5 全文检索、统计仪表盘、N 路对比、环境变量引用图与跨设备同步属于 M2/M3。

### 导入/导出

- **JSON**：结构化导出，含版本和时间戳
- **CSV**：UTF-8 BOM 编码，兼容 Excel
- **TXT**：纯文本，每行一个路径

### 安全

- 保存前自动备份注册表到 `~/.patheditor/backups/`（文件名含时间戳，如 `path_backup_20260921_200437_062.txt`）；同类备份默认保留最近 20 份，可在 `~/.patheditor/config.ini` 用 `path_backup_keep = N` 覆盖。轮换只删除本工具生成的 `path_backup_*.txt`，且**仅在默认备份目录生效**（用户指定目录不轮换）
- **CLI 的 `add` / `remove` / `edit` / `move-up` / `move-down` / `clean` / `enable` / `disable` / `import` / `profile apply` 每次成功写入 PATH 前也自动产生一份 `path_backup_*.txt`**（与 GUI 保存、与 `env` 侧写入前备份对称）
- 环境变量写入前自动备份到同一目录（`env_backup_*.json`），支持差异预览与一键恢复；**该文件含敏感值明文**，请勿同步到云端或提交到版本库
- PATH 长度检查（Windows 单变量上限 32767 字符）
- 非管理员仅系统 PATH 只读，用户 PATH 仍可编辑
- 保存中途失败精确提示哪个注册表 hive 出错
- 禁用路径从注册表移除后仍会以完整快照保留，重启后可恢复显示并重新启用
- 未保存修改时的退出确认使用 Tauri 异步对话框，绝不阻塞界面线程

### 界面

- 深色模式 / 浅色模式
- 中文 / English 界面切换
- 全局键盘快捷键
- 修改状态指示（黄点）+ 未保存退出确认

## 安装

> 本仓库不维护 scoop manifest；两个应用分别在 `LHY0125/scoop-bucket` 中发布。

### 图形界面（GUI）

**方式一：安装包**

从 [Releases](https://github.com/LHY0125/PathEditor/releases) 下载最新版 `PathEditor_5.1.7_x64-setup.exe` 安装。

**方式二：Scoop（免安装）**

```powershell
scoop bucket add lhy https://github.com/LHY0125/scoop-bucket
scoop install lhy/patheditor-gui
```

从 portable zip 解压即用（`PathEditor.exe` + `WebView2Loader.dll`），开始菜单生成 PathEditor 快捷方式。

**方式三：源码构建**

```bash
# 安装依赖
npm install

# 构建安装包
npx tauri build
```

### 命令行（CLI）

**方式一：Scoop**

```powershell
scoop bucket add lhy https://github.com/LHY0125/scoop-bucket
scoop install lhy/patheditor-cli
```

安装后即可在任意终端使用 `patheditor`（CLI 二进制在发布时重命名为 `patheditor-cli_<版本>_x64.exe`，manifest 用 `#/patheditor.exe` 装回原名）。

> **从 v5.1.3 升级**：v5.1.3 的发布流程存在产物覆盖缺陷——CLI 与 GUI 共用 Cargo target 目录，而 NTFS 大小写不敏感使 `patheditor.exe` 与 `PathEditor.exe` 实际是同一条目录项，CLI 链接产物覆盖了 GUI 本体，导致 `scoop install lhy/patheditor-cli` 装出的可执行文件是 GUI（且无法运行，报 `DLL_NOT_FOUND`）。
>
> 该缺陷已在 v5.1.4 修复（GUI 二进制改名 `PathEditor.exe`，CLI 改用独立 target 目录 `target/cli` 构建）。**若你在 v5.1.3 装过 CLI，执行 `scoop update patheditor-cli` 即可恢复。**

**方式二：从源码安装**

```bash
cargo install --path cli
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

| 层        | 技术                              |
| --------- | --------------------------------- |
| 前端框架  | React 19 + TypeScript (strict)    |
| UI 样式   | Tailwind CSS 4                    |
| 状态管理  | Zustand                           |
| 国际化    | i18next                           |
| 桌面框架  | Tauri 2.x                         |
| 核心库    | Rust workspace (core + gui + cli) |
| 前端测试  | Vitest + Playwright (305 + 30)    |
| Rust 测试 | cargo test (264 个测试)           |
| 构建      | Vite + Cargo                      |
| 打包      | NSIS                              |

### 项目结构

```
core/                         # Rust 核心库（零 Tauri 依赖）
├── error.rs                  # CoreError 结构化错误（code 驱动错误码与前端本地化）
├── service.rs                # 应用服务层（多 hive 事务编排 + 侧车快照）
├── persist.rs                # 持久化原语（schemaVersion / .bak 轮换 / 损坏隔离）
├── registry/                 # 注册表通路（path / access / env_var / conflict 子模块）
├── system.rs                 # 权限检测、路径验证、环境变量展开
├── scanner.rs                # 冲突检测、工具清单
├── profiles.rs               # 配置文件管理
├── backup.rs / disabled.rs   # 备份、禁用状态
└── fs.rs                     # 文件读写、导入导出解析
gui/                          # Tauri 桌面应用（bin 产物 PathEditor.exe）
└── src/commands/             # 薄包装 → 调用 core
cli/                          # 命令行工具（bin 产物 patheditor.exe）
├── src/main.rs               # Clap 定义、CRUD、分派
└── src/                      # runtime / import_export / profile_ops / scan_ops / env_ops
src/                          # React 前端
├── core/                     # 纯逻辑 — 零框架依赖
├── store/                    # Zustand 状态管理
├── services/                 # backend IPC + path session
├── components/               # UI 组件
├── hooks/                    # useAppActions、useKeyboard、usePathValidation
├── i18n/                     # zh-CN / en
└── config/                   # default.json
tests/unit/                   # 前端单元测试
docs/审核和开发/                # 审查与开发记录
```

## 快捷键

| 快捷键   | 功能     |
| -------- | -------- |
| `Ctrl+N` | 新建路径 |
| `Ctrl+S` | 保存     |
| `Ctrl+Z` | 撤销     |
| `Ctrl+Y` | 重做     |
| `Ctrl+F` | 搜索     |
| `Delete` | 删除选中 |
| `F1`     | 帮助     |

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
