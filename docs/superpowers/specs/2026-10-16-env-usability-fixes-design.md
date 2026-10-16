# env 子命令可用性收口 — 设计文档

- **日期**：2026-10-16
- **版本目标**：5.1.5
- **作者**：审核窗口
- **状态**：待开发窗口逐行核对
- **来源**：GitHub issues #33 / #34 / #35 / #36 / #37 / #38，逐条经代码级核实（核实结论见各 issue 的评论）

## 概述

`env` 子命令自 5.1.4 交付以来，用户在真实使用中登记了 7 条 issue。审核窗口对 7 条逐一做了代码级核实，结论分三类：

1. **描述有误，不需改代码**：#38（省略 `VALUE` 会立即报错，不阻塞）—— 只需补 help 说明并关闭 issue。
2. **文案与行为不符，改文案即可**：#33（`list` 的「不含明文」措辞绝对化）、#35（`add` 的纯新建语义未写进 help）。
3. **真实缺陷，需改代码**：#34（`env get` 泄漏 Win32 原生错误串）、#36（`env set/remove/add` 缺 dry-run）、#37（`path_backup_*` 无保留策略，无限累积）。

本版把这 6 条一次收口。**#32（批量往返）不在本版**，它需要跨命令 session，属架构变动，另开 spec。

**为什么这批要合并成一版**：6 条里 4 条是 `env` 子命令的**对外契约**（help 文案、错误语义、预览能力），单独发布会产生 4 次「改文案 → 跑全量质量门 → 发版」的重复开销；合起来一次把 `env` 子命令的契约收敛到自洽状态，且总量仍属小改（无架构变动）。

## 背景与现状（已核实的代码事实）

| 事实                                                                                       | 位置                                                     | 影响                                     |
| ------------------------------------------------------------------------------------------ | -------------------------------------------------------- | ---------------------------------------- |
| `sanitize_preview` **已实现截断**（超 `PREVIEW_MAX_CHARS` 加 `…`）                         | `core/src/env_var.rs:197-211`                            | #33 的方向 2「加截断」**已实现**，不需做 |
| 敏感判定命中则 `preview = None`，CLI 渲染 `(敏感)`                                         | `core/src/env_var.rs:294, 312-316`、`env_ops.rs:126-131` | 分层正确，只有 help 文案绝对化（#33）    |
| `read_env_var` 把 `store.get_raw` 的错误**原样**塞进 message，未映射 `NotFound`            | `core/src/registry/env_var.rs:157-159`                   | #34 根因                                 |
| `reg_store::get_raw` 把 `io::Error` 降级成 `String`，**丢失 `ErrorKind`**                  | `core/src/reg_store.rs:100-104`                          | #34 的深层原因（上层无从判 NotFound）    |
| `ErrorCode::NotFound` **已存在**且 `profiles.rs` 在用                                      | `core/src/error.rs:23`、`profiles.rs:208/248`            | #34 是遗漏而非设计，修法明确             |
| `Add` 结构体**无任何 revision/force 字段**，`create_env_var` 拒绝同名（`NameExists`）      | `cli/src/main.rs:206-219`、`registry/env_var.rs:583-591` | #35：纯新建语义成立，只是没写进 help     |
| `resolve_value(…, required=true)` 零通道时**立即 `exit_err`**，不读 stdin                  | `cli/src/env_ops.rs:29-52, 353`                          | #38：不阻塞，issue 推断不成立            |
| `env restore` **已有** `--dry-run` + `--json` 差异预览，走 `preview_restore`               | `cli/src/env_ops.rs:451-469`                             | #36 可直接沿用既有模式，非新设计         |
| `ENV_BACKUP_KEEP = 20` + `rotate_env_backups` 只匹配 `env_backup_*.json`                   | `core/src/backup.rs:24, 361-406`                         | #37：env 侧**已有**轮换，path 侧没有     |
| `path_backup_*.txt` **只被写入，无任何轮换调用**                                           | `core/src/backup.rs:1177`（写入点）                      | #37：确认无限累积                        |
| `config.ini` 已有 `env_backup_keep` 键 + 手写 INI 解析器 + `PATHEDITOR_CONFIG_FILE` 重定向 | `core/src/backup.rs:44-124`                              | #37 直接复用该解析器与回落语义           |
| `backup_registry` 是 `path_backup_*.txt` 的唯一写入者                                      | `core/src/backup.rs:1155-1199`                           | #37 轮换须在此函数写文件后调用           |

