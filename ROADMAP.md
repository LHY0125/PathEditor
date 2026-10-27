# 路线图

PathEditor 的交付记录与后续方向。

> 本文件只写两类内容：**已交付的能力**（可核对到 `CHANGELOG.md`）与**尚未排期的候选方向**。
> 候选不构成承诺，排期以 Issue 与 `docs/superpowers/specs/` 下的设计文档为准。

## 已交付

### v6.1（当前开发线）

- ✅ **6.1.1**（2026-10-24）— 合并预览虚拟行高修复、原生窗口标题版本修正、对话框无障碍
  （`role="dialog"` / `aria-modal` / 焦点圈定与归还）、警告色 token 达到 WCAG AA
- ✅ **6.1.0**（2026-10-23）— 源码统一到 `src/{core,cli,gui}`；移除 v5 过渡层，
  **DB 成为唯一持久化状态**；CLI 与 GUI 共用 `service::save_path_with_db`；
  错误模型统一为结构化 `CoreError`
- ✅ **6.0.0**（2026-10-22）— v6 M1–M4 收口：SQLCipher 加密存储、历史时间线与快照恢复、
  FTS5 全文检索、统计仪表盘、`%VAR%` 引用图、双 hive 快照对比、冲突历史、
  文件式跨设备同步（Argon2id + ChaCha20-Poly1305）、保留策略与遗留明文清理

### v5.x

- ✅ Tauri 2.x 重写、路径验证（红色/橙色标记）、导入导出 JSON/CSV/TXT、深色模式、全局快捷键
- ✅ Cargo workspace 三层架构（core + gui + cli）、18 条 CLI 命令、冲突检测与工具清单
- ✅ 配置方案、撤销/重做、中英双语、CI/CD 自动化
- ✅ 全环境变量管理（`Path` 除外）、CLI `env` 子命令、环境变量备份/恢复（`.pebak` + DPAPI）
- ✅ 性能：虚拟滚动 + 1000+ 条目有界验证/扫描队列；CLI 模块化拆分

## 未排期候选

### 数据与同步

- 环境变量的 N 路快照对比（当前 `compare` 只覆盖 PATH，`env_*` 字段恒为空）
- CLI 写入后重建派生表（当前用 CLI 改过 PATH 后，引用图与统计要重启 GUI 才对齐）
- 配置合并改为字段级 LWW（当前 `profile` 按整实体比较，两端同时改同一配置会互相覆盖）
- 同步包按依赖自动补齐（当前增量包必须从第一份起按顺序应用）

### 体验

- 内置 Tauri updater 自动更新，无需手动下载安装包
- 深色模式跟随 Windows 系统主题（当前仅手动切换 + `localStorage` 记忆）
- 规则引擎：自定义 PATH 整理规则（如「所有 Python 路径放最前」）
- 收藏夹：常用路径快速添加
- 冲突解决方案引导：可视化对比被遮蔽的可执行文件并给出优先级建议
- 与 winget / chocolatey 联动，识别包管理器安装的路径

### 平台与生态

- 跨平台：Linux（`/etc/environment` + `~/.profile`）、macOS（`path_helper`）
- Web 管理面板：远程管理多台 Windows 服务器的 PATH 环境变量
- 插件系统：第三方扩展生态（如 Anaconda / VSCode / VS 自动检测与配置）

---

欢迎通过 [Issues](https://github.com/LHY0125/PathEditor/issues) 提交功能建议。
