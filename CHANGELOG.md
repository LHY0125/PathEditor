# Changelog

## 5.0.0 (2026-05-29)

### Added
- Cargo workspace 三层架构 (core + gui + cli)
- CLI 命令行工具，17 条命令，支持 JSON 输出
- PATH 可执行文件冲突检测 (`scan_conflicts`)
- PATH 目录工具清单 (`scan_tools`)
- 配置文件管理：保存/加载/应用/重命名/删除
- 系统+用户合并预览视图
- CLI 原子性保护：写入前重新读取注册表对比
- `--steps N` 参数支持多格移动 (CLI 特有)

### Changed
- Rust + Tauri 2.x + React 19 + TypeScript strict 全重写
- 撤销/重做系统扩展至 10 种操作类型
- 禁用状态即时持久化，不依赖保存按钮
- 深色模式 / 浅色模式 CSS 变量驱动
- 中英双语界面 (i18next)
- 备份文件存储路径统一到 `~/.patheditor/`
- 版本号集中管理: Rust 端 `Cargo.toml` workspace, 前端 `package.json`

### Fixed
- 非管理员自动进入只读模式
- 保存失败精确提示哪个注册表 hive 出错 (Promise.allSettled)
- CLI `--system`/`--user` 互斥校验
- 修改操作后广播 `WM_SETTINGCHANGE`
- 深色模式下行选中颜色对比度不足
- 窗口内容溢出无法滚动

## 4.2.0

### Fixed
- Release workflow 兼容已存在的 release

## 4.1.0

### Added
- 路径验证 (红色无效、橙色重复)
- 环境变量路径悬浮展开预览
- 全局键盘快捷键
- 修改状态指示 + 未保存退出确认

## 4.0.0

### Added
- Tauri 2.x + React + TypeScript 首次发布
- Windows 系统/用户 PATH 的增删改查
- 拖拽排序、多选批量删除
- 实时搜索过滤
- 导入导出 JSON/CSV/TXT
- 撤销/重做支持
- 保存前自动备份注册表
