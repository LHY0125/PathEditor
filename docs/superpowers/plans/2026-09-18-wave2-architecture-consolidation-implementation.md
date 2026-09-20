# 架构收口（Wave 2）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把复审报告里的五项架构问题收口——结构化错误契约（F-06）、拆分 `registry.rs`（F-07）、GUI/CLI 共享应用服务层（F-08）、C→Rust 行为等价基线（F-10）、持久化 schema 版本化与损坏恢复（F-11）。

**Architecture:** 先立错误契约再动结构（F-06 是所有后续的接口基础）；`registry.rs` 纯搬家拆成 `registry/` 目录（F-07），以 Wave 0 的 `EnvHiveStore` 端口作为 `test_adapter` 边界；把 GUI 与 CLI 各自编排的「读→比→写→快照→广播」下沉成 core 的应用服务（F-08），消除 Wave 1 里 F-03 留下的策略分叉。

**Tech Stack:** Rust workspace（`serde`、`thiserror` 可选）、`winreg`、React 19 + TS strict。

**Spec:** `docs/superpowers/specs/2026-09-18-consistency-and-architecture-design.md`
**前置:** Wave 0（端口）与 Wave 1（一致性修复）必须已完成。

## Global Constraints

- **分支**：main 的 worktree。
- **不推送、不升级版本号**。
- **禁止真实注册表写入**：Rust 测试走 `MemoryHive`；GUI E2E 走 mock IPC。
- **文档注释**：所有 `pub` / `pub(crate)` 项必须有 `///` 文档注释。
- **代码风格**：UTF-8、CRLF；Rust 4 空格缩进（rustfmt 100 列）；前端 Prettier。
- **质量门**：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo test -p patheditor-cli --bins`、`npm test`、`npx tsc -b`、`npm run lint` 全绿。
- **提交规范**：Conventional Commits；每 Task 一次提交。
- **不删除文件**，未经用户明确同意。
- **纯搬家不夹带语义变更**：F-07 的拆分与 F-06 的错误迁移分属不同 Task，各自独立可验。

## File Structure

| 文件                                                                    | 职责                                                                                                                               | 变更     |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | -------- |
| `core/src/error.rs`                                                     | `CoreError` + `ErrorCode` + `Result` 别名 + 退出码映射                                                                             | **新建** |
| `core/src/registry/`                                                    | 由 `registry.rs`（编写时约 1101 行，现 1203 行，W2-N3）拆分而来                                                                    | **重组** |
| `core/src/registry/{mod,path,env_var,access,conflict,test_adapter}.rs`  | 见 F-07（用 `conflict` 而非 `error`，避免与 crate 根的 `crate::error` 混淆）                                                       | **新建** |
| `core/src/service.rs`                                                   | 应用服务层（F-08）                                                                                                                 | **新建** |
| `core/src/registry/golden_tests.rs` + `core/src/registry/golden/*.json` | C→Rust golden 基线（F-10）。**必须放 crate 内 `#[cfg(test)]`**：`split_path`/`join_path` 是私有 `fn`，`core/tests/` 集成测试够不到 | **新建** |
| `core/src/disabled.rs`、`core/src/profiles.rs`                          | `schemaVersion` + `.bak` + quarantine（F-11）                                                                                      | 修改     |
| `gui/src/commands/*.rs`                                                 | 返回 `CoreError`（F-06）                                                                                                           | 修改     |
| `cli/src/runtime.rs`、`cli/src/env_ops.rs`                              | 退出码按 `ErrorCode` 映射                                                                                                          | 修改     |
| `cli/src/main.rs`、`cli/src/import_export.rs`、`cli/src/profile_ops.rs` | `persist_snapshot` 的调用点；保留同名同签名则**无需改**（见 Task 5 Step 2）                                                        | 视情况   |
| `src/services/backend.ts`、`src/core/*.ts`、i18n                        | 按 `code` 判定与本地化                                                                                                             | 修改     |

---

### Task 1: 定义 `CoreError` 与 `ErrorCode`（F-06）

**Files:**

- Create: `core/src/error.rs`
- Modify: `core/src/lib.rs`（`pub mod error;` + 重导出）

**Interfaces:**

- Produces:
  - `pub struct CoreError { code: ErrorCode, operation: String, hive: Option<EnvHive>, name: Option<String>, retryable: bool, message: String }`
  - `pub enum ErrorCode { Conflict, ReservedName, Protected, UnsupportedType, PermissionDenied, NotFound, NameExists, InvalidName, InvalidValue, Io, Parse, Internal }`
  - `impl std::fmt::Display for CoreError`（渲染 `message`）
  - `impl From<String> for CoreError`（过渡期：旧自由文本降级为 `Internal`，`message` 原样）
  - `impl CoreError { pub fn exit_code(&self) -> i32 }`（`Conflict → 3`，其余 `→ 1`）

- [ ] **Step 1: 写 `error.rs`**

```rust
//! 结构化错误契约。
//!
//! `code` 是机器可读的稳定判定依据：GUI 按它选 i18n、CLI 按它映射退出码。
//! `message` 是安全展示文案（中文），仅供人工阅读；程序分支**不得**匹配它。

use crate::env_var::EnvHive;

/// 稳定错误码。新增变体应保持向后兼容（前端/CLI 对未知码统一按 Internal 处理）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    /// revision 冲突，重试（重新读取）后可能恢复
    Conflict,
    /// `Path` 等保留名，走专用通路
    ReservedName,
    /// 保护名单（windir 等系统内置变量）
    Protected,
    /// 注册表类型不受支持（REG_DWORD 等）
    UnsupportedType,
    /// 无写权限
    PermissionDenied,
    /// 目标不存在
    NotFound,
    /// 同名已存在（新建时）
    NameExists,
    /// 名称非法
    InvalidName,
    /// 值非法
    InvalidValue,
    /// 磁盘/文件 IO 失败
    Io,
    /// 解析失败（JSON/注册表解码）
    Parse,
    /// 兜底
    Internal,
}

/// core 统一错误类型。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreError {
    pub code: ErrorCode,
    pub operation: String,
    pub hive: Option<EnvHive>,
    pub name: Option<String>,
    /// 是否为「重试后可能恢复」的错误（冲突、权限瞬时失败等）
    pub retryable: bool,
    /// 安全展示文案
    pub message: String,
}

impl CoreError {
    /// 构造一个错误。
    pub fn new(code: ErrorCode, operation: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            operation: operation.into(),
            hive: None,
            name: None,
            retryable: matches!(code, ErrorCode::Conflict),
            message: message.into(),
        }
    }

    /// 附加 hive 与变量名上下文。
    pub fn with_target(mut self, hive: EnvHive, name: impl Into<String>) -> Self {
        self.hive = Some(hive);
        self.name = Some(name.into());
        self
    }

    /// CLI 退出码映射：冲突 3，其余 1。
    pub fn exit_code(&self) -> i32 {
        if self.code == ErrorCode::Conflict {
            3
        } else {
            1
        }
    }
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// 过渡期转换：旧自由文本错误降级为 `Internal`，`message` 原样保留。
/// 迁移完成后应移除。
impl From<String> for CoreError {
    fn from(message: String) -> Self {
        CoreError::new(ErrorCode::Internal, "legacy", message)
    }
}
```

- [ ] **Step 2: 注册并重导出**

`lib.rs` 加 `pub mod error;`，并 `pub use error::{CoreError, ErrorCode};`。

- [ ] **Step 3: 单测**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_maps_to_exit_code_3() {
        let e = CoreError::new(ErrorCode::Conflict, "update_env_var", "冲突");
        assert_eq!(e.exit_code(), 3);
        assert!(e.retryable);
    }

    #[test]
    fn other_codes_map_to_exit_code_1() {
        let e = CoreError::new(ErrorCode::Protected, "update_env_var", "保护");
        assert_eq!(e.exit_code(), 1);
        assert!(!e.retryable);
    }

    #[test]
    fn serde_uses_camel_case_code() {
        let e = CoreError::new(ErrorCode::Conflict, "op", "m");
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["code"], serde_json::json!("conflict"));
        assert!(v.get("retryable").is_some());
    }
}
```

- [ ] **Step 4: 提交**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p path-editor-core error
git add core/src/error.rs core/src/lib.rs
git commit -m "feat(core): 新增结构化错误契约 CoreError/ErrorCode"
```

---

### Task 2: 把环境变量通路的错误迁移到 `CoreError`（F-06）

**Files:**

- Modify: `core/src/registry.rs` 的环境变量函数与 `list` 通路
- Modify: `core/src/reg_store.rs`（仅 `WinregHive::open` 构造函数，见 Step 1，W2-N2）

**Interfaces:**

- Produces（迁移后签名）：
  - `pub fn update_env_var(...) -> Result<(), CoreError>`
  - `pub fn delete_env_var(...) -> Result<(), CoreError>`
  - `pub fn create_env_var(...) -> Result<(), CoreError>`
  - `pub fn reveal_env_var(...) -> Result<RevealedValue, CoreError>`
  - `pub fn list_all_env_vars() -> Result<EnvVarSnapshot, CoreError>`
  - `update_env_var_force` / `delete_env_var_force` 同。

**迁移规则**（逐处把 `format!(...)` 换成带 `code` 的 `CoreError`）：

| 现状文案特征                              | 目标 `code`        |
| ----------------------------------------- | ------------------ |
| `ERR_CONFLICT`                            | `Conflict`         |
| 由专用 PATH 通路管理                      | `ReservedName`     |
| 系统内置变量                              | `Protected`        |
| 注册表类型不受支持                        | `UnsupportedType`  |
| 无法打开…注册表项 / 需要管理员权限        | `PermissionDenied` |
| 找不到（端口 `get_raw` 的 NotFound 分支） | `NotFound`         |
| 已存在，请使用编辑功能                    | `NameExists`       |
| 变量名不能为空 / 含 null / 等号 / 过长    | `InvalidName`      |
| 值包含 null / 超长                        | `InvalidValue`     |
| 无法读取/写入/删除/枚举（winreg 错误）    | `Io`               |
| 无法解码                                  | `Parse`            |

- [ ] **Step 1: 端口层保留 `String`，在 `registry.rs` 边界转 `CoreError`**

`EnvHiveStore` 仍返回 `Result<_, String>`（Wave 0 定义，避免大改端口）；`registry.rs` 在各写入口读取处 `.map_err(|m| CoreError::new(ErrorCode::Io, "update_env_var", m).with_target(hive, name))?`。

- [ ] **Step 1b: `WinregHive::open` 单独迁移返回 `CoreError`（W2-N2，2026-09-20 裁断采纳）**

迁移表里「无法打开…注册表项 / 需要管理员权限 → `PermissionDenied`」按文案特征分类，正是 F-06 要杀死的文本匹配。`WinregHive::open` 是 `reg_store.rs` 的固有方法、不在 `EnvHiveStore` trait 上，可以单独迁移：

```rust
// core/src/reg_store.rs —— winreg 错误按 io::ErrorKind 诚实分类
let key = winreg::RegKey::predef(root)
    .open_subkey_with_flags(sub_path, flags)
    .map_err(|e| {
        let code = if e.kind() == std::io::ErrorKind::PermissionDenied {
            crate::error::ErrorCode::PermissionDenied
        } else {
            crate::error::ErrorCode::Io
        };
        crate::error::CoreError::new(code, "open_env_key", format!("无法打开{}环境变量注册表项: {}", label, e))
    })?;
```

trait 的四个方法**保持 `String` 不动**（Wave 0 边界）。`registry.rs` 消费端把 `open` 的 `CoreError` 用 `.with_target(hive, name)` 补齐目标信息后直接透传。

- [ ] **Step 1c: `read_env_var` 内部三分类（W2-B1，2026-09-20 裁断采纳）**

`read_env_var`（registry.rs:230，私有 fn）产生三种性质不同的错误：读取失败（Io）、类型不支持（UnsupportedType）、解码失败（Parse）。Step 3 示例把它整体 `map_err` 成 `Io` 与本任务自己的迁移表自相矛盾，且 UnsupportedType 被 Io 掩盖后前端/CLI 无法按 code 区分「只读变量」与「读取故障」。

**裁决：`read_env_var` 随迁移改为内部返回 `Result<(RegType, String), CoreError>`，三分类在函数内完成**：

```rust
fn read_env_var(store: &dyn EnvHiveStore, name: &str) -> Result<(RegType, String), CoreError> {
    let raw = store.get_raw(name).map_err(|m| {
        CoreError::new(ErrorCode::Io, "read_env_var", m) /* .with_target 由调用方补 */
    })?;
    let kind = EnvValueKind::from_reg_type(raw.vtype.clone());
    if !kind.is_writable() {
        return Err(CoreError::new(ErrorCode::UnsupportedType, "read_env_var",
            format!("环境变量 {} 的注册表类型不受支持", name)));
    }
    let value = String::from_reg_value(&raw).map_err(|e| {
        CoreError::new(ErrorCode::Parse, "read_env_var", format!("无法解码环境变量 {}: {}", name, e))
    })?;
    Ok((raw.vtype, value))
}
```

调用方（update/create/reveal/force 等）不再对 `read_env_var` 的返回做 `map_err` 包裹，改为 `.with_target(hive, name)` 补目标信息后透传。Step 3 示例中 `let (vtype, current) = read_env_var(store, name).map_err(|m| …Io…)?` 一行**作废**，以本步为准。

- [ ] **Step 2: `ERR_CONFLICT` 改为构造 `CoreError`**

```rust
/// revision 冲突错误。
pub(crate) fn conflict_error(operation: &str, hive: EnvHive, name: &str) -> CoreError {
    CoreError::new(
        ErrorCode::Conflict,
        operation,
        "[E_CONFLICT] 变量已被其他进程修改，请重新加载",
    )
    .with_target(hive, name)
}
```

`conflict_message()` 保留（过渡期 CLI/GUI 仍可能读），但新增的判定一律走 `code`。

- [ ] **Step 3: 逐个函数改签名与错误构造**

以 `update_env_var_in_store` 为例：

```rust
fn update_env_var_in_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    name: &str,
    value: &str,
    expected_revision: &str,
) -> Result<(), CoreError> {
    validate_env_name(name)
        .map_err(|m| CoreError::new(ErrorCode::InvalidName, "update_env_var", m).with_target(hive, name))?;
    if is_reserved(name) {
        return Err(CoreError::new(ErrorCode::ReservedName, "update_env_var",
            format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name)).with_target(hive, name));
    }
    if is_protected(name) {
        return Err(CoreError::new(ErrorCode::Protected, "update_env_var",
            format!("{} 是系统内置变量，不允许修改", name)).with_target(hive, name));
    }
    let (vtype, current) = read_env_var(store, name)
        .map_err(|m| CoreError::new(ErrorCode::Io, "update_env_var", m).with_target(hive, name))?;
    let current_revision = revision_of(name, vtype.clone(), &current);
    if current_revision != expected_revision {
        return Err(conflict_error("update_env_var", hive, name));
    }
    let kind = EnvValueKind::from_reg_type(vtype.clone());
    if !kind.is_writable() {
        return Err(CoreError::new(ErrorCode::UnsupportedType, "update_env_var",
            format!("{} 的注册表类型不受支持，无法修改（仅可查看）", name)).with_target(hive, name));
    }
    validate_env_value(value, name)
        .map_err(|m| CoreError::new(ErrorCode::InvalidValue, "update_env_var", m).with_target(hive, name))?;
    write_env_var(store, name, value, vtype)
        .map_err(|m| CoreError::new(ErrorCode::Io, "update_env_var", m).with_target(hive, name))
}
```

其余（create/delete/reveal/list/force）按同一规则迁移。

- [ ] **Step 3b: `read_env_var` 错误补 hive 标签（Wave 1 审查报告登记 3，原定 Wave 1 顺带处理）**

`read_env_var`（registry.rs:230）的错误不带 hive 标签，跨 hive 排障时无法区分来源。**机制裁断（W2-N5，2026-09-20）**：`read_env_var` 签名无 hive 参数，不必为此改签名——W2-B1 落地后 `CoreError` 已有结构化 `hive` 字段，调用方 `.with_target(hive, name)` 即携带；`message` 文本标签在调用侧包一层（「读取系统环境变量 XXX 失败…」样式，与 F-04 列表路径一致）。审核责任说明：Wave 0 报告登记为「Wave 1 顺带」，但未写进 Wave 1 计划，移交至此。

- [ ] **Step 4: 改测试断言**

已有测试断言 `result.unwrap_err() == ERR_CONFLICT` 的改为 `assert_eq!(result.unwrap_err().code, ErrorCode::Conflict)`；断言 `contains("类型不受支持")` 的改为断言 `code == ErrorCode::UnsupportedType`。

- [ ] **Step 5: 质量门 + 提交**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add core/src/registry.rs
git commit -m "refactor(core): 环境变量通路错误迁移到 CoreError"
```

