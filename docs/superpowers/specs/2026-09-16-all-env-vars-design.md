# 全环境变量编辑扩展 — 设计文档

**日期**: 2026-09-16
**分支**: main（后续创建 feature 分支）
**状态**: 已实现（2026-09-17 完成并合并于 main；经两轮对抗性复审整改，复验通过）
**关联**: 无（新功能，非既有 Issue 修复）

## 概述

PathEditor 目前只编辑一个环境变量：`Path`。本设计将其编辑范围扩展到**注册表中 Windows 环境变量项下的所有变量**，即：

| Hive | 注册表路径                                                          |
| ---- | ------------------------------------------------------------------- |
| 系统 | `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment` |
| 用户 | `HKCU\Environment`                                                  |

**采用方案 A（旁路新增）**：引入 `EnvVar` 领域对象承载通用变量读写，`Path` 继续走现有的 `PathEntry` 通路不变。两条通路通过窄接口交汇（写注册表时的并发校验），而不是把 PATH 泛化进通用模型。

**本版交付范围**：GUI 核心读写（查看 + 编辑 + 新增 + 删除）。导入导出、profile、CLI 扩展不在本版范围内。

### 为什么不是"泛化 PATH"

备选方案是把 `PathEntry` 上位为 `EnvVar`，让 `Path` 成为 `kind: 'path'` 的特例。该方案模型更统一，但代价是：

- 需一次性改动 12+ 处跨层契约、`tests/fixtures/path-capabilities.json`、CLI 18 条命令
- 现有 PATH 通路已修复多个真实缺陷（值类型降级、pending 快照补写、hive 独立权限、外部修改检测），泛化过程中这些修复有回归风险
- `enabled` / 快照合并语义是为"列表型变量"设计的，强加给 `windir` 这类单值变量后语义不成立

旁路方案让 PATH 通路零改动、195 个既有测试语义不变，新通路可独立演进。

---

## 实测依据

以下结论在目标机器（Windows 11 Home China 10.0.26200）实测得出，是本设计的事实基础。

### 1. 值类型分布

```text
SYSTEM   String             x18
SYSTEM   ExpandString        x6
USER     String             x28
USER     ExpandString        x5
```

**只有 `REG_SZ` 和 `REG_EXPAND_SZ` 两种类型**，没有 `REG_MULTI_SZ` / `REG_DWORD` / `REG_BINARY` 混入。

推论：本版不必实现二进制或多行字符串编辑，但**类型必须原样保留**。

### 2. 变量名大小写不统一

```text
SYSTEM: [path]     ← 全小写
USER:   [Path]     ← 首字母大写
```

注册表值名大小写不敏感，但**存储时保留原始大小写**。因此：

- 写回时必须以**原始名**写入，否则会因"大小写不同"被 Windows 视为新值而留下重复变量
- 改名操作需先写入新名、再删除旧名，且冲突检测必须忽略大小写

### 3. 存在敏感变量

User hive 实测包含 `HALO_MCP_TOKEN`、`MINIMAX_API_KEY` 等密钥类变量。这是纯 PATH 工具从未面对的暴露面。

### 4. 系统内置关键变量

以下变量一旦损坏会导致系统或登录异常：

| 变量                     | 实测值                         |
| ------------------------ | ------------------------------ |
| `windir`                 | `C:\WINDOWS`                   |
| `ComSpec`                | `C:\WINDOWS\system32\cmd.exe`  |
| `PATHEXT`                | `.COM;.EXE;.BAT;.CMD;.VBS;...` |
| `OS`                     | `Windows_NT`                   |
| `PROCESSOR_ARCHITECTURE` | `AMD64`                        |
| `TEMP` / `TMP`           | `C:\WINDOWS\TEMP`              |
| `USERNAME`               | `SYSTEM`                       |
| `NUMBER_OF_PROCESSORS`   | `24`                           |
| `PSModulePath`           | `...\Modules;...\v1.0\Modules` |

---

## Part 1: Rust core — 通用变量读写

### 新增模块 `core/src/env_var.rs`

契约分两个对象：**元数据**（列表用，不含明文）与**值**（按需单独读取）。

```rust
/// 列表元数据 — 绝不包含敏感变量的明文
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarMeta {
    /// 注册表中的原始名，保留大小写（如 system 的 "path"、user 的 "Path"）
    pub name: String,
    /// 真实注册表类型，来自 get_raw_value().vtype
    pub kind: EnvValueKind,
    /// 所属 hive
    pub hive: EnvHive,
    /// 编辑权限（保护名单 / 保留变量 / hive 写能力 / 类型可写性 四者取交集）
    pub can_edit: bool,
    /// 删除权限（与 can_edit 不同：可编辑但不可删除的变量存在）
    pub can_delete: bool,
    /// 是否为敏感变量 — 决定前端是否必须经 reveal 才能取值
    pub sensitive: bool,
    /// 安全展示摘要；仅当 kind 为 String/ExpandString 且敏感判定未命中时非 None
    pub preview: Option<String>,
    /// 并发校验用：基于 名称+类型+值 的稳定摘要
    pub revision: String,
}
```

