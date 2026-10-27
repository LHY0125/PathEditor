# 模块分层重组 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `src/core/src` 的 17 个并列项收敛成 `domain / platform / db / app` 四层，把 `src/cli/src` 的 6 个 `*_ops.rs` 收敛进 `commands/`，并以纯移动 + 改引用的方式完成，零行为变更。

**Architecture:** 按「谁依赖谁」而非「文件类型」分层。`registry` 与 `backup` 互为强连通分量，必须同处 `platform`；`db` 保持顶层路径不变以免 50 处引用变长；`error` 归入 `domain` 作为跨层契约。

**Tech Stack:** Rust 2021 workspace（core / cli / gui），`git mv` 保历史，`sed` 词边界替换。

**Spec:** `docs/superpowers/specs/2026-10-28-module-layering-design.md`

## Global Constraints

- 分支：从 `main` 切 `refactor/module-layering` 后作业；不直接改 `main`。
- **前置条件**：开工前工作区必须干净。当前有 5 个未提交的文档改动（`AGENTS.md`、`CLAUDE.md`、`README.md`、`ROADMAP.md`、`docs/dev/testing.md`），需由用户先提交或 stash——否则任务 6 会与其叠加，无法区分两类改动。
- **零行为变更**：不改任何业务判定、不改 DB schema、不改同步包格式、不改 CLI 退出码（0/1/2/3/4/5）、不改 IPC 契约。
- **不写注册表、不打开真实 `~/.patheditor`**：所有测试用 `TempDir`。
- **全程 `git mv`**：提交前 `git status` 必须是 `renamed:`，不能是 `deleted:` + `new file:`。
- **不保留过渡别名**：`lib.rs` 只导出新路径，不加 `pub use` 旧路径别名。
- **不删除任何文件**：唯一例外是 `rmdir` 一个 `git mv` 之后变空的目录（不是文件）。
- **历史文档不改**：`docs/superpowers/plans/`、`docs/superpowers/specs/`、`docs/审核和开发/` 一律不动。
- **`AGENTS.md` 与 `CLAUDE.md` 字节级一致**：改一份必须同步另一份，用 `cmp` 验证。
- 代码注释用中文；所有新增 `pub fn` / `pub(crate) fn` 带中文 `///`。
- GNU 工具链本地构建前先设三行（只对当前 shell 生效，**不要**写进 `.cargo/config.toml`）：

```powershell
$env:OPENSSL_SRC_PERL = "$(scoop prefix git)\usr\bin\perl.exe"
$env:PERL5LIB         = "$PWD\probes\v6-sqlcipher\perl5lib"
$env:PATH            += ";$(scoop prefix git)\usr\bin"
```

- 每任务的质量门（快速）：`cargo check --workspace --all-targets`。
- 最终质量门：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo test -p patheditor-cli --bins`、`npm run verify`。
- 提交信息用 Conventional Commits，允许 `refactor` / `docs`。

---

### Task 1: 抽出 `domain` 契约层

**Files:**

- Create: `src/core/src/domain/mod.rs`
- Move: `src/core/src/error.rs` → `src/core/src/domain/error.rs`
- Move: `src/core/src/path_entry.rs` → `src/core/src/domain/path_entry.rs`
- Move: `src/core/src/env_var.rs` → `src/core/src/domain/env_var.rs`
- Move: `src/core/src/history.rs` → `src/core/src/domain/history.rs`
- Modify: `src/core/src/lib.rs`

**Interfaces:**

- Consumes: 无（本任务是最底层，不依赖其他层）
- Produces: 模块路径 `crate::domain::{error, path_entry, env_var, history}`、`path_editor_core::domain::*`

- [ ] **Step 1: 记录改动前的测试基线**

```bash
cargo test --workspace 2>&1 | tail -5
```

Expected: `test result: ok. 324 passed; 0 failed; 2 ignored`（数字以 `docs/dev/testing.md` 的「质量门」一节为准）。若不是这个数，**先停下来报告**，不要继续。

- [ ] **Step 2: 移动文件**

```bash
mkdir -p src/core/src/domain
git mv src/core/src/error.rs      src/core/src/domain/error.rs
git mv src/core/src/path_entry.rs src/core/src/domain/path_entry.rs
git mv src/core/src/env_var.rs    src/core/src/domain/env_var.rs
git mv src/core/src/history.rs    src/core/src/domain/history.rs
```

- [ ] **Step 3: 写 `domain/mod.rs`**

```rust
//! 领域契约层：跨层共享的类型、错误码与端口（trait）。
//!
//! 本层**零 OS、零 IO、零数据库依赖**：不碰注册表、不碰文件系统、不打开 SQLite。
//! 只允许依赖 `serde` 与 `winreg::enums` 的类型常量。

