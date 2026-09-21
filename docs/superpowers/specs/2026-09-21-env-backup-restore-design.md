# 环境变量备份与恢复 — 设计文档

- **日期**：2026-09-21
- **版本目标**：5.1.4
- **作者**：审核窗口
- **状态**：待开发窗口逐行核对

## 概述

PathEditor 当前的注册表备份只覆盖 PATH（`core/src/backup.rs:38-39` 只读 `SYS_REG_PATH` / `USER_REG_PATH`），而写接口自 5.2 起已覆盖**全部**环境变量。结果是：用户用 `patheditor env set` 或 GUI 编辑 `JAVA_HOME`，写坏之后没有任何回退手段——PATH 有备份，通用环境变量没有。

本版补齐这个缺口：环境变量写入前自动备份，并提供恢复命令。同时把此前登记的「备份体系未覆盖环境变量」缺口（`docs/审核和开发/2026.09.19/PathEditor-备份体系未覆盖环境变量登记.md`）正式关闭。

**为什么必须与写入同波次交付**：备份文件一旦产生却没有恢复能力，等于只增加磁盘占用和敏感值暴露面，不产生任何价值。备份 + 恢复是一个不可分割的能力单元。

## 背景与现状（已核实的代码事实）

| 事实                                                                                        | 位置                                                                     | 影响                                   |
| ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | -------------------------------------- |
| 备份落点`~/.patheditor/backups/`，含系统目录拒绝逻辑                                        | `core/src/backup.rs:6-11, 21-33`                                         | 新备份复用同一目录与拒绝规则           |
| PATH 备份是纯文本`.txt`，无结构                                                             | `core/src/backup.rs:45-59`                                               | 本版**不动**，新增独立 JSON 格式       |
| 备份失败只置状态、不阻断                                                                    | `src/services/path-session.ts:129-133`                                   | env 通路沿用同语义                     |
| 类型信息齐备：`EnvValueKind`（String/ExpandString/Unsupported）+ `revision_of`（FNV-1a 64） | `core/src/env_var.rs:52-76, 186-194`                                     | 备份文件可直接携带，恢复时精确还原类型 |
| 敏感判定`is_sensitive()`（7 个关键词）                                                      | `core/src/env_var.rs:155-158`                                            | **仅用于前端打码**，不影响备份内容     |
| 版本化信封`Versioned<T>` + `.bak` 轮换 + 损坏隔离                                           | `core/src/persist.rs:37-45, 65-77, 102-129`                              | 恢复文件格式**直接复用**该信封         |
| 6 个写入口：CLI 3 命令（各含 revision/force 两条路径）+ GUI 3 命令                          | `cli/src/env_ops.rs:324/350/372`、`gui/src/commands/env_var.rs:35/51/66` | 备份挂载点                             |
| 存储端口`EnvHiveStore` 可注入（含 `MemoryHive`）                                            | `core/src/reg_store.rs:17-32, 205+`                                      | 测试无需真实注册表                     |

**现有 PATH 备份与本次 env 备份的关系**：两者并存，互不替代。PATH 备份是 `.txt`（人类可读，供手工比对），env 备份是 `.json`（机器可读，供恢复命令消费）。本版**不**改 PATH 备份文件名、内容或触发时机。

## 已确认的决策

| #   | 决策         | 选择                                               | 理由                                                                   |
| --- | ------------ | -------------------------------------------------- | ---------------------------------------------------------------------- |
| D1  | 备份粒度     | **全量快照**：每次写前备份两个 hive 的全部环境变量 | 与 PATH 备份语义一致；恢复正常语义清晰（完整状态还原），不必追溯操作链 |
| D2  | 敏感值处理   | **全量写入明文**                                   | 用户已确认的取舍。备份的价值在于能还原，打码后的备份无法用于恢复       |
| D3  | 触发时机     | CLI 与 GUI 写前都触发                              | 覆盖全部 6 个写入口，不留旁路                                          |
| D4  | 恢复能力     | **备份 + 恢复配套闭环**，CLI 与 GUI 双入口         | 备份无恢复则无价值                                                     |
| D5  | 备份失败语义 | **不阻断，仅警告**（同 PATH 现有语义）             | 备份是尽力而为；磁盘满/权限问题不应阻止用户管理环境变量                |
| D6  | 保留策略     | **保留最近 N 份，旧的自动轮换删除**                | 控制磁盘占用与明文暴露面（见 §安全边界 对其授权范围的限定）            |

## 文件格式规格

### 文件名

```text
env_backup_<YYYYMMDD>_<HHMMSS>_<毫秒3位>.json
```

时间戳格式与 PATH 备份一致（`core/src/backup.rs:41` 的 `%Y%m%d_%H%M%S_%3f`），落在同一目录 `~/.patheditor/backups/`，便于用户在一个地方找到全部备份。

### 内容结构

复用 `persist.rs` 的 `Versioned<T>` 信封（磁盘上是平铺 JSON：`schemaVersion` + 内容字段）。

```json
{
  "schemaVersion": 1,
  "capturedAt": 1758440000000,
  "hives": {
    "system": [
      {
        "name": "JAVA_HOME",
        "kind": "string",
        "value": "C:\\Program Files\\Java\\jdk-17",
        "revision": "a1b2c3d4e5f60718"
      }
    ],
    "user": []
  }
}
```

字段语义：