**`EnvVarMeta` 不含 `value` 字段**，敏感判定命中的变量连 `preview` 也为 `None`。

**关于安全边界的准确表述**：本设计能保证的是「**命中敏感规则的变量，其明文不进入前端**」，而**不是**「所有密钥都不进入前端」。原因是 `sensitive` 基于变量名启发式（匹配 `TOKEN` / `KEY` / `SECRET` / 密码 / `API` 等），名称不含这些词的密钥变量会被判为非敏感，其值仍会经 `preview` 下发。

这个限制是设计取舍而非疏忽，明文可见的范围因此被压缩到：

| 变量                                         | `preview`    | 明文来源                          |
| -------------------------------------------- | ------------ | --------------------------------- |
| 命中敏感规则                                 | `None`       | 仅 `reveal_env_var`               |
| 未命中，且 `kind` 为 `String`/`ExpandString` | 有值（截断） | `preview` 或 `reveal_env_var`     |
| `kind` 为 `Unsupported`                      | `None`       | 不可读，`reveal_env_var` 返回错误 |

**`reveal_env_var` 对 `Unsupported` 类型的行为**：返回 `Err`，不尝试把 `REG_DWORD` / `REG_BINARY` / `REG_MULTI_SZ` 转成字符串。理由：

- `REG_BINARY` 的字节序列可能不含合法 UTF-16，强行转换会产生损坏或不可打印内容
- `REG_MULTI_SZ` 是多值语义，单个 `String` 无法无损表达（用分隔符拼接会与值内合法换行混淆）
- `REG_DWORD` 转十进制字符串看似可行，但会让"值"在展示层与写回层语义不一致

因此这三类变量的语义是**元数据可见、值不可读**：列表显示类型标签与 `'(不支持的注册表类型)'`，编辑/删除按钮禁用，`reveal_env_var` 返回明确错误。

**若将来要支持其中的 `REG_DWORD`**，应当新增独立的类型化值对象（如 `EnvValue::Dword(u32)`），而不是在此处放松 `String` 约束 —— 这是独立的扩展，不在 v1 范围。

若要彻底消除该残留风险，需要在实现时二选一：

- **方案 R1（保守）**：列表只给元数据，`preview` 恒为 `None`，所有真实值都经 `reveal_env_var` 按需读取。代价是列表无法直接显示值，浏览体验变差。
- **方案 R2（当前设计）**：保留 `preview`，但**约束其内容**——截断长度、不返回换行、且仅在未命中敏感判定时下发。

**本设计采用 R2**，理由是路径类工具的主要用途是浏览与对比，全量 reveal 会让常规使用变得繁琐；同时 R2 已通过契约把"命中敏感规则"这一类风险完全隔离。**R1 作为备选记录在此**，若实现阶段认为残留风险不可接受，切换到 R1 只需把 `preview` 恒置 `None` 并把值列改为"点击查看"，不影响其他设计。

**`preview` 的截断与净化规则**（R2 下必须实现）：长度上限 256 字符，超长追加 `…`；剔除 `\r` / `\n` / `\0`；若净化后为空则置 `None`。

无 `value` 字段同时意味着前端**没有任何普通代码路径**能读到敏感明文——取明文必须显式调用 `reveal_env_var`。前端打码（Part 3）只是显示层，不承担安全职责。