pub mod env_var;
pub mod error;
pub mod history;
pub mod path_entry;
```

- [ ] **Step 4: 改 `lib.rs` 的模块声明与再导出**

把 `src/core/src/lib.rs` 整体改成：

```rust
pub mod backup;
pub mod bootstrap;
pub mod capabilities;
pub mod db;
pub mod disabled;
pub mod domain;
pub mod fs;
pub(crate) mod persist;
pub mod registry;
pub mod scanner;
pub mod service;
pub mod system;

pub use backup::BackupOutcome;
pub use domain::env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot, RevealedValue};
pub use domain::error::{CoreError, ErrorCode};
pub use domain::path_entry::{PathEntry, PathSnapshot};
pub use registry::WriteOutcome;
pub use scanner::{ConflictEntry, ConflictLocation, ToolGroup};
```

- [ ] **Step 5: 全仓库替换模块路径（core 内部 + 外部引用）**

```bash
grep -rlE --include='*.rs' '(crate|path_editor_core)::(error|path_entry|env_var|history)\b' src \
| xargs sed -i -E \
    -e 's/\bcrate::error\b/crate::domain::error/g' \
    -e 's/\bcrate::path_entry\b/crate::domain::path_entry/g' \
    -e 's/\bcrate::env_var\b/crate::domain::env_var/g' \
    -e 's/\bcrate::history\b/crate::domain::history/g' \
    -e 's/\bpath_editor_core::error\b/path_editor_core::domain::error/g' \
    -e 's/\bpath_editor_core::path_entry\b/path_editor_core::domain::path_entry/g' \
    -e 's/\bpath_editor_core::env_var\b/path_editor_core::domain::env_var/g' \
    -e 's/\bpath_editor_core::history\b/path_editor_core::domain::history/g'
```

⚠️ `src/gui/src-tauri/src/commands/history.rs` 是文件名含 `history`，但其中的 `use path_editor_core::history::` 会被正确改写；**不要**重命名这个 GUI 文件。

- [ ] **Step 6: 检查是否有遗漏的手写路径**

```bash
grep -rnE --include='*.rs' '\bcrate::(error|path_entry|env_var|history)\b' src | grep -v 'domain::'
grep -rnE --include='*.rs' '\bpath_editor_core::(error|path_entry|env_var|history)\b' src | grep -v 'domain::'
```

Expected: 两条命令都无输出。

- [ ] **Step 7: 编译验证**

```bash
cargo check --workspace --all-targets
```

Expected: `Finished` 无 error、无 warning。

- [ ] **Step 8: 提交**

```bash
git add -A
git commit -m "refactor: 抽出 core::domain 契约层

把 error / path_entry / env_var / history 收进 domain/，作为零 OS、零 IO 的
契约层。lib.rs 只导出新路径，不保留过渡别名。"
```

---

### Task 2: 抽出 `platform` 适配层

**Files:**

- Create: `src/core/src/platform/mod.rs`
- Move: `src/core/src/persist.rs` → `src/core/src/platform/persist.rs`
- Move: `src/core/src/fs.rs` → `src/core/src/platform/fs.rs`
- Move: `src/core/src/system.rs` → `src/core/src/platform/system.rs`
- Move: `src/core/src/scanner.rs` → `src/core/src/platform/scanner.rs`
- Move: `src/core/src/capabilities.rs` → `src/core/src/platform/capabilities.rs`
- Move: `src/core/src/registry/` → `src/core/src/platform/registry/`
- Move: `src/core/src/backup/` → `src/core/src/platform/backup/`
- Modify: `src/core/src/lib.rs`

**Interfaces:**

- Consumes: `crate::domain::{error, env_var, path_entry, history}`
- Produces: `crate::platform::{persist, fs, system, scanner, capabilities, registry, backup}`

- [ ] **Step 1: 移动文件**

```bash
mkdir -p src/core/src/platform
git mv src/core/src/persist.rs      src/core/src/platform/persist.rs
git mv src/core/src/fs.rs           src/core/src/platform/fs.rs
git mv src/core/src/system.rs       src/core/src/platform/system.rs
git mv src/core/src/scanner.rs      src/core/src/platform/scanner.rs
git mv src/core/src/capabilities.rs src/core/src/platform/capabilities.rs
git mv src/core/src/registry        src/core/src/platform/registry
git mv src/core/src/backup          src/core/src/platform/backup
```

⚠️ `registry/golden/` 是夹具数据目录（JSON + README），随 `registry` 整目录移动，不要单独处理。

- [ ] **Step 2: 写 `platform/mod.rs`**

```rust
//! 平台适配层：所有直接触达外部世界的实现。
//!
//! 注册表读写、管理员探测、环境广播、文件系统、备份文件都在本层。
//! `registry` 与 `backup` 互相引用（写前备份 / 备份后回读），**必须同层**——
//! 这是本仓库唯一的层内强连通分量，拆开会产生 storage ↔ platform 环。