**已知未被本版触及的既有事实**（避免开发窗口误以为要一并改）：`RestorePreview.modified` / `RestoreOutcome.skipped` / `EnvBackupInfo.variable_count` 恒为 0（规格级口径问题，见 `CLAUDE.md`），本版**不动**。

## 已确认的决策

| #   | 决策                 | 选择                                                                                                                        | 理由                                                           |
| --- | -------------------- | --------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| D1  | #34 的修法           | **新增 `get_raw_kind`（trait 默认实现 + 两个 impl 覆写），`get_raw` 签名不动**，在 `read_env_var` 内按 kind 映射 `NotFound` | 见 §关键设计决策 K1 + 核对轮裁断 N1；不违背 W2-N2 端口边界裁断 |
| D2  | #34 的文案           | 收敛成一句、变量名只出现一次：`用户环境变量 X 不存在`                                                                       | 消除「变量名重复两次」，同时保留「带变量名和操作」这一优点     |
| D3  | #33 的改法           | **只改 help 文案，不动截断逻辑**                                                                                            | `sanitize_preview` 已截断；改逻辑会破坏既有行为                |
| D4  | #35 的改法           | **只补 `add --help`**，不加 revision 参数                                                                                   | 纯新建语义本就安全；新建无「旧值」可覆盖，CAS 不适用           |
| D5  | #38 的处置           | help 补一句「省略 VALUE 会报错」，issue 关闭为「不成立」                                                                    | 代码行为正确，无需改代码                                       |
| D6  | #36 的范围           | `env set` / `env remove` / `env add` **三个命令都加** `--dry-run` + `--json`                                                | 三者都是破坏性写操作；对称性优先于最小改动                     |
| D7  | #37 的保留份数       | **默认 20 份**（与 `ENV_BACKUP_KEEP` 对称），键名 `path_backup_keep`                                                        | 用户 2026-10-16 拍板；数值一致避免两套心智模型                 |
| D8  | #37 的删除授权范围   | **只删本功能自产的 `path_backup_*.txt`**，与 `rotate_env_backups` 同构                                                      | 遵守「不删除文件」硬约束，见 §安全边界 S1                      |
| D9  | `--dry-run` 的退出码 | 恒为 0（成功预览）；不因「将会产生冲突」而返回 3                                                                            | 预览是只读操作；冲突判定留给真正执行时                         |

## 命令 / 接口规格

### 1. #34：`env get` 错误语义

无参数变化，仅**错误文本与错误码**变化。

| 情形               | 现状 message                                                                       | 改为                    | code              |
| ------------------ | ---------------------------------------------------------------------------------- | ----------------------- | ----------------- |
| 变量不存在         | `读取用户环境变量 X 失败: 无法读取环境变量 X: 系统找不到指定的文件。 (os error 2)` | `用户环境变量 X 不存在` | `notFound`        |
| 类型不受支持       | 保持                                                                               | 保持                    | `unsupportedType` |
| 其它 IO / 解码失败 | 保持                                                                               | 保持                    | `io` / `parse`    |

- 退出码不变：`notFound` → 1（`CoreError::exit_code()` 非 Conflict 恒 1）。
- `other_hive_hint`（`env_ops.rs:281-300`）**保持不动** —— 它给「变量在另一个 hive」的提示，本改造不触及；但要注意：改造后命中另一 hive 时，错误文本会变成 `用户环境变量 X 不存在\n提示：X 存在于系统 hive，请加 --system`，这是可接受的（两行，语义正确）。
- **不新增错误码**，复用既有 `ErrorCode::NotFound`。

### 2. #33：`env list` help 文案

`cli/src/main.rs:169` 的 `EnvCmd::List` 文档注释：

```rust
/// 列出环境变量元数据（不含明文）
```

改为：

```rust
/// 列出环境变量元数据（敏感变量不含明文，非敏感变量给出可能截断的 preview）
```

同时 `CLAUDE.md` / `AGENTS.md` 的 IPC 表中 `list_all_env_vars` 行（`CLAUDE.md:110`）措辞核对：「（不含敏感明文）」—— 该措辞**本就准确**（说的是「不含敏感明文」而非「不含明文」），保留不改。**只有 CLI 的 help 文案需要改。**

### 3. #35 / #38：`env add` help 补两处说明

`cli/src/main.rs:206-220` 的 `EnvCmd::Add`：

