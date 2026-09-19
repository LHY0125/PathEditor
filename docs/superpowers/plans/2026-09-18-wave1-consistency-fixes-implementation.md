# 数据一致性修复（Wave 1）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修掉三个 P1 数据一致性缺陷（F-01 编辑旧值覆盖、F-02 `--force` 语义、F-03 CLI 双写失败无恢复），并给双 hive 快照补上时刻字段（F-05）。

**Architecture:** core 新增 force 系列写入口（显式豁免 revision、不豁免安全校验）；`reveal_env_var` 返回值扩展为携带 revision 的结构体，前端编辑弹窗据此绑定读值版本；CLI 在 sidecar 写失败时落 pending 文件而非直接退出。

**Tech Stack:** Rust workspace、`serde`、`winreg`、React 19 + TypeScript strict、Zustand、vitest、Playwright。

**Spec:** `docs/superpowers/specs/2026-09-18-consistency-and-architecture-design.md`
**前置:** Wave 0 计划（`2026-09-18-wave0-registry-port-implementation.md`）必须先完成——本计划沿用 `EnvHiveStore` 端口与 `MemoryHive`。

## Global Constraints

- **分支**：main 的 worktree（`worktree-<name>`），不在 main 直接改。
- **不推送、不升级版本号**；提交落本地分支。
- **禁止真实注册表写入**：所有 Rust 单测走 `MemoryHive`；GUI 的 E2E 走 mock IPC。真实注册表闭环需用户显式授权。
- **文档注释**：所有 `pub` / `pub(crate)` 项必须有 `///` 文档注释。
- **代码风格**：UTF-8、CRLF；Rust 4 空格缩进；前端 Prettier。
- **质量门**：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo test -p patheditor-cli --bins`、`npm test`、`npx tsc -b`、`npm run lint` 全绿。
- **提交规范**：Conventional Commits；每 Task 一次提交。
- **不删除文件**，未经用户明确同意。
- **安全判定单一实现处**：`--force` 只豁免 revision 校验；保留名 / 保护名单 / `Unsupported` / hive 写权限**一律照旧由 core 判定**。

## File Structure

| 文件                                                                                              | 职责                                                             | 变更 |
| ------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- | ---- |
| `core/src/registry.rs`                                                                            | force 系列写入口；`reveal` 返回 revision                         | 修改 |
| `core/src/env_var.rs`                                                                             | `RevealedValue` 契约；`EnvVarSnapshot.captured_at`               | 修改 |
| `core/src/lib.rs`                                                                                 | 重导出 `RevealedValue`                                           | 修改 |
| `core/src/disabled.rs`                                                                            | pending 快照原语（F-03）                                         | 修改 |
| `gui/src/commands/env_var.rs`                                                                     | `reveal_env_var` 返回类型                                        | 修改 |
| `cli/src/env_ops.rs`                                                                              | force 分支改调 force API；删除模拟实现                           | 修改 |
| `cli/src/runtime.rs`                                                                              | sidecar 失败落 pending + 补写（F-03）                            | 修改 |
| `src/core/env-var.ts`                                                                             | `RevealedValue` / `capturedAt` 类型                              | 修改 |
| `src/services/backend.ts`                                                                         | `revealEnvVar` 返回结构；`parseEnvVarSnapshot` 接受 `capturedAt` | 修改 |
| `src/store/env-store.ts`                                                                          | `fetchFullValue` 与 `reveal` 均按新契约解构；`save` 校验读值版本 | 修改 |
| `src/components/dialogs/EditEnvVarDialog.tsx`                                                     | 绑定 `readRevision`；陈旧则重取                                  | 修改 |
| `src/components/layout/AppShell.tsx`                                                              | `onConfirm` 透传 `readRevision`                                  | 修改 |
| `src/i18n/locales/{zh-CN,en}.json`                                                                | 新增 `envVar.staleReloaded`                                      | 修改 |
| `docs/superpowers/specs/2026-09-17-cli-env-vars-design.md`、`README.md`、`AGENTS.md`、`CLAUDE.md` | `--force` 语义与退出码文案对齐                                   | 修改 |

---

### Task 1: core 真正的 force API（F-02）

**Files:**

- Modify: `core/src/registry.rs`（在 `delete_env_var` 之后追加）

**Interfaces:**

- Consumes: `EnvHiveStore`、`WinregHive`（Wave 0）
- Produces:
  - `pub fn update_env_var_force(hive: EnvHive, name: &str, value: &str) -> Result<(), String>`
  - `pub fn delete_env_var_force(hive: EnvHive, name: &str) -> Result<(), String>`
  - 内部：`update_env_var_force_in_store(&dyn EnvHiveStore, &str, &str)`、`delete_env_var_force_in_store(&dyn EnvHiveStore, &str)`

**用户裁决（2026-09-18）**：`--force` 必须是真的 force —— 最后写入者胜。**只豁免 revision 校验，不豁免任何安全校验。**

- [ ] **Step 1: 写失败测试**

在 `core/src/registry.rs` 的 `env_var_tests` 模块追加：

```rust
    #[test]
    fn update_env_var_force_overwrites_after_external_change() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);

        // 模拟：用户读到 revision 后，另一进程改了值
        hive.seed("MY_VAR", "changed-by-other", REG_SZ);

        // force 不比对 revision，必须成功覆盖
        update_env_var_force_in_store(&hive, "MY_VAR", "mine").expect("force 写入必须成功");

        let raw = hive.get_raw("MY_VAR").expect("读取失败");
        assert_eq!(String::from_reg_value(&raw).unwrap(), "mine");
    }

    #[test]
    fn update_env_var_force_preserves_registry_type() {
        let hive = MemoryHive::new(true);
        hive.seed("GOPATH", "C:\\Old", REG_EXPAND_SZ);

        update_env_var_force_in_store(&hive, "GOPATH", "C:\\New").expect("force 写入失败");

        assert_eq!(hive.get_raw("GOPATH").unwrap().vtype, REG_EXPAND_SZ);
    }

    #[test]
    fn update_env_var_force_still_rejects_protected_and_reserved() {
        let hive = MemoryHive::new(true);
        hive.seed("windir", "C:\\Windows", REG_EXPAND_SZ);

        assert!(update_env_var_force_in_store(&hive, "windir", "x").is_err(), "保护名单必须拒绝");
        assert!(update_env_var_force_in_store(&hive, "Path", "x").is_err(), "保留名必须拒绝");
    }

    #[test]
    fn delete_env_var_force_removes_after_external_change() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        hive.seed("MY_VAR", "changed-by-other", REG_SZ);

        delete_env_var_force_in_store(&hive, "MY_VAR").expect("force 删除必须成功");
        assert!(!hive.contains("MY_VAR"));
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p path-editor-core env_var_tests`

Expected: FAIL —— `update_env_var_force_in_store` 未定义。

- [ ] **Step 3: 实现 force 系列**

```rust
/// 强制写入已有变量（**最后写入者胜**）。不做 revision 比对。
///
/// 仅供 CLI `--force` 使用；GUI 一律走 [`update_env_var`] 的 CAS 语义。
/// force 只豁免并发校验，**不豁免**保留名 / 保护名单 / 类型 / 权限判定。
/// 这不是原子操作：读类型与写入仍是两次独立调用。
pub fn update_env_var_force(hive: EnvHive, name: &str, value: &str) -> Result<(), String> {
    let store = WinregHive::open(hive, true)?;
    update_env_var_force_in_store(&store, name, value)?;
    crate::system::broadcast_env_change();
    Ok(())
}