pub mod backup;
pub mod capabilities;
pub mod fs;
pub(crate) mod persist;
pub mod registry;
pub mod scanner;
pub mod system;
```

- [ ] **Step 3: 改 `lib.rs`**

```rust
pub mod bootstrap;
pub mod db;
pub mod disabled;
pub mod domain;
pub mod platform;
pub mod service;

pub use domain::env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot, RevealedValue};
pub use domain::error::{CoreError, ErrorCode};
pub use domain::path_entry::{PathEntry, PathSnapshot};
pub use platform::backup::BackupOutcome;
pub use platform::registry::WriteOutcome;
pub use platform::scanner::{ConflictEntry, ConflictLocation, ToolGroup};
```

- [ ] **Step 4: 全仓库替换模块路径**

```bash
grep -rlE --include='*.rs' '(crate|path_editor_core)::(persist|fs|system|scanner|capabilities|registry|backup)\b' src \
| xargs sed -i -E \
    -e 's/\bcrate::persist\b/crate::platform::persist/g' \
    -e 's/\bcrate::fs\b/crate::platform::fs/g' \
    -e 's/\bcrate::system\b/crate::platform::system/g' \
    -e 's/\bcrate::scanner\b/crate::platform::scanner/g' \
    -e 's/\bcrate::capabilities\b/crate::platform::capabilities/g' \
    -e 's/\bcrate::registry\b/crate::platform::registry/g' \
    -e 's/\bcrate::backup\b/crate::platform::backup/g' \
    -e 's/\bpath_editor_core::persist\b/path_editor_core::platform::persist/g' \
    -e 's/\bpath_editor_core::fs\b/path_editor_core::platform::fs/g' \
    -e 's/\bpath_editor_core::system\b/path_editor_core::platform::system/g' \
    -e 's/\bpath_editor_core::scanner\b/path_editor_core::platform::scanner/g' \
    -e 's/\bpath_editor_core::capabilities\b/path_editor_core::platform::capabilities/g' \
    -e 's/\bpath_editor_core::registry\b/path_editor_core::platform::registry/g' \
    -e 's/\bpath_editor_core::backup\b/path_editor_core::platform::backup/g'
```

- [ ] **Step 5: 检查遗漏**

```bash
grep -rnE --include='*.rs' '\bcrate::(persist|fs|system|scanner|capabilities|registry|backup)\b' src | grep -v 'platform::'
```

Expected: 无输出。

- [ ] **Step 6: 编译验证**

```bash
cargo check --workspace --all-targets
```

Expected: `Finished` 无 error、无 warning。

- [ ] **Step 7: 提交**

```bash
git add -A
git commit -m "refactor: 抽出 core::platform 适配层

注册表、系统探测、文件系统、备份文件收进 platform/。registry 与 backup
互为强连通分量，强制同层，避免 storage <-> platform 环。"
```

---

### Task 3: 抽出 `app` 编排层

**Files:**

- Create: `src/core/src/app/mod.rs`
- Move: `src/core/src/service.rs` → `src/core/src/app/service/mod.rs`
- Move: `src/core/src/service/{profile,restore,tests}.rs` → `src/core/src/app/service/`
- Move: `src/core/src/bootstrap.rs` → `src/core/src/app/bootstrap.rs`
- Move: `src/core/src/disabled.rs` → `src/core/src/app/disabled.rs`
- Modify: `src/core/src/lib.rs`

**Interfaces:**

- Consumes: `crate::domain::*`、`crate::platform::*`、`crate::db::*`
- Produces: `crate::app::{service, bootstrap, disabled}`

- [ ] **Step 1: 移动文件**

```bash
mkdir -p src/core/src/app/service
git mv src/core/src/service.rs         src/core/src/app/service/mod.rs
git mv src/core/src/service/profile.rs src/core/src/app/service/profile.rs
git mv src/core/src/service/restore.rs src/core/src/app/service/restore.rs
git mv src/core/src/service/tests.rs   src/core/src/app/service/tests.rs
git mv src/core/src/bootstrap.rs       src/core/src/app/bootstrap.rs
git mv src/core/src/disabled.rs        src/core/src/app/disabled.rs
rmdir src/core/src/service
```

⚠️ `src/core/src/service/` 此时已空，`rmdir` 只删空目录；若报 `Directory not empty`，说明有文件漏搬，回头检查而不是强删。

- [ ] **Step 2: 写 `app/mod.rs`**

```rust
//! 应用编排层：把平台适配与持久化组合成 GUI / CLI 直接调用的用例。
//!
//! 本层是唯一允许同时依赖 `platform` 与 `db` 的地方。

