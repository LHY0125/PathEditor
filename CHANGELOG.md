# Changelog

## [4.2.0] — 2026-05-28

### 新增
- 路径启用/禁用功能：复选框控制 PATH 中每条路径是否生效
- PathEntry 数据类型：替代原有 `string[]`，支持 `enabled` 状态
- `disabled.json` 持久化禁用状态的独立存储
- E2E 测试框架：Playwright + 4 条核心流程测试
- CI/CD 流水线：TypeScript + Rust 自动检查，Release 自动构建

### 修复
- undo/redo after toggle 未持久化 disabled 状态
- expand_env_vars 两次 API 调用间缓冲区截断风险
- E2E mock load_disabled_state 返回格式与 Rust 后端不匹配
- 双 hive 保存失败时错误信息只显示一个
- 导入 both 产生两条 undo 记录，需两次 Ctrl+Z
- 备份失败警告被"保存成功"覆盖
- 非连续多行删除后 undo 恢复到错误位置
- backup_registry 未 await 导致竞态保存新值

### 变更
- 导入改用原生文件对话框（`@tauri-apps/plugin-dialog`）
- PathTable 环境变量展开限流 20 并发
- CI 切换到 MSVC 工具链
- 版本号统一为 4.2.0

---

## [4.1.0] — 2026-05-26

### 新增
- app-store 单元测试：25 个测试覆盖 CRUD/undo-redo/loadPaths/savePaths
- 72 个前端单元测试 + 10 个 Rust 单元测试

### 修复
- NSIS 安装包缺少 WebView2Loader.dll
- AppShell overflow-hidden 导致窗口无法上下滚动

### 变更
- 清理 LOW 问题：样式去重、死代码删除、命名修正
- 抽取 Modal 共享组件、统一按钮样式
- 支持 JSON/CSV/TXT 三种导入导出格式

---

## [4.0.0] — 2026-05-25

### 重大变更
完全重写为 Tauri 2.x + React 19 + TypeScript + Rust 技术栈，替代原有的 C + IUP GUI。

### 新增
- 现代 Web UI（React 19 + Tailwind CSS 4 + Zustand）
- 深色/浅色模式切换
- 中英文界面即时切换
- 路径有效性颜色编码（红色无效、橙色重复）
- 环境变量展开悬停提示
- 文件夹拖拽添加路径
- 保存前 PATH 长度检查

### 改进
- 完整撤销/重做支持（8 种操作类型，50 步历史）
- JSON/CSV/TXT 三种格式导入导出
- 合并预览查看系统+用户路径
- 类型安全：TypeScript strict 模式 + Rust 编译期检查
- NSIS 安装包，约 8MB

### 移除
- 旧 C + IUP + Lua + gettext 代码库
- Lua 配置引擎 → JSON 配置文件
- gettext 国际化 → i18next

---

## [3.x] 及更早

C + IUP GUI 版本，已停止维护。历史发布记录见 [GitHub Releases](https://github.com/LHY0125/PathEditor/releases)。
