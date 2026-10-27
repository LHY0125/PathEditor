# PathEditor 模块分层重组 — 设计文档

- **日期**：2026-10-28
- **作者**：审核窗口
- **状态**：待开发窗口核对
- **范围**：`src/core/src/`、`src/cli/src/`，以及描述目录结构的当前态文档
- **目标版本**：v6.2.0（纯重构，无新特性）

## 一、背景与目标

### 1.1 背景

`src/core/src/` 顶层并列 17 项（13 个 `.rs` + 4 个目录），`src/cli/src/` 顶层并列 11 项。打开目录后无法从文件名判断某个模块属于「契约」「平台访问」「持久化」还是「编排」，必须逐个点开。

实测规模：

| 位置                 | 并列项 | 行数                      |
| -------------------- | ------ | ------------------------- |
| `src/core/src/` 顶层 | 17     | 16,390（含 1,996 行测试） |
| `src/core/src/db/`   | 17     | ~5,100                    |
| `src/cli/src/` 顶层  | 11     | 3,558（含 1,281 行测试）  |

触发本次重组的三个具体缺陷（比「并列太多」更实）：

1. **`history` 三重同名**：`core/src/history.rs`（131 行，`HistorySink` 端口）、`core/src/db/history.rs`（719 行，历史表）、`core/src/db/history_sink.rs`（108 行，端口实现）。
2. **`backup/` 与 `db/backups.rs` 指两个东西**：前者是磁盘备份文件，后者是数据库里的备份索引表。
3. **两套 mod 约定并存**：`backup/`、`db/`、`registry/` 用 `mod.rs`，`service` 用 `service.rs` + `service/`。

### 1.2 目标

- `src/core/src/` 顶层降为 `lib.rs` + 4 个分层目录。
- `src/cli/src/` 顶层降为 4 个基础设施文件 + `commands/`。
- 层间依赖方向成为**可机械校验的契约**，不再依赖读者记忆。
- 同一层内不再出现同名模块。

### 1.3 完成判定（可机械验证）

```bash
# 1. core 顶层只剩 lib.rs 与 4 个目录
ls -1 src/core/src
# 期望：app  db  domain  lib.rs  platform

# 2. cli 顶层只剩 4 个 .rs 与 commands
ls -1 src/cli/src
# 期望：cli.rs  commands  main.rs  output.rs  runtime.rs

# 3. 依赖方向零违反（三条命令都必须无输出）
grep -rn 'crate::\(platform\|db\|app\)' src/core/src/domain
grep -rn 'crate::\(db\|app\)'            src/core/src/platform
grep -rn 'crate::app'                    src/core/src/db

# 4. 当前态文档中不再出现旧路径
grep -rnE 'src/core/src/(error|path_entry|env_var|history|persist|fs|system|scanner|capabilities|registry|backup|service|bootstrap|disabled)\.rs' src docs/dev README.md CLAUDE.md AGENTS.md
# 期望：无输出（历史文档 docs/superpowers/{plans,specs} 与 docs/审核和开发 不在检查范围）
```

## 二、现状证据：模块依赖矩阵

以下是改造前实测的出边（`A → B` 表示 `A` 的源码里出现 `use crate::B`）：

```text
env_var, path_entry, system, scanner     → （无）
error                                    → env_var
history                                  → env_var, error
persist                                  → error
fs                                       → error, path_entry
db/*                                     → backup, persist, env_var, error
capabilities                             → registry, system
registry/*                               → backup, env_var, error, path_entry
backup/*                                 → registry, persist, env_var, error
disabled                                 → registry, persist, fs, error
service                                  → registry, system, backup, disabled, db, history, env_var, error, path_entry
bootstrap                                → service, db, history
```

### 2.1 关键发现：`registry` 与 `backup` 是唯一强连通分量

- `registry/env_edit.rs:2` → `use crate::backup::{self, backup_env_vars, BackupOutcome};`（写前备份）
- `backup/{env,path,restore,tests}.rs` → `use crate::registry::…`（读注册表取值）

两者互相引用，无法拆到不同层。**任何把 `registry` 放 A 层、`backup` 放 B 层的方案都会产生环。** 这是本次四层划分的硬约束，不是偏好。

### 2.2 关键发现：`db` 是引用热点，路径不宜加长