pub mod bootstrap;
pub mod disabled;
pub mod service;
```

⚠️ `app/service/mod.rs` 末尾已有 `mod profile; mod restore; mod tests;` 三行（原 `service.rs:583-641`），随文件搬移，**不要改动**。

- [ ] **Step 3: 改 `lib.rs` 为最终形态**

```rust
pub mod app;
pub mod db;
pub mod domain;
pub mod platform;

pub use domain::env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot, RevealedValue};
pub use domain::error::{CoreError, ErrorCode};
pub use domain::path_entry::{PathEntry, PathSnapshot};
pub use platform::backup::BackupOutcome;
pub use platform::registry::WriteOutcome;
pub use platform::scanner::{ConflictEntry, ConflictLocation, ToolGroup};
```

- [ ] **Step 4: 全仓库替换模块路径**

```bash
grep -rlE --include='*.rs' '(crate|path_editor_core)::(service|bootstrap|disabled)\b' src \
| xargs sed -i -E \
    -e 's/\bcrate::service\b/crate::app::service/g' \
    -e 's/\bcrate::bootstrap\b/crate::app::bootstrap/g' \
    -e 's/\bcrate::disabled\b/crate::app::disabled/g' \
    -e 's/\bpath_editor_core::service\b/path_editor_core::app::service/g' \
    -e 's/\bpath_editor_core::bootstrap\b/path_editor_core::app::bootstrap/g' \
    -e 's/\bpath_editor_core::disabled\b/path_editor_core::app::disabled/g'
```

- [ ] **Step 5: 检查遗漏与顶层形状**

```bash
grep -rnE --include='*.rs' '\bcrate::(service|bootstrap|disabled)\b' src | grep -v 'app::'
ls -1 src/core/src
```

Expected: 第一条无输出；第二条恰为 `app`、`db`、`domain`、`lib.rs`、`platform` 五项。

- [ ] **Step 6: 验证依赖方向契约**

```bash
grep -rn 'crate::\(platform\|db\|app\)' src/core/src/domain
grep -rn 'crate::\(db\|app\)'            src/core/src/platform
grep -rn 'crate::app'                    src/core/src/db
```

Expected: 三条命令全部无输出。

- [ ] **Step 7: 编译验证**

```bash
cargo check --workspace --all-targets
```

Expected: `Finished` 无 error、无 warning。

- [ ] **Step 8: 提交**

```bash
git add -A
git commit -m "refactor: 抽出 core::app 编排层

service / bootstrap / disabled 收进 app/，service.rs 改为 app/service/mod.rs，
统一全 crate 的 mod.rs 约定。"
```

---

### Task 4: CLI 命令实现收进 `commands/`

**Files:**

- Create: `src/cli/src/commands/mod.rs`
- Move: `src/cli/src/path_ops.rs` → `src/cli/src/commands/path.rs`
- Move: `src/cli/src/profile_ops.rs` → `src/cli/src/commands/profile.rs`
- Move: `src/cli/src/scan_ops.rs` → `src/cli/src/commands/scan.rs`
- Move: `src/cli/src/sync_ops.rs` → `src/cli/src/commands/sync.rs`
- Move: `src/cli/src/maintenance_ops.rs` → `src/cli/src/commands/maintenance.rs`
- Move: `src/cli/src/import_export.rs` → `src/cli/src/commands/import_export.rs`
- Move: `src/cli/src/env_ops/` → `src/cli/src/commands/env/`
- Modify: `src/cli/src/main.rs`、`src/cli/src/cli.rs`、`src/cli/src/runtime.rs`

**Interfaces:**

- Consumes: `path_editor_core::app::*`、`path_editor_core::db::*`、`crate::runtime`、`crate::output`
- Produces: `crate::commands::{env, import_export, maintenance, path, profile, scan, sync}`

- [ ] **Step 1: 移动文件**

```bash
mkdir -p src/cli/src/commands
git mv src/cli/src/path_ops.rs        src/cli/src/commands/path.rs
git mv src/cli/src/profile_ops.rs     src/cli/src/commands/profile.rs
git mv src/cli/src/scan_ops.rs        src/cli/src/commands/scan.rs
git mv src/cli/src/sync_ops.rs        src/cli/src/commands/sync.rs
git mv src/cli/src/maintenance_ops.rs src/cli/src/commands/maintenance.rs
git mv src/cli/src/import_export.rs   src/cli/src/commands/import_export.rs
git mv src/cli/src/env_ops            src/cli/src/commands/env
```

- [ ] **Step 2: 写 `commands/mod.rs`**

```rust
//! CLI 子命令实现。每个文件对应一组顶层命令，只做「参数 → core 调用 → 输出」的转换。
//!
//! 本 crate 无任何安全判定逻辑：保留名、保护名单、`Unsupported` 类型、hive 写权限、
//! revision 校验全部由 `path_editor_core` 判定，这里只透传错误文本。