`revision` 是并发校验的载体：由 Rust 计算并随列表下发，保存时原样回传。它不是版本号，而是 **name + vtype + value 的摘要** —— 只要注册表里这条值发生过任何变化（含被其他进程改动），摘要就不再匹配。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvValueKind {
    /// REG_SZ — 不做变量展开
    String,
    /// REG_EXPAND_SZ — 写入后由系统展开 %VAR%
    ExpandString,
    /// 其他类型（REG_DWORD / REG_BINARY / REG_MULTI_SZ 等），本版只读
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvHive {
    System,
    User,
}
```

**设计要点**：

- `canEdit` / `canDelete` / `sensitive` 在 Rust 侧计算并下发，前端不重复实现判定逻辑。原因：判定规则是安全边界，单一实现处比双端各写一份更可靠。
- `canEdit` 与 `canDelete` **必须分开** —— 存在"可改值但不可删除"的变量（如 `Path`，可编辑但必须走专用通路）。
- `EnvValueKind::Unsupported` 让未知类型可被"看到"但不被编辑，避免遇到 `REG_DWORD` 时静默丢弃或误写为字符串。
- camelCase 序列化与现有 `PathCapabilities` 保持一致（见 CLAUDE.md「关键约束」）。

### `core/src/registry.rs` 新增函数

`update_env_var` / `delete_env_var` 接收 `expected_revision`，**在同一个 Rust 调用内完成"比较 + 写入"**，消除 TOCTOU。`create_env_var` 无 revision 可传（变量尚不存在），改为在 Rust 内**原子检查"该名不存在"再创建**。

```rust
/// 一次读取两个 hive 的变量元数据（列表唯一入口）
pub fn list_all_env_vars() -> Result<EnvVarSnapshot, String>;

/// 按需读取单个变量的明文（敏感变量唯一取值入口）
pub fn reveal_env_var(hive: EnvHive, name: &str) -> Result<String, String>;

/// 写入已有变量；类型从注册表读取，不由前端决定
pub fn update_env_var(
    hive: EnvHive,
    name: &str,                 // 原始名，定位用
    value: &str,
    expected_revision: &str,    // 必填；不匹配则拒绝
) -> Result<(), String>;

/// 新建变量；kind 仅在此决定，且只接受 String / ExpandString
/// 在 Rust 内原子检查名称不存在，存在则拒绝（不覆盖已有变量）
pub fn create_env_var(
    hive: EnvHive,
    name: &str,
    value: &str,
    kind: EnvValueKind,
) -> Result<(), String>;

/// 删除变量
pub fn delete_env_var(
    hive: EnvHive,
    name: &str,
    expected_revision: &str,
) -> Result<(), String>;
```

```rust
/// 两个 hive 的变量元数据，来自同一次读取
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarSnapshot {
    pub system: Vec<EnvVarMeta>,
    pub user: Vec<EnvVarMeta>,
}
```

**为什么列表是 `list_all_env_vars()` 而不是 `list_env_vars(hive)`**：`allVars` Tab 同屏展示两个 hive，若由前端分两次调用，两次读取之间注册表可能变化，合并视图会出现"system 来自 T0、user 来自 T1"的不一致——尤其 `canEdit`/`canDelete` 是按 hive 权限算出的，不一致的视图会让用户看到自相矛盾的可编辑状态。单次 Rust 调用内读两个 hive 可保证快照一致。

**这是列表的唯一入口**，不存在按 hive 分列的 core 函数；Rust 与前端契约在这一点上完全对齐。

**实现约束**：

1. **类型只从注册表读取，前端无权覆盖。** `update_env_var` 完全忽略前端传来的类型：先 `get_raw_value(name).vtype` 取真实类型，原样写回。`kind` 参数只存在于 `create_env_var`（新建时无既有类型可继承）。这条直接沿用现有 `path_value_type()` 的原则（`registry.rs` 第 29-38 行），避免通用通路重新引入 Issue #26 的 `REG_EXPAND_SZ` 降级。

   > 注意现有 `select_path_value_type` 的实际语义是"`REG_SZ` 保持 `REG_SZ`，其余一律 `REG_EXPAND_SZ`" —— 它是"保留 SZ/EXPAND_SZ 之别"，而非"保留任意类型"。通用通路必须比它更严格：真实类型不属于 `String`/`ExpandString` 时**直接拒绝写入**，不做任何猜测性映射。

2. **TOCTOU 消除。** 每个写操作在 Rust 内按以下顺序执行，中间不释放对注册表键的持有：

   1. 打开键（`KEY_READ | KEY_WRITE`）
   2. 读取当前值，计算 `revision`
   3. 与 `expected_revision` 比较，不一致 → 返回 `"变量已被其他进程修改，请重新加载"` 并退出
   4. 校验名称/值/权限
   5. 写入

   前端不再承担并发校验职责，也不需要"保存前重新 list"这一步。

3. **值校验**：不复用 `validate_and_join_paths`（那是 PATH 分号语义专用）。新增 `validate_env_value()`：拒绝 `\0`，拒绝长度超 32767 UTF-16 字符。
4. **名称校验**：新增 `validate_env_name()`：非空、无 `\0`、不含 `=`（注册表值名不能含 `=`）、忽略大小写不与现有名冲突。
5. **权限在 Rust 侧最终裁决**：`update_env_var` / `create_env_var` / `delete_env_var` 入口处依次检查 —— hive 写能力（`canWriteSystem` / `canWriteUser`）、保留变量（`is_reserved()`）、保护名单（`is_protected()`）、类型可写性。任一不满足即返回错误。**前端禁用只是 UX 提示，不构成安全边界。**
6. **写入后广播**：成功的写操作调用 `system::broadcast_env_change()`，与 PATH 通路一致。

### 变量名分类：保留 / 保护

分三类，语义不同，**都不由通用通路编辑**：

```rust
/// 保留变量：由专用通路拥有，通用通路必须完全排除
/// Path 必须在此列 —— 否则用户可绕过 PathEntry / disabled.json /
/// _pendingSys / _pendingUser 与快照事务直接改 PATH
const RESERVED_NAMES: &[&str] = &["Path"];

/// 保护变量：系统内置关键项，改坏会导致系统或登录异常
const PROTECTED_NAMES: &[&str] = &[
    "windir", "ComSpec", "PATHEXT", "OS",
    "PROCESSOR_ARCHITECTURE", "PROCESSOR_IDENTIFIER",
    "PROCESSOR_LEVEL", "PROCESSOR_REVISION",
    "TEMP", "TMP", "USERNAME", "USERPROFILE",
    "NUMBER_OF_PROCESSORS", "SystemRoot", "SystemDrive",
];

/// 忽略大小写匹配
pub fn is_reserved(name: &str) -> bool;
pub fn is_protected(name: &str) -> bool;
```

**`Path` 的处理方式（P0）**：`list_all_env_vars` **从结果中过滤掉 `Path`**（`is_reserved` 判定，忽略大小写，同时覆盖 system 的 `path` 与 user 的 `Path`），UI 在"全部变量"Tab 中显示一条引导提示并跳转到"系统 PATH"/"用户 PATH" Tab。

选择"过滤"而非"显示但设 `canEdit=false`"的理由：只读展示仍会诱导用户认为可以在该处删除或重建 PATH，而真正的编辑入口是专用 Tab。过滤掉并从 UI 明确引导，比展示一个永久禁用的行更不容易误解。

**`PSModulePath` 不在任何名单内** —— 它虽然重要，但用户有正当理由（如配置自定义模块路径）修改，且改坏不会导致系统不可用。这是有意的边界取舍。

### 权限矩阵

`canEdit` / `canDelete` 由四个条件的交集决定，全部在 Rust 侧计算：

| 条件                                             | 影响                                           |
| ------------------------------------------------ | ---------------------------------------------- |
| hive 写能力（`canWriteSystem` / `canWriteUser`） | 不满足 →`canEdit = false`、`canDelete = false` |
| `is_reserved(name)`                              | 命中 → 已从列表过滤，不进矩阵                  |
| `is_protected(name)`                             | 命中 → 两者皆`false`                           |
| `kind == Unsupported`                            | 命中 → 两者皆`false`                           |

**非管理员场景（P1）**：普通用户读取系统变量时，`canWriteSystem = false`（现有 `check_admin()` 就是探测 System Environment 键的写权限，见 `system.rs`）。此时所有系统变量 `canEdit = canDelete = false`，**在列表加载时即正确标记**，而不是等用户点了保存才失败。

`canEdit` 与 `canDelete` 分开保留的扩展位：未来若出现"可改不可删"或反之的变量，无需再改契约。

### 敏感变量判定

```rust
/// 匹配 TOKEN / KEY / SECRET / 密码 / API 等
pub fn is_sensitive(name: &str) -> bool;
```

同样在 Rust 侧实现，理由同 `canEdit`。命中时该变量的 `preview = None`，明文只能经 `reveal_env_var` 获取。

---

## Part 2: IPC 与前端数据通路

### GUI command `gui/src/commands/env_var.rs`

```rust
#[tauri::command] pub fn list_all_env_vars() -> Result<EnvVarSnapshot, String>;
#[tauri::command] pub fn reveal_env_var(hive, name) -> Result<String, String>;
#[tauri::command] pub fn update_env_var(hive, name, value, expectedRevision) -> Result<(), String>;
#[tauri::command] pub fn create_env_var(hive, name, value, kind) -> Result<(), String>;
#[tauri::command] pub fn delete_env_var(hive, name, expectedRevision) -> Result<(), String>;
```

在 `gui/src/lib.rs` 的 `invoke_handler` 中注册。走与现有 command 相同的模式：薄包装、只做参数转换、业务规则在 core。

**command 名与 core 函数名一一对应**：`list_all_env_vars` 对应 `registry::list_all_env_vars()`，返回 `EnvVarSnapshot { system, user }`。不存在按 hive 分列的 command —— 这一点在 Rust core、GUI command、`backend.ts` 三层保持同一命名与同一形状。

### IPC 契约扩展

`src/services/backend.ts` 新增方法，并配套**运行时形状校验**（沿用现有 `parsePathEntries` / `parsePathCapabilities` 的写法，不依赖 TS 断言）：

```typescript
export interface EnvVarMeta {
  name: string;
  kind: 'string' | 'expandString' | 'unsupported';
  hive: 'system' | 'user';
  canEdit: boolean;
  canDelete: boolean;
  sensitive: boolean;
  preview: string | null;
  revision: string;
}

export interface EnvVarSnapshot {
  system: EnvVarMeta[];
  user: EnvVarMeta[];
}

// 校验点：kind / hive 必须是已知字面量；canEdit/canDelete/sensitive 为布尔；
// preview 为 string 或 null；revision 为非空 string；
// snapshot 的 system / user 必须都是合法 EnvVarMeta[]
function parseEnvVarSnapshot(value: unknown): EnvVarSnapshot;
```

校验失败时抛出带标签的错误（如 `'list_all_env_vars 返回了无效的 EnvVarSnapshot 契约'`），与现有风格一致。

**契约上不存在 `value` 字段**，因此前端没有任何代码路径可以"顺手"拿到命中敏感规则的明文 —— 类型系统与运行时校验双重保证。

### 纯逻辑 `src/core/env-var.ts`

只放**与安全无关的展示逻辑**，判定规则本身不在这里（避免与 Rust 双实现）：

- `maskValue(): string` — 返回固定占位 `'••••••••'`，**不接受真实值作参数**，避免明文经过这个函数
- `validateVarName(name: string): string | null` — 前端预校验，返回错误文案或 null；规则与 Rust `validate_env_name` 保持一致
- `displayValue(meta: EnvVarMeta, revealedValue: string | null): string` — 敏感且未 reveal → 占位符；未命中敏感 → `meta.preview ?? ''`；已 reveal → 明文

### 状态层 `src/store/env-store.ts`

**独立 store，不并入 `app-store.ts`。** 理由：`app-store.ts` 已 424 行且 PATH 逻辑密集，把通用变量混进去会同时降低两者的可读性。

```typescript
interface EnvStore {
  snapshot: EnvVarSnapshot | null; // 两个 hive 的元数据，来自同一次 list_all_env_vars
  revealed: Map<string, string>; // key: `${hive}:${name}` → 明文；仅存已 reveal 的
  draft: Map<string, string>; // 编辑中的草稿值，key 同上
  hiveFilter: 'system' | 'user' | 'all'; // "全部变量"Tab 内的来源筛选，默认 'all'
  isLoading: boolean;
  isSaving: boolean;
  statusMessage: string;

  load: () => Promise<void>;
  setHiveFilter: (filter: 'system' | 'user' | 'all') => void;
  setDraft: (meta: EnvVarMeta, value: string) => void;
  save: (meta: EnvVarMeta) => Promise<void>; // 携带 meta.revision
  create: (hive, name, value, kind) => Promise<void>;
  remove: (meta: EnvVarMeta) => Promise<void>; // 携带 meta.revision
  reveal: (meta: EnvVarMeta) => Promise<void>; // 调用 reveal_env_var 取明文
  hide: (meta: EnvVarMeta) => void;
}
```

**明文只在 `revealed` 中短暂存在**：`hide()` 与 `load()` 都清除它；命中敏感规则的变量在 reveal 之前，前端内存中不存在真实值。`draft` 只保存用户正在编辑的目标值，不回填当前值。

**`revealed` 用 `Map` 且不落盘** —— 刷新页面即恢复打码，避免"上次点了显示"变成永久明文状态。

**并发校验完全由 Rust 承担。** 前端保存时原样回传 `meta.revision`，由 `update_env_var` / `delete_env_var` 在同一 Rust 调用内完成比较与写入。

**前端不做并发校验**：不执行"保存前重新 list 再比对"—— 那是 TOCTOU，检查与写入之间的竞态窗口正是本设计要消除的。前端仅在收到 `"变量已被其他进程修改，请重新加载"` 错误时，提示用户并触发 `load()` 刷新。

**这是唯一的并发模型**，取代早期草稿中"前端比对"的方案。

---

## Part 3: UI

### UI 架构（与现有 AppShell 的接合方式）

现有 `AppShell.tsx` 是 **PATH 专用**的：工具栏（`onNew`/`onEdit`/`onBrowse`/`onDelete`/`onMoveUp`/`onMoveDown`/`onClean`/`onImport`/`onExport`/`onSave`）全部作用于 PATH 列表，拖放逻辑直接调 `addPath`，保存走 `savePaths`。新增 `allVars` 后必须显式决定接合方式，否则会撞上"工具栏按钮误作用于环境变量"这类问题。

**已确定的四项决策**：

1. **`allVars` 是一个合并列表**，同屏显示 system + user 两个 hive，用"来源"列区分。
   理由：环境变量按 name 跨 hive 去重后才会暴露真正的问题（同名变量在两层同时存在时，用户进程取 User 覆盖 System）。拆成两个子 Tab 会掩盖这一信息。代价是无法一次只看一个 hive，因此提供 `hiveFilter` 筛选（见下）。
2. **`allVars` 使用独立工具栏**，不复用 PATH 工具栏。
   PATH 的 `新建/编辑/浏览/上移/下移/一键清理` 对普通变量语义不成立（向上移动 `JAVA_HOME` 没有意义）。`allVars` 的工具栏只包含：新建变量 / 编辑 / 删除 / 刷新 / 来源筛选。`上移`/`下移`/`一键清理`/`导入`/`导出` 在 `allVars` 下不可见。
3. **拖放仅在 PATH Tab 生效。** `AppShell` 现有的 drop handler 加入 `if (activeTab === 'allVars') return;`，与现有 `merged` 的处理方式一致。
4. **未保存草稿纳入窗口关闭确认。** 现有确认只看 `app-store.isModified`。若用户在"全部变量"Tab 有未提交草稿后关窗，草稿会静默丢失。因此关闭确认需同时检查 `env-store.draft.size > 0`。

`EnvStore.hiveFilter` 的语义是**筛选器**，不是数据源 —— `load()` 一次取回两个 hive，`hiveFilter` 只控制 `EnvVarTable` 显示哪一部分（`'system' | 'user' | 'all'`）。这样切换筛选不触发 IPC，也不会出现两个 hive 来自不同时刻的不一致视图。

### Tab 结构

`AppShell.tsx` 顶部 Tab 从 3 个变为 4 个：

| Tab id    | 标签      | 内容                                   |
| --------- | --------- | -------------------------------------- |
| `system`  | 系统 PATH | 现有`PathTable`（PATH 专用视图，不变） |
| `user`    | 用户 PATH | 现有`PathTable`（不变）                |
| `allVars` | 全部变量  | **新增** `EnvVarTable`（独立工具栏）   |
| `merged`  | 合并预览  | 现有`MergePreview`（不变）             |

`TabId` 类型在 `src/core/path-capabilities.ts` 中扩展为 `'system' | 'user' | 'allVars' | 'merged'`。`targetForTab()` 对 `allVars` 返回 `null`（与 `merged` 一致，因为没有单一写入目标）。

**但仅靠 `targetForTab()` 返回 `null` 不够** —— 它只让 `canWriteCurrent()` 为 false，从而禁用 PATH 工具栏里那几个按钮。要真正隔离两套工具栏，`AppShell` 需要按 `activeTab === 'allVars'` 分支渲染：PATH 工具栏 vs 环境变量工具栏。这是本 Part 的主要改动点。

Tab 文案调整（消除"系统变量"与"全部变量"的语义重叠）：

| 现标签   | 新标签           |
| -------- | ---------------- |
| 系统变量 | 系统 PATH        |
| 用户变量 | 用户 PATH        |
| （新增） | 全部变量         |
| 合并预览 | 合并预览（不变） |

zh-CN 与 en 两份 locale 都要改。

### 新增 `src/components/env-list/EnvVarTable.tsx`

- 虚拟滚动：复用现有 `useVirtualizer` 模式（`PathTable.tsx` 已有实现可参考）
- 列：变量名 / 值 / 类型 / 来源 Hive / 操作
- 搜索：独立于 PATH 的 `searchQuery`，只匹配**变量名**（**绝不匹配值** —— 匹配值等于把密钥拿去比较，且会通过"命中/未命中"泄露信息）
- 类型列显示 `String` / `ExpandString` / `Unsupported`；`Unsupported` 行的值列显示 `meta.preview ?? '(不支持的注册表类型)'`，编辑按钮禁用
- `Path` 不在列表中出现（Rust 侧已过滤）。在 Tab 顶部固定显示一条引导：「`Path` 请在"系统 PATH"/"用户 PATH"中编辑」+ 跳转链接

### 敏感值交互

- 非敏感变量：直接渲染 `meta.preview`
- 敏感变量：默认渲染 `••••••••`（不含真实长度）
- 行内「显示」按钮 → 调 `reveal_env_var` 取明文 → 存入 `revealed` → 渲染明文；「隐藏」清除
- 打码状态下**不支持行内编辑** —— 必须先点「显示」。理由：防止用户在看不到当前值的情况下盲改而覆盖掉密钥
- reveal 失败（如键在此期间被删除）时显示错误并触发 `load()` 刷新

### 只读锁定交互

按 `canEdit` / `canDelete` 两个独立标志渲染：

| 状态                  | 编辑按钮 | 删除按钮 | tooltip                            |
| --------------------- | -------- | -------- | ---------------------------------- |
| 正常                  | 启用     | 启用     | —                                  |
| `is_protected`        | 禁用     | 禁用     | 系统内置变量，修改可能导致系统异常 |
| `kind == Unsupported` | 禁用     | 禁用     | 不支持的注册表类型，仅可查看       |
| `canWrite* == false`  | 禁用     | 禁用     | 没有写入该 hive 的权限             |

由于权限已由 Rust 在 `canEdit`/`canDelete` 中算好，前端只做映射，不再重复判定规则。批量操作（如全选删除）按 `canDelete` 过滤。

**当前权限矩阵下，`canEdit` 与 `canDelete` 恒为同值。** 四类规则（保护名单、保留变量、`Unsupported`、hive 无写权限）都是两者同时为 false，因此现阶段不存在"可编辑但不可删除"的变量。

保留两个独立字段的理由是**为未来留扩展位**，而非当前已有场景：例如将来若支持保留变量（如 `Path` 改为列表内只读展示）、或引入"允许改值但禁止删除"的策略，无需再动跨层契约。这一点在实现时不要误以为需要构造出两者不一致的测试数据。

---

## Part 4: 测试与质量门

### Rust 测试

- `list_all_env_vars` / `reveal_env_var` / `update_env_var` / `create_env_var` / `delete_env_var` 在**隔离测试键**下验证，复用现有 `registry.rs` 中 `TempRegistryKey` 的 RAII 模式（`Drop` 时 `delete_subkey_all`）
- 必须覆盖：
  - **`Path` 保护**：`list_all_env_vars` 结果不含 `path` / `Path`（忽略大小写两种拼写）
  - **敏感值不进列表**：`list_all_env_vars` 返回的敏感变量 `preview` 为 `None`，且 `reveal_env_var` 是唯一取值入口
  - **revision 冲突拒绝**：传入过期的 `expected_revision` 时写入被拒
  - **既有类型不被改写**：对 `REG_SZ` 变量写入后类型仍为 `REG_SZ`；对 `REG_EXPAND_SZ` 同理
  - **非字符串类型拒绝写入**：`REG_DWORD` / `REG_BINARY` 变量的 `update_env_var` 返回错误，且值未被修改
  - **保护变量拒绝写入**：`windir` 等命中 `is_protected` 的变量被拒
  - **非管理员系统 hive 只读**：`canWriteSystem = false` 时系统变量的 `can_edit` / `can_delete` 均为 `false`
  - 原始名大小写保持（system 的 `path` 写回后仍为小写）
- **不接触真实 PATH，不写真实环境变量键**

### Vitest

- `tests/unit/env-var.test.ts`：`maskValue` / `validateVarName` / `displayValue`
- `tests/unit/env-store.test.ts`：草稿、reveal/hide 状态、冲突错误后的刷新路径

### E2E

- `e2e/mocks/ipc.ts` 增加 `list_all_env_vars` / `reveal_env_var` / `update_env_var` / `create_env_var` / `delete_env_var` 的 mock
- fixture 至少包含：一条普通变量、一条敏感变量、一条保护变量、一条 `Unsupported` 类型、一条 `canEdit=false`（hive 无写权限）
- **不构造 `canEdit=false` 但 `canDelete=true` 的 fixture** —— 当前权限矩阵下不存在这种变量（见 Part 3"只读锁定交互"）。
- `Path` 出现在 mock 的注册表数据中但**不出现在 `list_all_env_vars` 返回值中**（Rust 侧过滤的契约）
- 新增 `e2e/tests/env-vars.spec.ts`，覆盖：加载显示、敏感值默认打码、点「显示」触发 `reveal_env_var`、切到"全部变量"Tab、保护行/只读行按钮禁用、编辑普通变量触发 `update_env_var` 且携带 revision、revision 冲突的报错路径、`allVars` 下 PATH 工具栏不可见、`allVars` 下拖放无效
- **不写真实注册表**（CLAUDE.md 硬约束）

### 质量门

新代码必须通过现有的 `npm run verify`：

- Prettier / ESLint
- `tsc -b` 类型检查
- 覆盖率 80% 行覆盖门槛（新模块需自带测试以维持门槛）
- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace`

### 文档同步

- `CLAUDE.md` 的「Tauri IPC 接口」表增加 5 条 command
- `CLAUDE.md` 的架构说明中 `core/src/` 目录树增加 `env_var.rs`
- `CLAUDE.md`「关键约束」增加一条：**通用变量通路必须排除 `Path`，PATH 只能经专用通路编辑**
- `README.md` 功能章节增加"全环境变量管理"
- `CLAUDE.md` 与 `AGENTS.md` 必须保持一致（文件头已有此约定）

---

## 明确不做的事

| 不做项                                           | 理由                                                                                                                                                              |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **改名（重命名变量）**                           | 改名会产生"写入新名成功、删除旧名失败"的中间态，需要补偿事务与修复流程。v1 不承担这个复杂度。UI 不提供改名入口，`update_env_var` 不接收目标名参数（从契约层杜绝） |
| 全变量导入导出                                   | 本版分阶段；且导出密钥是真实泄露风险                                                                                                                              |
| profile 支持全变量                               | `ProfileData` 的 `sys`/`user` 结构保持不动，避免影响既有配置                                                                                                      |
| CLI 扩展                                         | 18 条命令保持 PATH-only，不破坏现有脚本与自动化                                                                                                                   |
| `REG_MULTI_SZ` / `REG_DWORD` / `REG_BINARY` 编辑 | 实测 0 个；遇到时只返回类型与`preview` 摘要，不强行转字符串                                                                                                       |
| 全变量变更历史 / 时间线                          | 属于 ROADMAP v5.2 范围                                                                                                                                            |
| 敏感值的加密存储                                 | 本设计让明文不进入 WebView，比前端打码强，但仍不是静态加密；加密需要密钥管理                                                                                      |
| 阻止用户用`regedit` 修改保护变量                 | PathEditor 只保证自身不会改坏系统，不承担注册表级强制                                                                                                             |

### 关于改名（暂不支持）

`update_env_var` 的签名**不含目标名**，这是刻意的：只要契约里没有改名能力，就不需要处理部分失败。

若未来要支持，需要：① 先校验新名不与现有名冲突（忽略大小写）；② 在同一 Rust 调用内写新名、删旧名；③ 删旧名失败时**回滚删除新名**；④ 回滚也失败时向用户报告"存在两个变量，需手工清理其一"，并提供刷新后的列表。这是一条独立的设计，不塞进 v1。

---

## 风险与未覆盖项

| 风险                                   | 缓解                                                                                       |
| -------------------------------------- | ------------------------------------------------------------------------------------------ |
| PATH 被通用通路绕过                    | `Path` 列入 `RESERVED_NAMES`，`list_all_env_vars` 过滤，Rust 写入口拒绝；测试专项覆盖      |
| 敏感明文进入 WebView / DevTools / 日志 | `EnvVarMeta` 契约上无 `value` 字段；明文仅经 `reveal_env_var` 按需获取；`revealed` 不落盘  |
| 检查与写入之间的竞态（TOCTOU）         | `expected_revision` 在 Rust 同一调用内比较并写入；前端不承担校验职责                       |
| 通用通路重新引入类型降级（Issue#26）   | 已有变量类型只从`get_raw_value().vtype` 读取；前端 `kind` 仅用于新建；非字符串类型拒绝写入 |
| 大小写处理错误导致重复变量             | 全程以原始名定位；名称校验忽略大小写；Rust 测试专项覆盖                                    |
| 保护名单不完整，漏掉某个关键变量       | 名单集中在单处常量，便于扩充；`Unsupported` 类型天然不可编辑                               |
| 非管理员误以为可编辑系统变量           | `canEdit`/`canDelete` 由 `canWriteSystem`/`canWriteUser` 参与计算，加载时即正确标记        |
| 两套模型（PATH 与通用）理解成本        | `CLAUDE.md` 明确记录边界与交汇点；`allVars` 使用独立工具栏，不共享 PATH 的按钮与拖放       |
| 覆盖率门槛因新增无测试代码而失败       | 每个新模块在 Part 4 中都有对应测试文件                                                     |

**未覆盖项**（本设计有意不解决）：

1. Rust/IPC 仍返回自由文本 `Result<T, String>`，未统一错误码 —— 这是既有技术债（CLAUDE.md 已记录），本设计不扩大也不修复
2. 真实 Tauri/注册表闭环测试仍需显式授权，本设计仅覆盖隔离键与 mock IPC
3. 改名能力（见上），以及由此衍生的事务修复流程

---

## 修订记录

| 日期       | 变更                                                                                                                                                                                                                                                                                                                                                                                                         |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 2026-09-16 | 初稿                                                                                                                                                                                                                                                                                                                                                                                                         |
| 2026-09-16 | 复审修订（第一轮）：P0 排除`Path`（保留变量）、P0 敏感明文不进前端（`EnvVarMeta` + `reveal_env_var`）、P1 revision 消除 TOCTOU、P1 类型不从注册表之外覆盖、P1 `canEdit`/`canDelete` 纳入 hive 写权限、P1 `allVars` 与 AppShell 接合方式；补充不做改名、非字符串类型摘要、通用校验、广播、测试清单                                                                                                            |
| 2026-09-16 | 复审修订（第二轮）：统一列表契约为 `list_all_env_vars() -> EnvVarSnapshot`（消除 Rust/前端不一致）；删除"前端保存前重新 list 比对"的残留段落；更正安全表述为"命中敏感规则的变量不进前端"并给出 R1/R2 取舍与 `preview` 净化规则；`activeHive` 正名为 `hiveFilter` 并统一类型；删除不可实现的 `canEdit=false / canDelete=true` fixture 并说明两标志恒同值；定义 `reveal_env_var` 对 `Unsupported` 类型返回错误 |