| 路径                            | 出现次数 |
| ------------------------------- | -------- |
| `path_editor_core::db::`        | 50       |
| `path_editor_core::db::crypto`  | 43       |
| `path_editor_core::db::sync`    | 39       |
| `path_editor_core::db::history` | 37       |
| `crate::error::`                | 48       |

`db::` 已是「持久化」这一层的完整语义，再套一层 `storage/` 只会让 50 处引用变长而不增加信息。

## 三、目标布局

### 3.1 core 四层

```text
src/core/src/
├── lib.rs
├── domain/                    # 契约与端口：零 OS、零 IO、零数据库
│   ├── mod.rs
│   ├── error.rs               # CoreError / ErrorCode（含 exit_code）
│   ├── path_entry.rs          # PathEntry / PathSnapshot
│   ├── env_var.rs             # 通用环境变量契约 + 保留/保护/敏感判定
│   └── history.rs             # HistorySink 端口 + EventSource
├── platform/                  # 外部世界适配：Windows 注册表 / 系统 API / 文件系统 / 备份文件
│   ├── mod.rs
│   ├── persist.rs             # 版本化文件信封、轮换、损坏隔离
│   ├── fs.rs                  # 导入导出解析 + 受限文件读写
│   ├── system.rs              # 权限探测、变量展开、环境广播
│   ├── scanner.rs             # 单次目录枚举、冲突与工具清单
│   ├── capabilities.rs        # 按 hive 探测 PathCapabilities
│   ├── registry/              # 原样搬移（含 golden/ 夹具目录）
│   └── backup/                # 原样搬移
├── db/                        # SQLCipher 数据层（路径不变）
│   ├── mod.rs  schema.rs  derived.rs
│   ├── crypto.rs  dpapi.rs  dacl.rs
│   ├── history.rs  history_sink.rs  profiles.rs  search.rs  stats.rs
│   ├── refs.rs  compare.rs  scan_history.rs  maintenance.rs
│   ├── backup_index.rs        # 原 backups.rs，改名消除与 platform/backup/ 的歧义
│   └── sync/
└── app/                       # 编排：把 platform 与 db 组合成 GUI/CLI 直接调用的用例
    ├── mod.rs
    ├── bootstrap.rs
    ├── disabled.rs
    └── service/
        ├── mod.rs             # 原 service.rs
        ├── profile.rs  restore.rs  tests.rs
```

### 3.2 cli

```text
src/cli/src/
├── main.rs                    # 入口 + 命令分派
├── cli.rs                     # clap 定义
├── runtime.rs                 # AppContext、错误→退出码
├── output.rs                  # JSON/文本输出
└── commands/
    ├── mod.rs
    ├── path.rs                # 原 path_ops.rs
    ├── profile.rs             # 原 profile_ops.rs
    ├── scan.rs                # 原 scan_ops.rs
    ├── sync.rs                # 原 sync_ops.rs
    ├── maintenance.rs         # 原 maintenance_ops.rs
    ├── import_export.rs
    └── env/                   # 原 env_ops/（cmd / render / value / tests）
```

### 3.3 依赖方向契约

```text
app  →  db  →  platform  →  domain
 └──────────────↗
```

- `domain` 不得引用 `platform` / `db` / `app`。
- `platform` 不得引用 `db` / `app`。
- `db` 不得引用 `app`。
- `app` 可以引用全部。
- `platform` 内部 `registry` ↔ `backup` 的互相引用是允许的（同一强连通分量）。

### 3.4 模块路径映射表

