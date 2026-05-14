# Path Editor — Windows 系统环境变量编辑器

一个轻量级的 Windows 系统环境变量（PATH）编辑器，使用 C 语言和 [IUP](https://www.tecgraf.puc-rio.br/iup/) 图形库开发。

## 项目结构

```
PathEditor/
├── src/
│   ├── main.c          # 程序入口，构建 UI 布局
│   ├── callbacks.c      # 按钮回调 + 自定义输入对话框
│   ├── registry.c       # 注册表读写操作
│   └── utils.c          # 编码转换 + 权限检测 + 斑马纹样式
├── include/
│   ├── globals.h        # 全局控件句柄与常量
│   ├── callbacks.h
│   ├── registry.h
│   └── utils.h
├── libs/                # IUP 3.31 库文件（预编译）
├── ico/                 # 应用图标与资源文件
├── dist/
│   └── installer.iss    # Inno Setup 安装包脚本
├── ManagePath.bat       # 备用的命令行 PATH 管理脚本
├── CMakeLists.txt       # CMake 构建配置
├── Makefile             # GNU Make 构建配置（备用）
└── CLAUDE.md            # Claude Code 项目指南
```

## 功能特点

- **可视化编辑** — 以列表形式直观查看和管理系统 PATH 变量
- **增删改查** — 新建、编辑、删除条目，支持从文件管理器直接选择目录
- **拖拽排序** — 上移/下移按钮调整路径优先级
- **权限检测** — 非管理员模式自动切换为只读，防止误操作
- **自定义输入框** — 支持超长路径的编辑（80 字符可见宽度）
- **斑马纹列表** — 交替行背景色，方便阅读
- **双击编辑** — 双击列表项直接编辑对应路径
- **即时广播** — 保存后通过 `WM_SETTINGCHANGE` 通知系统

## 工作原理

程序直接读写 Windows 注册表键：

```
HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment\Path
```

PATH 值使用 `REG_EXPAND_SZ` 类型，支持 `%SystemRoot%` 等环境变量展开。

**编码转换**：IUP 控件使用 UTF-8，Windows 注册表 API 使用 UTF-16，`utils.c` 提供 `wide_to_utf8()` 和 `utf8_to_wide()` 完成双向转换。

## 下载与安装

从 [Releases](https://github.com/LHY0125/PathEditor/releases) 页面下载 `PathEditorSetup.exe` 安装包。

> **注意：** 安装后必须以管理员身份运行，否则只能查看，无法保存更改。

## 从源码构建

### 环境要求

| 工具 | 说明 |
|------|------|
| Windows 操作系统 | 需 Windows API 支持 |
| MinGW-w64 (GCC) | 编译器，路径 `D:\settings\Language\C\mingw64` |
| CMake 3.15+ | 构建工具 |
| IUP 3.31 | GUI 库（已包含在 `libs/` 中） |

### 编译

```bash
git clone https://github.com/LHY0125/PathEditor.git
cd PathEditor

# 配置（MinGW Makefiles 生成器）
cmake -B build -G "MinGW Makefiles"

# 编译
cmake --build build
```

编译产出在 `bin/PathEditor.exe`。

**其他 CMake 选项：**

```bash
# Debug 模式（带调试符号）
cmake -B build -G "MinGW Makefiles" -DCMAKE_BUILD_TYPE=Debug
cmake --build build

# Release 优化
cmake -B build -G "MinGW Makefiles" -DCMAKE_BUILD_TYPE=Release
cmake --build build
```

### 打包安装程序

使用 [Inno Setup 6](https://jrsoftware.org/isdl.php) 生成 `.exe` 安装包：

1. 确保已安装 Inno Setup 6
2. 先编译项目生成 `bin/PathEditor.exe` 及所需 DLL
3. 用 Inno Setup 编译 `dist/installer.iss`
4. 安装包输出至 `dist/dist/PathEditorSetup.exe`

## 使用说明

1. **启动** — 右键程序图标，选择「以管理员身份运行」
2. **查看** — 程序启动后自动加载当前系统 PATH 到列表
3. **新建** — 点击「新建」按钮，输入路径后确认，新条目追加到列表末尾
4. **编辑** — 选中条目后点击「编辑」，或直接双击条目
5. **浏览** — 点击「浏览」打开文件夹选择器，选中目录后自动添加
6. **删除** — 选中条目后点击「删除」
7. **排序** — 选中条目后点击「上移」或「下移」调整顺序
8. **保存** — 操作完成后点击「确定」写入注册表并广播变更
9. **生效** — CMD / PowerShell 窗口需要重新打开才能识别新变量；部分程序可能需要重启

**命令行备选方案：** 项目附带 `ManagePath.bat` 脚本，提供导出、导入和备份 PATH 的功能，无需 GUI 也可操作。

## 许可证

本项目基于 [MIT License](LICENSE) 开源。

Copyright (c) 2026 LHY
