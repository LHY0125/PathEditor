# Path Editor (系统环境变量编辑器)

一个简单、轻量级的 Windows 系统环境变量（PATH）编辑器，基于 C 语言和 IUP 图形库开发。

## ✨ 功能特点

* **可视化编辑**：直观地查看和管理系统 PATH 环境变量。
* **安全操作**：必须以管理员身份运行才能保存更改，防止误操作。
* **便捷管理**：
  * ➕ **新建**：添加新路径到列表。
  * 📂 **浏览**：直接从文件资源管理器选择目录添加。
  * ✏️ **编辑**：修改现有路径。
  * 🗑️ **删除**：移除不需要的路径。
  * ⬆️⬇️ **排序**：上移/下移调整路径优先级。
* **轻量级**：原生 C 语言编写，运行速度快，占用资源少。

## 📦 下载与安装

您可以从 [Releases](https://github.com/LHY0125/PathEditor/releases) 页面下载最新的安装包 (`PathEditorSetup.exe`)。

安装完成后，请**以管理员身份运行**程序以确保能够保存对系统环境变量的修改。

## 🛠️ 构建指南

如果您想从源码构建本项目，请按照以下步骤操作：

### 环境要求

* Windows 操作系统
* GCC 编译器 (推荐 MinGW-w64)
* Make 工具
* IUP 库 (已包含在 `libs` 目录下)

### 编译步骤

1. 克隆仓库：

   ```bash
   git clone https://github.com/LHY0125/PathEditor.git
   cd PathEditor
   ```

2. 编译项目：

   ```bash
   mingw32-make
   ```

3. 运行：
   编译成功后，可执行文件位于 `bin/PathEditor.exe`。

### 打包 (可选)

本项目使用 Inno Setup 生成安装包。

1. 确保已安装 [Inno Setup 6](https://jrsoftware.org/isdl.php)。
2. 编译项目生成 exe 文件。
3. 使用 Inno Setup 编译 `dist/installer.iss` 脚本。

## 📝 使用说明

1. **启动**：右键点击程序图标，选择“以管理员身份运行”。
2. **查看**：程序启动后会自动加载当前的系统 PATH 变量。
3. **修改**：使用右侧按钮栏进行添加、删除、移动等操作。
4. **保存**：操作完成后，务必点击底部的【确定】按钮保存更改。
5. **生效**：保存后，某些正在运行的程序可能需要重启才能识别新的环境变量。CMD 或 PowerShell 窗口需要重新打开。

## 👤 作者信息

* **作者**：LHY
* **邮箱**：<3364451258@qq.com>
* **GitHub**：[https://github.com/LHY0125/PathEditor](https://github.com/LHY0125/PathEditor)

如果您觉得这个工具对您有帮助，请给我的 GitHub 仓库点个 Star ⭐️！

## 📄 许可证

本项目基于 MIT 许可证开源，您可以在遵守许可证条款的前提下自由使用、修改和分发本项目的代码。

详细信息请参阅 [LICENSE](LICENSE) 文件。

Copyright © 2026 LHY. All Rights Reserved.
