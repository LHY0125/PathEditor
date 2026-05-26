# CLAUDE.md

## 项目概述

PathEditor v4.0 — Windows 系统环境变量 (PATH) 编辑器，使用 Tauri 2.x + React 19 + TypeScript + Rust 构建。

## 构建命令

```bash
# 安装前端依赖
npm install

# 开发模式（热更新）
npx tauri dev

# 仅前端（浏览器预览，无注册表功能）
npm run dev

# 前端测试
npm test
npm run test:watch

# Rust 后端检查
cd src-tauri && cargo check

# Rust 后端测试（需要 MinGW bin 在 PATH）
cd src-tauri && cargo test

# 生产构建
npm run build

# 完整构建（生成 NSIS 安装包）
npx tauri build
```

## 架构

前后端分离，通过 Tauri IPC 通信。核心原则：**纯逻辑放前端 `core/`，只有注册表/系统操作用 Rust 后端**。

```
src/                          # React 前端 (TypeScript strict 模式)
├── core/                     # 纯逻辑 — 零 React/零 Tauri 依赖
│   ├── undo-redo.ts          # 撤销/重做管理器（8 种操作类型，不可变 string[]）
│   ├── path-manager.ts       # 路径分析、清理（纯函数，不可变）
│   ├── import-export.ts      # JSON/CSV/TXT 导入导出
│   └── validation.ts         # 路径格式验证、split/join
├── store/                    # Zustand 状态管理
│   ├── app-store.ts          # 主状态（路径、撤销、CRUD、加载/保存、isSaving 守卫、snapshot 比较）
│   └── theme-store.ts        # 深色/浅色模式（localStorage 持久化）
├── components/
│   ├── layout/               # AppShell、TitleBar、StatusBar、ErrorBoundary
│   ├── path-list/            # PathTable（异步验证、颜色编码、tooltip）、MergePreview
│   ├── toolbar/              # ToolBar、ActionButtons、UndoRedoButtons、SearchInput
│   ├── dialogs/              # PathEditDialog、HelpDialog、ImportDialog
│   └── ui/                   # Modal（共享遮罩层）、buttons（共享样式）
├── hooks/
│   ├── use-app-actions.ts    # CRUD/导入导出/键盘/双击操作集中管理
│   ├── use-keyboard.ts       # 全局快捷键（ref 模式避免重复注册）
│   └── use-path-validation.ts # 路径验证 hook（空壳，验证逻辑在 PathTable 内联）
├── i18n/
│   ├── index.ts              # i18next 初始化
│   └── locales/              # zh-CN.json / en.json
└── config/
    └── default.json          # 撤销上限、PATH 长度限制

src-tauri/                    # Tauri Rust 后端
├── src/
│   ├── commands/
│   │   ├── registry.rs       # 注册表读写（通用 load/save 函数 + 6 个单元测试）
│   │   ├── system.rs         # check_admin、validate_path、expand_env_vars、broadcast（+ 4 个测试）
│   │   └── backup.rs         # backup_registry、get_appdata_dir（共享 backup_base_dir）
│   ├── error.rs              # AppError 结构体（保留供未来迁移）
│   └── lib.rs                # 注册所有 IPC commands + tauri-plugin-dialog
├── Cargo.toml                # winreg + dirs + chrono
└── tauri.conf.json

tests/unit/                   # Vitest 前端单元测试（4 文件 45 个测试）
```

## 数据模型

所有路径数据在前端用 **不可变 `string[]`** 存储。每次 CRUD 操作创建新数组通过 `set()` 写入 Zustand，Zustand 通过引用比较自动触发重渲染。

```typescript
// app-store.ts 核心模式
addPath: (path, target) => {
  const newList = [...list, path];     // 不可变：创建新数组
  state.undoRedo.push({ ... });       // 记录撤销
  set({ sysPaths: newList });          // Zustand 检测到新引用 → 重渲染
},
```