---

### Task 3: Tauri / CLI / 前端按 `code` 判定（F-06）

**Files:**

- Modify: `gui/src/commands/env_var.rs`（返回 `Result<_, CoreError>`）
- Modify: `cli/src/runtime.rs`（`exit_conflict`/`is_conflict` → `exit_core_error(&CoreError)`）
- Modify: `cli/src/env_ops.rs`（`apply_concurrency` 改收 `Result<(), CoreError>`）
- Modify: `src/services/backend.ts`（错误解析为 `{code,message}`）
- Modify: `src/store/env-store.ts`（`isConflictError` 改看 `code`）
- Modify: `src/i18n/locales/*.json`（`error.code.<code>` 文案）

- [ ] **Step 1: CLI 退出码**

```rust
/// 按结构化错误决定退出码与输出。
pub(crate) fn exit_core_error(err: &core::CoreError) -> ! {
    eprintln!("错误: {}", err.message);
    std::process::exit(err.exit_code());
}

/// 统一处理写操作结果：冲突 3，其余 1（由 CoreError 决定）。
pub(crate) fn apply_core_result(result: Result<(), core::CoreError>) {
    if let Err(e) = result {
        exit_core_error(&e);
    }
}
```

删除 `CONFLICT_PREFIX` / `is_conflict` / `exit_conflict` 的文字匹配（保留 `conflict_prefix_matches_core_constant` 契约测试改为断言 `CoreError::Conflict.exit_code() == 3`）。

