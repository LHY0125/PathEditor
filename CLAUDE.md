# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# PathEditor 开发指南

> 本文件是开发窗口的仓库说明，也是每轮开工默认加载的上下文。
> 详细参考已拆到 `docs/dev/`，**按需读取**（见文末「文档索引」），不要为了查细节把全文读一遍。
> `AGENTS.md` 是同一份内容的副本，当前保持字节级一致。

## 项目概述

PathEditor 是 Windows 系统环境变量（PATH）编辑器，采用 Tauri 2.x + React 19 + TypeScript strict + Rust workspace，提供 GUI 和 CLI 两种入口。当前版本见 `package.json`（v6.0.0）。

v6 起数据层是一个 **SQLCipher 加密数据库（sidecar）**：历史时间线、配置版本、备份索引、全文检索、统计、引用图与文件式跨设备同步。**注册表仍是唯一真相源**，DB 打不开时应用 fail-open（编辑照常、历史/同步/检索降级）。

系统 PATH（HKLM）写入需要管理员权限；用户 PATH（HKCU）由 `PathCapabilities` 按 hive 独立判断权限。不能再用一个全局 `isAdmin` 字段推断两个 PATH 是否可写。

## 快速命令

```powershell
npm install                         # 安装前端依赖
npx tauri dev                       # GUI 开发模式（热更新）
npm run dev                         # 仅运行 Vite 前端
npm run build                       # 前端类型检查 + Vite 构建
cargo check                         # Rust workspace 检查
cargo build --release -p patheditor-cli
npx tauri build                     # 生成 NSIS 安装包；GUI 本体 target\release\PathEditor.exe。NTFS 大小写不敏感，共享 target 目录下 CLI 构建会覆盖它——隔离构建须 --target-dir target/cli（CI 做法）

npm test                            # Vitest 单元测试
npm run test:watch                  # Vitest 监听模式
npm run test:coverage               # 单元测试 + 覆盖率门槛
npm run test:e2e                    # Playwright（生产构建 + mock IPC）
npm run lint                        # ESLint
npm run format:check                # Prettier 检查
cargo fmt --check                   # Rust 格式检查
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace              # Rust 全量测试
cargo test -p patheditor-cli --bins # CLI 单测（该 crate 无 lib target）
npm run verify                      # 前端 + Rust 全量质量门
npm run verify:all                  # verify + Playwright E2E
```

跑单个测试：

```powershell
npx vitest run tests/unit/<file>.test.ts          # 单个前端测试文件
npx vitest run -t "<test name>"                   # 按测试名过滤
cargo test -p <crate> <test_name>                 # 单个 Rust 测试
cargo test -p path-editor-core --test <file>      # core 集成测试（包名是连字符，不是下划线）
cargo test -p patheditor-cli --bins <test_name>   # CLI 单测（必须 --bins）
```

## 架构速览

```text
PathEditor/
├── core/src/
│   ├── registry/     # 注册表访问统一入口
│   ├── db/           # v6 SQLCipher 数据层（schema/history/profiles/backups/
│   │                 #   search/stats/refs/compare/scan_history/migrate_v5/
│   │                 #   v5_export/v5_import/maintenance/archive_v5/sync）
│   ├── service.rs    # 保存编排（save_path_with_sidecar / apply_path_snapshot）
│   ├── disabled.rs / env_var.rs / path_entry.rs / scanner.rs / persist.rs
│   ├── backup.rs     # 注册表备份、env 备份（.pebak）
│   └── profiles.rs   # v5 文件版配置（兼容通路；DB 版在 db/profiles.rs）
├── gui/src/commands/ # Tauri 薄包装（registry/system/backup/fs/disabled/scanner/
│                     #   env_var/service/history/m2/sync）
├── cli/src/          # Clap + 分派（runtime/env_ops/profile_ops/sync_ops/
│                     #   maintenance_ops/scan_ops/import_export）
├── src/              # React（core/services/store/components/hooks/i18n）
├── tests/unit / tests/fixtures / e2e/tests
├── probes/v6-sqlcipher（不在 workspace；M0 证据）
└── docs/{superpowers,dev,审核和开发}
```

**完整目录树、模块职责与数据流**：`docs/dev/architecture.md`。

### 关键约束（始终生效）