- 在变体文档注释补「纯新建语义」与「省略 VALUE 报错」两句。
- `value` 字段注释补「省略（且不给 --stdin/--value-file）会报错，不会交互提示」。

改后注释：

```rust
/// 新建变量
///
/// **纯新建语义**：同名变量会被拒绝（`NameExists`），不会覆盖 —— 因此无需
/// revision 守卫（新建没有「旧值」可被并发覆盖）。注意检查与写入是两步，
/// 存在毫秒级竞态窗口。
///
/// 值必须经三通道之一提供；都不给会**立即报错**（不会交互式提示）。
Add {
    name: String,
    /// 值。省略且不给 --stdin/--value-file 时报错（不进入交互提示）；
    /// 敏感值建议改用 --stdin 或 --value-file
    value: Option<String>,
    ...
}
```

### 4. #36：`env set` / `env remove` / `env add` 增加 `--dry-run` 与 `--json`

三个子命令各新增两个布尔参数：

```rust
/// 只显示将要发生的变更，不写注册表
#[arg(long)]
dry_run: bool,
/// 以 JSON 输出变更摘要（供脚本消费）
#[arg(long)]
json: bool,
```

**`--dry-run` 的输出契约**（非 `--json`）：

| 命令         | 输出格式                                                                         |
| ------------ | -------------------------------------------------------------------------------- |
| `env set`    | `将更新{系统\|用户}变量 NAME：{旧值} → {新值}（未写入任何内容）`                 |
| `env add`    | `将新建{系统\|用户}变量 NAME：{新值}（类型 {string\|expand}）（未写入任何内容）` |
| `env remove` | `将删除{系统\|用户}变量 NAME（当前值：{值}）（未写入任何内容）`                  |

**`--dry-run --json` 的输出契约**（统一结构，供脚本消费）：

```json
{
  "operation": "set",
  "hive": "user",
  "name": "JAVA_HOME",
  "oldValue": "C:\\Java\\jdk-17",
  "newValue": "C:\\Java\\jdk-21",
  "kind": "string",
  "writable": true
}
```

| 字段        | 类型           | 说明                                                             |
| ----------- | -------------- | ---------------------------------------------------------------- |
| `operation` | string         | `"set"` / `"add"` / `"remove"`                                   |
| `hive`      | string         | `"system"` / `"user"`                                            |
| `name`      | string         | 变量名                                                           |
| `oldValue`  | string \| null | `set`/`remove` 为当前值；`add` 为 `null`（本就不存在）           |
| `newValue`  | string \| null | `set`/`add` 为待写值；`remove` 为 `null`                         |
| `kind`      | string \| null | `set` 为注册表现有类型；`add` 为 `--kind` 值；`remove` 为 `null` |
| `writable`  | bool           | 该 hive 是否可写（复用 core 的权限判定结果）                     |

**行为语义**：

1. `--dry-run` **不写注册表、不产生备份文件**（只读）。
2. `--dry-run` **不要求** `--revision` / `--force`：预览没有并发风险，强制要 revision 只会制造摩擦（与 #32 的诉求方向一致）。即：`resolve_concurrency` **在 dry-run 分支不调用**。
3. `--dry-run` 仍**要求** `env set` / `env add` 提供值（`resolve_value(…, required=true)`），因为要展示 `newValue`。
4. 变量不存在时（`set`/`remove`）**照常报 `notFound`**、退出码 1 —— 预览与执行对「变量是否存在」的判定一致，不制造「预览说可以、执行失败」的落差。
5. 保护名单 / `Unsupported` 类型**不在预览层拒绝**（同 `env restore` 的 E5 裁断）：照常展示，`writable: false` 如实标记；真正的拒绝在执行阶段由 core 判定。**预览不做第二份判定规则**。
6. `--dry-run` 退出码恒 0；`--json` 与 `--dry-run` 可组合（`--json` 单独用由 clap 拒绝，见下方裁断 N3）。

**待裁断 A → 已裁断（N3）**：`--json` **必须**与 `--dry-run` 同用，用 clap 的 `#[arg(long, requires = "dry_run")]` 表达。单独 `--json` 由 clap 拒绝，退出码 **2**（参数解析失败），与 `CLAUDE.md` 退出码表一致。理由：若在 `cmd_env_*` 里手写 `exit_err` 则退出码为 1，与「参数用法错误」语义冲突（开发窗口 §4-A 补充）。

### 5. #37：`path_backup_*` 保留策略

**不新增命令**，采用与 env 侧完全对称的**写入后自动轮换**：