- [ ] **Step 2: 前端**

`backend.ts` 把 Tauri 的 rejection 解析为 `{ code: ErrorCode; message: string }`（形状校验，未知 code → `internal`）。**过渡期双形状兼容（W2-N4）**：Task 2/3 完成前 PATH 命令仍返回纯文本 `String`、env 命令已返回 `CoreError` 对象——解析必须两种形状都接受：对象 → 按 `code`；纯字符串 → 兜底为 `internal`、原文进 `message`。全部 PATH 命令迁移完毕后此兼容层可收紧（保留到 Wave 2 收口，届时在 Execution Notes 记录是否移除）。

`cli/src/env_ops.rs`：`exit_err(&e)` 收 `&str`，env 命令改用 `exit_core_error` 后，**该文件里所有**收 `Result<_, CoreError>` 的调用点都要取 `.message` 或改用 `exit_core_error`，不只 `apply_concurrency` 一处——实施时全文件排查 `unwrap_or_else(|e| exit_err(&e))` 形态。

`env-store.ts`：

```ts
function isConflictError(err: { code: string } | null): boolean {
  return err?.code === 'conflict';
}
```

i18n 增加 `error.code.conflict` / `error.code.protected` 等键，两端同步。

- [ ] **Step 3: 单测/E2E**