- `src/core/` 保持纯函数和零框架依赖；组件和 Store 不得直接调用 `invoke`，统一走 `src/services/backend.ts`。
- `gui` 和 `cli` 只做参数转换、命令分派和错误呈现，业务规则放在 `core`。CLI 侧**零安全判定逻辑**：保留名、保护名单、`Unsupported` 类型、hive 写权限、revision 校验全部由 core 判定；CLI 仅透传错误文本，新增命令时不要复制任何判定规则。
- `patheditor-cli` 是 **bin-only crate**；对 CLI 跑单测必须用 `--bins`。
- `PathCapabilities` 使用 camelCase 序列化；共享契约见 `tests/fixtures/path-capabilities.json`。
- 正式 PATH 解析以 Rust `registry::split_path` 为准；TS 的 `split_path` 仅保留为兼容/测试夹具。
- `PathEntry { path, enabled }` 是跨层契约；导入、配置、注册表和撤销重做都必须保留 `enabled`。
- 通用环境变量通路（`EnvVar`）与 PATH 通路（`PathEntry`）并存：`Path` 列入 `RESERVED_NAMES`，**PATH 只能经专用通路编辑**。`EnvVarMeta` 契约上不含 `value` 字段；命中敏感规则的变量明文只能经 `reveal_env_var` 获取。
- 环境变量读写的并发契约只有一套：core 的 `update_env_var` / `delete_env_var` 恒要求 `expected_revision`。前端与 CLI **不得**自行实现并发校验；冲突判定只认 `CoreError.code`，`[E_CONFLICT]` 前缀仅是展示文本。
- Windows 注册表无 CAS，读-比-写是两次独立调用，仍有毫秒级竞态窗口。缩小影响不等于消除 TOCTOU，不要宣称原子性。
- **v6 数据层是 sidecar**：所有 DB 访问在 `core/src/db/`；GUI/CLI 不得直接打开 SQL。注册表仍是唯一真相源，DB 不可用时 fail-open。
- **同步导入是单事务，且不写注册表**：导入只改 DB；用户要生效必须另走 `apply_path_snapshot` / `apply_profile`。
- **维护命令的删除类操作必须显式确认**：`cleanup-legacy-backups` 需 `--yes`；测试一律用 `TempDir`，不碰真实 `~/.patheditor`。
- **v5 文件已停写**：`disabled.json` / `profiles/*.json` 不再由应用写入；迁移成功后归档为 `.v5.bak`。
- `.codegraph/` 存在时，优先使用 CodeGraph 理解符号和调用路径，再决定是否读取文件。

## 设计系统（v5.1.7 起，v6 沿用）

- 语义 token 全部在 `src/styles/globals.css` 的 `:root` 与 `:root.dark`；组件**只允许**使用 `--app-*` 语义变量，禁止硬编码色值。
- 主界面控件类：`.toolbar` / `.toolbar-sep` / `.toolbar-search` / `.tab-pill`。
- 对话框控件类：`.modal-overlay` / `.modal-surface` / `.field-input` / `.dialog-list-item` / `.btn-block`。
- 按钮统一使用 `src/components/ui/Button.tsx`（`buttonVariants` 在本仓库内解析变体）；`btnClass` / `btnStyle` 已删除，不要再引用。
- **陷阱**：上述工具类是**未分层规则**，会覆盖 Tailwind `@layer utilities`。追加 Tailwind 工具类前先确认它不会失效——`field-input text-xs` 曾是一个永不生效的死类名。
- 对话框的视觉规格由 CSS 类承载，`Modal.tsx` 只负责挂载与 Escape 关闭；改视觉时改 `globals.css`，不要往组件里塞内联样式。

## v6 数据层（速览）

- **sidecar**：`~/.patheditor/patheditor.db`（SQLCipher）；DB 打不开时 fail-open，不静默重建空库。
- **密钥**：`~/.patheditor/keys/db.key.dpapi`，DPAPI 当前用户作用域；DACL 限当前用户 + SYSTEM。
- **schema**：`CURRENT_SCHEMA_VERSION = 3`；`PRAGMA user_version` 只在建库/迁移事务内写。
- **一文件一测试**：任何使用 SQLCipher 的 `.rs` 测试文件只放一个 `#[test]`（同进程第二个会 `STATUS_STACK_OVERFLOW`）。
- **v5 迁移**：`migrate_v5` 幂等导入，迁移期间不改名；成功后 `archive_v5` 在启动序列末尾把原件改名为 `.v5.bak`。
- **同步**：bundle = `PESYNC1\0 + version + header_len + header JSON + salt + nonce + ciphertext + tag`；Argon2id + ChaCha20-Poly1305；LWW；`snapshot`/`profile` 按整实体比较；导入不写注册表。
- **保留策略**：已导出 `changes` 90 天、已解决 `sync_conflicts` 30 天；**待导出队列永不删**。