pub mod env;
pub mod import_export;
pub mod maintenance;
pub mod path;
pub mod profile;
pub mod scan;
pub mod sync;
```

- [ ] **Step 3: 改 `main.rs` 的模块声明**

把第 5–14 行的九条 `mod` 声明替换为四条：

```rust
mod cli;
mod commands;
mod output;
mod runtime;
```

- [ ] **Step 4: 改 `main.rs` 的裸模块引用**

```bash
grep -rlE --include='*.rs' '\b(env_ops|path_ops|profile_ops|scan_ops|sync_ops|maintenance_ops|import_export)::' src/cli \
| xargs sed -i -E \
    -e 's/\benv_ops::/commands::env::/g' \
    -e 's/\bpath_ops::/commands::path::/g' \
    -e 's/\bprofile_ops::/commands::profile::/g' \
    -e 's/\bscan_ops::/commands::scan::/g' \
    -e 's/\bsync_ops::/commands::sync::/g' \
    -e 's/\bmaintenance_ops::/commands::maintenance::/g' \
    -e 's/\bimport_export::/commands::import_export::/g'
```

⚠️ 必须先做 Step 3 再做 Step 4——否则 `mod path_ops;` 会被改成 `mod commands::path;`。

- [ ] **Step 5: 改注释里的旧模块名（三处）**

| 文件                         | 原文                                                 | 改为                                                    |
| ---------------------------- | ---------------------------------------------------- | ------------------------------------------------------- |
| `src/cli/src/cli.rs:3`       | ``命令实现见 `path_ops` / `env_ops` 等模块。``       | ``命令实现见 `commands/path.rs` / `commands/env/`。``   |
| `src/cli/src/main.rs:140`    | ``放在 `main.rs`（crate root）而非 `env_ops.rs`：…`` | ``放在 `main.rs`（crate root）而非 `commands/env/`：…`` |
| `src/cli/src/runtime.rs:253` | ``见 `env_ops.rs` 的 5 处调用``                      | ``见 `commands/env/` 的 5 处调用``                      |

- [ ] **Step 6: 检查遗漏与顶层形状**

```bash
grep -rnE --include='*.rs' '\b(env_ops|path_ops|profile_ops|scan_ops|sync_ops|maintenance_ops)\b' src/cli
ls -1 src/cli/src
```

Expected: 第一条无输出；第二条恰为 `cli.rs`、`commands`、`main.rs`、`output.rs`、`runtime.rs` 五项。

- [ ] **Step 7: 编译并跑 CLI 单测**

```bash
cargo test -p patheditor-cli --bins
```

Expected: 全部通过（`patheditor-cli` 是 bin-only crate，必须用 `--bins`，`--lib` 会报 `no library targets found`）。

- [ ] **Step 8: 提交**

```bash
git add -A
git commit -m "refactor: CLI 命令实现收进 commands/

去掉 _ops 后缀（分类已在目录上），顶层从 11 项降到 5 项。"
```

---

### Task 5: `db/backups.rs` 改名为 `db/backup_index.rs`

**Files:**

- Move: `src/core/src/db/backups.rs` → `src/core/src/db/backup_index.rs`
- Modify: `src/core/src/db/mod.rs`

**Interfaces:**

- Consumes: `crate::platform::backup`
- Produces: `crate::db::backup_index`

- [ ] **Step 1: 确认候引用点**

```bash
grep -rnE --include='*.rs' '\bbackups\b' src/core/src/db src/core/tests src/gui src/cli
```

Expected: 命中 `db/mod.rs` 的 `pub mod backups;`，以及 `db::backups` / `backups::` 形式的引用。记下所有非 `backup`（单数）的命中。

- [ ] **Step 2: 移动并改声明**

```bash
git mv src/core/src/db/backups.rs src/core/src/db/backup_index.rs
sed -i 's/^pub mod backups;$/pub mod backup_index;/' src/core/src/db/mod.rs
```

- [ ] **Step 3: 替换限定路径**

```bash
grep -rlE --include='*.rs' '\bdb::backups\b' src | xargs sed -i -E 's/\bdb::backups\b/db::backup_index/g'
```

- [ ] **Step 4: 处理裸 `backups::` 引用**

```bash
grep -rnE --include='*.rs' '(^|[^:_])backups::' src/core/src/db src/core/tests
```

Expected: 若无输出则本步完成。若有输出，逐条改成 `backup_index::`——**注意不要**误改 `crate::platform::backup::`（单数）。

- [ ] **Step 5: 编译验证**

```bash
cargo check --workspace --all-targets
```

Expected: `Finished` 无 error、无 warning。

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "refactor: db::backups 改名为 db::backup_index

与 platform::backup（磁盘备份文件）区分：前者是数据库里的备份索引表。"
```