| 字段                          | 类型   | 说明                                                     |
| ----------------------------- | ------ | -------------------------------------------------------- |
| `schemaVersion`               | number | 固定`1`；由 `Versioned` 提供，缺失时按 v1 读取           |
| `capturedAt`                  | number | 采集时刻 Unix 毫秒，与`EnvVarSnapshot.capturedAt` 同口径 |
| `hives.system` / `hives.user` | array  | 该 hive 的全部**可写类型**环境变量                       |
| `name`                        | string | 注册表返回的原始大小写                                   |
| `kind`                        | string | `"string"`（REG_SZ）/ `"expandString"`（REG_EXPAND_SZ）  |
| `value`                       | string | **完整明文**（D2）                                       |
| `revision`                    | string | 16 位十六进制 FNV-1a 摘要，写入时的值；供恢复前冲突提示  |

**明确不备份的内容**：

- `Unsupported` 类型变量（REG_DWORD / REG_BINARY / REG_MULTI_SZ 等）——它们本就不可写，备份了也无法通过通用通路恢复。其存在不影响恢复安全性（恢复只覆盖文件里列出的变量）。
- `Path` 变量——由专用 PATH 通路管理（`RESERVED_NAMES`），已由 `.txt` 备份覆盖。

## 命令规格

### CLI

```text
patheditor env backup [--json]
patheditor env restore <FILE> [--dry-run] [--force] [--json]
patheditor env backups [--json]
```

#### `env backup`

手工触发一次备份，写入 `~/.patheditor/backups/env_backup_<ts>.json`，stdout 打印备份文件绝对路径。

- 退出码：0 成功；1 失败（目录不可创建、注册表读取失败）。
- 与自动备份共用同一实现，可用于「想手动留个还原点」的场景。

#### `env restore <FILE>`

从备份文件还原环境变量。

参数：

| 参数        | 语义                                                                                      |
| ----------- | ----------------------------------------------------------------------------------------- |
| `<FILE>`    | 备份文件路径；必须位于`~/.patheditor/backups/` 或以 `env_backup_` 开头（见 §安全边界 S4） |
| `--dry-run` | 只计算并打印将要发生的变更，不写注册表                                                    |
| `--force`   | 跳过 revision 冲突检查（最后写入者胜）；**不豁免**保护名单/类型/权限判定                  |

行为：

1. 读取并校验文件（`Versioned` 信封 → `schemaVersion` 不超过当前版本 → 内容形状校验）。
2. 与当前注册表比对，计算差异：**新增 / 修改 / 删除 / 冲突** 四类。
3. 默认模式（无 `--force`）逐变量比对 revision：备份中某变量的 `revision` 与注册表当前值不一致时，视为**该变量在备份后被外部修改**，报告冲突并中止（退出码 3），不做任何写入。这避免「恢复备份」意外覆盖备份之后由其他工具写入的新值。
4. 写入：新增走 `create_env_var`、修改走 `update_env_var_force`（此时 revision 已由第 3 步校验或由 `--force` 豁免）、删除走 `delete_env_var_force`。
5. **恢复自己不产生备份**（否则每次恢复都新增一份文件，与保留策略互相吞噬）。恢复前**打印手工兜底指引**（见 §手工兜底出口）。

**保护名单变量的处理（核对轮 E5 裁断）**：`preview` 阶段**不为保护名单新增枚举变体、不预先排除**——它们照常出现在差异列表中。真正的拒绝发生在写入阶段：`create_env_var` / `update_env_var_force` / `delete_env_var_force` 内部判定保护名单并返回 `ErrorCode::Protected`，该变量被记入 `RestoreOutcome.failures` 而**不中止其余变量的恢复**。

选这个路径的理由：保护名单判定在 core 的写函数内已经存在且是唯一真相源；在 preview 里再实现一遍等于**双份判定规则**，与项目「判定只在 core 一处」的硬约束冲突。代价是差异列表可能包含实际写不进去的变量——由 `failures` 如实报告，比提前隐藏更诚实。

**单 hive 失败语义**：恢复按 hive 独立执行，一个 hive 失败不影响另一个。单变量失败不中止整体，逐条记入 `failures`。退出码取最严重者。

#### 手工兜底出口（核对轮 C7 裁断）

恢复不产生新备份，因此「恢复错了」没有自动回退。恢复前必须提示用户两条手工出口：

```text
patheditor backup              # 备份当前 PATH（.txt）
patheditor env backup          # 备份当前全部环境变量（.json）
```

CLI 在 `env restore` 执行前打印这两条命令（不自动执行——自动备份会与保留策略互相吞噬）；GUI 在恢复确认弹窗中以文案提示。**不得**把这层提示做成自动调用。

退出码：

| 码  | 含义                                                          |
| --- | ------------------------------------------------------------- |
| 0   | 全部还原成功（或无差异）                                      |
| 1   | 一般错误（文件不存在 / 校验失败 / 注册表读写失败 / 权限不足） |
| 2   | clap 参数解析失败                                             |
| 3   | **仅**在默认模式下检测到 revision 冲突                        |

#### `env backups`

列出备份目录中的全部 env 备份，按时间倒序，打印**文件名、时间、大小**。`--json` 输出结构化数组，字段固定为 `{file, path, timestamp, sizeBytes}`。

- 该命令只读目录，不读文件内容（不解析 JSON），因此不会因某个备份文件损坏而整体失败。

> **J4 裁断（核对轮第二轮）**：本命令**不输出「包含变量数」**。初稿的命令规格写了该项，但与本节的 S5（列表不解析内容）直接冲突——变量数只能靠解析 JSON 得到。**采纳「删去变量数、保留 S5」**：损坏文件不得让列表整体失败，这个性质比多一列信息更重要（用户可用 `env restore --dry-run` 看某个备份的具体内容）。`EnvBackupInfo` 保留 `variableCount` 字段但恒为 0，**或者**直接从结构体中删除——实施时择一并保持前后一致。

### GUI

