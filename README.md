# PathEditor v4.0

Windows 系统环境变量 (PATH) 编辑器，基于 Tauri 2.x + React 19 + TypeScript + Rust 构建。

## 功能

- 查看和编辑系统/用户 PATH 环境变量
- 新建、编辑、删除、上移、下移路径条目
- 一键清理无效和重复路径
- 完整撤销/重做支持（最多 50 步）
- 导入/导出 JSON、CSV、TXT 三种格式
- 深色模式 / 浅色模式切换
- 中英文界面切换
- 合并预览（同时查看系统 + 用户路径）
- 搜索过滤
- 文件夹拖拽添加
- 注册表备份

## 运行

需要管理员权限才能编辑系统 PATH（非管理员自动进入只读模式）。

```bash
# 安装依赖
npm install

# 开发模式（热更新）
npx tauri dev

# 构建安装包
npx tauri build
```

## 技术栈

| 层 | 技术 |
|---|---|
| 前端框架 | React 19 + TypeScript |
| UI 样式 | Tailwind CSS 4 |
| 状态管理 | Zustand |
| 国际化 | i18next |
| 桌面框架 | Tauri 2.x |
| 后端语言 | Rust |
| 测试 | Vitest (前端) |
| 构建 | Vite |

## 架构

```
src/                      # React 前端
├── core/                 # 纯逻辑（StringList、撤销/重做、路径管理、导入导出）
├── store/                # Zustand 状态管理
├── components/           # UI 组件（列表、工具栏、对话框）
├── hooks/                # 自定义 Hooks（键盘快捷键、路径验证）
├── i18n/                 # 中英文翻译
└── config/               # UI 参数配置

src-tauri/                # Rust 后端
└── src/commands/
    ├── registry.rs       # 注册表读写
    ├── system.rs         # 权限检测、路径验证、环境变量展开、系统广播
    └── backup.rs         # 注册表备份
```

## 快捷键

| 快捷键 | 功能 |
|--------|------|
| Ctrl+N | 新建路径 |
| Ctrl+S | 保存 |
| Ctrl+Z | 撤销 |
| Ctrl+Y | 重做 |
| Ctrl+F | 搜索 |
| Delete | 删除选中 |
| F1 | 帮助 |

## 开发

```bash
# 前端测试
npm test

# 前端测试（监听模式）
npm run test:watch

# Rust 后端检查
cd src-tauri && cargo check

# Rust 后端测试
cd src-tauri && cargo test
```

## 许可证

MIT License

## 作者

刘航宇 — [GitHub](https://github.com/LHY0125/PathEditor)