---

### Task 6: 同步注释与当前态文档

**Files:**

- Modify: `src/core/src/**/*.rs`（只改注释里的路径文本）
- Modify: `src/core/src/platform/registry/golden/README.md`
- Modify: `docs/dev/architecture.md`
- Modify: `README.md`
- Modify: `CLAUDE.md`、`AGENTS.md`

**Interfaces:**

- Consumes: Task 1–5 的最终目录形状
- Produces: 无（本任务不改代码逻辑）

- [ ] **Step 1: 改源码注释里的旧路径**

```bash
grep -rlE --include='*.rs' 'core/src/(error|path_entry|env_var|history|persist|fs|system|scanner|capabilities|registry|backup|service|bootstrap|disabled)\.rs' src \
| xargs sed -i -E \
    -e 's|core/src/error\.rs|core/src/domain/error.rs|g' \
    -e 's|core/src/path_entry\.rs|core/src/domain/path_entry.rs|g' \
    -e 's|core/src/env_var\.rs|core/src/domain/env_var.rs|g' \
    -e 's|core/src/history\.rs|core/src/domain/history.rs|g' \
    -e 's|core/src/persist\.rs|core/src/platform/persist.rs|g' \
    -e 's|core/src/fs\.rs|core/src/platform/fs.rs|g' \
    -e 's|core/src/system\.rs|core/src/platform/system.rs|g' \
    -e 's|core/src/scanner\.rs|core/src/platform/scanner.rs|g' \
    -e 's|core/src/capabilities\.rs|core/src/platform/capabilities.rs|g' \
    -e 's|core/src/registry|core/src/platform/registry|g' \
    -e 's|core/src/backup\.rs|core/src/platform/backup/mod.rs|g' \
    -e 's|core/src/backup/|core/src/platform/backup/|g' \
    -e 's|core/src/service\.rs|core/src/app/service/mod.rs|g' \
    -e 's|core/src/bootstrap\.rs|core/src/app/bootstrap.rs|g' \
    -e 's|core/src/disabled\.rs|core/src/app/disabled.rs|g'
```

⚠️ 顺序有意义：`core/src/backup.rs` 必须先于 `core/src/backup/` 处理（两条例外规则的先后已在命令里固化）。`core/src/registry` 一条覆盖 `registry/xxx.rs` 与 `registry.rs` 两种写法。

⚠️ **行号不予校正**：注释里形如 `` `core/src/env_var.rs:127` `` 的行号在本轮之前就可能已漂移。本任务只改路径前缀，不动行号——逐条核对行号属于另一轮注释审计，混进来会让本次 diff 无法用「纯移动」来审。

- [ ] **Step 2: 改 `registry/golden/README.md` 里的路径**

```bash
sed -i -E \
  -e 's|core/src/registry/|core/src/platform/registry/|g' \
  -e 's|core/src/backup\.rs|core/src/platform/backup/mod.rs|g' \
  -e 's|core/src/service\.rs|core/src/app/service/mod.rs|g' \
  src/core/src/platform/registry/golden/README.md
```

- [ ] **Step 3: 重写 `docs/dev/architecture.md` 的「完整目录树」块**

把第 6–68 行的目录树替换为下列内容（保留其后的「模块边界速记」「数据流」两节不变）：

