# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

Path Editor 是一个 Windows 系统环境变量（PATH）管理工具，使用 C 语言和 IUP 图形库开发。支持系统变量和用户变量的双视图编辑、智能路径检测、自动备份、JSON 导入导出等功能。

## 构建与运行

```bash
# 配置（需要 MinGW-w64 和 CMake 在 PATH 中）
cmake -B build -G "MinGW Makefiles"

# 编译
cmake --build build

# 打包
build_installer.bat

# 以管理员身份运行
powershell -Command "Start-Process 'build\\PathEditor.exe' -Verb RunAs"
```

编译器为 MinGW-w64 GCC，C17 标准。IUP 3.31 和 Lua 5.5 库已包含在 `libs/` 目录下。CMake 在 POST_BUILD 阶段自动复制运行时 DLL 到输出目录。

## 架构

项目采用 **MVC 分层架构**：

```
src/
├── main.c                    # 入口：初始化日志/Lua/UI，创建 AppContext
├── core/                     # Model — 业务逻辑，零 IUP 依赖
│   ├── registry_service.c    #   Windows 注册表读写
│   ├── path_manager.c        #   路径增删移查与清理
│   ├── app_context.c         #   应用运行时状态（StringList sys/user）
│   ├── import_export.c       #   JSON/TXT 导入导出
│   └── lua_config.c          #   Lua 配置热加载
├── ui/                       # View — IUP 界面构建
│   ├── main_window.c         #   主窗口布局（Tab、列表、按钮、状态栏）
│   ├── dialogs.c             #   自定义输入对话框
│   └── ui_utils.c            #   斑马纹刷新等界面工具
├── controller/               # Controller — 连接 UI 与 Model
│   └── callbacks.c           #   所有按钮/搜索/拖拽/键盘回调
└── utils/                    # 纯工具层，无业务依赖
    ├── string_ext.c          #   StringList 动态字符串数组
    ├── os_env.c              #   编码转换 + 管理员权限检测
    ├── safe_string.c         #   安全字符串操作
    ├── logger.c              #   日志系统
    └── error_code.h          #   统一 ErrorCode 枚举
```

**数据流**: `registry_service` 从注册表加载 → `AppContext` 持有 StringList → UI 展示 → 用户操作经 `callbacks` 调用 `path_manager` 修改 StringList → `registry_service` 写回注册表 → `WM_SETTINGCHANGE` 广播。

**UI 控件寻址**：控件通过 `IupSetHandle("NAME", handle)` 注册名称，通过 `IupGetHandle("NAME")` 或 `IupGetDialogChild(dlg, "NAME")` 查找，不使用全局变量。AppContext 通过 `IupSetAttribute(dlg, "APP_CONTEXT", ctx)` 挂载到对话框上。

**Lua 配置**：`lua/config.lua` 定义所有 UI 文本和布局参数（按钮文字、颜色、尺寸等），通过 `lua_config_get_string("section", "key")` 读取，修改后无需重新编译。

## 关键约束

- IUP UTF8MODE 必须在 `IupOpen()` 之前通过 `putenv("IUP_UTF8MODE=YES")` 设置
- `IupFlatList` 数据操作必须在 `IupShowXY`（控件 Map）之后才能生效
- 管理员权限检测通过尝试以 KEY_WRITE 打开注册表键实现
- 非管理员模式下所有修改按钮和保存按钮被禁用
- PATH 注册表值使用 `REG_EXPAND_SZ` 类型，支持 `%SystemRoot%` 等变量展开
- 拖拽支持在管理员模式下需要调用 `ChangeWindowMessageFilter` 绕过 UIPI