- 新增具名常量 `PATH_BACKUP_KEEP: usize = 20`（`core/src/backup.rs`，紧邻 `ENV_BACKUP_KEEP`）。
- 新增 `config.ini` 键 `path_backup_keep`（非负整数，默认 20，回落语义与 `env_backup_keep` 完全一致）。
- 新增 `fn path_backup_keep() -> usize` 与 `read_path_backup_keep(path)`，复用既有手写 INI 解析器（**重构**：把 `read_env_backup_keep` 的参数化键名抽出为通用 `read_backup_keep(path, key)`，两个调用者共用；避免复制第二份解析器）。
- 新增 `fn rotate_path_backups(dir: &Path, keep: usize) -> Result<Vec<PathBuf>, CoreError>`，与 `rotate_env_backups` 同构，仅匹配规则不同：**`starts_with("path_backup_") && ends_with(".txt")`**。
- `backup_registry`（`core/src/backup.rs:1155`）在 `std::fs::write` 之后调用 `rotate_path_backups(&backup_dir, path_backup_keep())`。

**待裁断 B → 已裁断（N4）**：轮换**不作用于** `custom_dir` 分支（只在默认目录），与 env 侧「自定义备份目录不参与轮换」一致。开发窗口实测 `custom_dir` 确在 `backup_registry` 作用域内（`backup.rs:1155`），`if custom_dir.is_none()` 可直接写。

## 关键设计决策

### K1：新增 `get_raw_kind` 旁路，而非改动 `EnvHiveStore` 的既有方法签名

**选了什么**：`EnvHiveStore` trait **保持既有 5 个方法签名不动**（`get_raw` 仍 `Result<_, String>`），**新增**一个带默认实现的方法 `get_raw_kind(&self, name: &str) -> Result<RegValue, CoreError>` —— 它把 winreg 的 `io::ErrorKind::NotFound` 映射为 `ErrorCode::NotFound`，其余为 `ErrorCode::Io`。`WinregHive` 与 `MemoryHive` 各自覆写；`read_env_var` 改调该方法。

**为什么**：

1. **不违背既有架构裁断**：`core/src/reg_store.rs:48` 的 W2-N2（2026-09-20 裁断）明写「trait 的其他方法保持 `Result<_, String>`（Wave 0 端口边界不动）」。改签名是正面推翻一条已落盘、有日期的决策。
2. **改动面显著更小**：`get_raw` 的生产调用点有 4 处（`registry/env_var.rs:158/284`、`backup.rs:226/821`），其中本版只改 1 处（`:158`）；而 `set_raw` / `delete_value` / `enum_names` 另有 7 处生产调用点（`registry/env_var.rs:203/685/852`、`backup.rs:214/809`、`registry/env_var.rs:268/581`）。改 trait 签名会把这两类共 11 处全部牵动（含 `backup.rs` 恢复路径）；旁路方案只碰 2 个 impl + 1 个调用点。
3. **判定仍结构化**：不退回「在 `read_env_var` 里文本匹配『找不到』」—— 那是按错误文本做控制流，违反「判定只认 code」的硬约束，且非中文 Windows 直接失效。

**放弃了什么**：trait 上多一个方法（有默认实现，不强制每个 impl 都写）。代价可忽略。

> **核对轮裁断 N1（2026-10-16）**：初稿的 K1 写的是「blob `get_raw` 的返回类型改为 `CoreError`」，与 W2-N2 冲突且未提及该裁断（它离被引的 `reg_store.rs:100-104` 约 60 行）。开发窗口逐行实测时撞上，异议成立。**已按旁路方案重写本决策与计划 Task 1。**
>
> **实施自由度**：`get_raw_kind` 的具体形状（默认实现是否退回 `get_raw`、覆写如何组织）由开发窗口定，只要满足两条不变量：(1) `ErrorKind::NotFound` 的结构信息到达 `read_env_var`；(2) 其余错误的 message 文本与 v5.1.4 逐字节一致。

### K2：#36 的预览复用 core 读接口，不在 CLI 重算

**选了什么**：`env set/remove` 的 `oldValue` 经 core 的 `reveal_env_var`（或等价的 core 读接口）获取；`add` 的 `writable` 经 core 的 capabilities 获取。**CLI 只负责把 core 返回的结构渲染成文本或 JSON。**

**为什么**：与 `env restore --dry-run` 的既有做法一致（`preview_restore` 在 core）。判定（保护名单、类型、权限）只在 core 一处。