```text
PathEditor/
├── src/
│   ├── core/src/                # Rust 核心库（零 Tauri 依赖）
│   │   ├── domain/              # 契约与端口：零 OS、零 IO、零数据库
│   │   │   ├── error.rs         # CoreError / ErrorCode（含 exit_code）
│   │   │   ├── path_entry.rs    # PathEntry / PathSnapshot
│   │   │   ├── env_var.rs       # 通用环境变量契约 + 保留/保护/敏感判定
│   │   │   └── history.rs       # HistorySink 端口 + EventSource
│   │   ├── platform/            # 外部世界适配
│   │   │   ├── persist.rs       # 版本化文件信封、轮换、损坏隔离
│   │   │   ├── fs.rs            # 受限文件读取、导入导出
│   │   │   ├── system.rs        # 权限、变量展开、环境广播
│   │   │   ├── scanner.rs       # scan_paths 单次枚举、冲突与工具清单
│   │   │   ├── capabilities.rs  # 按 hive 探测 PathCapabilities
│   │   │   ├── registry/        # 注册表访问统一入口（path/access/env_edit/conflict/store）
│   │   │   └── backup/          # 注册表备份、env 备份（.pebak）
│   │   ├── db/                  # v6 SQLCipher 数据层
│   │   │   ├── mod.rs           # Db 句柄、事务、open_default / open_and_migrate
│   │   │   ├── schema.rs        # DDL v1/v2/v3；CURRENT_SCHEMA_VERSION = 3
│   │   │   ├── crypto.rs / dpapi.rs / dacl.rs   # 安全边界：密钥、DPAPI、DACL
│   │   │   ├── history.rs / history_sink.rs      # 快照、事件、HistorySink 实现
│   │   │   ├── profiles.rs      # 配置元数据/版本（DB 版）
│   │   │   ├── backup_index.rs  # 备份索引表（磁盘文件在 platform/backup/）
│   │   │   ├── search.rs / stats.rs / refs.rs / compare.rs / scan_history.rs
│   │   │   ├── maintenance.rs   # 保留策略 + 遗留明文清理
│   │   │   └── sync/            # bundle / change_log / merge / conflict / export / import
│   │   ├── app/                 # 编排：唯一同时依赖 platform 与 db 的层
│   │   │   ├── bootstrap.rs     # 启动引导序列
│   │   │   ├── disabled.rs      # 快照合并、pending 磁盘文件
│   │   │   └── service/         # 保存编排：save_path_with_db / save_path_registry_only
│   │   └── lib.rs               # 四层声明 + 契约再导出
│   ├── cli/src/                 # bin-only crate，无 lib target
│   │   ├── main.rs              # 入口 + 命令分派
│   │   ├── cli.rs               # clap 定义
│   │   ├── runtime.rs           # AppContext、错误退出码、快照提交
│   │   ├── output.rs            # JSON / 文本输出出口
│   │   └── commands/
│   │       ├── path.rs          # PATH 子命令
│   │       ├── profile.rs       # profile 子命令
│   │       ├── scan.rs          # 冲突、扫描、权限检查
│   │       ├── sync.rs          # sync 子命令（口令隐藏输入）
│   │       ├── maintenance.rs   # maintenance 子命令
│   │       ├── import_export.rs # 导入导出命令
│   │       └── env/             # env 子命令（cmd / render / value / tests）
│   └── gui/
│       ├── src-tauri/src/commands/ # #[tauri::command] 薄包装，调用 core
│       └── web/                    # React + Vite
```

同时在该节末尾补一段依赖方向说明：

```markdown
层间依赖只允许往下走：

    app → db → platform → domain
     └──────────↗

`platform` 内部 `registry` ↔ `backup` 互相引用（写前备份 / 备份后回读），
是唯一的层内强连通分量，因此两者必须同层。
```

- [ ] **Step 4: 改 `README.md` 的「项目结构」代码块**

把 `src/core/` 下的条目按第 3 步的四层结构改写；`src/cli/` 下的 `cli / path_ops / runtime / env_ops / …` 一行改为：

```text
├── cli/                      # 命令行工具（bin 产物 patheditor.exe）
│   └── src/                  # main / cli（clap）/ runtime / output
│                             #   commands/（path / profile / scan / sync /
│                             #     maintenance / import_export / env）
```

- [ ] **Step 5: 同步 `CLAUDE.md` 与 `AGENTS.md`**

两份文件的「架构速览」代码块与「关键约束」中出现的模块路径，按同一份新内容改；改完执行：

```bash
cmp CLAUDE.md AGENTS.md && echo "IDENTICAL"
```

Expected: 输出 `IDENTICAL`。若 `cmp` 有输出，把 `AGENTS.md` 覆盖为 `CLAUDE.md` 的内容后重跑。

- [ ] **Step 6: 检查当前态文档不再含旧路径**

```bash
grep -rnE 'src/core/src/(error|path_entry|env_var|history|persist|fs|system|scanner|capabilities|registry|backup|service|bootstrap|disabled)\.rs' src docs/dev README.md CLAUDE.md AGENTS.md
```

Expected: 无输出。

- [ ] **Step 7: 编译与格式验证**

