# CLI 环境变量管理 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `patheditor` CLI 增加 `env` 子命令组，让脚本与自动化场景获得与 GUI 等价的环境变量管理能力（列表 / 取值 / 修改 / 新建 / 删除）。

**Architecture:** core 层已有 5 个公开 API（`list_all_env_vars` / `reveal_env_var` / `update_env_var` / `create_env_var` / `delete_env_var`）并经 97 个测试覆盖，CLI 侧**零新增核心代码**。新建 `cli/src/env_ops.rs` 承载命令实现与纯函数格式化器，`cli/src/main.rs` 只新增 Clap 枚举与 match 分派。保留名、保护名单、`Unsupported` 类型、hive 写权限、revision 校验全部由 core 判定，CLI 仅透传错误。

**Tech Stack:** Rust（edition/version 见 workspace）、clap 4（derive）、serde_json 1、path-editor-core（本地 path 依赖）

**Spec:** `docs/superpowers/specs/2026-09-17-cli-env-vars-design.md`

## Global Constraints

- **分支**：在 `worktree-all-env-vars` 分支开发（延续全环境变量特性，尚未合并 main）
- **架构约束**（AGENTS.md/CLAUDE.md）：「gui 和 cli 只做参数转换、命令分派和错误呈现，业务规则放在 core」——CLI 内**不得复制任何安全判定逻辑**（保留/保护/敏感/权限/revision）
- **文档注释**：所有 `pub fn` 必须有 `///` 文档注释（CONTRIBUTING.md 硬性要求）；`pub(crate)` 函数同样补注释
- **代码风格**：UTF-8、CRLF；Rust 4 空格缩进；rustfmt 默认 100 列；Prettier 不涉及
- **质量门**：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- **禁止真实注册表写入**：本计划所有测试必须为纯函数单测（不碰注册表）；`cargo test` 内不得出现 `HKLM:`/`HKCU:` 真实 hive 操作
- **提交规范**：Conventional Commits（`feat`/`fix`/`docs`/`test`/`chore`）；**不推送**（用户明确要求时才推送）
- **不删除文件**：未经用户明确同意不得删除任何文件
- **退出码契约**：成功 0；一般错误 1（沿用 `exit_err`）；revision 冲突 **3**（仅 env 命令，PATH 命令保持 1 不动）
- **冲突错误文本契约**：core 的 `ERR_CONFLICT` 常量值固定为 `[E_CONFLICT] 变量已被其他进程修改，请重新加载`，CLI 按 `[E_CONFLICT]` 前缀匹配，**不得**按中文正文匹配

## File Structure

| 文件                      | 职责                                                                                         | 变更     |
| ------------------------- | -------------------------------------------------------------------------------------------- | -------- |
| `cli/src/env_ops.rs`      | env 子命令实现 + 纯函数格式化器（值通道解析、并发选项校验、表格渲染、JSON 组装、退出码映射） | **新建** |
| `cli/src/main.rs`         | Clap 枚举 `EnvCmd` + 顶层 `Command::Env` 变体 + match 分派                                   | 修改     |
| `cli/src/runtime.rs`      | 新增 `exit_conflict`（退出码 3）                                                             | 修改     |
| `README.md`               | CLI 命令表追加 env 组                                                                        | 修改     |
| `AGENTS.md` / `CLAUDE.md` | CLI 命令节 + 错误处理节（退出码 3）；顺带修复遗留的 IPC 表格分隔行宽度不一致                 | 修改     |

---

### Task 1: 退出码基础设施 — `exit_conflict`

**Files:**

- Modify: `cli/src/runtime.rs`（在 `exit_err` 之后追加）
- Test: `cli/src/runtime.rs`（文件末尾 `#[cfg(test)] mod tests`）

**Interfaces:**

- Consumes: 无
- Produces:
  - `pub(crate) fn exit_conflict(msg: &str) -> !` —— 冲突专用退出，stderr 输出原文、退出码 3
  - `pub(crate) fn is_conflict(msg: &str) -> bool` —— 按 `[E_CONFLICT]` 前缀判定（供 env_ops 分派退出码）

- [ ] **Step 1: Write the failing test**