**放弃了什么**：`env set --dry-run` 需要读一次当前值，比直接执行多一次注册表读 —— 但 dry-run 本就是只读操作，这个代价是它的定义。

### K3：#37 的删除授权严格限界（**开工前必须确认**）

**选了什么**：`rotate_path_backups` 的删除范围严格限定为四个条件（与 `rotate_env_backups` 的 S1 同构）：(1) 文件名 `path_backup_*.txt`；(2) 位于给定目录内；(3) 超出 `keep` 之外的最旧文件；(4) **绝不删除** `env_backup_*` / `.bak` / `.corrupt-*` / 目录中任何其它文件。

**为什么**：全局规则「未经书面同意不得删除任何文件」。本版对 path 备份的自动轮换是**用户 2026-10-16 明确授权的例外**。

**放弃了什么**：`path_backup_*` 含完整 PATH 明文（可能含内网路径、用户名），保留 N 越大暴露面越大 —— 但这是备份的固有属性，与 env 侧同性质。

## 安全边界

### S1：自动删除的授权与限界（**开发前必须确认**）

`rotate_path_backups` 的授权范围，逐条与 `rotate_env_backups`（`core/src/backup.rs:347-353`）同构：

1. 只删除**本功能自己生成**的文件：文件名匹配 `path_backup_*.txt`；
2. 只删除**位于给定备份目录内**的文件（自定义目录不参与轮换，见待裁断 B）；
3. 只删除**在保留数 N 之外**的最旧文件；
4. **绝不删除**：`env_backup_*.json`、`.bak` 文件、`.corrupt-*` 隔离文件、目录中的任何其它文件（含用户手工放入的文件）。

**必须写对抗性测试**：在备份目录中放入 `env_backup_*.json`、`path_backup_*.txt.bak`、`.corrupt-*`、以及一个无关文件（如 `notes.txt`），执行 path 轮换后断言它们**全部仍在**。此测试与 env 侧既有的 `rotate_env_backups_never_deletes_foreign_files`（`core/src/backup.rs:1399`）同构，且**必须验证两个方向**：env 轮换不删 path 文件（已有测试），path 轮换不删 env 文件（**新增**）。

### S2：dry-run 是纯读操作

`--dry-run` 路径**不得**：(1) 写注册表；(2) 产生备份文件；(3) 触发环境广播。测试须以注册表快照前后比对 + 备份目录文件数前后比对来证明。

### S3：判定仍在 core，CLI 零判定逻辑

- `#34` 的 NotFound 判定在 `read_env_var` / `reg_store`（core），CLI 只透传。
- `#36` 的 `writable` / 类型 / 保护名单判定经 core 接口获取，CLI 不重算。
- `#37` 的保留份数解析与轮换全在 core，CLI 无感知。
- 任何新增的 `pub fn` / `pub(crate) fn` 必须有 `///` 文档注释。

### S4：错误契约不得回退

`#34` 改造后，`env get` 的错误判定仍只认 `CoreError.code`；**不得**在 CLI 侧对 message 做字符串匹配。`other_hive_hint` 的触发逻辑（`env_ops.rs:281-300`）保持基于 `reveal_env_var` 的 `Err`，不经文本匹配。

## 实现布局

```text
core/src/
├── reg_store.rs                # #34：新增 get_raw_kind 旁路（get_raw 签名不动，K1/N1）
├── registry/env_var.rs         # #34：read_env_var 映射 NotFound；新增预览用读接口（K2）
├── backup.rs                   # #37：PATH_BACKUP_KEEP / path_backup_keep() / rotate_path_backups()
│                               #      重构 read_env_backup_keep → read_backup_keep(path, key)（共用）
│                               #      backup_registry 写后调用 rotate_path_backups
cli/src/
├── main.rs                     # #33/#35/#38：help 文案；#36：三个命令加 dry_run/json 参数
└── env_ops.rs                  # #36：三个 cmd_* 增加 dry-run 分支与 JSON 渲染
```

**不涉及**：GUI（`src/` 前端、`gui/` Tauri 命令）—— 本版 6 条全是 CLI/ core 侧，GUI 无改动。这是刻意的范围限制（GUI 的 env 预览若要做，另开 spec）。

## 测试策略