/// `update_env_var_force` 的核心逻辑，存储可注入。
fn update_env_var_force_in_store(
    store: &dyn EnvHiveStore,
    name: &str,
    value: &str,
) -> Result<(), String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name));
    }
    if is_protected(name) {
        return Err(format!("{} 是系统内置变量，不允许修改", name));
    }

    // 读现有类型以便原样保留；**不比对** revision
    let (vtype, _current) = read_env_var(store, name)?;
    let kind = EnvValueKind::from_reg_type(vtype.clone());
    if !kind.is_writable() {
        return Err(format!("{} 的注册表类型不受支持，无法修改（仅可查看）", name));
    }
    validate_env_value(value, name)?;

    write_env_var(store, name, value, vtype)
}

/// 强制删除变量（**最后写入者胜**）。不做 revision 比对，其余校验照旧。
pub fn delete_env_var_force(hive: EnvHive, name: &str) -> Result<(), String> {
    let store = WinregHive::open(hive, true)?;
    delete_env_var_force_in_store(&store, name)?;
    crate::system::broadcast_env_change();
    Ok(())
}

/// `delete_env_var_force` 的核心逻辑，存储可注入。
fn delete_env_var_force_in_store(store: &dyn EnvHiveStore, name: &str) -> Result<(), String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name));
    }
    if is_protected(name) {
        return Err(format!("{} 是系统内置变量，不允许删除", name));
    }

    let (vtype, _current) = read_env_var(store, name)?;
    if EnvValueKind::from_reg_type(vtype) == EnvValueKind::Unsupported {
        return Err(format!("{} 的注册表类型不受支持，无法删除", name));
    }

    store.delete_value(name)
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p path-editor-core env_var_tests`

Expected: PASS（含 4 个新测试）

- [ ] **Step 5: 提交**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add core/src/registry.rs
git commit -m "feat(core): 新增 update/delete_env_var_force，显式豁免 revision 校验"
```

---

### Task 2: CLI 接线到 force API（F-02）

**Files:**

- Modify: `cli/src/env_ops.rs`（`cmd_env_set` / `cmd_env_remove`；删除 `current_revision` / `find_revision` / `expected_revision` 及其测试）

**Interfaces:**

- Consumes: `core::registry::{update_env_var_force, delete_env_var_force}`
- Produces: 无新公开符号

**问题**：现状 `expected_revision()` 在 `Concurrency::Force` 时调 `current_revision()` 重读 revision 再传给 core —— 读与 core 写之间仍有窗口，`--force` 仍会退出码 3。

- [ ] **Step 1: 改写 `cmd_env_set`（env_ops.rs:352-371）**