新增 Tauri 命令（`gui/src/commands/backup.rs`）：

| Command              | 参数 / 返回值                                                      | 说明                   |
| -------------------- | ------------------------------------------------------------------ | ---------------------- |
| `backup_env_vars`    | `() -> Result<String, CoreError>`                                  | 手工备份，返回文件路径 |
| `list_env_backups`   | `() -> Result<Vec<EnvBackupInfo>, CoreError>`                      | 备份列表（不读内容）   |
| `restore_env_backup` | `(file: String, force: bool) -> Result<RestoreOutcome, CoreError>` | 执行恢复               |
| `preview_env_backup` | `(file: String) -> Result<RestorePreview, CoreError>`              | 只算差异，用于确认弹窗 |

前端：`src/services/backend.ts` 增加对应四个方法；在「全部变量」视图新增「备份与恢复」入口，展示备份列表 + 恢复确认弹窗（复用现有关窗/删除确认所用的异步对话框 `backend.confirmDialog`，不引入新的阻塞式 `window.confirm`）。

**确认弹窗必须展示的差异摘要**：新增 N 个 / 修改 N 个 / 删除 N 个，并单独高亮「将要删除」的变量名——删除是最不可逆的部分。

### 公开 API 签名（供 CLI 与 GUI 消费）

```rust
/// 计算备份相对当前注册表的差异（不写入任何内容）。
///
/// **不含 force 参数**——force 只在执行层影响「冲突是否中止」，
/// 差异计算本身与 force 无关（核对轮 E3 裁断）。
pub fn preview_restore(payload: &EnvBackupPayload) -> Result<RestorePreview, CoreError>;

/// 从文件读取备份后计算差异（GUI 的确认弹窗用）。
pub fn preview_restore_file(path: &Path) -> Result<RestorePreview, CoreError>;
```

> `preview_restore_in_stores(sys, usr, payload)` 是 `pub(crate)` 的存储可注入版本，仅供 core 内部测试使用；CLI / GUI **不得**依赖它。

## 配置文件 `config.ini`

### 落点与理由（核对轮：方案 A，用户已定）

```text
~/.patheditor/config.ini
```

**不放在 exe 同目录**，理由三条（均已实证）：

| 理由                                                  | 事实                                                                                                                                          |
| ----------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI 与 GUI 是两份独立 exe，位于不同目录               | `target/release/PathEditor.exe` 与 `target/cli/release/patheditor.exe`；scoop 安装后亦分属 `patheditor-gui` 与 `patheditor-cli` 两个 app 目录 |
| scoop 用 junction + 版本目录，升级会换目录            | `apps/<name>/current` → `apps/<name>/<version>`；写在 exe 同目录的配置在升级后丢失                                                            |
| NSIS 默认 perMachine 装 Program Files，普通用户不可写 | 写 exe 同目录需提权                                                                                                                           |

放用户目录使**双 exe 共享**、**升级不丢**、**免提权**，且与既有 `disabled.json` / `backups/` / `profiles/` 同处（`core/src/disabled.rs:17`、`core/src/backup.rs:9`、`core/src/profiles.rs:16` 均在 `~/.patheditor/` 下）。

### 格式与键

采用 INI 格式，本版只有一个键：

```ini
; PathEditor 配置文件
; env 备份保留份数（默认 20）
env_backup_keep = 20
```

| 键                | 类型     | 默认值                    | 语义             |
| ----------------- | -------- | ------------------------- | ---------------- |
| `env_backup_keep` | 非负整数 | `20`（`ENV_BACKUP_KEEP`） | env 备份保留份数 |

### 解析与回落行为

| 情形                         | 行为                                                         |
| ---------------------------- | ------------------------------------------------------------ |
| 文件不存在                   | 用默认值 20，**不创建文件**                                  |
| 键不存在                     | 用默认值 20                                                  |
| 值非法（非整数 / 负数 / 空） | 用默认值 20，`log::warn!` 记录一次；**不报错、不中止备份**   |
| 值极大（如 100000）          | 照常接受（用户自担磁盘占用）；不做上限钳制，仅在 README 提示 |
| 文件含未知键                 | 忽略                                                         |
| 文件整体不可读（权限）       | 用默认值 20 + `log::warn!`                                   |

**解析器不得引入新的依赖**：INI 格式极简（`key = value`，`;` 或 `#` 起始为注释行），用约 20 行手写解析即可。引入 `ini` / `configparser` crate 为单键配置不值当——项目已有「手写 FNV-1a 而不引 sha2」的先例（`core/src/env_var.rs:160-170`）。

### 读取时机

**每次备份时读一次，不做缓存**。理由：备份是低频操作（每次环境变量写入一次），一次文件读的代价可忽略；缓存会引入「改了配置要重启才生效」的困惑，还得处理失效逻辑（YAGNI）。

> **注意（核对轮 P3）**：本配置文件**不**使用 `persist::Versioned` 信封——`.ini` 是面向用户手工编辑的格式，加 `schemaVersion` 会破坏其可读性，且 INI 的未知键忽略语义已提供足够的向前兼容。`Versioned` 仅用于程序自有的 JSON 持久化文件。

## 关键设计决策

### K1：备份挂载在 core 写函数内部，而非调用方

**选了什么**：在 `core::registry::env_var` 的 6 个公开写函数入口处触发备份，CLI 与 GUI 自动继承。

**为什么**：备份是「任何环境变量写入都应有」的不变式。挂在调用方意味着每新增一个写入口都要记得补——本项目的 `Path` 专用通路绕过 disabled.json 的历史事故正是这类疏漏。挂在 core 内部，新入口天然获得备份。