更新 CLI 的退出码测试与前端 mock 的冲突注入（改为注入 `{code:'conflict', ...}`）。

- [ ] **Step 4: 质量门 + 提交**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace && cargo test -p patheditor-cli --bins
npx tsc -b && npm run lint && npm test && npm run test:e2e
git add gui/src/commands/env_var.rs cli/src src/services/backend.ts src/store/env-store.ts src/i18n tests e2e
git commit -m "refactor: 错误判定改为按 CoreError.code（Tauri/CLI/前端）"
```

---

### Task 4: 拆分 `registry.rs` 为 `registry/` 目录（F-07，纯搬家）

**Files:**

- Create: `core/src/registry/{mod,path,env_var,access,error,test_adapter}.rs`
- Modify: `core/src/registry.rs`（内容搬空后**保留为模块根**，只留子模块声明——W2-N6，2026-09-20 裁断：首选方案）
- Modify: `core/src/lib.rs`

> **W2-N6 模块组织裁断（2026-09-20）**：Rust 2018 起允许 `registry.rs` 作模块根 + `registry/` 目录放子模块（两者并存合法；E0761 只在 `registry.rs` 与 `registry/mod.rs` 同时存在时触发）。**首选保留 `registry.rs` 作模块根**（仅 `mod access; mod conflict; …` 声明），完全避开文件删除授权问题；原「Delete + 需用户确认」降为备选：若实施中选定 `registry/mod.rs` 形态，才需用户确认删除 `registry.rs`。

**目标结构**（内容按职责搬，**不改行为**）：

```text
core/src/registry/
  registry.rs     # 模块根（保留）：只留 mod 声明与重导出（W2-N6 首选形态）
  path.rs         # load_paths/save_paths/split_path/join_path/validate_and_join_paths/clean_*
  env_var.rs      # update/delete/create/reveal/list(_store)/force 系列/revision 辅助
  access.rs       # hive_location/can_write_user/env_key 语义/权限探测
  conflict.rs     # 注册表侧冲突：ERR_CONFLICT 常量 + conflict_error 构造
  test_adapter.rs # 测试用装配（MemoryHive 装配、测试夹具）