| 现路径                           | 新路径                                  |
| -------------------------------- | --------------------------------------- |
| `src/core/src/error.rs`          | `src/core/src/domain/error.rs`          |
| `src/core/src/path_entry.rs`     | `src/core/src/domain/path_entry.rs`     |
| `src/core/src/env_var.rs`        | `src/core/src/domain/env_var.rs`        |
| `src/core/src/history.rs`        | `src/core/src/domain/history.rs`        |
| `src/core/src/persist.rs`        | `src/core/src/platform/persist.rs`      |
| `src/core/src/fs.rs`             | `src/core/src/platform/fs.rs`           |
| `src/core/src/system.rs`         | `src/core/src/platform/system.rs`       |
| `src/core/src/scanner.rs`        | `src/core/src/platform/scanner.rs`      |
| `src/core/src/capabilities.rs`   | `src/core/src/platform/capabilities.rs` |
| `src/core/src/registry/**`       | `src/core/src/platform/registry/**`     |
| `src/core/src/backup/**`         | `src/core/src/platform/backup/**`       |
| `src/core/src/service.rs`        | `src/core/src/app/service/mod.rs`       |
| `src/core/src/service/*.rs`      | `src/core/src/app/service/*.rs`         |
| `src/core/src/bootstrap.rs`      | `src/core/src/app/bootstrap.rs`         |
| `src/core/src/disabled.rs`       | `src/core/src/app/disabled.rs`          |
| `src/core/src/db/backups.rs`     | `src/core/src/db/backup_index.rs`       |
| `src/core/src/db/**`             | 不变                                    |
| `src/cli/src/path_ops.rs`        | `src/cli/src/commands/path.rs`          |
| `src/cli/src/profile_ops.rs`     | `src/cli/src/commands/profile.rs`       |
| `src/cli/src/scan_ops.rs`        | `src/cli/src/commands/scan.rs`          |
| `src/cli/src/sync_ops.rs`        | `src/cli/src/commands/sync.rs`          |
| `src/cli/src/maintenance_ops.rs` | `src/cli/src/commands/maintenance.rs`   |
| `src/cli/src/import_export.rs`   | `src/cli/src/commands/import_export.rs` |
| `src/cli/src/env_ops/**`         | `src/cli/src/commands/env/**`           |

对应的 Rust 路径：`crate::<旧>` → `crate::<新>`，`path_editor_core::<旧>` → `path_editor_core::<新>`；`crate::db::backups` → `crate::db::backup_index`。

## 四、关键设计决策

### D1：四层定为 `domain / platform / db / app`，不设 `storage/`

- **选了什么**：`db/` 保持在顶层，与另外三层并列。
- **为什么**：`db::` 已是「持久化」的完整语义；把 `db/` 塞进 `storage/db/` 会让 50 处 `path_editor_core::db::`、43 处 `db::crypto`、39 处 `db::sync` 全部变长，换来的只是「四层名字更整齐」。
- **放弃了什么**：`domain / platform / storage / app` 的对称命名。四层的对称性让位于「最少无谓改动」。

### D2：`registry/` 与 `backup/` 同放 `platform/`

- **选了什么**：两个目录一起进 `platform/`。
- **为什么**：见 2.1，二者互为强连通分量。若把 `backup/` 当「存储」单独分层，`platform → storage` 与 `storage → platform` 会同时成立。
- **放弃了什么**：按「注册表 vs 文件」把两者拆开的直觉分类。

### D3：`persist.rs` 进 `platform/`，不新建第五层

- **选了什么**：`persist.rs` 与 `fs.rs` 同放 `platform/`。
- **为什么**：`backup` → `persist`（备份文件用版本化信封），所以 `persist` 必须在 `backup` 之下；`persist` 只依赖 `error`，放 `domain` 也可编译，但它做的是文件 IO，放进「零 IO 的契约层」名不副实。
- **放弃了什么**：`primitive/` 第五层。该层只装得下两个文件，属于概念税。

### D4：不做 `db/security/` 分组

- **选了什么**：`crypto.rs` / `dpapi.rs` / `dacl.rs` 留在 `db/` 顶层，改为在 `db/mod.rs` 顶部用文档注释标注安全边界文件清单。
- **为什么**：初稿曾建议把这 3 个文件收进 `db/security/`（安全审查工作面）。实测 `path_editor_core::db::crypto` 出现 43 次、`crate::db::crypto` 12 次，路径变长的成本远超「3 个文件相邻」带来的导航收益。
- **放弃了什么**：`db/` 从 17 项降到 14 项的观感改善。

### D5：`mod.rs` 统一约定

- **选了什么**：所有目录一律用 `<dir>/mod.rs`；`service.rs` → `app/service/mod.rs`。
- **为什么**：现状已有 3 个目录用 `mod.rs`，只有 `service` 例外。改为多数派成本最低；`backup/mod.rs` 本来就承载 `BackupOutcome` 等实体，`mod.rs` 放代码在本仓库已是既成风格。
- **放弃了什么**：Rust 2018 之后的 `foo.rs + foo/` 写法。它会迫使 `backup/`、`db/`、`registry/` 三个目录同时改名，收益仅是少一个 `mod.rs`。