**放弃了什么**：core 写函数因此多了文件系统副作用，不再是纯注册表操作。测试需注入备份目录（见 §测试策略），且 `MemoryHive` 测试会触碰真实备份目录，需用环境变量或参数重定向到临时目录。

### K2：备份失败不阻断，但必须可见

**选了什么**：备份失败 → 记 `log::warn!` + 通过返回值/状态告知调用方 → **继续执行写入**。

**为什么**：与 PATH 现有语义一致（`path-session.ts:129-133` 的 `backupFailed` 只置状态）。备份失败多半是磁盘满或权限问题，此时仍应允许用户改环境变量——拒绝写入会让工具在异常环境下完全不可用。

**放弃了什么**：无法保证「凡写入必有备份」。这是明确接受的取舍。

**可见性要求**：

- **CLI**：必须经 `WriteOutcome.backup` 在 `cli/src/env_ops.rs` 显式 `eprintln!` 警告，且**不改变退出码**（写入成功了就是成功）。
  **不得**依赖 core 的 `log::warn!` —— 实证：`cli/Cargo.toml` 无 `log` 依赖，`cli/src/` 中 `log::` 零命中，**CLI 未初始化任何 logger**，core 里的 warn 在 CLI 下被静默丢弃（核对轮 J1b）。
- **GUI**：状态栏显示警告文案，**复用既有同义键 `status.saved_without_backup`**（zh 文案「保存成功（备份失败）」，`src/i18n/locales/zh-CN.json:97`）。
  **不新增键** —— 实证：`status` 命名空间下并没有 `backupFailed`（核对轮 J1a 勘误了本 spec 的初稿），既有 PATH 通路用的正是 `status.saved_without_backup`（`src/services/path-session.ts:211`）。

> **GUI 侧的消费链路必须一并接线（核对轮 J2）**：目前 `src/services/backend.ts:303-308` 的三个 env 写方法都是 `invoke<void>`，`src/store/env-store.ts:159/186/202` 用 `await` 丢弃返回值并直接设 `statusMessage: i18n.t('status.saved')`。若不改这三处，`WriteOutcome.backup` 会产生但**永不消费**，验收标准 5 的 GUI 半边不达标。这属于验收标准范围，不是 scope creep。

### K3：恢复默认走 revision 校验，`--force` 才覆盖

**选了什么**：默认模式下，备份中记录 `revision` 与注册表当前值不一致即判冲突、中止、退出码 3。

**为什么**：备份是历史状态。用户可能 3 天前备份，之后用别的工具改了 `JAVA_HOME`。此时「恢复」若不校验，会静默抹掉这 3 天的变更。`revision` 字段正是为此存在——它是备份「知道」的那个状态。

**放弃了什么**：批量恢复时只要有一个变量冲突就全盘中止，稍显严格。用户可用 `--dry-run` 先看差异，再决定是否 `--force`。

### K4：恢复复用 `persist.rs` 的 `Versioned` 信封

**选了什么**：备份文件顶层用 `Versioned<EnvBackupPayload>`，复用 `.bak` 轮换与损坏隔离原语。

**为什么**：与项目其他三个持久化文件（`disabled.json` / `profiles/*.json` / `pending_path_snapshot.json`）格式一致，F-11 建立的信封语义在此自然延伸；且未来 schema 演进有明确机制（`migrate` 对更高版本返回 `Parse` 错误而非误读）。

**放弃了什么**：备份文件对人类不友好一点（多一个字段）。可接受——PATH `.txt` 备份仍供人类阅读。

### K5：轮换删除只针对本工具生成的备份文件

**选了什么**：保留最近 N 份，删除时**只匹配 `env_backup_*.json` 且位于 `~/.patheditor/backups/`** 的文件。N 来自 `config.ini` 的 `env_backup_keep`，缺省 `ENV_BACKUP_KEEP = 20`。

**为什么**：见 §安全边界 S1——本项目的「未经书面同意不得删除任何文件」约束要求这个自动删除行为被显式授权并严格限界。

**放弃了什么**：`N=20` 是拍脑袋的默认值。选择依据：全量备份单文件约几十 KB（取决于变量数），20 份在正常使用下是几百 KB 量级。**该值必须是具名常量 `ENV_BACKUP_KEEP` 并带注释说明可调**，不得散落为魔数；用户可经 `config.ini` 覆盖。

## 安全边界

### S1：自动删除的授权与限界（**开发前必须确认**）

`AGENTS.md` / `CLAUDE.md` 与全局规则均要求「未经书面同意不得删除任何文件」。本设计的 D6 决策（旧的自动轮换删除）是**用户 2026-09-21 明确授权的例外**，其授权范围严格限定为：

1. 只删除**本功能自己生成**的文件：文件名匹配 `env_backup_*.json`；
2. 只删除**位于 `~/.patheditor/backups/` 目录内**的文件（自定义备份目录不参与轮换）；
3. 只删除**在保留数 N 之外**的最旧文件；
4. **绝不删除**：`.txt` 格式的 PATH 备份、`.bak` 文件、`.corrupt-*` 隔离文件、目录中的任何其他文件（包括用户手工放入的文件）。

开发计划必须为第 4 条写一个**对抗性测试**：在备份目录中放入 `path_backup_*.txt`、`env_backup_*` 的 `.bak`、`.corrupt-*`、以及一个无关文件，执行轮换后断言它们**全部仍在**。

### S2：明文备份的暴露面（缓解措施）

备份文件含 API key / token 明文。这是 D2 已接受的取舍，但必须有缓解：

1. **备份目录权限收紧**：创建 `~/.patheditor/backups/` 时尽可能设置仅当前用户可访问。
   **核对轮裁断（C5）：本波登记为未覆盖项，不扩任务。** Windows 上需 `icacls` 或 Win32 API，实现复杂度未评估，不在本波范围。回执的「未覆盖项」必须显式列出，**不得静默略过**。