```

> W2-N3（2026-09-20）：行号锚点按符号定位——`registry.rs` 现为 **1203 行**（计划基线写 1101 已漂移）；`env_key` 已在 Wave 0 删除，上文「若仍在用」措辞保留；`split_path`/`join_path` 行号以实施时 grep 为准。

`EnvHiveStore` 端口仍在 `core/src/reg_store.rs`（Wave 0），`test_adapter` 只做装配。

- [ ] **Step 1: 建立目录并搬 `path.rs`**

把 PATH 相关函数整体移入 `path.rs`，逐个 `pub(crate)` 保持可见性；`mod.rs` 里 `pub(crate) use path::*;`。

- [ ] **Step 2: 搬 `access.rs`**

`hive_location`、`can_write_user`、`env_key`（若仍在用）、权限探测。

- [ ] **Step 3: 搬 `env_var.rs`（registry 侧）**

环境变量 CRUD 与 `list` 通路。

- [ ] **Step 4: 模块根重导出，保证外部路径不变**

**W2-N6 首选形态**：重导出写在保留的 `core/src/registry.rs`（模块根）里，不建 `mod.rs`：

```rust
//! 注册表访问统一入口。外部（gui/cli）一律经此路径引用，拆分对调用方不可见。
//!
//! 与 crate 根的 `crate::error` 区分：那是 F-06 的结构化 `CoreError`；
//! 本模块的 `conflict` 只承载注册表侧的冲突常量与构造。
mod access;
mod conflict;
mod env_var;
mod path;

pub use access::{can_write_user, hive_location};
pub use conflict::conflict_message;
pub use env_var::{
    create_env_var, delete_env_var, delete_env_var_force, list_all_env_vars, reveal_env_var,
    update_env_var, update_env_var_force, validate_env_name, validate_env_value,
};
pub use path::{
    clean_path_entries, clean_paths, load_system_paths, load_user_paths, save_system_paths,
    save_user_paths,
};

/// 注册表内部使用：冲突构造与常量，不对外暴露。
pub(crate) use conflict::{conflict_error, ERR_CONFLICT};

#[cfg(test)]
mod test_adapter;
```

> **不要留占位符号**：上面列出的符号全部真实存在。落实时逐个核对；`ERR_CONFLICT` 目前是 `pub(crate)`，只能 `pub(crate) use` 重导出，不能 `pub use`。

- [ ] **Step 5: 验证零行为变化**

Run: `cargo test --workspace`

Expected: 与拆分前**同样的测试数、全绿**。`git diff --stat` 只应看到文件移动，不应有语义改动（用 `git diff -M` 检查重命名检测）。

- [ ] **Step 6: 提交**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add core/src/registry core/src/registry.rs core/src/lib.rs
git commit -m "refactor(core): registry.rs 拆分为 registry/ 目录（纯搬家）"
```

---

### Task 5: 共享应用服务层（F-08）

**Files:**

- Create: `core/src/service.rs`
- Modify: `core/src/lib.rs`（`pub mod service;`）
- Modify: `gui/src/services` 与 `cli/src/runtime.rs`、`cli/src/profile_ops.rs` 调用新服务

**Interfaces:**

- Produces:

```rust
/// 单个 hive 的应用结果。
///
/// 需过 Tauri IPC（W2-B3），必须可序列化。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HiveOutcome { Applied, Skipped, Failed(CoreError) }

/// sidecar（disabled.json / PATH 快照 / pending）的落盘结果。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SidecarOutcome { Saved, Failed(CoreError), Pending }

/// 一次 PATH/变量应用的整体结果。
///
/// **按 hive 分字段**：多 hive 应用必须能同时表达「系统成功 / 用户失败」这种
/// partial 结果，单个 `registry: HiveOutcome` 字段装不下（评审 W2-B1）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    pub system: HiveOutcome,
    pub user: HiveOutcome,
    pub sidecar: SidecarOutcome,
}

/// 把某 hive 的 PATH 写入注册表并落盘快照（含 pending 恢复语义）。
pub fn apply_path_snapshot(hive: EnvHive, entries: Vec<PathEntry>) -> Result<ApplyOutcome, CoreError>;

/// 同时写注册表与 sidecar，明确 partial 语义。
pub fn save_path_with_sidecar(
    system: Option<Vec<PathEntry>>,
    user: Option<Vec<PathEntry>>,
) -> Result<ApplyOutcome, CoreError>;

/// 补写上次未落盘的快照。
pub fn retry_pending_path_state() -> Result<ApplyOutcome, CoreError>;

/// 应用一个 profile（多 hive；语义为 best-effort，逐 hive 结果可见）。
pub fn apply_profile(name: &str) -> Result<ApplyOutcome, CoreError>;
```

- [ ] **Step 1: 实现 `service.rs`（把 CLI 的 `runtime.rs` 编排与 GUI 的 `path-session.ts` 语义收敛到此处）**

多 hive 语义**明确选定 best-effort**：逐 hive 尝试，失败不阻断另一 hive，结果在 `ApplyOutcome` 的 `system` / `user` 字段里分别表达，并据此决定整体退出码/提示。单 hive 的 `apply_path_snapshot(hive, …)` 把未触及的另一 hive 置为 `HiveOutcome::Skipped`。

- [ ] **Step 2: CLI 改调服务**

`cli/src/runtime.rs` 的 `load_and_save` / `load_operate_save` / `persist_snapshot` 改为调用 `core::service::save_path_with_sidecar` / `apply_path_snapshot`；`profile_ops.rs` 改调 `apply_profile`。Wave 1 在 CLI 侧写的 pending 逻辑移入服务层（`save_path_with_sidecar` 内部处理失败落 pending）。