```rust
/// `env set` —— 修改已有变量的值。类型跟随注册表现状，不可更改。
///
/// `--revision` 走 CAS（冲突退出码 3）；`--force` 走真正的覆盖写
/// （最后写入者胜，**不会**产生退出码 3）。
pub(crate) fn cmd_env_set(
    name: String,
    value: Option<String>,
    stdin: bool,
    value_file: Option<String>,
    revision: Option<String>,
    force: bool,
    system: bool,
) {
    let hive = select_hive(system, false);
    let mode = resolve_concurrency(revision, force);
    let src = resolve_value(value, stdin, value_file, true);
    let new_value = read_value(&src);
    match mode {
        Concurrency::Revision(r) => {
            apply_concurrency(core::registry::update_env_var(hive, &name, &new_value, &r));
        }
        Concurrency::Force => {
            core::registry::update_env_var_force(hive, &name, &new_value)
                .unwrap_or_else(|e| exit_err(&e));
        }
    }
    println!("已更新{}变量: {name}", hive_label(hive));
}
```

- [ ] **Step 2: 改写 `cmd_env_remove`（env_ops.rs:393-400）**

```rust
/// `env remove` —— 删除变量。
///
/// `--revision` 走 CAS；`--force` 直接删除（最后写入者胜，不产生退出码 3）。
pub(crate) fn cmd_env_remove(name: String, revision: Option<String>, force: bool, system: bool) {
    let hive = select_hive(system, false);
    let mode = resolve_concurrency(revision, force);
    match mode {
        Concurrency::Revision(r) => {
            apply_concurrency(core::registry::delete_env_var(hive, &name, &r));
        }
        Concurrency::Force => {
            core::registry::delete_env_var_force(hive, &name).unwrap_or_else(|e| exit_err(&e));
        }
    }
    println!("已删除{}变量: {name}", hive_label(hive));
}
```

- [ ] **Step 3: 删除已无用的模拟实现**

删除 `find_revision`（env_ops.rs:323-330）、`current_revision`（**333-341**）、`expected_revision`（**344-349**），以及测试 `force_revision_source_is_documented`（739-756）、`find_revision_prefers_exact_case_then_falls_back`（758-763）。留着会触发 `dead_code` 的 clippy 警告。

`Concurrency::Force` 的文档注释改为：

```rust
    /// `--force`：跳过 revision 校验直接覆盖（最后写入者胜，脚本 setx 风格）
    Force,
```

- [ ] **Step 4: 更新 `force_mode_skips_revision_check` 测试（env_ops.rs:507-513）**

```rust
    #[test]
    fn force_mode_never_carries_a_revision() {
        // Force 分支不产生 revision 字符串，也不调用 current_revision ——
        // 由 cmd 层直接调用 core 的 update_env_var_force。
        let mode = resolve_concurrency(None, true);
        assert!(matches!(mode, Concurrency::Force));
    }
```

- [ ] **Step 5: 质量门 + 提交**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p patheditor-cli --bins
git add cli/src/env_ops.rs
git commit -m "fix(cli): --force 改调 core force API，删除重读 revision 的模拟实现"
```

---

### Task 3: 文档同步 `--force` 语义与退出码（F-02）

**Files:**

- Modify: `docs/superpowers/specs/2026-09-17-cli-env-vars-design.md`（`:62-63`、`:107`）
- Modify: `README.md`（`:202`、`:204`）
- Modify: `AGENTS.md`（`:144`、`:165`）与 `CLAUDE.md`（**必须字节级一致**）

- [ ] **Step 1: 改 spec 的命令规格（:62-63）**

把 `--force` 一行改为：

```text
  - `--force`：跳过 revision 校验直接覆盖（最后写入者胜；仍受保留名 / 保护名单 / 类型 / 权限判定）
```

- [ ] **Step 2: 改 spec 的退出码说明（:107）**

把「`--force` 模式下不会产生退出码 3」改为：

```text
- `--force` 不携带 revision，**不会**因并发冲突产生退出码 3；退出码 3 仅在 `--revision` 不匹配时出现。
```

- [ ] **Step 3: 改 README 与 AGENTS/CLAUDE**

把 `--force`（跳过校验直接覆盖）的表述统一为「跳过 revision 校验直接覆盖（最后写入者胜，仍受保护名单/类型/权限约束）」。同步 AGENTS.md 与 CLAUDE.md，改完 `cmp` 确认两文件一致。

- [ ] **Step 4: 校验一致性**

Run: `git diff --stat AGENTS.md CLAUDE.md` 且逐字节比较两文件差异为 0。

- [ ] **Step 5: 提交**

```bash
git add docs/superpowers/specs/2026-09-17-cli-env-vars-design.md README.md AGENTS.md CLAUDE.md
git commit -m "docs: 统一 --force 为最后写入者胜语义，修正退出码说明"
```

---

### Task 4: `reveal_env_var` 返回携带 revision（F-01）

**Files:**

- Modify: `core/src/env_var.rs`（新增 `RevealedValue`）
- Modify: `core/src/lib.rs`（重导出）
- Modify: `core/src/registry.rs`（`reveal_env_var` / `reveal_env_var_in_store`）
- Modify: `gui/src/commands/env_var.rs:20`
- Modify: `cli/src/env_ops.rs:271-284`（`cmd_env_get` 是同一命令的另一个消费端，必须一并改）

**Interfaces:**

- Produces:
  - `pub struct RevealedValue { pub value: String, pub revision: String }`（serde camelCase）
  - `pub fn reveal_env_var(hive: EnvHive, name: &str) -> Result<RevealedValue, String>`
  - `fn reveal_env_var_in_store(store: &dyn EnvHiveStore, name: &str) -> Result<RevealedValue, String>`

- [ ] **Step 1: 定义契约（env_var.rs）**

```rust
/// 单个变量的完整明文及其读取时的 revision。
///
/// 编辑弹窗用 `revision` 绑定「这个值是在哪一版读到的」，避免用陈旧值
/// 配新 revision 提交、覆盖外部更新（F-01）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealedValue {
    pub value: String,
    pub revision: String,
}
```

`lib.rs` 重导出：`pub use env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot, RevealedValue};`

- [ ] **Step 2: 写失败测试**

在 `env_var_tests` 追加：

```rust
    #[test]
    fn reveal_returns_value_with_matching_revision() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "hello", REG_SZ);

        let revealed = reveal_env_var_in_store(&hive, "MY_VAR").expect("reveal 失败");
        assert_eq!(revealed.value, "hello");

        let raw = hive.get_raw("MY_VAR").unwrap();
        let expected = revision_of("MY_VAR", raw.vtype, "hello");
        assert_eq!(revealed.revision, expected, "revision 必须与列表项一致");
    }

    #[test]
    fn reveal_revision_changes_after_external_edit() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "one", REG_SZ);
        let first = reveal_env_var_in_store(&hive, "MY_VAR").unwrap().revision;

        hive.seed("MY_VAR", "two", REG_SZ);
        let second = reveal_env_var_in_store(&hive, "MY_VAR").unwrap().revision;

        assert_ne!(first, second);
    }
