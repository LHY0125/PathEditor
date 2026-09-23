# Changelog

## 5.1.4 (2026-09-23)

本版包含三批内容：**一致性与架构收口**（Wave 0/1/2，2026-09-18 复审的 11 项问题）、**GUI 关窗死锁修复**、以及**环境变量备份与恢复**新特性。

### 新增

- 环境变量备份：CLI 与 GUI 的每一次环境变量写入前自动备份到 `~/.patheditor/backups/env_backup_<时间戳>.json`，含两个 hive 的全部可写变量与注册表类型（`REG_SZ` / `REG_EXPAND_SZ`）。
- 环境变量恢复：新增 `patheditor env restore <FILE>` 与 GUI 备份恢复界面，支持差异预览（新增 / 删除 / 冲突）与冲突保护；`--dry-run` 只打印差异、不写注册表。
- 新增 `patheditor env backup`（立即备份）与 `patheditor env backups`（列出备份，按时间倒序）。
- 新增 GUI 备份与恢复对话框（`envBackupPanel`），含删除项逐名提示、手工兜底命令提示与冲突二次确认。
- 保留份数可通过 `~/.patheditor/config.ini` 的 `env_backup_keep` 覆盖（默认 20 份）。
- 结构化错误契约 `CoreError` / `ErrorCode`：环境变量通路的错误不再是自由文本，前端按 `code` 判定，CLI 退出码由 `CoreError::exit_code()` 驱动。`[E_CONFLICT]` 消息前缀降级为过渡期展示文本，不再作为判定机制。
- 持久化文件（`disabled.json` / profiles）增加 `schemaVersion`、写入前保留上一份 `.bak` 轮换，以及损坏文件的隔离与恢复提示 —— 解析失败不再只返回通用 JSON 错误。
- 环境变量写入口新增 `reveal_env_var` 携带读取时 revision，编辑弹窗据此绑定「本值读取到的版本」。

### 变更

- 环境变量写入口（CLI `env set/add/remove`、GUI 编辑 / 新建 / 删除）返回值携带备份结果（`WriteOutcome.backup`）；备份失败不阻断写入，仅在 stderr 或状态栏提示。
- 恢复复用既有的 `create_env_var_in_store` / `update_env_var_force_in_store` / `delete_env_var_force_in_store` 写函数，保护名单、类型可写性、hive 权限判定仍只在 core 一处；CLI 与 GUI 均不做二次判定。
- 差异列表排序键固定为 `(hive, kind, name)`（user 在前、system 在后），使 `--dry-run --json` 的输出可复现。
- core 新增共享应用服务层（`apply_path_snapshot` / `apply_profile` / `save_path_with_sidecar` / `retry_pending_path_state`），统一 GUI 与 CLI 的 PATH 事务编排；GUI 侧接线延后，`path-session.ts` 仍走旧编排。
- 注册表访问改为 `EnvHiveStore` 端口：生产用 `WinregHive`，测试用内存替身 `MemoryHive`，`cargo test --workspace` 不再写真实 HKCU，可在任意环境执行。
- `core/src/registry.rs`（1101 行）拆分为 `registry/` 目录模块（纯搬家，零行为变化）。
- 新增 C→Rust 行为等价 golden 基线，覆盖 PATH 分割、写回类型、备份格式与广播时机。

### 修复

- **GUI 关窗死锁（v5.1.3 已知问题）**：关窗确认从阻塞式 `window.confirm` 改为 Tauri 异步对话框，并补齐 `core:window:allow-destroy` 权限（v5.1.3 中缺失，导致无草稿关窗挂起）。同步把其余阻塞式 `confirm` 全部换为异步对话框。
- **CLI 安装包装错二进制（v5.1.3 已知问题）**：GUI 与 CLI 产物同名（`patheditor.exe`）且共用 Cargo target 目录，NTFS 大小写不敏感使两者实为同一条目录项，CLI 链接产物覆盖了 GUI 本体——`scoop install lhy/patheditor-cli` 因此装到的是 GUI。现已将 GUI 二进制改名为 `PathEditor.exe`，并让 CI 用独立 target 目录（`--target-dir target/cli`）构建 CLI。
- `--force` 语义与文档不符：此前实为「重读一次 revision 再 CAS」，读与写之间的窗口仍会以退出码 3 失败。现改为真正的 force API（最后写入者胜，不做 revision 比对），仍保留名称校验、保护名单、类型可写性与 hive 写权限判定。
- 编辑弹窗可能用陈旧值覆盖外部更新：现在记录读值时的 revision，快照刷新时若已变化则提示并重载，提交前断言一致。
- CLI 注册表与 `disabled.json` 双写失败后状态丢失：sidecar 写失败会落 pending 待补写状态，PATH 命令启动时自动补写。
- 环境变量列表失败不再静默：此前枚举 / 读取 / 解码错误被 `Iterator::flatten()` 与 `continue` 吞掉，只留 warning；现在同一 hive 内任一步失败即返回错误，调用方明确知道结果不可用。

### 说明

- `~/.patheditor/backups/` 下的 `env_backup_*.json` 含敏感值**明文**（可能包括 API key、token），请勿同步到云端或提交到版本库。备份目录的文件权限**未做收紧**（登记为未覆盖项）。
- 备份默认保留最近 20 份，旧的自动轮换删除；只删除本工具生成的 `env_backup_*.json`，绝不触碰 `.txt` PATH 备份、`.bak` 与 `.corrupt-*` 文件。
- **差异计数的两个已知口径**：`RestorePreview.modified` 与 `RestoreOutcome.skipped` 恒为 0。备份 revision 与注册表不一致时一律判为「冲突」而非「修改」（`revision_of` 是 `(name, type, value)` 的纯函数，两者是同一条件）。因此 `--dry-run` 与 GUI 确认弹窗的「修改 N」恒显示 0，而 `--force` 下被冲突覆盖的变量确实被改写却计入「冲突」——dry-run 会**低报** force 模式的实际改动量。
- **未提权时恢复恒失败**：普通用户无法以写权限打开系统 hive，`restore_env_backup_from` 必然返回 `permissionDenied`（退出码 1），GUI 会提示需要管理员权限，且**不提供**「仅恢复用户 hive」这类降级路径。
- 恢复**逐条失败不改变退出码**（仍为 0），脚本无法从退出码检出部分失败；单变量失败以 stderr 警告呈现。
- `env restore --dry-run` 对**损坏备份**并非严格纯读：`read_env_backup` 对不可解析文件会经 persist 层将其重命名为 `<file>.corrupt-<ts>`（注册表未触碰）。
- `EnvBackupInfo.variableCount` 恒为 0 —— 列表只枚举目录与 stat，**不解析内容**，单个损坏备份不会让列表整体失败。
- GUI 关窗修复只在本机 GNU 构建上验证（WM_CLOSE 退出 + 异步确认对话框）；死锁仅复现于 CI/MSVC 构建，**CI 侧最终验证待本次发布构建后确认**。
- 4 个服务层 IPC 命令（`apply_path_snapshot` 等）已在 GUI 注册，但前端尚未接线，`path-session.ts` 仍走旧编排。

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
