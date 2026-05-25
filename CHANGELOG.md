# Changelog

## v4.0.0 (2026-05-26)

### 重大变更

完全重写为 Tauri 2.x + React 19 + TypeScript + Rust 技术栈，替代原有的 C + IUP GUI。

### 新增

- 现代 Web UI（React + Tailwind CSS 4 + Zustand）
- 深色/浅色模式切换
- 中英文界面即时切换
- 路径有效性颜色编码（红色无效、橙色重复）
- 环境变量展开悬停提示
- 文件夹拖拽添加路径
- 保存前 PATH 长度检查
- 66 个前端单元测试 + 10 个 Rust 单元测试

### 改进

- 安装包体积从 ~3MB 降至 ~8MB（含 WebView2 运行时）
- 完整撤销/重做支持（8 种操作类型，50 步历史）
- JSON/CSV/TXT 三种格式导入导出
- 合并预览查看系统+用户路径
- 类型安全：TypeScript strict 模式 + Rust 编译期检查

### 移除

- 旧 C + IUP + Lua + gettext 代码库
- Lua 配置引擎 → JSON 配置文件
- gettext 国际化 → i18next

### 已知限制

- 需要 Windows 10+ 系统预装的 WebView2 运行时
- 内存占用约 50MB（旧版约 15MB）
- 文件系统路径验证在清理功能中为同步检查（不含实际目录存在性验证）