```

- [ ] **Step 3: 实现**

```rust
/// 按需读取单个变量的明文及读取时的 revision。
///
/// `Unsupported` 类型返回 `Err`，不尝试转字符串。
pub fn reveal_env_var(hive: EnvHive, name: &str) -> Result<RevealedValue, String> {
    let store = WinregHive::open(hive, false)?;
    reveal_env_var_in_store(&store, name)
}

/// `reveal_env_var` 的核心逻辑，存储可注入。
fn reveal_env_var_in_store(
    store: &dyn EnvHiveStore,
    name: &str,
) -> Result<RevealedValue, String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name));
    }
    let (vtype, value) = read_env_var(store, name)?;
    let revision = revision_of(name, vtype, &value);
    Ok(RevealedValue { value, revision })
}
```

- [ ] **Step 4: 更新既有 reveal 测试**

把 `reveal_env_var_returns_plaintext_and_sensitive_preview_is_none` 的断言改为 `revealed.value`：

```rust
        let revealed = reveal_env_var_in_store(&hive, "MY_API_TOKEN").expect("reveal 失败");
        assert_eq!(revealed.value, "super-secret-plaintext");
```

- [ ] **Step 5: 更新 Tauri 命令（gui/src/commands/env_var.rs:20）**

```rust
pub fn reveal_env_var(hive: EnvHive, name: String) -> Result<RevealedValue, String> {
```

（`RevealedValue` 从 `path_editor_core` 导入。）

- [ ] **Step 6: 修 CLI 消费端（`cli/src/env_ops.rs:271-284`）**

`cmd_env_get` 也消费 `reveal_env_var` 的返回值：`format_get_output(value: &str)` 收 `&str`，返回类型改成 `RevealedValue` 后会 E0308。改为取 `.value`：

```rust
pub(crate) fn cmd_env_get(name: String, system: bool) {
    let hive = select_hive(system, false);
    match core::registry::reveal_env_var(hive, &name) {
        Ok(revealed) => print!("{}", format_get_output(&revealed.value)),
        Err(msg) => {
            // 仅只读路径提供「变量存在于另一 hive」的提示，帮助用户加 --system。
            // 写操作不做此兜底 —— 见设计文档「hive 选择」。
            if let Some(other) = other_hive_hint(&name, hive) {
                exit_err(&format!("{msg}\n{other}"));
            }
            exit_err(&msg)
        }
    }
}
```

CLI「stdout 只打印裸值」的契约不变（`format_get_output` 的入参仍是 `&str`）。

- [ ] **Step 7: 跑测试**

Run: `cargo test --workspace`

Expected: PASS。

- [ ] **Step 8: 提交**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add core/src/env_var.rs core/src/lib.rs core/src/registry.rs gui/src/commands/env_var.rs
git commit -m "feat(core): reveal_env_var 返回明文与读取时 revision"
```

---

### Task 5: 前端绑定读值版本（F-01）

**Files:**

- Modify: `src/core/env-var.ts`（`RevealedValue` 类型）
- Modify: `src/services/backend.ts`（`revealEnvVar`）
- Modify: `src/store/env-store.ts`（`fetchFullValue`、`save`、`reveal`）
- Modify: `src/components/dialogs/EditEnvVarDialog.tsx`
- Modify: `src/components/layout/AppShell.tsx:291-301`
- Modify: `src/i18n/locales/zh-CN.json` / `en.json`

**Interfaces:**

- Produces:
  - `type RevealedValue = { value: string; revision: string }`（`src/core/env-var.ts`）
  - `fetchFullValue(hive, name): Promise<RevealedValue>`
  - `save(meta: EnvVarMeta, readRevision: string | null): Promise<boolean>`
  - `EditEnvVarDialog` 的 `onConfirm: (value: string, readRevision: string | null) => Promise<boolean>`

**设计要点**：陈旧判定必须发生在**保存那一刻**，用与 `updateEnvVar` 同一个 `meta.revision` 比对 —— 否则「检查」与「保存」之间的刷新仍会漏过。

- [ ] **Step 1: core 类型（`src/core/env-var.ts`）**

```ts
/** 完整明文及其读取时的 revision（与 Rust `RevealedValue` 契约一致）。 */
export type RevealedValue = {
  value: string;
  revision: string;
};
```

- [ ] **Step 2: backend 包装（`src/services/backend.ts`）**

```ts
  revealEnvVar: (hive: EnvHive, name: string) =>
    invoke<RevealedValue>('reveal_env_var', { hive, name }),
```

并在运行时形状校验处对返回值校验 `value`/`revision` 均为 string（与既有 EnvVarMeta 校验同风格）。

- [ ] **Step 3: store（`src/store/env-store.ts`）**

`fetchFullValue`：

```ts
    fetchFullValue: async (hive, name) => {
      // 编辑数据源专用：完整明文 + 读取时 revision。不进入 revealed，
      // 不影响表格打码状态。revision 用于提交时校验值是否已陈旧（F-01）。
      return backend.revealEnvVar(hive, name);
    },
```

`save` 增加 `readRevision` 形参并在**保存点**做陈旧校验：

```ts
    save: async (meta, readRevision) => {
      // F-01：编辑值的读取版本与将写入的 revision 不一致 → 值已陈旧，
      // 拒绝提交，刷新快照让弹窗重取，避免用旧值覆盖外部新值。
      if (readRevision !== null && readRevision !== meta.revision) {
        await refreshAfterError(i18n.t('envVar.staleReloaded'));
        return false;
      }
      const value = get().draft.get(envVarKey(meta));
      if (value === undefined) return false;
      set({ isSaving: true });
      try {
        await backend.updateEnvVar(meta.hive, meta.name, value, meta.revision);
        // …以下与现状一致
```

接口签名：`save: (meta: EnvVarMeta, readRevision: string | null) => Promise<boolean>;`

**同一命令的另一个消费端 `reveal`（`env-store.ts:178-199`）也必须改**：它把 `backend.revealEnvVar` 的返回值直接塞进 `Map<string,string>`，返回类型改成 `RevealedValue` 后会 TS2345。解构出 `.value`：

```ts
    reveal: async (meta) => {
      const requestedRevision = meta.revision;
      try {
        const { value } = await backend.revealEnvVar(meta.hive, meta.name);
        // 竞态防护：请求期间快照若已换代（revision 变化或条目消失），
        // 旧明文绑定不到当前状态，直接丢弃 —— 避免旧值经新 revision 覆盖外部更新。
        const snap = get().snapshot;
        const current = snap
          ? [...snap.system, ...snap.user].find((m) => envVarKey(m) === envVarKey(meta))
          : undefined;
        if (!current || current.revision !== requestedRevision) return;
        const revealed = new Map(get().revealed);
        revealed.set(envVarKey(meta), value);
        set({ revealed });
      } catch (error) {
        // …以下与现状一致
      }
    },
```

- [ ] **Step 4: 编辑弹窗（`EditEnvVarDialog.tsx`）**

新增 `readRevision` 状态并在加载时一并记录；提交时陈旧则重取：

```tsx
const [readRevision, setReadRevision] = useState<string | null>(null);
```

加载 effect（替换 44-63）：

```tsx
useEffect(() => {
  const store = useEnvStore.getState();
  const current = store.snapshot ? findMetaByKey(store.snapshot, varKey) : null;
  if (!current) return;
  let cancelled = false;
  store
    .fetchFullValue(current.hive, current.name)
    .then((full) => {
      if (cancelled) return;
      setValue(full.value);
      setReadRevision(full.revision);
    })
    .catch((err: unknown) => {
      if (!cancelled) {
        setError(String(err));
        setValue('');
      }
    });
  return () => {
    cancelled = true;
  };
}, [varKey]);
```

`submit`（替换 65-72）：

```tsx
const submit = async () => {
  if (value === null) return;
  setSubmitting(true);
  // F-01：提交前若快照 revision 已变（外部修改），先重取最新完整值，
  // 本次不保存；用户看到新值后可再次确认。
  const store = useEnvStore.getState();
  const current = store.snapshot ? findMetaByKey(store.snapshot, varKey) : null;
  if (current && readRevision !== null && current.revision !== readRevision) {
    const fresh = await store.fetchFullValue(current.hive, current.name);
    setValue(fresh.value);
    setReadRevision(fresh.revision);
    setDraftByKey(varKey, fresh.value);
    setError(t('envVar.staleReloaded'));
    setSubmitting(false);
    return;
  }
  const ok = await onConfirm(value, readRevision);
  setSubmitting(false);
  if (!ok) setError(useEnvStore.getState().statusMessage);
};
```

Props 类型改为：

```tsx
onConfirm: (value: string, readRevision: string | null) => Promise<boolean>;
```

- [ ] **Step 5: AppShell 透传（`AppShell.tsx:291-301`）**

```tsx
          onConfirm={async (value, readRevision) => {
            const store = useEnvStore.getState();
            // 从最新快照派生 meta：冲突刷新后重试自动携带新 revision（F-02）
            const meta = store.snapshot ? findMetaByKey(store.snapshot, editVarKey) : null;
            if (!meta) return false;
            store.setDraft(meta, value);
            const ok = await store.save(meta, readRevision);
            if (ok) setEditVarKey(null);
            // 失败不清草稿（c2 统一策略）：草稿镜像输入，供重试与关窗确认。
            return ok;
          }}
```

- [ ] **Step 6: i18n**

`src/i18n/locales/zh-CN.json` 加 `"staleReloaded": "变量已被外部修改，已重新加载最新值，请确认后再保存"`；`en.json` 加对应英文。两端 key 必须同步。

- [ ] **Step 7: 单测**

- `tests/unit/env-store.test.ts`：新增「`readRevision` 与 `meta.revision` 不一致时 `save` 返回 false、不发 `update_env_var`、且触发刷新」。
- `tests/unit/app-shell-env-vars.test.tsx`：新增「快照刷新后弹窗提交旧值 → 不调用 `update_env_var`」。
- 更新既有 mock 的 `reveal_env_var` 返回值，从字符串改为 `{ value, revision }`。

- [ ] **Step 8: E2E**

`e2e/tests/env-vars.spec.ts` 增加用例：打开编辑弹窗 → mock 注入快照换代（revision 变化）→ 点保存 → 断言 `__capturedCalls` 中**没有** `update_env_var`，且界面提示重新加载。同步更新 `e2e/mocks/ipc.ts` 的 `reveal_env_var` 返回形状。

- [ ] **Step 9: 质量门 + 提交**

```bash
npx tsc -b && npm run lint && npm test && npm run test:e2e
git add src/core/env-var.ts src/services/backend.ts src/store/env-store.ts src/components/dialogs/EditEnvVarDialog.tsx src/components/layout/AppShell.tsx src/i18n/locales/zh-CN.json src/i18n/locales/en.json tests e2e
git commit -m "fix(ui): 编辑弹窗绑定读值 revision，陈旧值不得覆盖外部更新"
```

---

### Task 6: 快照时刻字段 `capturedAt`（F-05）

**Files:**

- Modify: `core/src/env_var.rs`（`EnvVarSnapshot` 加字段）
- Modify: `core/src/registry.rs`（`list_all_env_vars` 填值）
- Modify: `src/core/env-var.ts`、`src/services/backend.ts`
- Modify: `cli/src/env_ops.rs`（测试夹具 `sample_snapshot`）

**Interfaces:**

- Produces: `EnvVarSnapshot.captured_at: u64`（Unix 毫秒，serde `capturedAt`）

- [ ] **Step 1: core 结构体**

```rust
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarSnapshot {
    #[serde(default)]
    pub system: Vec<EnvVarMeta>,
    #[serde(default)]
    pub user: Vec<EnvVarMeta>,
    /// 快照采集时刻（Unix 毫秒）。两个 hive 是先后两次读取，不是原子快照，
    /// 此字段让调用方知道「接近哪个时刻」。
    #[serde(default)]
    pub captured_at: u64,
}
```

- [ ] **Step 2: 填值（registry.rs）**

```rust
/// 一次读取两个 hive 的变量元数据（列表唯一入口）。
///
/// 两个 hive 是**先后两次独立读取**，没有跨键事务，返回的是「两个接近
/// 时刻的快照」，不是原子一致快照。`captured_at` 记录采集时刻。
pub fn list_all_env_vars() -> Result<EnvVarSnapshot, String> {
    let captured_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Ok(EnvVarSnapshot {
        system: list_hive_env_vars(EnvHive::System)?,
        user: list_hive_env_vars(EnvHive::User)?,
        captured_at,
    })
}
```

- [ ] **Step 3: TS 契约**

`src/core/env-var.ts` 的 `EnvVarSnapshot` 加 `capturedAt: number;`；`src/services/backend.ts` 的 `parseEnvVarSnapshot` 接受并校验该字段（缺失时回退 0，保持向后兼容）。

- [ ] **Step 4: 修夹具**

`EnvVarSnapshot { … }` 的字面量构造点只有两处（已核实）：

- `core/src/env_var.rs:360`（测试内的字面量）
- `cli/src/env_ops.rs:622`（`sample_snapshot()`）

两处补 `captured_at: 0`（或用 `..Default::default()`）。既有 `EnvVarSnapshot::default()` 的用法不受影响（`#[serde(default)]` + derive `Default`）。

- [ ] **Step 5: 质量门 + 提交**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
npx tsc -b && npm test
git add core/src/env_var.rs core/src/registry.rs cli/src/env_ops.rs src/core/env-var.ts src/services/backend.ts
git commit -m "feat(core): 环境变量快照增加 capturedAt 采集时刻"
```

---

### Task 7: core pending 快照原语（F-03）

**Files:**

- Modify: `core/src/disabled.rs`

**Interfaces:**

- Produces:
  - `pub fn save_pending_path_snapshot(system: Option<Vec<PathEntry>>, user: Option<Vec<PathEntry>>) -> Result<(), String>`
  - `pub fn load_pending_path_snapshot() -> Result<Option<PathSnapshot>, String>`
  - `pub fn clear_pending_path_snapshot() -> Result<(), String>`
  - `pub fn has_pending_path_snapshot() -> bool`
- 落盘：`~/.patheditor/pending_path_snapshot.json`

- [ ] **Step 1: 实现**

```rust
/// 待补写快照文件路径。
fn pending_path() -> PathBuf {
    // ~/.patheditor/pending_path_snapshot.json
}

/// 记录「注册表已写、快照未落盘」的待补写状态，供下次运行补写。
///
/// 只保留最后一次待补写内容（覆盖式），避免堆积多个互相矛盾的版本。
pub fn save_pending_path_snapshot(
    system: Option<Vec<PathEntry>>,
    user: Option<Vec<PathEntry>>,
) -> Result<(), String> { /* 序列化 PathSnapshot 到 pending_path() */ }

/// 读取待补写状态；无文件返回 `Ok(None)`。
pub fn load_pending_path_snapshot() -> Result<Option<PathSnapshot>, String> { /* ... */ }

/// 清除待补写状态（补写成功后调用）。
pub fn clear_pending_path_snapshot() -> Result<(), String> { /* 删除文件，忽略 NotFound */ }

/// 是否存在待补写状态。
pub fn has_pending_path_snapshot() -> bool { /* pending_path().exists() */ }
```

写入用既有 `atomic_write`。

- [ ] **Step 2: 测试（临时目录，不碰真实注册表）**

```rust
    #[test]
    fn pending_snapshot_roundtrip() {
        // 用 tempdir 覆盖 appdata 目录（沿用 disabled_tests 的既有隔离方式）
        // save → load 得到同一内容 → clear → has=false
    }

    #[test]
    fn pending_snapshot_missing_file_is_none() {
        assert!(load_pending_path_snapshot().unwrap().is_none());
    }
```

（若既有测试已用环境变量或注入路径隔离 appdata，复用它；不要写真实 `~/.patheditor`。）

- [ ] **Step 3: 提交**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p path-editor-core
git add core/src/disabled.rs
git commit -m "feat(core): 新增 PATH 快照待补写原语（pending）"
```

---

### Task 8: CLI sidecar 失败落 pending 并补写（F-03）

**Files:**

- Modify: `cli/src/runtime.rs`

**设计**：sidecar 写失败**不再直接 `exit_err` 丢掉状态** —— 落 pending 文件，退出时明确告知「注册表已写、快照未落、已记录待补写」；任一 PATH 写命令启动时先尝试补写 pending。

- [ ] **Step 1: 替换 `persist_snapshot`（runtime.rs:126-131）**

```rust
/// 注册表写入成功后提交完整有序快照。
///
/// 失败时不丢状态：把待补写内容落到 pending 文件，退出码 1 并在 stderr
/// 明确说明「注册表已改、快照未落、已记录待补写」，供下次运行自动补写。
pub(crate) fn persist_snapshot(
    system: Option<Vec<core::PathEntry>>,
    user: Option<Vec<core::PathEntry>>,
) {
    if let Err(e) = core::disabled::save_path_snapshot(system.clone(), user.clone()) {
        match core::disabled::save_pending_path_snapshot(system, user) {
            Ok(()) => exit_err(&format!(
                "注册表已写入，但快照保存失败: {e}\n已记录待补写状态，下次运行 PATH 命令会自动补写"
            )),
            Err(pe) => exit_err(&format!(
                "注册表已写入，但快照保存失败: {e}\n且待补写状态记录失败: {pe}\n注册表与快照可能不一致，请手工核对"
            )),
        }
    }
}
```

- [ ] **Step 2: `load_and_save` / `load_operate_save` 改调 `persist_snapshot`**

把这两处（runtime.rs:67、72、97-101）直接 `save_path_snapshot(...).unwrap_or_else(...)` 替换为 `persist_snapshot(...)`，复用失败恢复。

- [ ] **Step 3: 启动补写**

在 `load_and_save` 与 `load_operate_save` 开头（`ensure_single_target` 之后）调用：

```rust
/// 若存在上次未落盘的快照，先补写；成功即清除待补写状态。
///
/// 补写是 best-effort：失败不阻断当前命令，但会打印警告。
fn flush_pending_snapshot() {
    let pending = match core::disabled::load_pending_path_snapshot() {
        Ok(Some(p)) => p,
        Ok(None) => return,
        Err(e) => {
            eprintln!("警告: 无法读取待补写快照状态: {e}");
            return;
        }
    };
    match core::disabled::save_path_snapshot(Some(pending.system), Some(pending.user)) {
        Ok(()) => {
            let _ = core::disabled::clear_pending_path_snapshot();
        }
        Err(e) => eprintln!("警告: 待补写快照仍未能落盘: {e}"),
    }
}
```

- [ ] **Step 4: 测试**

在 `cli/src/runtime.rs` 的测试模块新增对 `persist_snapshot` 纯逻辑的测试不易（涉及真实文件系统）——改为断言「错误消息包含『注册表已写入』与『待补写』」的构造逻辑抽为纯函数后测试：

```rust
/// 构造 sidecar 失败时的错误文案（纯函数，便于测试）。
pub(crate) fn sidecar_failure_message(err: &str, pending_ok: bool) -> String { /* ... */ }
```

对这纯函数断言两种分支的文案都包含「注册表已写入」。

- [ ] **Step 5: 质量门 + 提交**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p patheditor-cli --bins
git add cli/src/runtime.rs
git commit -m "fix(cli): sidecar 写失败落 pending 待补写状态，PATH 命令启动时自动补写"
```

---

### Task 9: 收口质量门与文档

**Files:**

- Modify: `AGENTS.md` / `CLAUDE.md`（若涉及 CLI 行为描述）
- Modify: spec 的 `## Execution Notes`

- [ ] **Step 1: 全量质量门**

```bash
npm run verify:all
```

Expected: 全绿（含 E2E）。

- [ ] **Step 2: 真实注册表闭环（**需用户显式授权**）**

若用户授权：按既有闭环测试记录格式（`docs/审核和开发/2026.09.18/PathEditor-CLI环境变量闭环测试记录.md`）执行 `--force` 覆盖、sidecar 失败注入、编辑陈旧拦截三项，记录备份/快照/回滚。**未授权则如实写「未执行」。**

- [ ] **Step 3: 回填 Execution Notes + 提交**

```bash
git add -A
git commit -m "chore: Wave 1 质量门收口"
```

---

## Self-Review

**1. Spec coverage**

| Spec 条目                                       | 任务                                                     |
| ----------------------------------------------- | -------------------------------------------------------- |
| F-01 编辑弹窗完整值绑定 revision                | Task 4、5                                                |
| F-01 回归测试（fetch 延迟期间刷新不得提交旧值） | Task 5 Step 7/8                                          |
| F-02 core 真正 force API + CLI 接线             | Task 1、2                                                |
| F-02 退出码 3 仅 --revision 出现                | Task 1（无冲突路径）、Task 2（分支分离）、Task 3（文档） |
| F-02 文档/帮助/退出码一致                       | Task 3                                                   |
| F-03 sidecar 失败留可重试状态                   | Task 7、8                                                |
| F-05 快照契约措辞 + capturedAt                  | Wave 0 Task 5 Step 3（措辞）、Task 6（字段）             |

**2. Placeholder scan**

Task 7/8 中 pending 的文件路径与序列化细节以伪代码块给出（`/* ... */`）—— 这是**唯一**未展开处，原因是 `disabled.rs` 的 appdata 隔离方式需按既有测试基建实现；开发窗口在 Wave 0 之后能看到该文件的真实隔离方式再落地。其余步骤均为完整代码。

**3. Type consistency**

| 符号                                            | 定义   | 使用                        | 一致 |
| ----------------------------------------------- | ------ | --------------------------- | ---- |
| `RevealedValue{value,revision}`                 | Task 4 | Task 5（TS 同名类型）       | ✓    |
| `update_env_var_force` / `delete_env_var_force` | Task 1 | Task 2                      | ✓    |
| `save(meta, readRevision)`                      | Task 5 | Task 5（AppShell）          | ✓    |
| `onConfirm(value, readRevision)`                | Task 5 | Task 5（dialog + AppShell） | ✓    |
| `EnvVarSnapshot.captured_at`                    | Task 6 | Task 6（CLI/TS）            | ✓    |
| `persist_snapshot` / `flush_pending_snapshot`   | Task 8 | Task 8                      | ✓    |
| pending 四个原语                                | Task 7 | Task 8                      | ✓    |

## Execution Notes

> 一行一项，格式：**计划原文 / 实际 / 处理**。

- **R1 行号漂移** / 计划按行号定位 `registry.rs` 修改点，实际开发时行号已漂移 / 全部改按符号定位，未按行号硬套。
- **R2 capturedAt 夹具** / brief 预估 TS 侧 capturedAt 修复面很小，实际涉及 4 个测试文件 + `EditEnvVarDialog` + 约 15 处 mocks / 按实际范围完成类型化夹具修复。
- **R3 pending 隔离** / brief 假设 `disabled.rs` 已有测试隔离机制，实际不存在 / 按 `disabled.rs` 既有 `cfg(test)` 固定临时路径样式实现；原计划两个 pending 测试合并为单生命周期测试。
- **R4 基线计数** / 计划写测试基线 103 / 实际以 106/2 为准；最终 workspace 152 passed / 2 ignored（0 failed）。
- **R5 delete force 的 Unsupported 检查** / 可达性存疑 / 保留该不可达检查并加注释说明，不删除。
- **Task 8 裁决（fix round 1，commit `346e86d`）** / 发现 pending 的 None→空数组语义陷阱：`save_pending_path_snapshot` 的 None 语义是「空数组」而非「保留该 hive」，若把 None 原样落 pending，补写时会把未操作的 hive 清空 / `persist_snapshot` 落盘前先用当前快照把 None 侧填充为现有内容；flush 覆盖面按 spec 补齐 `import` 与 `profile apply` 两个入口。
- **Task 5 偏离** / 单测环境 i18n 检测为 en，断言不能只匹配中文文案 / 错误提示断言改用双语正则；mock revision 改为动态跟随快照，避免硬编码失效。
