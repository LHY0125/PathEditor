# Changelog

## 5.1.3 (2026-09-18)

### 新增

- CLI 新增 `env` 子命令组（`list` / `get` / `set` / `add` / `remove`），脚本与自动化场景获得与 GUI 等价的通用环境变量管理能力。
- 值输入三通道：位置参数 / `--stdin` / `--value-file`，互斥。后两者让敏感值可绕开 shell 历史与进程列表。
- `env list --json` 输出直接序列化 core 契约（camelCase，不含 `value` 字段），供脚本消费与获取 revision。
- `--force` 模式：跳过并发校验直接覆盖（脚本 `setx` 风格）。

### 变更

- CLI 写操作强制显式选择并发模式：`--revision <R>`（CAS 校验）或 `--force`（跳过校验），两者都不给报错、都给也报错。
- CLI 新增退出码 **3** 表示 revision 冲突，使脚本可凭退出码区分「重试后可恢复」与致命错误，无需 grep 中文文案。PATH 命令保持退出码 1 不变（向后兼容）。
- `env add` 的 `--kind` 默认 `string`（`REG_SZ`），可选 `expand`（`REG_EXPAND_SZ`）。

### 说明

- CLI 侧零安全判定逻辑：保留名、保护名单、`Unsupported` 类型、hive 写权限、revision 校验全部由 `core` 判定，CLI 仅透传错误文本。
- `env get` 是 CLI 侧唯一明文出口，stdout 只打印裸值 + 换行（管道友好，对齐 `git config --get`）。
- `Path` 不在通用通路内，仍只能经 PATH 专用命令编辑。
- 真实注册表闭环测试通过（`add` / `set --force` / 退出码 3 冲突 / `remove` CAS 四项），快照对比零污染。记录见 `docs/审核和开发/2026.09.18/PathEditor-CLI环境变量闭环测试记录.md`。
- 已知边界：Windows 注册表无 CAS，读-比-写是两次独立调用，revision 校验缩小竞态窗口但不能完全消除 TOCTOU。

## 5.1.2 (2026-09-15)

### 修复

- 修复保存 PATH 时注册表值类型被降级为 `REG_SZ` 的问题；现在会保持原有 `REG_EXPAND_SZ` / `REG_SZ` 类型，新值默认使用 `REG_EXPAND_SZ`（Issue #26）。

### 迁移说明

- 旧版本已经把 PATH 写成 `REG_SZ` 的机器不会自动改回 `REG_EXPAND_SZ`；下一次保存只会保持当前类型。
- 如需恢复 `%VAR%` 展开，请先备份注册表，再使用系统注册表编辑器或后续提供的显式修复命令将 PATH 改回 `REG_EXPAND_SZ`。
- 修复方案和人工验收步骤见：`docs/审核和开发/2026.09.15/PathEditor-Issue-26-PATH值类型降级修复方案.md`。

## 5.1.1 (2026-09-14)

### 修复

- 修复非管理员模式下用户 PATH 被错误锁定，系统 PATH 与用户 PATH 改为独立权限判断
- 修复路径验证批次在列表重渲染后任务丢失、条目长期停留在 pending 的问题
- 修复禁用路径重启后丢失，`disabled.json` 会保留完整有序快照并与注册表合并
- 修复禁用状态写入失败后仍被视为已保存、无法再次补写的问题
- 修复导入配置时无权限注册表 hive 仍可被操作的问题，并补充明确提示
- 修复超过 20 条路径后停止验证和环境变量展开的问题
- 修复 GUI 清理路径时不检查目录是否存在的问题
- 修复 `PathCapabilities` 序列化字段大小写不一致导致的真实 Tauri 运行时读取错误
- 修复 TS/Rust 导入导出语义漂移，CSV/JSON 的 `enabled` 状态处理保持一致
- 修复 CLI 禁用操作只写 sidecar、与 GUI 快照语义不一致的问题

### 优化

- 长列表引入虚拟滚动，优化 `PathTable` 与 `MergePreview` 的渲染性能
- 注册表保存增加原始路径并发比对，降低覆盖外部修改的风险
- 统一 Windows API 调用到 `windows-sys`，减少手写 FFI 声明
- 增加 PathCapabilities、PathEntry、PathSnapshot、禁用状态和 IPC 运行时形状校验
- 拆分 CLI、AnalyzeDialog、ProfileDialog、app-store 等过大模块，降低维护成本
- 补充路径验证、禁用状态、导入权限、快捷键和 E2E mock 回归测试

### 工程化

- 增加 Husky、lint-staged、commitlint、Prettier、ESLint 和覆盖率门禁
- 增加 Dependabot、CODEOWNERS、Issue/PR 模板
- 精简 GitHub Actions，改为 tag 触发的自动构建和 GitHub Release
- 发布产物同时包含 NSIS 安装包和 CLI 可执行文件

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
