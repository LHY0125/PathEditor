# 路线图

PathEditor 的未来发展方向。

## v5.1 (下一个版本)

- [ ] **CLI 模块化** — `cli/src/main.rs` 拆分为 `commands/` 子模块
- [ ] **自动更新** — 内置 Tauri updater，无需手动下载安装包
- [ ] **深色模式优化** — 对齐 Windows 系统主题自动切换
- [ ] **性能优化** — 虚拟滚动支持超长 PATH 列表（1000+ 条目）

## v5.2

- [ ] **PATH 历史快照** — 保存每次修改的时间线，支持回退到任意历史节点
- [ ] **规则引擎** — 自定义 PATH 整理规则（如「所有 Python 路径放最前」）
- [ ] **收藏夹** — 常用路径快速添加
- [ ] **冲突解决方案引导** — 可视化的可执行文件冲突对比与解决建议

## v6.0 (长期)

- [ ] **跨平台支持** — 适配 Linux (`/etc/environment` + `~/.profile`) 和 macOS (`path_helper`)
- [ ] **Web 管理面板** — 远程管理多台 Windows 服务器的 PATH 环境变量
- [ ] **插件系统** — 第三方扩展生态（如 Anaconda/VSCode/VS 自动检测与配置）
- [ ] **Windows Package Manager 集成** — 与 winget/chocolatey 联动，检测包管理器安装的路径

## 已交付

### v5.0.0

- ✅ Cargo workspace 三层架构 (core + gui + cli)
- ✅ CLI 命令行工具 (18 条命令)
- ✅ 冲突检测 + 工具清单
- ✅ 配置文件管理
- ✅ 撤销/重做 (10 种操作)
- ✅ 中英双语界面
- ✅ CI/CD 自动化

### v4.x 系列

- ✅ Tauri 2.x 重写
- ✅ 路径验证 (红色/橙色标记)
- ✅ 导入/导出 JSON/CSV/TXT
- ✅ 深色/浅色模式
- ✅ 全局键盘快捷键

---

欢迎通过 [Issues](https://github.com/LHY0125/PathEditor/issues) 提交功能建议！