```bash
cargo fmt --check
cargo check --workspace --all-targets
```

Expected: 两条都通过。若有格式差异，`cargo fmt` 后重跑（只应涉及注释重排）。

- [ ] **Step 8: 提交**

```bash
git add -A
git commit -m "docs: 同步模块分层后的目录树与路径引用

architecture.md / README / CLAUDE.md / AGENTS.md 与源码注释统一到四层布局。
顺带修正既有漂移：registry/env_var.rs 实为 env_edit.rs，backup.rs 已拆为目录。"
```

---

### Task 7: 全量质量门

**Files:**

- 无改动；只做验证

**Interfaces:**

- Consumes: Task 1–6 的全部成果
- Produces: 可复核的绿灯证据

- [ ] **Step 1: Rust 质量门**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p patheditor-cli --bins
```

Expected:

- `fmt` 无输出
- `clippy` 无 warning（`-D warnings` 下若有 warning 即为失败）
- `cargo test --workspace` → 324 passed / 0 failed / 2 ignored（与 Task 1 Step 1 基线一致）
- CLI 单测全通过

- [ ] **Step 2: 前端质量门（确认未被波及）**

```bash
npm run verify
```

Expected: 通过；Vitest 37 files / 413 passed。

- [ ] **Step 3: 结构与依赖契约复核**

```bash
ls -1 src/core/src
ls -1 src/cli/src
grep -rn 'crate::\(platform\|db\|app\)' src/core/src/domain
grep -rn 'crate::\(db\|app\)'            src/core/src/platform
grep -rn 'crate::app'                    src/core/src/db
cmp CLAUDE.md AGENTS.md && echo IDENTICAL
```

Expected: 前两条各输出 5 项（见验收标准）；中间三条无输出；最后一条输出 `IDENTICAL`。

- [ ] **Step 4: 确认没有残留的空目录与未跟踪文件**

```bash
git status --porcelain
find src -type d -empty
```

Expected: 两条都无输出。

- [ ] **Step 5: 把结果写进开发回执**

在 `docs/审核和开发/2026.10.28/PathEditor-module-layering开发回执.md` 中逐项记录 Step 1–4 的原始输出，并按交接规范列出「未覆盖项」与「偏离记录」。

---

## Self-Review

### 规格覆盖

逐条对照 spec 的验收标准：

| spec 验收项                           | 落在哪个 Task                                                                  |
| ------------------------------------- | ------------------------------------------------------------------------------ |
| `ls -1 src/core/src` 恰为 5 项        | Task 3 Step 5、Task 7 Step 3                                                   |
| `ls -1 src/cli/src` 恰为 5 项         | Task 4 Step 6、Task 7 Step 3                                                   |
| 三条依赖方向检查无输出                | Task 3 Step 6、Task 7 Step 3                                                   |
| `cargo fmt --check`                   | Task 6 Step 7、Task 7 Step 1                                                   |
| `cargo clippy -D warnings`            | Task 7 Step 1                                                                  |
| `cargo test --workspace` 324/2        | Task 1 Step 1（基线）、Task 7 Step 1                                           |
| `cargo test -p patheditor-cli --bins` | Task 4 Step 7、Task 7 Step 1                                                   |
| `npm run verify`                      | Task 7 Step 2                                                                  |
| `cmp CLAUDE.md AGENTS.md`             | Task 6 Step 5、Task 7 Step 3                                                   |
| 旧路径检查无输出                      | Task 6 Step 6                                                                  |
| 无残留空目录                          | Task 3 Step 1（`rmdir`）、Task 7 Step 4                                        |
| 映射表 16 条路径                      | Task 1（4 条）、Task 2（7 条）、Task 3（3 条）、Task 4（7 条）、Task 5（1 条） |
| D4 不做 `db/security/`                | 无对应 Task——刻意不做                                                          |
| D9 历史文档不改写                     | Task 6 的 grep 白名单不含 `docs/superpowers` 与 `docs/审核和开发`              |

### 占位符扫描

本计划不含 `TBD` / `TODO` / 「适当处理」/「参考 Task N」。每个 sed 命令、每个新建文件内容都写全。

### 类型一致性

本计划只改路径，不改任何类型签名、函数名或参数顺序。核对的对应关系是**模块路径**：Task 1–5 产出的 `crate::domain::*` / `crate::platform::*` / `crate::app::*` / `crate::db::backup_index` / `crate::commands::*` 与 spec 第 3.4 节映射表逐条一致。

## Execution Notes

> 留给开发窗口回填偏离记录：哪条命令的输出与预期不符、怎么处置的、是否改了计划。

- （待填）
  </content>