`isModified` 通过与上次保存时的快照比较判断，而非简单设 `true`/`false`：
```typescript
_markDirty: () => {
  const { _savedSys, _savedUser, sysPaths, userPaths } = get();
  set({ isModified: !(arraysEqual(sysPaths, _savedSys) && arraysEqual(userPaths, _savedUser)) });
},
```

## IPC 接口（Rust → Frontend）

| Command | 参数 | 返回值 | 功能 |
|---------|------|--------|------|
| `load_system_paths` | — | `Result<Vec<String>, String>` | 从 HKLM 注册表读取系统 PATH |
| `load_user_paths` | — | `Result<Vec<String>, String>` | 从 HKCU 注册表读取用户 PATH |
| `save_system_paths` | `paths: Vec<String>` | `Result<(), String>` | 保存系统 PATH（含 32767 字符上限检查） |
| `save_user_paths` | `paths: Vec<String>` | `Result<(), String>` | 保存用户 PATH |
| `check_admin` | — | `bool` | 通过尝试 KEY_WRITE 检测管理员权限 |
| `validate_path` | `path: &str` | `bool` | 检查目录是否存在（含 `%` 自动返回 true） |
| `expand_env_vars` | `path: &str` | `String` | 展开 `%VAR%` 环境变量（API 失败时返回原始路径 + `log::warn`） |
| `broadcast_env_change` | — | `()` | 广播 `WM_SETTINGCHANGE` 通知其他进程 |
| `backup_registry` | `custom_dir, sysPaths, userPaths` | `Result<String, String>` | 备份 PATH 到时间戳文件 |
| `get_appdata_dir` | — | `String` | 获取备份目录路径 |

## 撤销/重做

`UndoRedoManager` 类（`src/core/undo-redo.ts`），8 种操作类型，最多 50 步历史。

核心 API：`undo/redo` 接收 `string[]`，返回新 `[string[], string[]] | null`。

```typescript
class UndoRedoManager {
  push(record: OpRecord): void;
  undo(sys: string[], user: string[]): [string[], string[]] | null;
  redo(sys: string[], user: string[]): [string[], string[]] | null;
  canUndo(): boolean;
  canRedo(): boolean;
}
```

## 错误处理

### 前端

| 场景 | 处理 |
|------|------|
| IPC 调用失败 | `Promise.allSettled` 精确报告哪个 hive 保存失败 |
| JSON 文件损坏 | `importFromJson` try/catch，返回空结果 |
| 保存并发双击 | `isSaving` 守卫，第二次调用直接 return |
| 备份创建失败 | `.catch()` 显示 warning_backup，保存继续 |
| 渲染异常 | ErrorBoundary 捕获 + console.error + 重试按钮 |
| 取消按钮误点 | 检查 `isModified`，弹出确认框 |
| 拖拽非文件夹 | `webkitGetAsEntry().isDirectory` 过滤 |

### Rust 后端

- 注册表操作：全部返回 `Result<T, String>`，中文错误消息
- FFI 调用：失败时 `log::warn` + 返回安全回退值
- PATH 长度：写入前检查 32767 字符上限
- SAFETY 注释：所有 `unsafe` 块均有文档

## 关键约束

- **TypeScript**：`strict: true`，零编译错误
- **Rust 工具链**：`stable-x86_64-pc-windows-gnu`（项目已设 override）
- **MinGW 兼容**：`.cargo/config.toml` 添加 `-lmcfgthread`（GCC 15.2.0 运行时）
- **crate-type**：移除 `cdylib` 避免 DLL 导出序数溢出
- **运行权限**：需要管理员权限才能编辑系统 PATH，非管理员自动进入只读模式
- **Rust 测试**：`cargo test` 需要 MinGW bin 在 PATH 中（`libmcfgthread-2.dll`），开发模式建议 `npx tauri dev`
- **构建产物**：NSIS 安装包，约 8MB
