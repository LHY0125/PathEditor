# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

Path Editor 是一个 Windows 系统环境变量 PATH 编辑器，使用 C 语言和 IUP 图形库开发。通过读写注册表 `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment\Path` 来管理系统 PATH。

## 构建与运行

```bash
# 配置（需要 MinGW-w64 和 CMake 3.15+ 在 PATH 中）
cmake -B build -G "MinGW Makefiles"

# 编译
cmake --build build

# 运行（必须以管理员身份）
bin/PathEditor.exe
```

编译器为 MinGW-w64 GCC。CMake 自动处理 `windres` 资源文件编译和 IUP DLL 复制。IUP 3.31 库已包含在 `libs/` 目录下，无需额外安装。

## 打包

使用 Inno Setup 6 生成安装包。先编译生成 exe，再用 Inno Setup 编译 `dist/installer.iss`。

## 架构

```
src/
  main.c       入口，构建 UI 布局（IupDialog + IupFlatList + 按钮）
  callbacks.c  所有按钮回调 + 自定义输入对话框
  registry.c   注册表读写：load_path() / save_path()
  utils.c      编码转换 + 管理员权限检测 + 斑马纹刷新
include/
  globals.h    全局 Ihandle* 指针和注册表路径常量
  callbacks.h  回调函数声明
  registry.h   注册表操作声明
  utils.h      工具函数声明
```

**数据流**: `load_path()`（注册表 → 列表） → 用户操作（按钮回调） → `save_path()`（列表 → 注册表 → `WM_SETTINGCHANGE` 广播）

**编码约定**: IUP 使用 UTF-8，Windows 注册表 API 使用 UTF-16 (wide char)。`utils.c` 提供 `wide_to_utf8()` 和 `utf8_to_wide()` 完成转换。

**全局控件**: 所有 IUP 控件句柄（`dlg`, `list_path`, `lbl_status`, 各按钮）在 `main.c` 定义并通过 `globals.h` 声明为 `extern`，回调函数和工具函数直接访问它们。

## 关键约束

- 必须以管理员身份运行才能保存更改（`check_admin()` 通过尝试以 `KEY_WRITE` 打开注册表来检测）
- IUP 的 `UTF8MODE` 必须在 `IupOpen()` 之前通过 `putenv("IUP_UTF8MODE=YES")` 设置
- `IupFlatList` 的数据操作（`APPEND` 等属性）必须在控件 Map 之后（`IupShowXY` 之后）才能生效
- PATH 使用 `REG_EXPAND_SZ` 类型以支持 `%SystemRoot%` 等变量展开