在 `cli/src/runtime.rs` 末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_prefix_is_detected() {
        // core 的 ERR_CONFLICT 原文（前缀 + 中文正文）必须判为冲突
        assert!(is_conflict("[E_CONFLICT] 变量已被其他进程修改，请重新加载"));
        assert!(is_conflict("[E_CONFLICT]"));
    }

    #[test]
    fn non_conflict_messages_are_not_detected() {
        assert!(!is_conflict("错误: 索引 3 超出范围"));
        assert!(!is_conflict("变量已被其他进程修改，请重新加载"));
        // 前缀必须在开头，中段出现不算
        assert!(!is_conflict("前置文本 [E_CONFLICT] 变量已被其他进程修改"));
        assert!(!is_conflict(""));
    }

    #[test]
    fn conflict_prefix_matches_core_constant() {
        // 契约：core 常量必须以该前缀开头，否则 CLI 退出码 3 永不触发
        use path_editor_core as core;
        let msg = core::registry::conflict_message();
        assert!(
            is_conflict(&msg),
            "core 冲突消息必须以 [E_CONFLICT] 开头，实际: {msg}"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p patheditor-cli --bins`
Expected: 编译失败——`is_conflict` 未定义（`cannot find function is_conflict in this scope`）；`core::registry::conflict_message` 未定义

- [ ] **Step 3: 在 core 暴露冲突消息读取器**

`ERR_CONFLICT` 当前是 `pub(crate)`，CLI 无法访问。在 `core/src/registry.rs` 中 `ERR_CONFLICT` 定义之后追加：

```rust
/// 冲突错误的完整文本。CLI / GUI 凭此前缀判定冲突，避免二次硬编码文案。
pub fn conflict_message() -> String {
    ERR_CONFLICT.to_string()
}
```

- [ ] **Step 4: 实现 exit_conflict 与 is_conflict**

在 `cli/src/runtime.rs` 的 `exit_err` 之后追加：

```rust
/// 冲突错误的结构化前缀。与 core 的 `ERR_CONFLICT` 常量对齐 ——
/// 中文正文仅供人工阅读，判定只看前缀。
pub(crate) const CONFLICT_PREFIX: &str = "[E_CONFLICT]";

/// 消息是否表示 revision 冲突（可按前缀重试恢复）。
pub(crate) fn is_conflict(msg: &str) -> bool {
    msg.starts_with(CONFLICT_PREFIX)
}

/// 冲突退出：stderr 输出 core 原文，退出码 3。
///
/// 与 `exit_err`（退出码 1）分开，使脚本能区分「重新 list 取 revision 后可恢复」
/// 与致命错误，无需 grep 中文文案。
pub(crate) fn exit_conflict(msg: &str) -> ! {
    eprintln!("错误: {msg}");
    std::process::exit(3);
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p patheditor-cli --bins`
Expected: PASS（3 passed）

- [ ] **Step 6: 确认 core 侧契约测试仍绿**

Run: `cargo test --workspace`
Expected: 全部通过（core 97 + cli 新增 3）

- [ ] **Step 7: Commit**

```bash
git add cli/src/runtime.rs core/src/registry.rs
git commit -m "feat(cli): 新增冲突退出码与 [E_CONFLICT] 前缀判定"
```

---

### Task 2: 值输入通道解析

**Files:**

- Create: `cli/src/env_ops.rs`
- Modify: `cli/src/main.rs`（仅加 `mod env_ops;`）
- Test: `cli/src/env_ops.rs`（`#[cfg(test)] mod tests`）

**Interfaces:**

- Consumes: `crate::runtime::exit_err`
- Produces:
  - `pub(crate) enum ValueSource { Argv(String), Stdin, File(String) }` —— 三通道互斥的输入描述
  - `pub(crate) fn resolve_value(argv: Option<String>, stdin: bool, file: Option<String>, required: bool) -> ValueSource` —— 校验互斥并返回通道
  - `pub(crate) fn strip_trailing_newline(raw: &str) -> String` —— 剥离末尾一个换行序列
  - `pub(crate) fn read_value(src: &ValueSource) -> String` —— 实际取值（stdin/file/argv）
  - `pub(crate) fn is_stdin_tty() -> bool` —— stdin 是否为终端（供报错提示）

- [ ] **Step 1: Write the failing test**

创建 `cli/src/env_ops.rs`：

```rust
//! 环境变量子命令实现：参数转换、core 调用与输出格式化。
//!
//! 本模块**不实现任何安全判定**（保留 / 保护 / 敏感 / 权限 / revision 校验），
//! 全部由 `path_editor_core` 负责，此处仅透传错误文本。

use crate::runtime::{exit_conflict, exit_err, is_conflict};
use path_editor_core as core;
use path_editor_core::env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot};
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;

    // ── 值通道互斥 ──

    #[test]
    fn value_source_argv_alone_is_accepted() {
        let src = resolve_value(Some("C:\\Java".into()), false, None, true);
        assert!(matches!(src, ValueSource::Argv(v) if v == "C:\\Java"));
    }

    #[test]
    fn value_source_stdin_alone_is_accepted() {
        let src = resolve_value(None, true, None, true);
        assert!(matches!(src, ValueSource::Stdin));
    }

    #[test]
    fn value_source_file_alone_is_accepted() {
        let src = resolve_value(None, false, Some("secret.txt".into()), true);
        assert!(matches!(src, ValueSource::File(f) if f == "secret.txt"));
    }

    #[test]
    fn value_source_rejects_multiple_channels() {
        // 互斥校验必须在读取任何输入之前失败；用 catch_unwind 捕获 process::exit 不现实，
        // 因此这里只断言「判定函数」的纯逻辑：通道计数 > 1 时为错误。
        assert_eq!(channel_count(Some("v"), true, None), 2);
        assert_eq!(channel_count(Some("v"), false, Some("f")), 2);
        assert_eq!(channel_count(None, true, Some("f")), 2);
        assert_eq!(channel_count(Some("v"), true, Some("f")), 3);
        assert_eq!(channel_count(Some("v"), false, None), 1);
        assert_eq!(channel_count(None, false, None), 0);
    }

    #[test]
    fn value_source_missing_when_required() {
        assert_eq!(channel_count(None, false, None), 0);
        // required=true 且 0 通道 → 调用方报错；required=false 且 0 通道 → 空值合法
    }

    // ── 末尾换行剥离 ──

    #[test]
    fn strips_single_lf() {
        assert_eq!(strip_trailing_newline("C:\\Java\n"), "C:\\Java");
    }

    #[test]
    fn strips_single_crlf() {
        assert_eq!(strip_trailing_newline("C:\\Java\r\n"), "C:\\Java");
    }

    #[test]
    fn strips_only_one_trailing_sequence() {
        // 只剥一个 —— 值本身以换行结尾时靠第二个换行表达
        assert_eq!(strip_trailing_newline("a\n\n"), "a\n");
        assert_eq!(strip_trailing_newline("a\r\n\r\n"), "a\r\n");
    }

    #[test]
    fn leaves_value_without_newline_untouched() {
        assert_eq!(strip_trailing_newline("C:\\Java"), "C:\\Java");
        assert_eq!(strip_trailing_newline(""), "");
    }

    #[test]
    fn strips_lone_cr() {
        // 单独的 \r（旧式 Mac / 误操作）也按换行处理
        assert_eq!(strip_trailing_newline("a\r"), "a");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p patheditor-cli --bins`
Expected: 编译失败 —— `resolve_value` / `channel_count` / `strip_trailing_newline` / `ValueSource` 均未定义

- [ ] **Step 3: 实现通道解析**

在 `cli/src/env_ops.rs` 顶部（`use` 之后、`#[cfg(test)]` 之前）插入：

```rust
/// 值的输入通道。三选一，互斥。
pub(crate) enum ValueSource {
    /// 位置参数或 `--value` 直接给出
    Argv(String),
    /// `--stdin`：从标准输入读到 EOF
    Stdin,
    /// `--value-file <F>`：读取文件全部内容
    File(String),
}

/// 统计显式给出的通道数（互斥校验用）。
pub(crate) fn channel_count(argv: Option<&str>, stdin: bool, file: Option<&str>) -> usize {
    usize::from(argv.is_some()) + usize::from(stdin) + usize::from(file.is_some())
}

/// 校验三通道互斥并选出唯一通道。
///
/// `required` 为 `true`（`set`/`add` 需要值）时零通道报错；为 `false` 时零通道
/// 由调用方决定语义。
pub(crate) fn resolve_value(
    argv: Option<String>,
    stdin: bool,
    file: Option<String>,
    required: bool,
) -> ValueSource {
    let count = channel_count(argv.as_deref(), stdin, file.as_deref());
    if count > 1 {
        exit_err("只能指定一种取值方式：位置参数 / --value / --stdin / --value-file");
    }
    if count == 0 {
        if required {
            exit_err("缺少值：请用位置参数、--value、--stdin 或 --value-file 提供");
        }
        return ValueSource::Argv(String::new());
    }
    if let Some(v) = argv {
        return ValueSource::Argv(v);
    }
    if stdin {
        return ValueSource::Stdin;
    }
    ValueSource::File(file.expect("count==1 且非 argv/stdin 时 file 必有值"))
}

/// 剥离末尾**一个**换行序列（`\n` / `\r\n` / `\r`）。
///
/// 管道 `echo value |` 会带一个换行；只剥一个使「值本身以换行结尾」仍可表达
/// （echo 两次或 `--value-file`）。空串与非换行结尾原样返回。
pub(crate) fn strip_trailing_newline(raw: &str) -> String {
    raw.strip_suffix("\r\n")
        .or_else(|| raw.strip_suffix('\n'))
        .or_else(|| raw.strip_suffix('\r'))
        .unwrap_or(raw)
        .to_string()
}

/// 按通道实际读取值。`Argv` 原样返回（argv 里的换行是用户输入的真实内容）。
pub(crate) fn read_value(src: &ValueSource) -> String {
    match src {
        ValueSource::Argv(v) => v.clone(),
        ValueSource::Stdin => {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .unwrap_or_else(|e| exit_err(&format!("读取标准输入失败: {e}")));
            strip_trailing_newline(&buf)
        }
        ValueSource::File(path) => {
            let raw = std::fs::read_to_string(path)
                .unwrap_or_else(|e| exit_err(&format!("读取值文件失败 ({path}): {e}")));
            strip_trailing_newline(&raw)
        }
    }
}
```

- [ ] **Step 4: 在 main.rs 注册模块**

`cli/src/main.rs` 的 `mod` 列表（第 5-8 行）追加：

```rust
mod env_ops;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p patheditor-cli --bins`
Expected: PASS（Task 1 的 3 个 + 本任务 9 个）

- [ ] **Step 6: 确认 clippy 干净**

Run: `cargo clippy -p patheditor-cli --all-targets -- -D warnings`
Expected: 零警告（注意：`exit_conflict` / `is_conflict` / `EnvHive` 等此时可能报 dead_code，本任务先不加 `#[allow]`——Task 3 起会用到；若 clippy 报 dead_code，在本步骤临时确认报错内容后再决定，**不要**为此提前加 allow）

- [ ] **Step 7: Commit**

```bash
git add cli/src/env_ops.rs cli/src/main.rs
git commit -m "feat(cli): 值输入三通道与末尾换行剥离"
```

---

### Task 3: 并发选项校验（`--revision` / `--force`）

**Files:**

- Modify: `cli/src/env_ops.rs`

**Interfaces:**

- Consumes: `crate::runtime::exit_err`
- Produces:
  - `pub(crate) enum Concurrency { Revision(String), Force }` —— 并发模式
  - `pub(crate) fn resolve_concurrency(revision: Option<String>, force: bool) -> Concurrency` —— 互斥校验
  - `pub(crate) fn apply_concurrency(concurrency: &Concurrency, result: Result<(), String>)` —— 统一处理结果与退出码

- [ ] **Step 1: Write the failing test**

在 `cli/src/env_ops.rs` 的 `mod tests` 内追加：

```rust
    // ── 并发模式互斥 ──

    #[test]
    fn concurrency_accepts_revision_alone() {
        let mode = resolve_concurrency(Some("a1b2c3".into()), false);
        assert!(matches!(mode, Concurrency::Revision(r) if r == "a1b2c3"));
    }

    #[test]
    fn concurrency_accepts_force_alone() {
        let mode = resolve_concurrency(None, true);
        assert!(matches!(mode, Concurrency::Force));
    }

    #[test]
    fn concurrency_channel_count_rule() {
        // 互斥与缺失的判定是纯函数，单独断言避免依赖 process::exit
        assert_eq!(concurrency_count(Some("r"), false), 1);
        assert_eq!(concurrency_count(None, true), 1);
        assert_eq!(concurrency_count(Some("r"), true), 2); // 互斥冲突
        assert_eq!(concurrency_count(None, false), 0); // 缺失
    }

    // ── 冲突结果映射到退出码 3 ──

    #[test]
    fn conflict_result_maps_to_exit_3() {
        let core_msg = core::registry::conflict_message();
        assert!(is_conflict(&core_msg), "core 冲突消息必须被判为冲突");
        assert!(!is_conflict("变量不存在"));
    }

    #[test]
    fn force_mode_skips_revision_check() {
        // Force 模式不携带 revision；写入时不传 expected_revision 的路径由 cmd 层负责。
        // 此处断言模式本身：Force 不产生 revision 字符串。
        let mode = resolve_concurrency(None, true);
        assert!(matches!(mode, Concurrency::Force));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p patheditor-cli --bins`
Expected: 编译失败 —— `resolve_concurrency` / `concurrency_count` / `Concurrency` 未定义

- [ ] **Step 3: 实现并发模式**

在 `cli/src/env_ops.rs` 中 `ValueSource` 定义之后追加：

```rust
/// 写操作的并发控制模式。二选一，强制显式给出。
pub(crate) enum Concurrency {
    /// `--revision <R>`：CAS 校验，与 GUI 同强度
    Revision(String),
    /// `--force`：跳过校验直接覆盖（脚本 setx 风格）
    Force,
}

/// 统计并发选项数（互斥与缺失校验用）。
pub(crate) fn concurrency_count(revision: Option<&str>, force: bool) -> usize {
    usize::from(revision.is_some()) + usize::from(force)
}

/// 校验并发选项：必须且只能给一个。
///
/// 设计意图：CLI 是一次性进程，静默降级为「现读现写」会让用户在毫秒级竞态窗口下
/// 最后写入者胜，与仓库 `verify_and_save` 的安全文化相悖。强制显式选择让每次
/// 覆盖都是知情决策。
pub(crate) fn resolve_concurrency(revision: Option<String>, force: bool) -> Concurrency {
    match concurrency_count(revision.as_deref(), force) {
        1 => match revision {
            Some(r) => Concurrency::Revision(r),
            None => Concurrency::Force,
        },
        0 => exit_err("需要提供 --revision（并发校验）或 --force（跳过校验）"),
        _ => exit_err("--revision 与 --force 互斥，只能指定一个"),
    }
}

/// 统一处理写操作结果：冲突走退出码 3，其他错误走退出码 1。
pub(crate) fn apply_concurrency(result: Result<(), String>) {
    match result {
        Ok(()) => {}
        Err(msg) if is_conflict(&msg) => exit_conflict(&msg),
        Err(msg) => exit_err(&msg),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p patheditor-cli --bins`
Expected: PASS（累计 17 个）

- [ ] **Step 5: Commit**

```bash
git add cli/src/env_ops.rs
git commit -m "feat(cli): 并发选项互斥校验与冲突退出码映射"
```

---

### Task 4: 表格渲染（list 的人类可读输出）

**Files:**

- Modify: `cli/src/env_ops.rs`

**Interfaces:**

- Consumes: `path_editor_core::env_var::{EnvVarMeta, EnvValueKind}`
- Produces:
  - `pub(crate) fn kind_label(kind: EnvValueKind) -> &'static str` —— `string` / `expand` / `unsupported`
  - `pub(crate) fn render_preview(meta: &EnvVarMeta) -> String` —— 敏感 / 空值 / 正常三态
  - `pub(crate) fn render_name(meta: &EnvVarMeta) -> String` —— 只读标记
  - `pub(crate) fn render_table(metas: &[EnvVarMeta]) -> String` —— 完整表格（含表头与对齐）

- [ ] **Step 1: Write the failing test**

在 `cli/src/env_ops.rs` 的 `mod tests` 内追加：

```rust
    // ── 表格渲染 ──

    fn meta(name: &str, kind: EnvValueKind, preview: Option<&str>, can_edit: bool, sensitive: bool) -> EnvVarMeta {
        EnvVarMeta {
            name: name.into(),
            kind,
            hive: EnvHive::User,
            can_edit,
            can_delete: can_edit,
            sensitive,
            preview: preview.map(|p| p.to_string()),
            revision: "0000000000000000".into(),
        }
    }

    #[test]
    fn kind_label_maps_all_variants() {
        assert_eq!(kind_label(EnvValueKind::String), "string");
        assert_eq!(kind_label(EnvValueKind::ExpandString), "expand");
        assert_eq!(kind_label(EnvValueKind::Unsupported), "unsupported");
    }

    #[test]
    fn sensitive_variable_hides_preview() {
        let m = meta("MY_TOKEN", EnvValueKind::String, None, true, true);
        assert_eq!(render_preview(&m), "(敏感)");
    }

    #[test]
    fn non_sensitive_variable_shows_preview() {
        let m = meta("JAVA_HOME", EnvValueKind::String, Some("C:\\Java"), true, false);
        assert_eq!(render_preview(&m), "C:\\Java");
    }

    #[test]
    fn empty_preview_renders_dash() {
        // preview=None 且非敏感（空值 / Unsupported）→ 占位
        let m = meta("EMPTY_VAR", EnvValueKind::String, None, true, false);
        assert_eq!(render_preview(&m), "-");
    }

    #[test]
    fn unwritable_variable_shows_readonly_marker() {
        let m = meta("windir", EnvValueKind::String, Some("C:\\Windows"), false, false);
        assert_eq!(render_name(&m), "windir (只读)");
    }

    #[test]
    fn writable_variable_has_no_marker() {
        let m = meta("JAVA_HOME", EnvValueKind::String, Some("C:\\Java"), true, false);
        assert_eq!(render_name(&m), "JAVA_HOME");
    }

    #[test]
    fn table_contains_header_and_rows() {
        let metas = vec![
            meta("JAVA_HOME", EnvValueKind::String, Some("C:\\Java"), true, false),
            meta("MY_TOKEN", EnvValueKind::String, None, true, true),
        ];
        let table = render_table(&metas);
        assert!(table.contains("NAME"));
        assert!(table.contains("KIND"));
        assert!(table.contains("PREVIEW"));
        assert!(table.contains("JAVA_HOME"));
        assert!(table.contains("string"));
        assert!(table.contains("C:\\Java"));
        assert!(table.contains("(敏感)"));
    }

    #[test]
    fn empty_table_reports_none() {
        assert_eq!(render_table(&[]), "（无变量）");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p patheditor-cli --bins`
Expected: 编译失败 —— `kind_label` / `render_preview` / `render_name` / `render_table` 未定义

- [ ] **Step 3: 实现表格渲染**

在 `cli/src/env_ops.rs` 中 `Concurrency` 相关代码之后追加：

```rust
/// 注册表类型的人类可读标签。
pub(crate) fn kind_label(kind: EnvValueKind) -> &'static str {
    match kind {
        EnvValueKind::String => "string",
        EnvValueKind::ExpandString => "expand",
        EnvValueKind::Unsupported => "unsupported",
    }
}

/// PREVIEW 列内容。敏感变量永不显示值（core 已置 `preview=None`，此处再加一层显式标记）；
/// 非敏感但无 preview（空值 / `Unsupported`）显示 `-`。
pub(crate) fn render_preview(meta: &EnvVarMeta) -> String {
    if meta.sensitive {
        return "(敏感)".to_string();
    }
    meta.preview.clone().unwrap_or_else(|| "-".to_string())
}

/// NAME 列内容。不可编辑的变量加 `(只读)` 后缀。
pub(crate) fn render_name(meta: &EnvVarMeta) -> String {
    if meta.can_edit {
        meta.name.clone()
    } else {
        format!("{} (只读)", meta.name)
    }
}

/// 渲染变量表格。空列表返回占位文案。
pub(crate) fn render_table(metas: &[EnvVarMeta]) -> String {
    if metas.is_empty() {
        return "（无变量）".to_string();
    }
    let rows: Vec<(String, &str, String)> = metas
        .iter()
        .map(|m| (render_name(m), kind_label(m.kind), render_preview(m)))
        .collect();
    let name_w = rows
        .iter()
        .map(|r| r.0.chars().count())
        .chain(std::iter::once("NAME".len()))
        .max()
        .unwrap_or(4);
    let kind_w = rows
        .iter()
        .map(|r| r.1.len())
        .chain(std::iter::once("KIND".len()))
        .max()
        .unwrap_or(4);

    let mut out = format!(
        "{:<name_w$}  {:<kind_w$}  {}\n",
        "NAME",
        "KIND",
        "PREVIEW",
        name_w = name_w,
        kind_w = kind_w
    );
    for (name, kind, preview) in &rows {
        let pad = name_w.saturating_sub(name.chars().count());
        out.push_str(&format!(
            "{}{}  {:<kind_w$}  {}\n",
            name,
            " ".repeat(pad),
            kind,
            preview,
            kind_w = kind_w
        ));
    }
    out.trim_end().to_string()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p patheditor-cli --bins`
Expected: PASS（累计 25 个）

- [ ] **Step 5: Commit**

```bash
git add cli/src/env_ops.rs
git commit -m "feat(cli): 环境变量表格渲染与只读/敏感标记"
```

---

### Task 5: JSON 输出组装（list --json）

**Files:**

- Modify: `cli/src/env_ops.rs`

**Interfaces:**

- Consumes: `path_editor_core::env_var::EnvVarSnapshot`、`serde_json`
- Produces:
  - `pub(crate) fn snapshot_json(snapshot: &EnvVarSnapshot, system: bool, user: bool) -> serde_json::Value` —— 单 hive 过滤后的 JSON

**契约说明：** `EnvVarSnapshot` 的 serde 派生为 `camelCase`（`canEdit`/`canDelete`/`preview`/`revision`），且**不含 `value` 字段**。JSON 输出必须直接序列化 core 契约，不得手工重组字段——这是契约单一来源。

- [ ] **Step 1: Write the failing test**

在 `cli/src/env_ops.rs` 的 `mod tests` 内追加：

```rust
    // ── JSON 输出 ──

    fn sample_snapshot() -> EnvVarSnapshot {
        EnvVarSnapshot {
            system: vec![meta("windir", EnvValueKind::String, Some("C:\\Windows"), false, false)],
            user: vec![meta("JAVA_HOME", EnvValueKind::String, Some("C:\\Java"), true, false)],
        }
    }

    #[test]
    fn json_both_hives_included_by_default() {
        let value = snapshot_json(&sample_snapshot(), false, false);
        assert!(value.get("system").is_some());
        assert!(value.get("user").is_some());
    }

    #[test]
    fn json_system_only_filters_user() {
        let value = snapshot_json(&sample_snapshot(), true, false);
        assert!(value.get("system").is_some());
        assert!(value.get("user").is_none(), "单 hive 模式不得输出另一 hive");
    }

    #[test]
    fn json_user_only_filters_system() {
        let value = snapshot_json(&sample_snapshot(), false, true);
        assert!(value.get("user").is_some());
        assert!(value.get("system").is_none());
    }

    #[test]
    fn json_uses_camel_case_contract() {
        let value = snapshot_json(&sample_snapshot(), false, true);
        let first = &value["user"][0];
        assert!(first.get("canEdit").is_some(), "契约字段必须是 camelCase");
        assert!(first.get("canDelete").is_some());
        assert!(first.get("name").is_some());
        assert!(first.get("revision").is_some());
        assert!(
            first.get("value").is_none(),
            "契约上不得出现 value 字段（明文只经 env get 输出）"
        );
        assert!(first.get("can_edit").is_none(), "不得混入 snake_case");
    }

    #[test]
    fn json_empty_hive_is_empty_array() {
        let empty = EnvVarSnapshot::default();
        let value = snapshot_json(&empty, false, false);
        assert_eq!(value["system"].as_array().map(Vec::len), Some(0));
        assert_eq!(value["user"].as_array().map(Vec::len), Some(0));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p patheditor-cli --bins`
Expected: 编译失败 —— `snapshot_json` 未定义

- [ ] **Step 3: 实现 JSON 组装**

在 `cli/src/env_ops.rs` 中 `render_table` 之后追加：

```rust
/// 按 hive 过滤构造 JSON 输出对象。
///
/// 该字段直接来自 core 的 `EnvVarSnapshot` 契约（camelCase、无 `value`），
/// 不做任何字段重组，避免出现第二套契约。
pub(crate) fn snapshot_json(snapshot: &EnvVarSnapshot, system: bool, user: bool) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    if system || !user {
        map.insert(
            "system".to_string(),
            serde_json::to_value(&snapshot.system).unwrap_or(serde_json::Value::Null),
        );
    }
    if user || !system {
        map.insert(
            "user".to_string(),
            serde_json::to_value(&snapshot.user).unwrap_or(serde_json::Value::Null),
        );
    }
    serde_json::Value::Object(map)
}
```

- [ ] **Step 4: 移除未使用的 json 导入（若 clippy 报未使用）**

Task 2 引入的 `use serde_json::json;` 在本任务后可能仍未被使用。运行 clippy 后若报 `unused import`，将 `cli/src/env_ops.rs` 顶部的 `use serde_json::json;` 一行删除。

Run: `cargo clippy -p patheditor-cli --all-targets -- -D warnings`
Expected: 零警告（若报 `unused import: serde_json::json`，删掉该行后重跑）

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p patheditor-cli --bins`
Expected: PASS（累计 30 个）

- [ ] **Step 6: Commit**

```bash
git add cli/src/env_ops.rs
git commit -m "feat(cli): env list 的 JSON 输出按 hive 过滤"
```

---

### Task 6: 命令实现（list / get）

**Files:**

- Modify: `cli/src/env_ops.rs`

**Interfaces:**

- Consumes: `resolve_value`（Task 2）、`render_table`（Task 4）、`snapshot_json`（Task 5）、`crate::runtime::{exit_err, exit_conflict, is_conflict}`
- Produces:
  - `pub(crate) fn select_hive(system: bool, user: bool) -> EnvHive` —— 默认 user
  - `pub(crate) fn cmd_env_list(system: bool, user: bool, json_out: bool)`
  - `pub(crate) fn cmd_env_get(name: String, system: bool)`
  - `pub(crate) fn hive_label(hive: EnvHive) -> &'static str`

**行为说明（依 spec）：**

- `list` 默认两个 hive；`--system`/`--user` 过滤单侧
- `get` 的 stdout **只打印值本身 + 换行，零装饰**（管道友好）
- `get` 在当前 hive 未命中时，查询另一 hive 仅为生成更好的错误提示；**写操作不做此兜底**

- [ ] **Step 1: Write the failing test**

在 `cli/src/env_ops.rs` 的 `mod tests` 内追加：

```rust
    // ── hive 选择 ──

    #[test]
    fn hive_defaults_to_user() {
        assert_eq!(select_hive(false, false), EnvHive::User);
    }

    #[test]
    fn hive_system_flag_wins() {
        assert_eq!(select_hive(true, false), EnvHive::System);
    }

    #[test]
    fn hive_user_flag_selects_user() {
        assert_eq!(select_hive(false, true), EnvHive::User);
    }

    #[test]
    fn hive_label_is_chinese() {
        assert_eq!(hive_label(EnvHive::System), "系统");
        assert_eq!(hive_label(EnvHive::User), "用户");
    }

    // ── get 的输出必须是裸值 ──

    #[test]
    fn get_output_is_bare_value() {
        // stdout 契约：值 + 换行，无前缀、无引号、无标签
        assert_eq!(format_get_output("C:\\Java"), "C:\\Java\n");
        assert_eq!(format_get_output(""), "\n");
        assert_eq!(format_get_output("a b"), "a b\n");
    }

    #[test]
    fn get_output_preserves_inner_newlines() {
        // 值内部的换行不处理，仅补一个结尾换行
        assert_eq!(format_get_output("a\nb"), "a\nb\n");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p patheditor-cli --bins`
Expected: 编译失败 —— `select_hive` / `hive_label` / `format_get_output` 未定义

- [ ] **Step 3: 实现 list 与 get**

在 `cli/src/env_ops.rs` 中 `snapshot_json` 之后追加：

```rust
/// hive 选择：默认 user，`--system` 切换。
///
/// 写操作绝不跨 hive 兜底 —— 用户没指定 `--system` 时改的是 user hive，
/// 同名变量存在于系统 hive 也不会被意外写入。`--system` 与 `--user` 同时给出
/// 由 clap 的 `conflicts_with` 拒绝（见 main.rs）。
pub(crate) fn select_hive(system: bool, user: bool) -> EnvHive {
    let _ = user;
    if system {
        EnvHive::System
    } else {
        EnvHive::User
    }
}

/// hive 的中文标签（用于提示文案）。
pub(crate) fn hive_label(hive: EnvHive) -> &'static str {
    match hive {
        EnvHive::System => "系统",
        EnvHive::User => "用户",
    }
}

/// `env get` 的 stdout 负载：裸值 + 换行。
///
/// 无前缀、无引号、无标签 —— 使 `patheditor env get JAVA_HOME` 可直接被
/// 命令替换或管道消费（对齐 `git config --get`）。
pub(crate) fn format_get_output(value: &str) -> String {
    format!("{value}\n")
}

/// `env list` —— 列出变量元数据（不含明文）。
pub(crate) fn cmd_env_list(system: bool, user: bool, json_out: bool) {
    let snapshot = core::registry::list_all_env_vars().unwrap_or_else(|e| exit_err(&e));
    if json_out {
        let value = snapshot_json(&snapshot, system, user);
        println!("{}", serde_json::to_string_pretty(&value).unwrap());
        return;
    }
    let show_sys = system || !user;
    let show_usr = user || !system;
    if show_sys {
        println!(
            "═══ 系统环境变量（{} 个）═══",
            snapshot.system.len()
        );
        println!("{}", render_table(&snapshot.system));
    }
    if show_usr {
        println!(
            "═══ 用户环境变量（{} 个）═══",
            snapshot.user.len()
        );
        println!("{}", render_table(&snapshot.user));
    }
}

/// `env get` —— 读取单个变量的明文。这是 CLI 侧唯一的明文出口。
pub(crate) fn cmd_env_get(name: String, system: bool) {
    let hive = select_hive(system, false);
    match core::registry::reveal_env_var(hive, &name) {
        Ok(value) => print!("{}", format_get_output(&value)),
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

/// 当前 hive 未命中时，探测另一 hive 以生成更准确的错误提示。
///
/// 返回 `None` 表示另一 hive 也没有该变量（沉默，避免误导）。
fn other_hive_hint(name: &str, current: EnvHive) -> Option<String> {
    let other = match current {
        EnvHive::System => EnvHive::User,
        EnvHive::User => EnvHive::System,
    };
    let snapshot = core::registry::list_all_env_vars().ok()?;
    let metas = match other {
        EnvHive::System => &snapshot.system,
        EnvHive::User => &snapshot.user,
    };
    let found = metas.iter().any(|m| m.name.eq_ignore_ascii_case(name));
    if found {
        Some(format!(
            "提示：{name} 存在于{} hive，请加 --system",
            hive_label(other)
        ))
    } else {
        None
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p patheditor-cli --bins`
Expected: PASS（累计 36 个）

- [ ] **Step 5: Commit**

```bash
git add cli/src/env_ops.rs
git commit -m "feat(cli): env list 与 env get 命令实现"
```

---

### Task 7: 命令实现（set / add / remove）

**Files:**

- Modify: `cli/src/env_ops.rs`

**Interfaces:**

- Consumes: `resolve_value`/`read_value`（Task 2）、`resolve_concurrency`/`apply_concurrency`（Task 3）、`select_hive`（Task 6）
- Produces:
  - `pub(crate) fn cmd_env_set(name: String, value: Option<String>, stdin: bool, value_file: Option<String>, revision: Option<String>, force: bool, system: bool)`
  - `pub(crate) fn cmd_env_add(name: String, value: Option<String>, stdin: bool, value_file: Option<String>, kind: String, system: bool)`
  - `pub(crate) fn cmd_env_remove(name: String, revision: Option<String>, force: bool, system: bool)`
  - `pub(crate) fn parse_kind(raw: &str) -> EnvValueKind`

**行为说明（依 spec）：**

- `set` 更新已存在变量，类型跟随注册表现状（由 core 的 `update_env_var` 保证）
- `--force` 模式下 core 仍要求 `expected_revision` 参数；此时先用 `reveal`/`list` 取当前 revision 再传入 —— 即「现读现写」，竞态窗口仅毫秒级，语义等价于无条件覆盖
- `add` 需 `--kind` 指定类型（`string` / `expand`，**默认 `string`**；非法值由 clap 的 `value_parser` 拒绝），保护名/保留名/重名由 core 拒绝
- `remove` 需 `--revision` 或 `--force`
- 三个命令的**环境变更广播由 core 写入口内部完成**（`update_env_var` / `create_env_var` / `delete_env_var` 写入成功后各自调用 `broadcast_env_change`），CLI 不再重复广播（见 Execution Notes 与本计划 Task 7 Step 3 的最终实现）
- `--kind` 非法值由 clap 的 `value_parser` 拒绝（见 main.rs），`parse_kind` 仅服务于已知合法输入

- [ ] **Step 1: Write the failing test**

在 `cli/src/env_ops.rs` 的 `mod tests` 内追加：

```rust
    // ── kind 解析 ──

    #[test]
    fn parse_kind_maps_both_writable_kinds() {
        assert_eq!(parse_kind("string"), EnvValueKind::String);
        assert_eq!(parse_kind("expand"), EnvValueKind::ExpandString);
    }

    #[test]
    fn parse_kind_is_case_insensitive() {
        assert_eq!(parse_kind("STRING"), EnvValueKind::String);
        assert_eq!(parse_kind("Expand"), EnvValueKind::ExpandString);
    }

    // ── force 模式的 revision 获取 ──

    #[test]
    fn force_revision_source_is_documented() {
        // Force 模式仍需向 core 传 expected_revision；取当前值的方式是
        // 从 list_all_env_vars 的元数据里找同名项。此处断言匹配规则：
        // 注册表名大小写不敏感，匹配必须忽略大小写。
        let metas = vec![meta("Java_Home", EnvValueKind::String, Some("C:\\Java"), true, false)];
        assert_eq!(find_revision(&metas, "JAVA_HOME").as_deref(), Some("0000000000000000"));
        assert_eq!(find_revision(&metas, "GOPATH"), None);
    }

    #[test]
    fn find_revision_prefers_exact_case_then_falls_back() {
        // 同名不同大小写（注册表允许）时，优先精确匹配以保留原始大小写
        let metas = vec![meta("JAVA_HOME", EnvValueKind::String, None, true, false)];
        assert!(find_revision(&metas, "java_home").is_some());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p patheditor-cli --bins`
Expected: 编译失败 —— `parse_kind` / `find_revision` 未定义

- [ ] **Step 3: 实现 set / add / remove**

在 `cli/src/env_ops.rs` 中 `other_hive_hint` 之后追加：

```rust
/// 解析 `--kind` 取值。非法值由 clap 的 `value_parser` 提前拒绝。
pub(crate) fn parse_kind(raw: &str) -> EnvValueKind {
    if raw.eq_ignore_ascii_case("expand") {
        EnvValueKind::ExpandString
    } else {
        EnvValueKind::String
    }
}

/// 从元数据中按名（忽略大小写）查找 revision。
///
/// 用于 `--force` 模式：core 的写入口签名恒要求 `expected_revision`，
/// 跳过校验的语义由「立即重新读取当前 revision 并传入」表达。
pub(crate) fn find_revision(metas: &[EnvVarMeta], name: &str) -> Option<String> {
    // 优先精确大小写匹配，保证写回时使用注册表中的原始名对应的 revision
    metas
        .iter()
        .find(|m| m.name == name)
        .or_else(|| metas.iter().find(|m| m.name.eq_ignore_ascii_case(name)))
        .map(|m| m.revision.clone())
}

/// 按 hive 取当前 revision（`--force` 模式用）。
fn current_revision(hive: EnvHive, name: &str) -> String {
    let snapshot = core::registry::list_all_env_vars().unwrap_or_else(|e| exit_err(&e));
    let metas = match hive {
        EnvHive::System => &snapshot.system,
        EnvHive::User => &snapshot.user,
    };
    find_revision(metas, name).unwrap_or_else(|| {
        exit_err(&format!(
            "{} hive 中未找到变量 {name}",
            hive_label(hive)
        ))
    })
}

/// 把并发模式转换成 core 需要的 `expected_revision`。
fn expected_revision(hive: EnvHive, name: &str, mode: &Concurrency) -> String {
    match mode {
        Concurrency::Revision(r) => r.clone(),
        Concurrency::Force => current_revision(hive, name),
    }
}

/// `env set` —— 修改已有变量的值。类型跟随注册表现状，不可更改。
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
    let expected = expected_revision(hive, &name, &mode);
    apply_concurrency(core::registry::update_env_var(hive, &name, &new_value, &expected));
    // 环境变更广播由 core 写入口负责，此处不重复广播
    println!("已更新{}变量: {name}", hive_label(hive));
}

/// `env add` —— 新建变量。`--kind` 决定注册表类型。
pub(crate) fn cmd_env_add(
    name: String,
    value: Option<String>,
    stdin: bool,
    value_file: Option<String>,
    kind: String,
    system: bool,
) {
    let hive = select_hive(system, false);
    let src = resolve_value(value, stdin, value_file, true);
    let new_value = read_value(&src);
    let kind = parse_kind(&kind);
    // 新建无并发语义：core 会拒绝重名（检查与写入是两步，存在竞态窗口，见 IPC 文档）
    core::registry::create_env_var(hive, &name, &new_value, kind).unwrap_or_else(|e| exit_err(&e));
    // 环境变更广播由 core 写入口负责，此处不重复广播
    println!("已新建{}变量: {name}", hive_label(hive));
}

/// `env remove` —— 删除变量。
pub(crate) fn cmd_env_remove(
    name: String,
    revision: Option<String>,
    force: bool,
    system: bool,
) {
    let hive = select_hive(system, false);
    let mode = resolve_concurrency(revision, force);
    let expected = expected_revision(hive, &name, &mode);
    apply_concurrency(core::registry::delete_env_var(hive, &name, &expected));
    // 环境变更广播由 core 写入口负责，此处不重复广播
    println!("已删除{}变量: {name}", hive_label(hive));
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p patheditor-cli --bins`
Expected: PASS（累计 40 个）

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p patheditor-cli --all-targets -- -D warnings`
Expected: 零警告

- [ ] **Step 6: Commit**

```bash
git add cli/src/env_ops.rs
git commit -m "feat(cli): env set / add / remove 命令实现"
```

---

### Task 8: 接线到 Clap 与 main 分派

**Files:**

- Modify: `cli/src/main.rs`

**Interfaces:**

- Consumes: `env_ops::cmd_env_{list,get,set,add,remove}`（Task 6/7）
- Produces: 可执行的 `patheditor env ...` 命令族

- [ ] **Step 1: 添加 Clap 枚举**

`cli/src/main.rs` 的 `enum Command` 内，在 `Profile(ProfileCmd)` 变体**之前**追加：

```rust
    /// 管理通用环境变量（Path 除外，请使用 PATH 专用命令）
    #[command(subcommand)]
    Env(EnvCmd),
```

并在 `enum ProfileCmd` 定义之后追加：

```rust
#[derive(Subcommand)]
enum EnvCmd {
    /// 列出环境变量元数据（不含明文）
    List {
        #[arg(short, long)]
        system: bool,
        #[arg(short, long, conflicts_with = "system")]
        user: bool,
        #[arg(long)]
        json: bool,
    },
    /// 读取单个变量的明文
    Get {
        name: String,
        #[arg(short, long)]
        system: bool,
    },
    /// 修改已有变量的值（类型不变）
    Set {
        name: String,
        /// 值（敏感值建议改用 --stdin 或 --value-file，避免进 shell 历史）
        #[arg(long)]
        value: Option<String>,
        /// 从标准输入读取值（读到 EOF）
        #[arg(long, conflicts_with_all = ["value", "value_file"])]
        stdin: bool,
        /// 从文件读取值
        #[arg(long, conflicts_with_all = ["value", "stdin"])]
        value_file: Option<String>,
        /// 并发校验摘要（来自 `env list --json` 的 revision 字段）
        #[arg(long, conflicts_with = "force")]
        revision: Option<String>,
        /// 跳过并发校验直接覆盖
        #[arg(long)]
        force: bool,
        #[arg(short, long)]
        system: bool,
    },
    /// 新建变量
    Add {
        name: String,
        /// 值（敏感值建议改用 --stdin 或 --value-file）
        value: Option<String>,
        #[arg(long, conflicts_with_all = ["value", "value_file"])]
        stdin: bool,
        #[arg(long, conflicts_with_all = ["value", "stdin"])]
        value_file: Option<String>,
        /// 注册表类型：string (REG_SZ) 或 expand (REG_EXPAND_SZ)
        #[arg(long, default_value = "string", value_parser = ["string", "expand"])]
        kind: String,
        #[arg(short, long)]
        system: bool,
    },
    /// 删除变量
    Remove {
        name: String,
        #[arg(long, conflicts_with = "force")]
        revision: Option<String>,
        /// 跳过并发校验直接删除
        #[arg(long)]
        force: bool,
        #[arg(short, long)]
        system: bool,
    },
}
```

- [ ] **Step 2: 添加 match 分派**

在 `cli/src/main.rs` 的 `main()` 中，`Command::Profile(cmd) => match cmd { ... }` **之前**追加：

```rust
        Command::Env(cmd) => match cmd {
            EnvCmd::List { system, user, json } => env_ops::cmd_env_list(system, user, json),
            EnvCmd::Get { name, system } => env_ops::cmd_env_get(name, system),
            EnvCmd::Set {
                name,
                value,
                stdin,
                value_file,
                revision,
                force,
                system,
            } => env_ops::cmd_env_set(name, value, stdin, value_file, revision, force, system),
            EnvCmd::Add {
                name,
                value,
                stdin,
                value_file,
                kind,
                system,
            } => env_ops::cmd_env_add(name, value, stdin, value_file, kind, system),
            EnvCmd::Remove {
                name,
                revision,
                force,
                system,
            } => env_ops::cmd_env_remove(name, revision, force, system),
        },
```

- [ ] **Step 3: 构建并验证帮助输出**

Run: `cargo build -p patheditor-cli`
Expected: 编译成功，零错误

Run: `cargo run -p patheditor-cli -- env --help`
Expected: 输出 5 个子命令（list / get / set / add / remove）及其参数说明

Run: `cargo run -p patheditor-cli -- env set --help`
Expected: 显示 `--value` / `--stdin` / `--value-file` / `--revision` / `--force` / `--system`，且 `--revision` 与 `--force` 标注互斥

- [ ] **Step 4: 验证只读命令的正确性（不写注册表）**

Run: `cargo run -p patheditor-cli -- env list --user`
Expected: 输出用户环境变量表格（表头 NAME / KIND / PREVIEW），敏感变量显示 `(敏感)`

Run: `cargo run -p patheditor-cli -- env list --user --json | head -20`
Expected: JSON 含 `canEdit`/`revision` 等 camelCase 字段，**不含 `value`**

Run: `cargo run -p patheditor-cli -- env get Path`
Expected: 错误退出（退出码 1），提示 `Path 由专用 PATH 通路管理`（`env get` 只有 `--system`，没有 `--user`；默认即 user hive，加 `--user` 会被 clap 以退出码 2 拒绝）

- [ ] **Step 5: 验证参数校验（不写注册表）**

Run: `cargo run -p patheditor-cli -- env set JAVA_HOME --value a --stdin`
Expected: clap 报错（互斥），退出码 2，**未触碰注册表**

Run: `cargo run -p patheditor-cli -- env set JAVA_HOME --value a`
Expected: 报错 `需要提供 --revision（并发校验）或 --force（跳过校验）`，退出码 1

Run: `cargo run -p patheditor-cli -- env set JAVA_HOME --value a --revision r --force`
Expected: clap 报错（`--revision` 与 `--force` 互斥），退出码 2

- [ ] **Step 6: 全量质量门**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: 全部通过

- [ ] **Step 7: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat(cli): 接入 env 子命令组到命令分派"
```

---

### Task 9: 文档同步

**Files:**

- Modify: `README.md`（CLI 命令表）
- Modify: `AGENTS.md` 和 `CLAUDE.md`（CLI 命令节 + 错误处理节）

**Interfaces:**

- Consumes: Task 8 的最终命令面
- Produces: 文档与实现一致

- [ ] **Step 1: 更新 README 的 CLI 命令表**

在 `README.md` 的 CLI 命令代码块中，`patheditor profile ...` 行之后追加：

```text
patheditor env list     [--system|--user] [--json]
patheditor env get      <NAME> [--system]
patheditor env set      <NAME> [--value <V>|--stdin|--value-file <F>] (--revision <R>|--force)
patheditor env add      <NAME> [<VALUE>] [--kind string|expand] [--system]
patheditor env remove   <NAME> (--revision <R>|--force)
```

- [ ] **Step 2: 在 README 补充说明段**

在 CLI 命令说明段落中追加：

```markdown
`env` 子命令管理通用环境变量（`Path` 除外，请用 PATH 专用命令）。默认操作用户 hive，加 `--system` 操作系统 hive。

`set` 与 `remove` 必须显式选择并发模式：`--revision <R>`（取自 `env list --json`，外部修改时拒绝写入，退出码 3）或 `--force`（跳过校验直接覆盖）。敏感值建议用 `--stdin` 或 `--value-file` 传入，避免明文进入 shell 历史与进程列表。
```

- [ ] **Step 3: 同步 AGENTS.md 与 CLAUDE.md 的 CLI 命令节**

在两个文件的 CLI 命令代码块中同样的位置追加同一段命令清单（Step 1 的内容），并在命令说明段落追加：

```markdown
`env` 子命令默认操作用户 hive，加 `--system` 操作系统 hive；`Path` 不在通用通路内。`set`/`remove` 必须显式给出 `--revision` 或 `--force`（互斥，缺一报错）；revision 冲突时退出码为 3，其余错误为 1。
```

- [ ] **Step 4: 在两个文件的错误处理节补充退出码约定**

在 `错误处理与安全` 段落中追加一行：

```markdown
- CLI 退出码：0 成功、1 一般错误、3 revision 冲突（仅 `env set`/`env remove`；PATH 命令恒为 1）。冲突判定按 core 的 `[E_CONFLICT]` 前缀，不匹配中文正文。
```

- [ ] **Step 5: 修复 AGENTS.md / CLAUDE.md 的 IPC 表格分隔行宽度不一致**

复审遗留问题：`AGENTS.md` 的 IPC 表格因 `create_env_var` 行说明变长而整表加宽，`CLAUDE.md` 保持旧列宽。

操作：用同一份内容覆盖两个文件——把 `AGENTS.md` 的 IPC 表格整段（含分隔行）复制到 `CLAUDE.md` 的对应位置，或反之，使两文件的该表格**逐字节相同**。

- [ ] **Step 6: 验证两文件内容一致**

Run:

```bash
diff <(sed 's/[[:space:]]*$//' AGENTS.md) <(sed 's/[[:space:]]*$//' CLAUDE.md) && echo "CONTENT_IDENTICAL"
```

Expected: 输出 `CONTENT_IDENTICAL`（无差异）。若仍有差异，检查是否漏同步了某个段落

- [ ] **Step 7: Commit**

```bash
git add README.md AGENTS.md CLAUDE.md
git commit -m "docs: 同步 CLI env 子命令与环境变量管理说明"
```

---

## Self-Review

**1. Spec coverage**

| Spec 要求                                                  | 对应任务                                                                                                                 |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `env list` — 表格 + `--json` + hive 过滤                   | Task 4（渲染）、Task 5（JSON）、Task 6（命令）、Task 8（CLI 接线）                                                       |
| `env get` — 裸值输出、单 hive 提示                         | Task 6（`format_get_output` / `other_hive_hint`）、Task 8                                                                |
| `env set` — 值三通道、并发双模式                           | Task 2（通道）、Task 3（并发）、Task 7（命令）、Task 8                                                                   |
| `env add` — `--kind` 默认 `string`，可选 `string`/`expand` | Task 7（`parse_kind`）、Task 8（`value_parser` 默认值与合法值约束）                                                      |
| `env remove` — 并发双模式                                  | Task 3、Task 7、Task 8                                                                                                   |
| 双模式强制显式选择                                         | Task 3（`resolve_concurrency`）、Task 8 Step 5 验证                                                                      |
| 三通道值输入互斥                                           | Task 2、Task 8 Step 5 验证                                                                                               |
| 默认 user hive、写操作不跨 hive                            | Task 6（`select_hive`）                                                                                                  |
| 退出码 0/1/3                                               | Task 1（`exit_conflict` / `is_conflict`）、Task 3（`apply_concurrency`）                                                 |
| 安全判定单一实现处                                         | Task 2 模块注释 + 全部任务仅透传 core 错误；Task 8 Step 4 验证 Path 拒绝                                                 |
| 环境变更广播（写操作后）                                   | 由 core 写入口内部完成（`update_env_var` / `create_env_var` / `delete_env_var`）；CLI 侧不重复广播（见 Execution Notes） |
| 纯函数单测                                                 | Task 1-7 各自的 `mod tests`                                                                                              |
| 文档同步（README/AGENTS/CLAUDE + 表格宽度修复）            | Task 9                                                                                                                   |

无遗漏。

**2. Placeholder scan**

逐任务检查：无 TBD/TODO/"适当处理"/"类似 Task N"。每个实现步骤均含完整代码块，每个测试步骤均含完整测试函数体。

**3. Type consistency**

| 符号                                                                                        | 定义任务  | 使用任务                 | 签名一致        |
| ------------------------------------------------------------------------------------------- | --------- | ------------------------ | --------------- |
| `exit_conflict(msg: &str) -> !`                                                             | Task 1    | Task 3                   | ✓               |
| `is_conflict(msg: &str) -> bool`                                                            | Task 1    | Task 2, 3                | ✓               |
| `core::registry::conflict_message() -> String`                                              | Task 1    | Task 1 测试, Task 3 测试 | ✓               |
| `ValueSource` / `resolve_value` / `read_value` / `strip_trailing_newline` / `channel_count` | Task 2    | Task 7                   | ✓               |
| `Concurrency` / `resolve_concurrency` / `concurrency_count` / `apply_concurrency`           | Task 3    | Task 7                   | ✓               |
| `kind_label` / `render_preview` / `render_name` / `render_table`                            | Task 4    | Task 6                   | ✓               |
| `snapshot_json`                                                                             | Task 5    | Task 6                   | ✓               |
| `select_hive` / `hive_label` / `format_get_output` / `other_hive_hint`                      | Task 6    | Task 7                   | ✓               |
| `parse_kind` / `find_revision` / `current_revision` / `expected_revision`                   | Task 7    | Task 8                   | ✓               |
| `cmd_env_{list,get,set,add,remove}`                                                         | Task 6, 7 | Task 8                   | ✓（参数序一致） |

---

## Execution Notes

- **Task 2 Step 6 的 clippy 提示**：Task 2 结束时 `exit_conflict` / `is_conflict` 尚未被调用，可能触发 `dead_code` 警告。**不要**为此添加 `#[allow(dead_code)]`——Task 3 会立即使用它们。若 clippy 阻塞该步骤，跳过 Step 6 的 clippy 检查，留到 Task 7 Step 5 统一验证。
- **禁止真实注册表写入**：Task 8 Step 4/5 只执行**只读** command（`env list` / `env get`）与**参数校验失败**的 command（clap 在解析阶段即拒绝，不会执行到写路径）。**不得**执行任何会真正写注册表的 `env set` / `env add` / `env remove`。真实写入验证需用户显式授权并记录备份/快照/回滚。
- **不推送**：所有 commit 留在本地 `worktree-all-env-vars` 分支，等待用户决定合并时机。

### 实际执行偏离记录（Task 1-9 执行后补记）

本节如实记录执行过程中与计划原文的偏离，供后续复盘与计划模板改进使用。

| #   | 计划原文                                                              | 实际                                                                                                                   | 处理                                                                                                                     |
| --- | --------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| 1   | `cargo test -p patheditor-cli --lib`                                  | `patheditor-cli` 是 **bin-only crate，没有 lib target**，`--lib` 会报 `no library targets found`                       | 全文改为 `cargo test -p patheditor-cli --bins`（本文档已同步修正）                                                       |
| 2   | 分支名 `workspace-all-env-vars`                                       | 实际分支为 **`worktree-all-env-vars`**（由 git worktree 创建）                                                         | 本文档 Global Constraints、Execution Notes 与 spec 第 4 行已同步修正                                                     |
| 3   | Task 7 Step 5 要求 clippy 转绿                                        | Task 7 结束时 `cmd_env_*` 尚未接线到 `main.rs`，`dead_code` 无法消除，clippy **不可能在 Task 7 转绿**                  | 实际留到 **Task 8 Step 6 全量质量门**统一收口                                                                            |
| 4   | 测试计数：Task 6 后 36、Task 7 后 40                                  | 实际 **Task 6 后 37 个、Task 7 后 41 个**（计划中的 40 计数有误）                                                      | 以实际为准：`runtime` 3 + `env_ops` 38 = 41                                                                              |
| 5   | Task 7 三个写命令各自 `broadcast_env_change()`                        | core 的 `update_env_var` / `create_env_var` / `delete_env_var` **在写入口内部已广播**，CLI 重复调用是多余的 Win32 广播 | 经用户裁决移除（提交 `7b410b6`）；本文档 Task 7 代码块与 Self-Review 表已同步修正                                        |
| 6   | Task 8 Step 4 验证命令 `patheditor env get Path --user`，期望退出码 1 | `env get` 只有 `--system`（无 `--user`），`--user` 会被 **clap 以退出码 2 拒绝**，无法验证预期行为                     | 改为 `patheditor env get Path`（默认 user hive），实测退出码 1 + `错误: Path 由专用 PATH 通路管理，请使用 PATH 视图编辑` |
| 7   | `env add` 的 `--kind` 标注为「必填」                                  | 最终裁决保留**选填**：`#[arg(long, default_value = "string", value_parser = ["string", "expand"])]`                    | spec 命令规格/决策段/验收标准、本文档 Self-Review 表、Task 7 行为说明均已改为「默认 `string`，可选 `string`/`expand`」   |

**未偏离但值得记录**：`env list --json` 的返回为对象 `{"system": [...], "user": [...]}`（按 hive 分键），而非原计划测试设想的「顶层数组」；该形状直接来自 core 的 `EnvVarSnapshot` 契约，已按契约实现。