**`persist_snapshot` 的全部调用点（评审 W2-N3，已核实；开发窗口核对补注：runtime.rs 内部另有 4 处调用——68/73/99/101，均在 `load_and_save` / `load_operate_save` 内部，本任务改造这两个函数时自然覆盖，无需单列）**：

- 定义：`cli/src/runtime.rs:126`
- 调用：`cli/src/profile_ops.rs:64`、`cli/src/main.rs:451`、`cli/src/main.rs:453`、`cli/src/import_export.rs:12`、`cli/src/import_export.rs:18`、`cli/src/import_export.rs:30`

**推荐做法**：保留 `persist_snapshot` 同名同签名，只把它的**函数体**改为转调 `core::service::save_path_with_sidecar`。这样上述 6 个站点无需改动。若确要换名/换签名，必须在同一提交内同步这 6 处——本 Task 请明确选择其一并写进 Execution Notes。

- [ ] **Step 3: GUI 改调服务**

`src/services/path-session.ts` 的编排经 Tauri 命令转发到 `service`（新增/调整 `gui/src/commands/` 的封装）。

- [ ] **Step 4: 故障注入测试**

对 `save_path_with_sidecar` 注入两类故障：

- 注册表成功 + sidecar 失败 → `outcome.sidecar == SidecarOutcome::Pending`，且 pending 文件存在；
- 系统成功 + 用户失败 → `outcome.system == HiveOutcome::Applied && matches!(outcome.user, HiveOutcome::Failed(_))`（partial 现在可表达）。

- [ ] **Step 4b: Wave 1 让步的第三类故障注入（Wave 1 审查报告登记 5）**

Wave 1 只交付了 pending 生命周期单测 + 行为验证，spec F-03 要求的三类注入中「快照成功/注册表失败」路径缺测试。

**注入机制裁断（W2-B2，2026-09-20）**：原拟「`MemoryHive` 注入 `set_raw` 失败」不成立——`MemoryHive` 只服务环境变量通路（`fail_enum`/`fail_get`，无 `fail_set`），而 PATH 通路 Wave 0 明确不纳入端口，`save_path_with_sidecar` 走 `registry::save_paths` 真实注册表。**改用输入校验强制注册表阶段失败**：构造含 null 字节 / 超过 32767 UTF-16 长度的 PATH 条目，使 `validate_and_join_paths`（registry.rs:149，在触碰注册表前调用，`save_paths` 首行）返回 Err——与「注册表写失败」走同一段代码路径（注册表阶段失败 → 不得产生 pending 文件）。断言：`outcome` 表达注册表侧 `Failed`、`~/.patheditor/pending_path_snapshot.json` 不产生。

flush 重放验证：先注入 sidecar 失败产生 pending（sidecar 失败注入方式见上类），恢复后调 `retry_pending_path_state()`，断言快照落盘且 pending 清除。**retry 保持 sidecar-only**（W2-B2 附带裁断）：`retry_pending_path_state` 经 `save_path_snapshot` 只写 sidecar、**不触碰注册表**——pending 的产生前提就是注册表已写成功，补写不得重写注册表。

**flush 语义分层（W2-N1，2026-09-20 裁断采纳）**：服务层返回 `Result<ApplyOutcome, CoreError>` 是诚实的错误报告；CLI 策略层保持 best-effort——`flush_pending_snapshot` 把 `retry_pending_path_state` 的 `Err` 映射为 `eprintln!` 警告、不阻断当前命令（Wave 1 行为不变）。即：**报告层收紧、策略层不变**，迁移后补写失败仍是警告 + 继续执行。

- [ ] **Step 5: 质量门 + 提交（对应 Task 5 整体）**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
npx tsc -b && npm test
git add core/src/service.rs core/src/lib.rs cli/src gui/src src
git commit -m "feat(core): 新增共享应用服务层，统一 PATH/profile 事务编排"
```

---

### Task 6: 持久化 schema 版本化与损坏恢复（F-11）

**Files:**

- Modify: `core/src/disabled.rs`、`core/src/profiles.rs`
- Modify（牵连，W2-B3 裁断 (a)）：`core/src/service.rs`（Task 5 刚建，消费 `load_path_snapshot` 等的错误类型变化）、`cli/src/runtime.rs`（`flush_pending_snapshot` 直接调 `save_path_snapshot`）、`gui/src/commands/disabled.rs`

**W2-B3 裁断（2026-09-20，二选一取 (a)）**：本任务对 `disabled.rs` + `profiles.rs` **全面迁移 `CoreError`**（与 F-11 的 `ErrorCode::Parse` 一致，quarantine 返回 `Result<_, CoreError>` 保持），牵连调用点随同编译修复——宁可 Files 清单诚实，不留编译期惊喜。**放弃 (b) quarantine 降级返回 `String`**：那会在 F-06 贯通四层的同一波里再留一个自由文本孤岛，下波还得拆。

**Interfaces:**

- `disabled.json` / `profiles/*.json` / **`pending_path_snapshot.json`** 顶层增加 `schemaVersion: u32`（当前 `1`）。

  > **pending 文件一并纳入（Wave 1 审查报告登记 4）**：`pending_path_snapshot.json` 是 Wave 1 新增的持久化文件，与 `disabled.json` 一样无版本头。F-11 覆盖清单若只写 `disabled.json` / `profiles`，会漏掉这个新成员。

- 写入前保留上一份 `<file>.bak`（轮换）。
- 读取解析失败：把坏文件移到 `<file>.corrupt-<ts>` 并返回可识别错误（`ErrorCode::Parse`），**不再只返回通用 JSON 错误**。
- `migrate(value, from_version)`：当前只支持 `1`；未知更高版本 → 返回 `ErrorCode::Parse` 并提示「文件由更新版本写入」。

- [ ] **Step 1: 加 `schemaVersion` 与 `.bak`**

`save_*` 时先 `atomic_write` 到 `<file>.bak`（若非首次），再写主文件。

- [ ] **Step 2: quarantine**

```rust
/// 读取失败时把损坏文件隔离到 `<file>.corrupt-<ts>`，返回可识别错误。
fn quarantine(path: &Path) -> Result<PathBuf, CoreError> { /* rename */ }
```

- [ ] **Step 3: migration 测试**

- 无 `schemaVersion` 的旧文件 → 按 v1 读取（向后兼容）。
- 截断的 JSON → 文件被隔离、返回 `Parse`。
- `schemaVersion: 999` → 返回 `Parse` 且提示版本过高。

- [ ] **Step 4: 质量门 + 提交**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p path-editor-core
git add core/src/disabled.rs core/src/profiles.rs
git commit -m "feat(core): 持久化文件增加 schemaVersion、.bak 与损坏隔离"
```