2. **文档提示**：`README.md` 的备份章节与 CLI `--help` 文案必须写明该目录含敏感值明文，建议不要同步到云端或提交到版本库。
3. **不扩大暴露面**：备份文件的路径不得经 IPC 传给前端之外的任何地方；`list_env_backups` **不读文件内容**（S5）。

### S3：判定仍在 core，CLI / GUI 不做二次实现

新增的恢复命令必须遵守项目既有的分层约束：保护名单、`Unsupported` 类型、hive 写权限、名称合法性**全部由 core 判定**。CLI 只做参数转换与错误透传（`exit_core_error`），GUI 命令只做参数转换。

**特别地**：`--force` 语义必须复用既有的 core force API（`update_env_var_force` / `delete_env_var_force`），**不得**在恢复逻辑中自己实现「跳过校验」——既有 force API 只豁免 revision 校验，保护名单/类型/权限照旧，这正是需要的语义。

### S4：恢复文件路径必须校验来源

`restore_env_backup` / `env restore` 接收用户给定的文件路径。校验规则：

1. 文件扩展名必须是 `.json`；
2. 路径必须位于 `~/.patheditor/backups/` 之内**或**文件名以 `env_backup_` 开头（与既有的导入文件路径校验 `core/src/fs.rs` 同源思路：限制在已知目录内，避免被诱导读取任意文件）；
3. 文件大小上限（建议 1 MiB）——正常备份几十 KB，超大文件说明不是本工具产物。

### S5：列表命令不读内容

`list_env_backups` / `env backups` 只做目录枚举与 `stat`，**不解析 JSON 内容**。这样单个损坏的备份文件不会让恢复功能整体不可用，也减少敏感值被读入内存的次数。

### S6：恢复自身的写入也必须逐变量走 core API

恢复**不得**用 `EnvHiveStore::set_raw` 直接写裸值绕过校验——那会绕过保护名单与类型判定。必须逐变量调用 `create_env_var` / `update_env_var_force` / `delete_env_var_force`。

**核对轮 E5 补充**：保护名单变量在 `preview` 阶段不特殊处理（照常出现在差异列表），拒绝发生在写入阶段并被记入 `failures`。这是有意的——判定只在 core 写函数内实现一处，不在 preview 里复制第二份规则。

## 实现布局

```text
core/src/
├── backup.rs                   # 新增 env 备份/恢复实现（与 PATH 备份同文件，路径相邻）
│   ├── backup_env_vars()       # 采集 + 写文件 + 轮换
│   ├── list_env_backups()      # 目录枚举（S5）
│   ├── preview_env_backup()    # 计算差异（供 --dry-run 与 GUI 确认弹窗）
│   └── restore_env_backup()    # 执行恢复
├── registry/env_var.rs         # 6 个写函数入口触发备份（K1）
gui/src/commands/backup.rs      # 4 个新 Tauri 命令（薄包装）
gui/src/lib.rs                  # 注册新命令
cli/src/env_ops.rs              # env backup / restore / backups 三个子命令
cli/src/main.rs                 # EnvCmd 枚举扩充
src/services/backend.ts         # 4 个前端方法
src/core/                     # 差异摘要的展示逻辑（纯函数）
```

## 测试策略

| 层                             | 覆盖内容                                                                                                                                                                                       |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust 单测（`MemoryHive` 注入） | 备份内容形状正确（含 hive/kind/value/revision）；轮换只删自己的文件（S1 对抗性测试）；`preview` 的三类差异计算；恢复的 revision 冲突判定；`--force` 语义；`Unsupported` 变量不进备份也不被删除 |
| Rust 单测（文件层）            | `Versioned` 信封读写往返；`schemaVersion` 过高 → `Parse` 错误；损坏文件 → 隔离而非崩溃                                                                                                         |
| 备份目录重定向                 | 测试必须不触碰真实`~/.patheditor/backups/`。设计上通过参数注入备份根目录（core 函数接受 `Option<PathBuf>`，测试传临时目录）                                                                    |
| CLI 集成测试（`--bins`）       | 三个子命令的参数解析、退出码（0/1/3）；`--dry-run` 不写注册表                                                                                                                                  |
| Vitest                         | 差异摘要的展示函数（纯函数）；前端确认弹窗的差异文案                                                                                                                                           |
| E2E（mock IPC）                | 备份列表渲染、恢复确认弹窗的差异展示（**不写真实注册表**）                                                                                                                                     |

**质量门**：`npm run verify:all`；CLI 单测须用 `cargo test -p patheditor-cli --bins`。

## 文档同步

| 文件                                  | 改动                                                                                                                      |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `CLAUDE.md` + `AGENTS.md`（保持一致） | 新增`env backup` / `env restore` / `env backups` 三条 CLI 命令；新增 4 个 IPC 命令；备份章节补充 env 备份格式与敏感值提示 |
| `README.md`                           | 备份与恢复章节（含 S2 的敏感值提示）                                                                                      |
| `CHANGELOG.md`                        | 5.1.4 段落                                                                                                                |

## 验收标准

1. [ ] CLI `env backup` 生成 `~/.patheditor/backups/env_backup_<ts>.json`，stdout 输出路径，退出码 0。
2. [ ] 备份文件含两个 hive 的全部可写环境变量，每条含 name / kind / value（明文）/ revision。
3. [ ] **采集遇到 `Unsupported` 类型时跳过该变量并继续（不使整次备份失败）**，`Path` 不出现在备份中。
       （**B1 裁断后收紧**：初稿写「`Unsupported` 类型变量与 `Path` 不出现在备份中」——在修好 B1 之前，这句话对 `Unsupported` 是**不可达的**，它描述的是一个走不到的分支。把「跳过而非失败」写进验收标准才具备可判定的完成标准。）