### D6：同名规则——同层禁止，跨层允许

- **选了什么**：同一层内不得出现同名模块；跨层同名允许，因为全路径唯一。因此保留 `domain/history.rs`（端口）、`db/history.rs`（表）、`db/history_sink.rs`（实现）。
- **为什么**：`history_sink` 的实现必须有一个表级模块与之并列，强行改名会破坏 `db/` 下「一表一模块」的命名一致性。层级前缀已足够消除歧义。
- **例外**：`db/backups.rs` → `db/backup_index.rs`。这不是同名，而是**同义词指两物**（磁盘备份 vs 索引表），改名才是正确处置。

### D7：不保留过渡别名

- **选了什么**：`lib.rs` 只导出新路径，不做 `pub use platform::registry as registry;` 之类的旧路径别名。
- **为什么**：72 个文件引用 `path_editor_core::`，别名会让两套路径长期并存；仓库已有「清理过渡别名」的先例（v6.1.1 的对话框重皮任务）。一次性机械替换后，下一次重构不必再改一遍。
- **放弃了什么**：分批迁移、逐步收口的方案。代价是单个提交的 diff 较大。

### D8：CLI 用 `commands/` 取代 `_ops` 后缀

- **选了什么**：6 个 `*_ops.rs` 与 `env_ops/` 进 `commands/`，去掉 `_ops` 后缀。
- **为什么**：`_ops` 后缀本身已经声明了分类，只是载体错了（写在文件名里而非目录上）。搬进 `commands/` 后 `path` 已无歧义，后缀是冗余。
- **放弃了什么**：给 CLI 再往下分层。全 crate 3,558 行、其中 1,281 行是测试，再多一层就是概念税。

### D9：历史文档不回溯改写

- **选了什么**：只改当前态文档（`docs/dev/*.md`、`README.md`、`CLAUDE.md`/`AGENTS.md`、源码注释）。
- **为什么**：`docs/superpowers/plans/`（146 处旧路径）、`docs/superpowers/specs/`（51 处）、`docs/审核和开发/**`（约 130 处）是当时决策的记录。改写会让历史文档与当时的提交对不上。
- **放弃了什么**：全仓库路径一致性。

## 五、安全边界

本次是**纯移动 + 改引用**，零行为变更。以下必须保持不变：

- 不产生任何注册表写入；不打开真实 `~/.patheditor`。
- 不改 DB schema、不改同步包格式、不改 CLI 退出码（0/1/2/3/4/5）。
- 不改 `core` 的任何业务判定（保留名、保护名单、敏感规则、revision 校验）。
- CLI 侧仍然零安全判定逻辑——`commands/` 只做参数转换与错误呈现。
- `src/gui/web/src/core/` 与 `services/backend.ts` 不动。

验证手段：改动前后 `cargo test --workspace` 的通过数必须一致（324 passed / 2 ignored），`npm run verify` 的前端通过数必须一致（37 files / 413）。

## 六、验收标准

- [ ] `ls -1 src/core/src` 输出恰为 `app  db  domain  lib.rs  platform`。
- [ ] `ls -1 src/cli/src` 输出恰为 `cli.rs  commands  main.rs  output.rs  runtime.rs`。
- [ ] 1.3 节的三条依赖方向检查全部无输出。
- [ ] `cargo fmt --check` 通过。
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- [ ] `cargo test --workspace` 为 324 passed / 0 failed / 2 ignored。
- [ ] `cargo test -p patheditor-cli --bins` 通过。
- [ ] `npm run verify` 通过（前端 37 files / 413）。
- [ ] `cmp CLAUDE.md AGENTS.md` 无输出（字节级一致）。
- [ ] 1.3 节第 4 条旧路径检查无输出。
- [ ] `git status --porcelain` 中没有未跟踪的残留目录（如空的 `src/core/src/service/`）。

## 七、不在本版范围