---

### Task 7: C→Rust 行为等价基线（F-10）

**Files:**

- Create: `core/src/registry/golden_tests.rs`（声明为 `registry/mod.rs` 内的 `#[cfg(test)] mod golden_tests;`）
- Create: `core/src/registry/golden/*.json`（输入快照 + 期望结果，用 `include_str!` 读入）

> **为什么不放 `core/tests/`（评审 W2-B3，指第一轮评审的 golden 位置异议，与 Task 6 的 W2-B3 无关）**：`core/tests/` 是集成测试，只能访问 crate 的 `pub` API；而 F-10 要覆盖的 `split_path`/`join_path` 是私有 `fn`（行号按符号定位，W2-N3）。放进 crate 内的 `#[cfg(test)] mod` 才能 `use super::*` 够到它们。且 `core/tests/` 目录当前不存在，不必新建。若某个 golden 用例只涉及公开 API（如 `clean_paths`），放哪都行；但为统一，全部放这一个模块。

**目标**：以数据驱动的方式固定「输入注册表快照 + 操作 → 期望注册表/文件/广播结果」，覆盖复审报告列出的六类行为。

- [ ] **Step 1: 定义 golden 格式**

```json
{
  "name": "path_split_trims_and_drops_empty",
  "input": { "path": " C:\\ ; ; D:\\ " },
  "op": "split_path",
  "expect": ["C:\\", "D:\\"]
}
```

- [ ] **Step 2: 覆盖六类行为**

1. PATH 分割、空项、空白、重复项（`split_path` / `join_path` / `clean_path_entries`）
2. `REG_SZ` / `REG_EXPAND_SZ` 写回类型保持
3. 权限失败时的行为（`MemoryHive::new(false)` 下的 capabilities）
4. 备份格式与恢复可用性（`backup.rs`）
5. profile / 导入导出 / 禁用项在升级后的兼容性
6. `WM_SETTINGCHANGE` 广播时机（以「写入口调用点」为可断言的替身：断言成功路径会调用广播、失败/冲突路径不调用——如已抽出可注入的广播端口则直接断言）

- [ ] **Step 3: 迁移记录**

每个与旧 C 行为**有意不同**的用例，在 `core/src/registry/golden/README.md` 标注「旧行为 / 新行为 / 改变原因」。

- [ ] **Step 4: 提交**

```bash
cargo test --workspace
git add core/src/registry
git commit -m "test(core): 新增 C→Rust 行为等价 golden 基线"
```

---

### Task 8: 收口质量门与文档收口

- [ ] **Step 1: 全量质量门**

```bash
npm run verify:all
```

- [ ] **Step 1b: 契约测试补漏（Wave 1 审查报告登记 2）**

`tests/unit/backend-env-contract.test.ts` 目前只覆盖 `EnvVarMeta` 契约。补三类直接用例（若 Task 3 重构了 `parseRevealedValue`，按新形态写）：

- `parseRevealedValue`：非 record 拒绝、`value`/`revision` 非 string 拒绝、空 revision 拒绝；
- `parseEnvVarSnapshot`：`capturedAt` 缺失 / 非 number 时回退 0 的直接断言；
- 契约测试文件补 `RevealedValue` 分组（现文件 34-78 行的 describe 只测 EnvVarMeta）。

- [ ] **Step 1c: 文档收口（开发窗口自报 Minor #4/#5 + 流程固化）**

1. `CLAUDE.md`/`AGENTS.md` 的 Tauri IPC 表中 `reveal_env_var` 签名仍写 `Result<String, String>`，与 Wave 1 实际（`RevealedValue`，Wave 2 起为 `Result<RevealedValue, CoreError>`）不符——同步两份文件（保持字节级一致）。
2. `env set` / `env remove` 的 clap 帮助文本与 Wave 1 后的实际语义核对（`--force` 描述已改过一轮，本轮终态核对）。
3. README 版本徽章/命令说明与终态核对。
4. **流程固化**：开发窗口的开发回执必须落盘为 `docs/审核和开发/YYYY.MM.DD/PathEditor-<feature>开发回执.md`（Wave 0/Wave 1 连续两轮以聊天摘要交付，Wave 0 审查报告 Minor 3 与 Wave 1 审查报告 Minor 2 两次登记）。

- [ ] **Step 2: 真实 Tauri/注册表集成**

**需用户显式授权与专用环境**。未授权则如实登记为未覆盖项，不得声称完成。