4. [ ] CLI 三个 `env` 写命令执行前自动产生备份；GUI 三个写命令同样。
5. [ ] 备份失败时写入照常成功，CLI stderr 有警告且退出码不受影响；GUI 状态栏显示警告。
6. [ ] `env restore` 默认模式检测到 revision 冲突时中止、退出码 3、注册表零改动。
7. [ ] `env restore --force` 覆盖写入，退出码 0（非并发错误场景）。
8. [ ] `env restore --dry-run` 打印差异且不写注册表（可用注册表快照前后比对验证）。
9. [ ] `--force` 不豁免保护名单：备份中含 `windir` 时仍被拒绝。
10. [ ] `env backups` 列出备份，只读目录不解析内容；损坏文件不影响列表。
11. [ ] 轮换保留 `config.ini` 的 `env_backup_keep`（缺省 20）份；**对抗性测试**：目录中的 `.txt` 备份 / `.bak` / `.corrupt-*` / 无关文件零删除。
12. [ ] GUI 备份列表可展示、可触发恢复、确认弹窗显示新增/修改/删除数量并高亮删除项。
13. [ ] 恢复路径校验（S4）：非 `.json`、超出限定目录、超大文件均被拒绝。
14. [ ] `CLAUDE.md` 与 `AGENTS.md` 字节级一致，且均已同步本版命令。
15. [ ] `npm run verify:all` 全绿；`cargo test -p patheditor-cli --bins` 通过。
16. [ ] `config.ini` 缺失 / 键缺失 / 值非法 / 文件不可读四种情形均回落默认值 20 且备份不中止。
17. [ ] `env restore` 执行前打印两条手工兜底命令（`patheditor backup` 与 `patheditor env backup`），且不自动执行。
18. [ ] 保护名单变量出现在 preview 差异列表、在写入阶段被拒、记入 `failures`，且不中止其余变量的恢复。

## 不在本版范围

- **备份目录 ACL 收紧**（S2.1）：本波登记为未覆盖项（核对轮 C5 裁断）。
- **PATH 备份格式变更**：`.txt` 保持原样，不改名、不改内容、不改触发时机。
- **备份内容的加密**：D2 已明确全量明文。加密会引入密钥管理问题（密钥存哪里？丢了怎么办？），且备份的价值在于手工可查。
- **远程/云端备份**：本地文件系统之外的一切不做。
- **定时自动备份**：只在写操作前触发，不做后台定时任务。
- **恢复单个变量**（`--only NAME`）：本版恢复粒度是整个备份文件。若开发窗口认为实现成本低，可作为规格外补全提出，但不得挤占主线。
- **F-08 的 GUI 服务层接线**：与本版无关，仍为独立波次候选。
- **scoop bucket 提交**：本波次不出 release。bucket 清单的 `PathEditor.exe` 改动与 version/hash 一起，等 5.1.4 发布后再提交（用户 2026-09-21 明确）。

## 遗留与风险

| 项                | 说明                                                                                                                          |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| 备份目录 ACL 收紧 | **本波登记为未覆盖项**（C5 裁断），不得静默略过                                                                               |
| 保留数 N=20       | 拍脑袋默认值，已提升为 `ENV_BACKUP_KEEP` 具名常量 + `config.ini` 可覆盖                                                       |
| 备份文件增长速度  | 全量快照 + 密集写操作会产生大量含明文密钥的文件。D6 的轮换是主要缓解，但用户改大 N 时暴露面随之上升——README 提示需覆盖这一点  |
| 恢复的 TOCTOU     | 与既有写通路同性质：比对与写入是两次独立注册表调用，revision 校验缩小影响但不能完全消除。文档与注释须如实措辞，不得宣称原子性 |

## 核对轮裁断记录（2026-09-21）

开发窗口对本 spec 与实施计划提出异议，逐条核实后裁断如下。**每条都附了核实方式**。

### 采纳并已折入本文档

| #   | 异议                                                                                                    | 裁断                                                                                          | 落点                             |
| --- | ------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | -------------------------------- |
| E3  | `preview_restore` 不应带 `force` 参数——force 只在执行层生效                                             | **成立**：差异计算与 force 无关，加 force 是多余的耦合                                        | §公开 API 签名                   |
| E5  | 保护名单在 preview 阶段不新增枚举变体、不预排除，走「写入时被拒 + 记入 failures」                       | **成立**：在 preview 里再实现一遍保护名单判定等于双份规则，违反「判定只在 core 一处」的硬约束 | §`env restore` 行为第 5 条后、S6 |
| C5  | ACL 收紧登记为未覆盖项，不扩任务                                                                        | **成立**：Windows 上需 `icacls`/Win32 API，复杂度未评估                                       | S2.1、验收标准、范围外           |
| C7  | 恢复不产生新备份 + 恢复前提示手工兜底出口                                                               | **成立**，且补强为「**不得**做成自动调用」——自动备份会与保留策略互相吞噬                      | §手工兜底出口                    |
| —   | 配置文件落 `~/.patheditor/config.ini`（方案 A）                                                         | **成立**（用户已定）：双 exe 共享、升级不丢、免提权                                           | §配置文件                        |
| P1  | `raw_name_of(store, key)` 全库不存在，且 `current` 解构类型不匹配                                       | **成立**：我核对了计划原文，该函数确实不存在，是计划代码的实质缺陷                            | 计划 Task 5 已降级为行为契约     |
| P2  | `preview_restore` 调用签名与 Task 5 产出冲突，两处表述不一致                                            | **成立**（与 E3 同源）                                                                        | 计划 Task 7 + Self-Review        |
| P3  | `persist` 的 `Versioned` / `PERSIST_SCHEMA_VERSION` / `mod persist` 均为 `pub(crate)`，gui/cli 无法使用 | **成立**：`core/src/lib.rs:8` 实证 `pub(crate) mod persist;`                                  | 计划 Global Constraints          |