| 层                        | 覆盖内容                                                                                                                               |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Rust 单测（`MemoryHive`） | #34：`get_raw_kind` 返回 NotFound → `read_env_var` 产出 `ErrorCode::NotFound`；其它错误 message 逐字节不变（K1 不变量 2）              |
| Rust 单测（文件层）       | #37：`rotate_path_backups` 只删 `path_backup_*.txt`；**对抗性测试**（S1）；保留 `path_backup_keep` 份；`read_backup_keep` 复用回归     |
| Rust 单测（`config.ini`） | #37：`path_backup_keep` 的四种回落（缺失/非法/负数/不可读）与 `env_backup_keep` 同口径                                                 |
| CLI 集成测试（`--bins`）  | #36：三个命令 `--dry-run` 的文本与 JSON 输出契约；**不写注册表、不产生备份**（S2）；`--dry-run` 不需要 revision；`notFound` 仍退出码 1 |
| CLI 集成测试（`--bins`）  | #34：`env get` 不存在变量时 stderr 含「不存在」且**不含** `os error`                                                                   |
| Vitest                    | 无（本版不涉前端）                                                                                                                     |

**质量门**：`npm run verify:all`（含 `cargo test -p patheditor-cli --bins`）。本版不涉前端逻辑，但 `npm run verify` 的 ESLint / Prettier / 构建仍须全绿。

## 文档同步

| 文件                                  | 改动                                                                                                             |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `CLAUDE.md` + `AGENTS.md`（保持一致） | CLI 命令表：`env set`/`env add`/`env remove` 补 `[--dry-run] [--json]`；#34 的错误语义；#37 的 `config.ini` 新键 |
| `README.md`                           | `env` 子命令章节补 dry-run 说明；备份章节补 path 备份保留策略                                                    |
| `CHANGELOG.md`                        | 5.1.5 段落                                                                                                       |
| 对应 GitHub issues                    | 开发合入后在各 issue 回填「已修复」；#38 关闭为「不成立」                                                        |

## 验收标准

1. [ ] `env get` 读取不存在的变量时，stderr 为 `错误: 用户环境变量 X 不存在`，**不含** `os error` 与重复的变量名，退出码 1。
2. [ ] `env get` 遇 `Unsupported` 类型 / 其它 IO 错误的 message **与 v5.1.4 逐字节一致**（K1 不变量 2）。
3. [ ] `env list --help` 不再声称「不含明文」，改为「敏感变量不含明文，非敏感变量给出可能截断的 preview」。
4. [ ] `env add --help` 写明「纯新建语义：同名会被拒绝，不会覆盖」与「省略 VALUE 会报错，不进入交互提示」。
5. [ ] `env set/remove/add` 均支持 `--dry-run`，输出「将…（未写入任何内容）」文案，且**不写注册表、不产生备份文件、不广播**（以注册表快照 + 备份目录文件数前后比对证明）。
6. [ ] `env set/remove/add --dry-run --json` 输出 §命令规格 定义的统一结构，字段与类型准确。
7. [ ] `env set/remove/add --dry-run` **不要求** `--revision` / `--force`；缺失时不报错。
8. [ ] `env set/remove --dry-run` 对不存在的变量仍报 `notFound`、退出码 1（预览与执行判定一致）。
9. [ ] `env add --dry-run` 对已存在同名变量**照常展示** `writable`（不在预览层拒绝），执行时才由 core 拒 `NameExists`。
10. [ ] `path_backup_*.txt` 超过 `path_backup_keep`（缺省 20）份时自动轮换，保留最近 N 份。
11. [ ] **对抗性测试**：path 备份目录中的 `env_backup_*.json` / `.bak` / `.corrupt-*` / 无关文件**零删除**（S1）。
12. [ ] `config.ini` 的 `path_backup_keep` 缺失 / 非法 / 负数 / 不可读四种情形均回落 20，且备份不中止。
13. [ ] `env_backup_keep` 的既有行为与测试**全部不回归**（`read_backup_keep` 重构后逐用例通过）。
14. [ ] `CLAUDE.md` 与 `AGENTS.md` 字节级一致，且均已同步本版命令与错误语义。
15. [ ] `npm run verify:all` 全绿；`cargo test -p patheditor-cli --bins` 通过。
16. [ ] 所有新增 `pub fn` / `pub(crate) fn` 有 `///` 文档注释；无 `unsafe` 新增。

## 不在本版范围