**完整 schema、迁移、同步与构建成本**：`docs/dev/sqlite-storage.md`。

## 接口速览（IPC / CLI）

- **IPC 唯一入口**：`src/services/backend.ts` + 运行时形状校验；v6 的 DB/同步命令返回结构化 `CoreError`，前端按 `code` 判定。
- **CLI 退出码**：0 成功 / 1 一般错误（含 `SyncFormat`）/ 2 clap / 3 revision 冲突 / 4 同步合并冲突 / 5 密钥。
- **同步与维护**：`patheditor sync …`、`patheditor maintenance …`；同步只改 DB，不写注册表。

**完整 IPC 表、CLI 命令与语义**：`docs/dev/interfaces.md`。

## 数据与安全红线

- PATH 的唯一保存编排是 `save_path_with_sidecar`；注册表成功但快照失败 → 落 pending，下次补写。
- 保存前用原始值检测外部修改；清理/禁用/启用必须保留 `PathEntry`。
- env 备份默认 `.pebak`（DPAPI）；密钥不可用时回落明文 `.json` 并告警。遗留 `.json` 清理需 `--yes`。
- E2E 使用 mock IPC，**不得写真实注册表**；同步导入同样不写注册表。真实 Tauri/注册表闭环测试必须显式授权，并记录备份、操作前后快照、重启结果和回滚结果。
- 所有 `unsafe` 块必须有 `// SAFETY:` 注释；所有 `pub fn` / `pub(crate) fn` 有中文 `///`。
- `backend.ts` 必须拒绝来路不明的 `EnvVarMeta`（含 `value` 字段者一律拒绝）。

**完整的数据/事务/安全规则**：`docs/dev/data-and-security.md`。

## 测试与质量门（速览）

- 提交前优先 `npm run verify:all`；覆盖率门槛 **80% 行**。
- 基线（v6.0.0）：Rust **57 binaries / 350 passed**；前端 **33 files / 398**；E2E **54**。
- **一文件一测试**（SQLCipher）；测试必须显式传路径，不碰真实 `~/.patheditor`（`config_dir()` 不可重定向）。
- CI：`.github/workflows/ci.yml` 在 push `main` / PR / 手动时跑 MSVC 质量门；`release.yml` 只在 `v*` tag 时发布。**CI 不跑 E2E**。

**完整测试布局与 CI 步骤**：`docs/dev/testing.md`。

## 发布（速览）

- **发布由 CI 自动完成**：`release.yml` 在 `v*` tag 触发（校验版本 → `npx tauri build` → CLI → 产物 → 日志 → Release）。
- 打 tag 前：版本四处一致、`CHANGELOG.md` 有当前版本段落、**该提交的 CI 已绿**。
- **禁止**手工 `gh release create`；产物命名与故障处理见 `docs/dev/release.md`。

## 提交与协作

使用 Conventional Commits：`<type>: <description>`，允许 `feat`、`fix`、`refactor`、`docs`、`test`、`chore`、`perf`、`ci`、`style`、`revert`。Husky + lint-staged 会在提交前运行 Prettier/ESLint；不要绕过失败的质量门。

审查或开发大改动前先看 `docs/审核和开发/` 的历史记录，说明风险、验证证据和未覆盖项。特性开发先写 `docs/superpowers/specs/` 下的设计文档、再写 `docs/superpowers/plans/` 下的实施计划，然后按任务逐步实施。未经明确要求不要提交、推送、升级版本或执行真实注册表写入。

## 文档索引

| 文档                            | 内容                                           |
| ------------------------------- | ---------------------------------------------- |
| `docs/dev/architecture.md`      | 完整目录树、模块职责、保存数据流               |
| `docs/dev/sqlite-storage.md`    | schema、迁移、同步、保留策略、OpenSSL 构建成本 |
| `docs/dev/interfaces.md`        | Tauri IPC 全表、CLI 命令与退出码               |
| `docs/dev/data-and-security.md` | 保存/事务、备份、错误处理与安全边界            |
| `docs/dev/testing.md`           | 测试布局、一文件一测试、CI 步骤、基线          |
| `docs/dev/release.md`           | 版本号清单、发布流程、产物与事故教训           |
| `docs/superpowers/specs/`       | v6 设计文档                                    |
| `docs/superpowers/plans/`       | M1–M4 实施计划                                 |
| `docs/审核和开发/`              | 审查与开发记录                                 |