### 驳回（附实证）

| #     | 异议                                                                            | 裁断                                                                                                                                                                                                                                                                                                                          | 实证           |
| ----- | ------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- |
| E1/E4 | 开发窗口**撤回**，改称「`crate::registry::env_var::X` 全路径在同 crate 内合法」 | **驳回撤回，原异议成立**：`core/src/registry.rs:14` 是 `mod env_var;`（私有模块），Rust 的可见性规则不允许从兄弟模块经私有模块全路径访问。最小实验（独立 crate 复刻同形结构）报 `error[E0603]: module env_var is private`；改经 `registry` 根的 re-export 则编译通过。计划 Task 1/Task 6 正依赖该错误写法，**照抄会编译失败** | 见下方实验记录 |

### E1/E4 的核实过程（决定性证据）

```rust
// 复刻 PathEditor 的模块结构
pub mod registry {
    mod env_var;                        // 与 core/src/registry.rs:14 同形（私有）
    pub(crate) use access::hive_location;
    pub use env_var::pub_fn;
}
mod backup {
    pub fn a() { crate::registry::hive_location(); }            // 经 registry 根：✅ 编译通过
    pub fn b() { crate::registry::env_var::read_env_var(); }    // 经私有子模块全路径：❌
}
```

```text
error[E0603]: module `env_var` is private
  --> src\lib.rs:14:43
   |
14 |     pub fn b() { crate::registry::env_var::read_env_var(); }
   |                                   ^^^^^^^ private module
```

**正确写法**：core 内部跨模块引用必须经 `crate::registry::X`（根 re-export）或新增 `pub(crate) use`。计划 Task 6 对三个 `*_in_store` 函数改 `pub(crate)` 时，**必须同时在 `registry.rs` 加 `pub(crate) use`**，否则即使函数本身是 `pub(crate)`，也因模块私有而不可达。

### E6 撤回确认无误

`EnvValueKind` 的 `#[serde(rename_all = "camelCase")]` 确实产出 `"string"` / `"expandString"`（`core/src/env_var.rs:50-59`），与 spec 的 JSON 示例一致。撤回正确。

## 核对轮裁断记录（第二轮，2026-09-21）

开发窗口在 `docs/审核和开发/2026.09.21/PathEditor-环境变量备份恢复核对轮交接.md` 提出 3 阻断 + 3 判断意见 + 1 项紧急发现。**逐条独立核实后全部成立**（含对审核窗口自身疏漏的指正），裁断如下。

### 阻断项

| #      | 异议                                                                                    | 核实结果                                                                                                                                                              | 裁断                                                                                                                                                  | 落点                                                                                                                                |
| ------ | --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| **B1** | Task 1 采集用 `read_env_var`，遇 `Unsupported` 会 `?` 短路 → **整次备份失败**           | ✅ 成立。`core/src/registry/env_var.rs:29-35` 实证对 `Unsupported` 直接返回 `Err(UnsupportedType)`；`collect_env_backup` 对两 hive 都 `?`。**这是审核窗口的计划错误** | 采**方案 A**：改用 `store.get_raw` + `from_reg_type` + `is_writable()` 跳过，与 `list_env_vars_in_store`（`env_var.rs:151-166`）同形                  | 计划 Task 1 Step 3 已重写实现；Step 1 测试改为**先断言 `Ok`** 并新增 `collect_hive_vars_keeps_others_when_unsupported_present` 回归 |
| **B2** | `apply_core_result` 泛型化漏了 `apply_concurrency`（`cli/src/env_ops.rs:120`）          | ✅ 成立。实证 `env_ops.rs:120` 形参仍是 `Result<(), CoreError>`，而 `:339`/`:377` 将传入 `Result<WriteOutcome, _>` → 编译失败。**审核窗口的计划漏项**                 | 采**方案 A**：`apply_concurrency` 一并泛型化；计划补全 6 处改动清单                                                                                   | 计划 Task 3 Step 3 已补表格                                                                                                         |
| **B3** | Task 7 测试用了从 `cli` crate 不可达的 `path_editor_core::persist::test_persist_lock()` | ✅ 成立。`core/src/lib.rs:8` 是 `pub(crate) mod persist;`，外部 crate 视角等价私有                                                                                    | 采**方案 B（不取锁）**：`--bins` 测试跑在独立进程，与 core 测试不共享进程级环境变量，那把进程内锁跨进程无意义。**不**提升可见性（不为测试扩大公开面） | 计划 Task 7 Step 1 已删取锁行并注明理由                                                                                             |

### 判断意见