- **#32 批量往返**：需跨命令 session 状态，架构变动，另开 spec。本版只做其诉求①②的**间接缓解**（dry-run 减少「改了才发现错」的往返）。
- **#33 的 expand 型展开值新列**：issue 自身说可另开；本版只改文案。
- **GUI 侧 env 预览**：本版 6 条全在 CLI/core；GUI 若有对称需求另开 spec。
- **`env add` 的 revision 守卫**：#35 核实后确认纯新建语义已足够安全，本版**不加**。
- **备份目录 ACL 收紧**：env 侧既有未覆盖项（spec `2026-09-21` §S2.1），path 侧同理，本版不扩任务。
- **`RestorePreview.modified` 等恒为 0 的口径问题**：既有已知项，本版不动。
- **#38 的代码改动**：核实为「不成立」，本版只补 help 文案 + 关闭 issue，无代码改动。

## 遗留与风险

| 项                     | 说明                                                                                                                         |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| K1 改动面              | N1 裁断后已从「改 trait 签名（7 处调用点）」缩小为「新增 `get_raw_kind`（2 impl + 1 调用点）」，回归风险显著降低             |
| `path_backup` 保留份数 | 与 env 同为「拍脑袋默认 20」，已提升为具名常量 + `config.ini` 可覆盖                                                         |
| path 备份含明文路径    | `path_backup_*.txt` 含完整 PATH（可能含用户名、内网地址），保留策略是主要缓解，README 提示须覆盖                             |
| dry-run 的 TOCTOU      | 预览读值与真正执行之间有窗口，其他进程可能改动变量。文档须如实措辞：dry-run 展示的是「预览时刻」的状态，**不保证**执行时不变 |
| 轮换失败静默           | path 轮换失败只 `log::warn!`，CLI 未初始化 logger 故**不可见**；与 env 侧对称，本波接受（N4）。独立 observability 议题另开   |
| GitHub issue 回填      | 合入后须回填 6 条 issue 的状态；#38 关闭为「不成立」，其余标注修复版本；#34 须补「W2-B1 注释与实现不符」的发现               |

## 核对轮裁断记录（2026-10-16，第 1 轮）

开发窗口在 `docs/审核和开发/2026.10.16/PathEditor-env子命令可用性收口-开发回执.md` 提出 1 处头号异议 + 3 处事实性缺陷。**逐条独立核实后全部成立**（含对审核窗口自身疏漏的指正），裁断如下。

### 异议（1 条，成立）

| #   | 异议                                                                                                                                                                                       | 核实结果                                                                                                                                                                                 | 裁断                                                                                                   |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| N1  | plan Task 1 要把 `EnvHiveStore::get_raw` 签名改为 `Result<_, CoreError>`，**与 W2-N2 裁断（`reg_store.rs:48`「trait 的其他方法保持 `Result<_, String>`」）正面冲突**，且 plan 未提及该裁断 | ✅ 成立（审核窗口疏漏）。`reg_store.rs:48` 确有此裁断，离被引的 `:100-104` 约 60 行。改签名将连带改动 `set_raw`/`delete_value`/`enum_names` 的 7 处生产调用点（含 `backup.rs` 恢复路径） | **采纳旁路方案**：新增带默认实现的 `get_raw_kind`，trait 既有签名不动。K1 已重写，plan Task 1 全节重写 |

### 事实性缺陷（3 条，成立）

| #   | 缺陷                                                                                                                     | 核实结果                                                                                                                                      | 裁断                                                                     |
| --- | ------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| N2  | Task 5 引用了不存在的 `kind_label_for(hive,&name)` 与 `parse_kind_label(&kind)`；且把「core 加预览函数」列为推荐而非必选 | ✅ 成立。`kind_label(EnvValueKind)` 在 `env_ops.rs:116`、`parse_kind(&str)->EnvValueKind` 在 `:303`（返回枚举非字符串）；两处 plan 名均不存在 | **`preview_env_change` 提为必选**（新增 Task 5 Step 3b）；修正两处函数名 |
| N3  | 待裁断 A 的补充：`--json` 应用 clap `requires = "dry_run"` 表达，退出码 2 而非手写 `exit_err`（退出码 1）                | ✅ 成立，与 `CLAUDE.md` 退出码表一致                                                                                                          | **采纳**，Task 5 Step 4 已改                                             |
| N4  | 待裁断 B 补充（`custom_dir` 在作用域内可直接判断）+ 要求 Task 6 注释写明「轮换失败 CLI 不可见」                          | ✅ 成立。`backup_registry(custom_dir: Option<String>)` 确在 `backup.rs:1155`                                                                  | **采纳**，Task 6 Step 5 已补注释；B 裁断为 (a)                           |