- **`db/` 内部再分组**（`db/security/`、`db/tables/`）：见 D4。
- **`src/gui/` 的任何结构调整**。`gui/src-tauri/src/commands/` 保持现名——它已经是「薄包装」这一层的名字，且 Tauri 的 `invoke_handler` 列表不因目录变化而变。
- **`src/gui/web/src/` 的重组**。本轮只动 Rust。
- **分层校验进 CI**：本次只在验收标准里保留 grep 命令。若要长期防侵蚀，应另开一轮加 `.github/workflows` 步骤或 `cargo xtask`。
- **消除 `registry` ↔ `backup` 的相互依赖**：需要把「写前备份」改成调用方注入的回调，属于行为契约变更，不在纯重构范围内。
- **合并 `db/history_sink.rs` 到 `db/history.rs`**：108 行的独立适配器，合并会破坏「一文件一关注点」。

## 八、风险与未决项

### 8.1 风险

| 风险                                                                                                      | 影响                                  | 缓解                                                               |
| --------------------------------------------------------------------------------------------------------- | ------------------------------------- | ------------------------------------------------------------------ |
| 机械替换误伤同名前缀（如 `crate::error` 命中 `crate::error_xxx`）                                         | 编译失败或静默改错                    | 全部替换用 `\b` 词边界；每步后立刻 `cargo check`                   |
| `git mv` 后遗漏 `mod` 声明，`git` 只当新增+删除                                                           | 历史丢失、审查困难                    | 全程 `git mv`，提交前 `git status` 应显示 `renamed:`               |
| 工作区已有 5 个未提交的文档改动（`AGENTS.md`/`CLAUDE.md`/`README.md`/`ROADMAP.md`/`docs/dev/testing.md`） | 任务 6 会在其上叠加修改，混合两类改动 | **已解除**：这 5 个文件已于 `4734188` 提交到 `main`，分支已 rebase |
| 历史文档被误改                                                                                            | 记录失真                              | D9 明确排除；任务 6 的 sed 只对白名单路径执行                      |

### 8.2 未决项

- 无。四层划分、CLI 布局、改名范围均已在 D1–D9 定案。

## 九、文档同步清单

| 文件                                              | 动作                                                                                                             |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `docs/dev/architecture.md`                        | 重写「完整目录树」块；顺带修正既有漂移（`registry/env_var.rs` 实为 `env_edit.rs`；`backup.rs` 已拆成 `backup/`） |
| `README.md`                                       | 「项目结构」代码块同步四层                                                                                       |
| `CLAUDE.md`                                       | 「架构速览」目录树 + 「关键约束」中提到的模块路径                                                                |
| `AGENTS.md`                                       | 与 `CLAUDE.md` 字节级一致（`cmp` 验证）                                                                          |
| `src/core/src/*/` 注释                            | 形如 `` `core/src/backup.rs` `` 的路径引用改为新路径                                                             |
| `src/core/src/platform/registry/golden/README.md` | 同上                                                                                                             |
| `docs/dev/interfaces.md`                          | **无需改动**（已 grep 确认不含旧模块路径）                                                                       |
| `docs/superpowers/**`、`docs/审核和开发/**`       | **不改**                                                                                                         |

## 十、核对轮裁断（2026-10-28）

开发窗口提出 6 条（B1、B2、H1、F1、F2、M1），审核窗口逐条复跑后**全部采纳**，无驳回。审核窗口另补 3 条自查项（E1、E2、E3）。

**本节的裁断全部落在实施计划层面（命令、行号、正则、步骤），spec 的 D1–D9 与验收标准没有变化。** 明细见：

- 实施计划 `## 修订记录`（v2 九条逐项对照表）
- `docs/审核和开发/2026.10.28/PathEditor-module-layering核对轮交接.md` 第 11 节（含实测命令与输出）

三条需要留档的结论：

1. **D4（不做 `db/security/`）不变**。F2 讨论的是 `platform/registry/golden/README.md` 的既有漂移，与 D4 无关。
2. **D9（历史文档不改）扩展一条**：`golden/README.md` 里提到的 `core/src/registry/golden/merge_golden_tests.rs` 在仓库中不存在，属另一处既有漂移，本轮同样不改。
3. **新增全局约束**：`sed` 不得使用 `$` 行尾锚点（CRLF 下永不匹配），且每个任务提交前要核对 `w/crlf` 计数未下降。这条已写进计划的 Global Constraints，因为它防的是「改一行变成全文件重写」——那会让本轮失去「纯移动」的可审性。