| #       | 事项                                                 | 核实结果                                                                                                                                                     | 裁断                                                                                                                                                                                                 |
| ------- | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **J1a** | i18n 键名 `status.backupFailed` 不存在               | ✅ 成立。`src/i18n/locales/zh-CN.json:97` 是 `saved_without_backup`（「保存成功（备份失败）」）、`:99` 是 `warning_backup`。**本 spec 初稿的键名是错的**     | 复用 `status.saved_without_backup`，**不新增键**                                                                                                                                                     |
| **J1b** | K2 的「CLI stderr 警告」实际不会出现                 | ✅ 成立。`cli/Cargo.toml` 无 `log` 依赖、`cli/src/` 中 `log::` 零命中 → **CLI 未初始化任何 logger**，core 的 `log::warn!` 被静默丢弃                         | CLI 侧改由 `WriteOutcome.backup` 在 `env_ops.rs` 显式 `eprintln!`；新增 `warn_if_backup_failed` 辅助                                                                                                 |
| **J2**  | 验收标准 5 的 GUI 半边无任务承载                     | ✅ 成立。`src/services/backend.ts:303-308` 三个方法仍是 `invoke<void>`，`src/store/env-store.ts:159/186/202` 丢弃返回值 → `WriteOutcome.backup` **永不消费** | 补进计划 Task 8：`backend.ts` 三方法返回 `WriteOutcome` + 运行时形状校验；`env-store.ts` 消费并按 `saved_without_backup` 换文案。**属验收标准范围，非 scope creep**                                  |
| **J3**  | `validate_write(store, hive)` 函数不存在且抽象不成立 | ✅ 成立。开发窗口逐入口数出 5 个写函数的校验项互不相同（见计划内表格），单参数签名表达不了                                                                   | 采**方案甲**：接受校验在公开包装与 `*_in_store` 各写一份，`*_in_store` 那份是防御性兜底，判定源仍是三个 core 函数，**不构成判定源分裂**。计划已删除 `prepare_and_backup`，改为「只抽备份、不抽校验」 |
| **J4**  | 命令规格「包含变量数」与 S5「不解析内容」冲突        | ✅ 成立（本 spec 自身内部矛盾）                                                                                                                              | 删去「变量数」，保留 S5                                                                                                                                                                              |

### 紧急发现（独立于本波）：已发布的 v5.1.3 CLI 安装包装的是 GUI 二进制

**审核窗口独立复核，全部证实：**

| 检验                                                                                | 实测结果                                                                                         |
| ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `scoop/apps/patheditor-cli/current/patheditor.exe` 与 `-gui/current/patheditor.exe` | sha256 **均为 `0bdc814a02356fd4e32585b50183f8e4920329215c5ea6e68de2c0b16ba83603`**（逐字节相同） |
| 该 hash 的来源                                                                      | 正是 `bucket/patheditor-cli.json:9` 声明的 hash → **CLI 的发布资产本身就是 GUI**                 |
| PE 子系统                                                                           | `0x0002` = **GUI (WINDOWS)**，非控制台程序                                                       |
| GUI 清单 hash                                                                       | `c8a1c568…`（portable zip），与实际装出的 `0bdc814a…` 不符                                       |

**根因**：v5.1.3 的 workflow 用共享 target + `Copy-Item target\release\patheditor.exe`，NTFS 大小写不敏感使 CLI 链接产物覆盖了 GUI 本体。**已于 gui-fix 波次（`17777a8`）修好**（`release.yml:112` 的 `--target-dir target/cli` + portable zip 步骤），**5.1.4 不会再犯**。本条要求的是 **5.1.4 发布后立即修复存量**。

**裁断（J5/J6）**：

1. **J5 — README 口径：立即补已知问题提示。** `README.md:173-183`、`:307` 把 `scoop install lhy/patheditor-cli` 列为推荐安装方式，而按实测该路径在 5.1.3 下装不出能用的 CLI。用户按 README 操作会拿到一个跑不起来的 GUI —— 这是**主动误导**，不是「等发版再说」的问题。
2. **J6 — 手工冒烟基线：必须提示用户先重建。** 本地 `target/release/PathEditor.exe` 现为 **3,102,348 字节 / PE `CONSOLE` / `clap_builder` 命中 593 次 / `tauri` 命中 0 次** —— 是 CLI 二进制（共享 target 残留）。**拿它做 GUI 手工冒烟会得到「双击无窗口」的错误结论**；须先 `npx tauri build` 重新生成 GUI 本体。
3. **两者都不阻塞本波开发**，作为**发布后处置清单**登记（见下）。

### 发布后处置清单（5.1.4 发布时执行，本波不执行）

| #   | 动作                                                                                                                     | 前置条件                                                     |
| --- | ------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ |
| 1   | **立即补 README 的 v5.1.3 已知问题提示**（`README.md:173-183`、`:307` 的 scoop CLI 安装路径在 5.1.3 下装不出能用的工具） | **无**——J5 裁断：这是主动误导，现在就补，不等发版            |
| 2   | `scoop update patheditor-cli`                                                                                            | 5.1.4 已发布（修复存量：5.1.3 装出的 exe 是 GUI 且无法运行） |
| 3   | `scoop update patheditor-gui`                                                                                            | 同上（新 zip 内才是 `PathEditor.exe`）                       |
| 4   | 提交 bucket 的 `patheditor-gui.json`（`shortcuts` + `version` + `hash` 一起）                                            | 同上；**bucket 仓的提交由用户决定**                          |
| 5   | 移除 README 的 v5.1.3 已知问题提示                                                                                       | 存量修复后                                                   |

**在 5.1.4 发布前不要跑 `scoop update patheditor-cli` / `patheditor-gui`** —— 现 bucket 仍是 5.1.3 的 URL 与 hash，而本地工作区已有未提交的 `shortcuts` 改动，此刻更新会装到名字都对不上的包。

**手工冒烟基线（J6 裁断）**：本地 `target/release/PathEditor.exe` 现为 **3,102,348 字节 / PE `CONSOLE` / `clap_builder` 命中 593 次 / `tauri` 命中 0 次**——是 CLI 二进制（共享 target 残留）。**做 GUI 手工冒烟前必须先 `npx tauri build` 重新生成**，否则会得到「双击无窗口」的错误结论。