### 额外发现（须回填 issue #34）

`env_var.rs:153-155` 的 W2-B1 注释声称「三类错误在函数内部分类完成……不再整体包成 Io」，但 `:157-159` 当前代码**恰恰在整体包 Io** —— **该注释描述的行为从未实现（或被回退）**。#34 的根因因此不只是「漏判 NotFound」，而是「注释与实现不符」。开发窗口完成 Task 1 后须在 issue #34 补评论记录。

### 行号勘误（已就地修正）

`main.rs` 的 `List` 描述在 `:169`、`Add` 变体在 `:206-220`（初稿写 `:205-219`，差 1 行）。`env_ops.rs` 的 `resolve_value` 在 `cmd_env_add` 内为 `:357`（初稿 spec §2.1 引 `:353`，方向正确但行号需修正）。

### 核对轮第 2 轮（2026-10-16，开发窗口对裁断的复核）

开发窗口复核了 N1–N4 的落盘质量（确认无旧方案残留），并新发现一条**自洽性要求**：

| #   | 事项                                                                                                                                                                                                                            | 核实结果                                                                                                                                                                                                            | 裁断                                                                                                                                                           |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| N5  | Task 1 改完后 `registry/env_var.rs` 内会并存 `get_raw`（`:284` list）与 `get_raw_kind`（`:158` read）两种读法；**必须写明这是有意的**，否则后人会「统一」成 `get_raw_kind`，把 `list_env_vars` 的错误文案一并改掉（非预期回归） | ✅ 成立。核实：`:284` 刻意 `with_hive(ErrorCode::Io, "list_env_vars", …)`（列表级一致文案）；`backup.rs:226` 同；`backup.rs:821` 是 `let Ok(raw) … else { continue }`（跳过不分类）。三处都**不需要** NotFound 分类 | **折入 plan**（Task 1 Step 4 新增「邻近改动禁区」表 + 要求落注释）。**不采纳**开发窗口「自行遵守、不折入」的提议 —— 计划是下一个会话的权威依据，口头约定不持久 |
| —   | 口径分歧：「7 处」vs 开发窗口实测「26 处」                                                                                                                                                                                      | ✅ 两者都对但集合不同：26 = 全部 `.get_raw(` 文本出现（含测试）；7 = `set_raw`/`delete_value`/`enum_names` 生产调用点；`get_raw` 生产调用点 = 4 处（本版只改 1 处）                                                 | 已在 K1 与 plan Task 1 更正为「`get_raw` 4 处 + 其它方法 7 处」的准确口径                                                                                      |

### 核对轮第 3 轮（2026-10-16，开发窗口实施 Task 1/2 时的实测）

开发窗口实施 Task 1/2 时实测出 3 项 plan 细节问题（1 真缺陷 + 1 自证非缺陷 + 1 补充），审核窗口复验后裁断：

| #   | 事项                                                                                                                  | 核实结果                                                                                                                                                             | 裁断                                                      |
| --- | --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| N6  | plan Task 1 Step 1 测试代码写 `let hive = MemoryHive::new(true);`，但 `hive.fail_get = Some(...)` 需要 `let mut hive` | ✅ 成立。既有测试 `memory_hive_injects_failures`（`reg_store.rs:363-364`）本就带注释「brief 原文为 `let hive`，故障注入需要可变绑定，此处加 `mut`」，plan 抄写时丢了 | **已就地修正** plan Step 1 为 `let mut hive`；spec 记此条 |
| —   | 开发窗口自证：Task 2 测试的 `let mut v = …; v.vtype = …` 是 `RegValue` 局部可变，非 hive mut，无缺陷                  | ✅ 确认无误。`seed_raw(&self, …)` 签名不可变，`let mut v` 是局部变量                                                                                                 | 采纳，Task 2 测试**不改**                                 |
| —   | 补充：Task 2 的测试模块名实为 `env_var_tests`（非通用 `tests`）                                                       | ✅ 成立                                                                                                                                                              | 已补进 plan Task 2 的 Files 清单                          |

**实施状态（审核窗口独立复验）**：Task 1 + Task 2 完成。`cargo test -p path-editor-core` = **214 passed / 0 failed**（较基线 +2）；`clippy -p path-editor-core --all-targets -D warnings` 零警告；`cargo fmt --check` 干净。N5 的「邻近改动禁区」注释已落在 `read_env_var` 文档块。