- [ ] **Step 3: 回填各计划 Execution Notes + 更新 spec 状态**

把 spec 状态从「待评审」改为「已实现（Wave 2）」，并在 `## Execution Notes` 记录偏离。

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "chore: Wave 2 架构收口质量门与文档同步"
```

---

## Self-Review

**1. Spec coverage**

| Spec 条目                                                   | 任务         |
| ----------------------------------------------------------- | ------------ |
| F-06 `CoreError` 贯通四层、退出码与 i18n 由 code 驱动       | Task 1、2、3 |
| F-07 `registry.rs` 拆目录、外部路径兼容、零行为变化         | Task 4       |
| F-08 应用服务层、多 hive 语义明确                           | Task 5       |
| F-10 golden behavior matrix + 迁移记录                      | Task 7       |
| F-11 `schemaVersion` + `.bak` + quarantine + migration 测试 | Task 6       |

**Wave 1 审查报告移交的 6 项登记（2026-09-20 折入）**：

| 登记项                                                       | 落点              |
| ------------------------------------------------------------ | ----------------- |
| 1. 开发回执落盘固化（复发项）                                | Task 8 Step 1c    |
| 2. `parseRevealedValue` / capturedAt 直接契约测试            | Task 8 Step 1b    |
| 3. `read_env_var` 错误 hive 标签（随 F-06）                  | Task 2 Step 3b    |
| 4. pending 文件纳入 F-11 覆盖清单                            | Task 6 Interfaces |
| 5. F-03 故障注入测试（注册表失败 / flush 重放，随 F-08）     | Task 5 Step 4b    |
| 6. IPC 表 `RevealedValue` 签名 + clap 帮助文本 + README 核对 | Task 8 Step 1c    |

**2. Placeholder scan**

Task 4 Step 4 的 `pub use` 列表与 Task 5/6/7 的若干函数体留待实现时按真实内容补全（`/* ... */`）——这些步骤的**接口签名与验证命令已给全**，且 Task 4 明确要求「以实际搬迁结果为准，不留占位符号」。其余步骤为完整代码。

**3. Type consistency**

| 符号                                                    | 定义   | 使用         | 一致 |
| ------------------------------------------------------- | ------ | ------------ | ---- |
| `CoreError{code,operation,hive,name,retryable,message}` | Task 1 | Task 2、3    | ✓    |
| `ErrorCode` 12 变体                                     | Task 1 | Task 2、3、6 | ✓    |
| `CoreError::exit_code()`                                | Task 1 | Task 3       | ✓    |
| `registry/` 六模块                                      | Task 4 | Task 5、6    | ✓    |
| `ApplyOutcome{HiveOutcome,SidecarOutcome}`              | Task 5 | Task 5       | ✓    |
| `schemaVersion` / quarantine                            | Task 6 | Task 6       | ✓    |

## 开发窗口核对轮（2026-09-20，W2-B1~B4 / W2-N1~N6）

开发窗口逐行核对，4 阻塞 + 6 非阻塞全部经审核窗口对源码核实后成立并折入：

| #     | 异议                                                                   | 裁断                                                                   | 落点                                       |
| ----- | ---------------------------------------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------ |
| W2-B1 | `read_env_var` 三种错误被整体包成 `Io`，与迁移表自相矛盾               | 采纳：内部返回 `CoreError` 三分类                                      | Task 2 Step 1c                             |
| W2-B2 | 故障注入拟用 `MemoryHive`+`fail_set`，但 PATH 通路不在端口上且无该字段 | 采纳：输入校验（null/超长）强制注册表阶段失败；retry 保持 sidecar-only | Task 5 Step 4b                             |
| W2-B3 | Task 6 引入 `CoreError` 牵连 3 文件未列；Outcome 枚举缺 serde          | 采纳 (a) 全面迁移并列牵连文件；Outcome 补 `Serialize/Deserialize`      | Task 6 Files/Interfaces；Task 5 Interfaces |
| W2-B4 | 迁移记录/提交写 `core/tests/golden`，与 File Structure 裁决矛盾        | 采纳：统一 `core/src/registry/golden/`                                 | Task 7 Step 3/4                            |
| W2-N1 | flush 语义收紧疑虑                                                     | 采纳开发窗口分层方案：服务层诚实报错、CLI 策略层保持 best-effort 警告  | Task 5 Step 4b 末段                        |
| W2-N2 | 「无法打开→PermissionDenied」按文案分类                                | 采纳：`WinregHive::open` 固有方法单独迁移，`ErrorKind` 诚实分类        | Task 2 Step 1b                             |
| W2-N3 | 行号漂移（1203 vs 1101）、`env_key` 已删                               | 采纳：符号定位                                                         | Task 4 备注                                |
| W2-N4 | 过渡期双错误形状兼容 + `exit_err` 收 `&str` 的调用点排查               | 采纳                                                                   | Task 3 Step 2                              |
| W2-N5 | Step 3b 机制未指明（`read_env_var` 无 hive 参数）                      | 采纳：结构化 `hive` 字段 + 调用侧文本包装                              | Task 2 Step 3b                             |
| W2-N6 | Step 5 标题重复；建议 `registry.rs` 保留作模块根避开删除授权           | 采纳：保留模块根为首选                                                 | Task 4 Files/Step 4；Task 5 Step 5         |

另核实：`persist_snapshot` 的 4 处 runtime.rs 内部调用点（68/73/99/101）由 Task 5 改造 `load_and_save`/`load_operate_save` 自然覆盖，非缺陷。

## Execution Notes

（留给开发窗口回填：一行一项，「计划原文 / 实际 / 处理」。）
