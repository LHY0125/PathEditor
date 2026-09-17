# 全环境变量编辑扩展 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:assistant-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 PathEditor 的编辑范围从单一的 `Path` 扩展到 Windows 环境变量项下的所有变量，采用旁路方案 —— 新增 `EnvVar` 通路，PATH 继续走现有 `PathEntry` 通路不变。

**Architecture:** Rust core 新增 `env_var.rs` 承载 `EnvVarMeta` / `EnvVarSnapshot` 契约与安全判定（保留/保护/敏感/权限），`registry.rs` 新增 5 个通用变量读写函数。GUI 层是 5 个薄包装 command，前端经 `backend.ts` 单一 IPC 边界调用。前端新增独立 `env-store` 与 `EnvVarTable`，在 `AppShell` 中以 `allVars` Tab 呈现，使用独立工具栏。

**Tech Stack:** Rust (workspace: core + gui + cli) · Tauri 2.x · React 19 · TypeScript strict · Zustand · Vitest · Playwright · winreg 0.52

**Spec:** `docs/superpowers/specs/2026-09-16-all-env-vars-design.md`

## Global Constraints

- **不写真实注册表**：所有 Rust 测试使用隔离测试键（`TempRegistryKey` RAII 模式，`Drop` 时 `delete_subkey_all`）；E2E 全部走 mock IPC。真实 Tauri/注册表闭环测试需显式授权。
- **`Path` 是保留变量**：`RESERVED_NAMES` 必须含 `Path`；`list_all_env_vars` 过滤（忽略大小写，覆盖 `path` 与 `Path`）；Rust 写入口拒绝。
- **敏感明文不进前端**：`EnvVarMeta` 契约上**不得有 `value` 字段**；命中 `is_sensitive` 的变量 `preview` 为 `None`；明文只能经 `reveal_env_var` 获取。
- **类型只从注册表读取**：`update_env_var` 不得接收也不得使用前端传来的类型；从 `get_raw_value().vtype` 取真实类型原样写回。非 `REG_SZ`/`REG_EXPAND_SZ` 直接拒绝写入（避免重新引入 Issue #26 的 `REG_EXPAND_SZ` 降级）。
- **并发校验在 Rust 内**：`update_env_var` / `delete_env_var` 接收 `expected_revision`，在同一调用内「读→算 revision→比对→校验→写」，前端不得做"保存前重新 list 再比对"。
- **`canEdit` / `canDelete` 由 Rust 计算下发**，前端只做映射，不重复实现判定规则。
- **代码风格**：UTF-8、CRLF；TS 2 空格，Rust/TOML 4 空格；Prettier 单引号、尾逗号、100 列。
- **质量门**：`npm run verify` 全绿（Prettier / ESLint / `tsc -b` / 覆盖率 80% / `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace`）。
- **文档一致性**：`CLAUDE.md` 与 `AGENTS.md` 内容必须保持一致（改动一份即同步另一份）。
- **`unsafe` 块**必须有 `// SAFETY:` 注释。**Tauri CSP 不得设为 `null`**，不得放松 `gui/tauri.conf.json` 安全配置。

---

## 文件结构

### 新建

| 文件                                        | 职责                                                                                                                                                     |
| ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `core/src/env_var.rs`                       | `EnvVarMeta` / `EnvVarSnapshot` / `EnvValueKind` / `EnvHive` 契约；`is_reserved` / `is_protected` / `is_sensitive` 判定；`revision` 计算；`preview` 净化 |
| `gui/src/commands/env_var.rs`               | 5 个 `#[tauri::command]` 薄包装                                                                                                                          |
| `src/core/env-var.ts`                       | 纯展示逻辑：`maskValue` / `validateVarName` / `displayValue` / `filterEnvVars` / `envVarKey`                                                             |
| `src/store/env-store.ts`                    | 独立 Zustand store：snapshot / revealed / draft / hiveFilter + 5 个 action                                                                               |
| `src/components/env-list/EnvVarTable.tsx`   | 虚拟滚动表格 + 敏感打码 + 只读锁定                                                                                                                       |
| `src/components/env-list/EnvVarToolbar.tsx` | 环境变量专用工具栏（新建/编辑/删除/刷新/来源筛选）                                                                                                       |
| `tests/unit/env-var.test.ts`                | `env-var.ts` 纯逻辑测试                                                                                                                                  |
| `tests/unit/env-store.test.ts`              | `env-store.ts` 状态测试                                                                                                                                  |
| `e2e/tests/env-vars.spec.ts`                | 全变量 Tab 的 E2E                                                                                                                                        |

### 修改

| 文件                                      | 改动                                                                                                                                              |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `core/src/lib.rs`                         | `pub mod env_var;` + re-export                                                                                                                    |
| `core/src/registry.rs`                    | 新增 `list_all_env_vars` / `reveal_env_var` / `update_env_var` / `create_env_var` / `delete_env_var` / `validate_env_name` / `validate_env_value` |
| `gui/src/commands/mod.rs`                 | `pub mod env_var;`                                                                                                                                |
| `gui/src/lib.rs`                          | 注册 5 个 command                                                                                                                                 |
| `src/services/backend.ts`                 | 5 个方法 + `parseEnvVarSnapshot` 运行时校验                                                                                                       |
| `src/core/path-capabilities.ts`           | `TabId` 增加 `'allVars'`                                                                                                                          |
| `src/components/layout/AppShell.tsx`      | 4 个 Tab；按 `activeTab === 'allVars'` 分支渲染两套工具栏；drop handler 早退；关窗确认纳入草稿                                                    |
| `src/i18n/locales/zh-CN.json` / `en.json` | Tab 文案 + 新 UI 文案                                                                                                                             |
| `e2e/mocks/ipc.ts`                        | 5 个新 command 的 mock                                                                                                                            |
| `CLAUDE.md` / `AGENTS.md`                 | IPC 表、目录树、关键约束                                                                                                                          |
| `README.md`                               | 功能章节                                                                                                                                          |

---

## Task 1: Rust core 契约与判定函数

**Files:**

- Create: `core/src/env_var.rs`
- Modify: `core/src/lib.rs`
- Test: `core/src/env_var.rs`（模块内 `#[cfg(test)]`）

**Interfaces:**

- Consumes: 无（本任务是最底层）
- Produces:
  - `pub struct EnvVarMeta { pub name: String, pub kind: EnvValueKind, pub hive: EnvHive, pub can_edit: bool, pub can_delete: bool, pub sensitive: bool, pub preview: Option<String>, pub revision: String }`
  - `pub struct EnvVarSnapshot { pub system: Vec<EnvVarMeta>, pub user: Vec<EnvVarMeta> }`
  - `pub enum EnvValueKind { String, ExpandString, Unsupported }`
  - `pub enum EnvHive { System, User }`
  - `pub fn is_reserved(name: &str) -> bool`
  - `pub fn is_protected(name: &str) -> bool`
  - `pub fn is_sensitive(name: &str) -> bool`
  - `pub fn revision_of(name: &str, vtype: u32, value: &str) -> String`
  - `pub fn sanitize_preview(value: &str) -> Option<String>`
  - `pub fn capabilities_for(hive: EnvHive, name: &str, kind: EnvValueKind) -> (bool, bool)`

- [ ] **Step 1: 写失败测试**

在 `core/src/env_var.rs` 末尾写测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use winreg::enums::{REG_BINARY, REG_DWORD, REG_EXPAND_SZ, REG_SZ};

    #[test]
    fn reserved_matches_path_case_insensitively() {
        assert!(is_reserved("Path"));
        assert!(is_reserved("path"));
        assert!(is_reserved("PATH"));
        assert!(!is_reserved("PATHEXT"));
        assert!(!is_reserved("PSModulePath"));
    }

    #[test]
    fn protected_matches_builtin_names_case_insensitively() {
        assert!(is_protected("windir"));
        assert!(is_protected("Windir"));
        assert!(is_protected("ComSpec"));
        assert!(is_protected("PATHEXT"));
        // PSModulePath 有意不在名单内
        assert!(!is_protected("PSModulePath"));
        assert!(!is_protected("JAVA_HOME"));
    }

    #[test]
    fn sensitive_matches_secret_like_names() {
        assert!(is_sensitive("HALO_MCP_TOKEN"));
        assert!(is_sensitive("MINIMAX_API_KEY"));
        assert!(is_sensitive("MY_SECRET"));
        assert!(is_sensitive("DB_PASSWORD"));
        assert!(is_sensitive("AZURE_CREDENTIAL"));
        assert!(is_sensitive("OPENAI_API"));
        assert!(!is_sensitive("JAVA_HOME"));
        assert!(!is_sensitive("GOPATH"));
        assert!(!is_sensitive("TEMP"));
    }

    #[test]
    fn revision_changes_with_any_component() {
        let base = revision_of("JAVA_HOME", REG_SZ, "C:\\Java");
        assert_eq!(base, revision_of("JAVA_HOME", REG_SZ, "C:\\Java"));
        assert_ne!(base, revision_of("JAVA_HOME", REG_SZ, "C:\\Other"));
        assert_ne!(base, revision_of("JAVA_HOME", REG_EXPAND_SZ, "C:\\Java"));
        assert_ne!(base, revision_of("GOPATH", REG_SZ, "C:\\Java"));
    }

    #[test]
    fn sanitize_preview_truncates_and_strips_control_chars() {
        assert_eq!(sanitize_preview("C:\\Java"), Some("C:\\Java".to_string()));
        assert_eq!(sanitize_preview("line1\nline2"), Some("line1line2".to_string()));
        assert_eq!(sanitize_preview("a\rb\0c"), Some("abc".to_string()));
        assert_eq!(sanitize_preview(""), None);
        assert_eq!(sanitize_preview("\n\r\0"), None);
        let long = "x".repeat(300);
        let sanitized = sanitize_preview(&long).expect("长值不应为 None");
        assert_eq!(sanitized.chars().count(), 257); // 256 + '…'
        assert!(sanitized.ends_with('…'));
    }

    #[test]
    fn capabilities_deny_unsupported_kind() {
        let (can_edit, can_delete) = capabilities_for(
            EnvHive::User,
            "SOME_BINARY_VAR",
            EnvValueKind::Unsupported,
        );
        assert!(!can_edit);
        assert!(!can_delete);
    }

    #[test]
    fn capabilities_deny_protected_even_when_writable() {
        let (can_edit, can_delete) = capabilities_for(EnvHive::User, "windir", EnvValueKind::String);
        assert!(!can_edit);
        assert!(!can_delete);
    }

    #[test]
    fn kind_maps_from_reg_type() {
        assert_eq!(EnvValueKind::from_reg_type(REG_SZ), EnvValueKind::String);
        assert_eq!(
            EnvValueKind::from_reg_type(REG_EXPAND_SZ),
            EnvValueKind::ExpandString
        );
        assert_eq!(
            EnvValueKind::from_reg_type(REG_DWORD),
            EnvValueKind::Unsupported
        );
        assert_eq!(
            EnvValueKind::from_reg_type(REG_BINARY),
            EnvValueKind::Unsupported
        );
    }

    #[test]
    fn snapshot_serializes_camel_case() {
        let snapshot = EnvVarSnapshot {
            system: vec![],
            user: vec![EnvVarMeta {
                name: "JAVA_HOME".into(),
                kind: EnvValueKind::String,
                hive: EnvHive::User,
                can_edit: true,
                can_delete: true,
                sensitive: false,
                preview: Some("C:\\Java".into()),
                revision: "abc".into(),
            }],
        };
        let value = serde_json::to_value(&snapshot).expect("序列化失败");
        assert!(value.get("user").is_some());
        let first = &value["user"][0];
        assert!(first.get("canEdit").is_some());
        assert!(first.get("canDelete").is_some());
        assert!(first.get("can_edit").is_none());
        assert!(first.get("value").is_none(), "契约上不得出现 value 字段");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cargo test -p path-editor-core env_var 2>&1 | head -40
```

Expected: FAIL — `error[E0433]: failed to resolve: use of undeclared crate or module env_var`（模块尚未创建）

- [ ] **Step 3: 写最小实现**

创建 `core/src/env_var.rs`（测试模块之外的部分）：

```rust
//! 通用环境变量契约与安全判定。
//!
//! 本模块的判定函数（保留 / 保护 / 敏感 / 权限）在 Rust 侧计算并下发，
//! 前端不重复实现 —— 判定规则是安全边界，单一实现处比双端各写一份可靠。

use serde::{Deserialize, Serialize};
use winreg::enums::{REG_EXPAND_SZ, REG_SZ};

/// 保留变量：由专用通路拥有，通用通路必须完全排除。
///
/// `Path` 必须在此列 —— 否则用户可绕过 `PathEntry` / `disabled.json` /
/// `_pendingSys` / `_pendingUser` 与快照事务直接改 PATH。
const RESERVED_NAMES: &[&str] = &["Path"];

/// 保护变量：系统内置关键项，改坏会导致系统或登录异常。
///
/// `PSModulePath` 有意不在名单内 —— 用户有正当理由修改，且改坏不致命。
const PROTECTED_NAMES: &[&str] = &[
    "windir",
    "ComSpec",
    "PATHEXT",
    "OS",
    "PROCESSOR_ARCHITECTURE",
    "PROCESSOR_IDENTIFIER",
    "PROCESSOR_LEVEL",
    "PROCESSOR_REVISION",
    "TEMP",
    "TMP",
    "USERNAME",
    "USERPROFILE",
    "NUMBER_OF_PROCESSORS",
    "SystemRoot",
    "SystemDrive",
];

/// 敏感变量名关键词，忽略大小写匹配。
const SENSITIVE_KEYWORDS: &[&str] = &[
    "TOKEN",
    "KEY",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "CREDENTIAL",
    "API",
];

/// `preview` 的截断上限（字符数）。
const PREVIEW_MAX_CHARS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvValueKind {
    /// `REG_SZ` — 不做变量展开
    String,
    /// `REG_EXPAND_SZ` — 写入后由系统展开 `%VAR%`
    ExpandString,
    /// 其他类型（`REG_DWORD` / `REG_BINARY` / `REG_MULTI_SZ` 等），只读
    Unsupported,
}

impl EnvValueKind {
    pub fn from_reg_type(vtype: u32) -> Self {
        match vtype {
            REG_SZ => EnvValueKind::String,
            REG_EXPAND_SZ => EnvValueKind::ExpandString,
            _ => EnvValueKind::Unsupported,
        }
    }

    /// 是否可写。`Unsupported` 不可写。
    pub fn is_writable(self) -> bool {
        !matches!(self, EnvValueKind::Unsupported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvHive {
    System,
    User,
}

/// 列表元数据 —— 契约上**不含 `value` 字段**。
///
/// 这是核心安全边界：`list_all_env_vars` 的返回值会流经 Tauri IPC、
/// WebView 内存、Zustand store 与 React DevTools，因此命中敏感规则的
/// 变量其明文根本不进入前端。取明文必须显式调用 `reveal_env_var`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarMeta {
    /// 注册表中的原始名，保留大小写（system 的 `path`、user 的 `Path`）
    pub name: String,
    /// 真实注册表类型，来自 `get_raw_value().vtype`
    pub kind: EnvValueKind,
    pub hive: EnvHive,
    /// 编辑权限（保护名单 / hive 写能力 / 类型可写性 取交集）
    pub can_edit: bool,
    /// 删除权限（与 `can_edit` 分开保留，为未来扩展位）
    pub can_delete: bool,
    /// 是否为敏感变量
    pub sensitive: bool,
    /// 安全展示摘要；仅当类型可写且未命中敏感判定时非 `None`
    pub preview: Option<String>,
    /// 并发校验用：`name + vtype + value` 的稳定摘要
    pub revision: String,
}

/// 两个 hive 的变量元数据，来自同一次读取。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarSnapshot {
    #[serde(default)]
    pub system: Vec<EnvVarMeta>,
    #[serde(default)]
    pub user: Vec<EnvVarMeta>,
}

fn matches_any(name: &str, candidates: &[&str]) -> bool {
    candidates.iter().any(|c| name.eq_ignore_ascii_case(c))
}

/// 是否为保留变量（忽略大小写）。命中的变量必须从通用通路完全排除。
pub fn is_reserved(name: &str) -> bool {
    matches_any(name, RESERVED_NAMES)
}

/// 是否为保护变量（忽略大小写）。命中则不可编辑、不可删除。
pub fn is_protected(name: &str) -> bool {
    matches_any(name, PROTECTED_NAMES)
}

/// 是否为敏感变量（忽略大小写，基于名称关键词启发式）。
///
/// 注意：这是启发式而非保证。名称不含关键词的密钥变量会被判为非敏感。
/// 文档中已明确该限制 —— 本函数保证的是"命中者不进前端"。
pub fn is_sensitive(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SENSITIVE_KEYWORDS.iter().any(|kw| upper.contains(kw))
}

/// 计算并发校验摘要。任一组成部分变化都会改变结果。
///
/// 注意：这是变更检测摘要，不是密码学哈希。目的是发现"值被改动"，
/// 不用于防篡改。
pub fn revision_of(name: &str, vtype: u32, value: &str) -> String {
    format!("{}|{}|{}", name.to_ascii_lowercase(), vtype, value)
}

/// 净化并截断展示摘要。剔除控制字符；净化后为空则返回 `None`。
pub fn sanitize_preview(value: &str) -> Option<String> {
    let stripped: String = value
        .chars()
        .filter(|c| !matches!(c, '\r' | '\n' | '\0'))
        .collect();
    if stripped.is_empty() {
        return None;
    }
    if stripped.chars().count() > PREVIEW_MAX_CHARS {
        let mut truncated: String = stripped.chars().take(PREVIEW_MAX_CHARS).collect();
        truncated.push('…');
        return Some(truncated);
    }
    Some(stripped)
}

/// 计算某变量的编辑/删除权限。
///
/// 保护名单、`Unsupported` 类型都是硬拒绝；hive 写能力由 `writable` 传入
/// （调用方从 `crate::capabilities` 取，避免本模块依赖注册表探测）。
pub fn capabilities_for_with(
    writable: bool,
    name: &str,
    kind: EnvValueKind,
) -> (bool, bool) {
    if is_reserved(name) || is_protected(name) || !kind.is_writable() {
        return (false, false);
    }
    (writable, writable)
}

/// 按 hive 与当前进程权限计算 `(can_edit, can_delete)`。
pub fn capabilities_for(hive: EnvHive, name: &str, kind: EnvValueKind) -> (bool, bool) {
    let writable = match hive {
        EnvHive::System => crate::system::check_admin(),
        EnvHive::User => crate::registry::can_write_user(),
    };
    capabilities_for_with(writable, name, kind)
}
```

在 `core/src/lib.rs` 中注册模块并导出：

```rust
pub mod env_var;
```

并在现有 `pub use` 段追加：

```rust
pub use env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot};
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cargo test -p path-editor-core env_var 2>&1 | tail -20
```

Expected: PASS — 8 个测试全部通过

- [ ] **Step 5: 运行 clippy 与格式检查**

```bash
cargo fmt --check && cargo clippy -p path-editor-core --all-targets -- -D warnings
```

Expected: 无输出（通过）

- [ ] **Step 6: 提交**

```bash
git add core/src/env_var.rs core/src/lib.rs
git commit -m "feat(core): 新增通用环境变量契约与安全判定函数"
```

---

## Task 2: Rust core 变量读写函数

**Files:**

- Modify: `core/src/registry.rs`（在 `clean_paths` 之后、`#[cfg(test)]` 之前追加）
- Test: `core/src/registry.rs`（新增独立测试模块）

**Interfaces:**

- Consumes: Task 1 的 `env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot, is_reserved, is_protected, is_sensitive, revision_of, sanitize_preview, capabilities_for}`
- Produces:
  - `pub fn list_all_env_vars() -> Result<EnvVarSnapshot, String>`
  - `pub fn reveal_env_var(hive: EnvHive, name: &str) -> Result<String, String>`
  - `pub fn update_env_var(hive: EnvHive, name: &str, value: &str, expected_revision: &str) -> Result<(), String>`
  - `pub fn create_env_var(hive: EnvHive, name: &str, value: &str, kind: EnvValueKind) -> Result<(), String>`
  - `pub fn delete_env_var(hive: EnvHive, name: &str, expected_revision: &str) -> Result<(), String>`
  - `pub fn validate_env_name(name: &str) -> Result<(), String>`
  - `pub fn validate_env_value(value: &str, label: &str) -> Result<(), String>`

**关键实现要点（必须遵守）：**

1. **读写分离的内部辅助函数**：新增 `fn read_env_var(key: &RegKey, name: &str) -> Result<(u32, String), String>` 返回 `(vtype, value)`；`fn write_env_var(key: &RegKey, name: &str, value: &str, vtype: RegType) -> Result<(), String>` 用现有 `make_path_value` 的同款做法（`to_reg_value()` 后覆盖 `vtype`）。
2. **`list_all_env_vars` 必须复用同一把键句柄读两个 hive**，但两个 hive 是不同键，无法真正"同一个句柄"——语义要求是"同一次调用内完成"，即同一函数调用，中间不插入前端往返。实现为顺序读 HKCU 与 HKLM。
3. **过滤 `Path`**：枚举时对每个值名调 `is_reserved`，命中即跳过（不入结果）。
4. **`Unsupported` 变量的 `preview` 恒为 `None`**，不尝试读值。
5. **`update_env_var` 的 TOCTOU 顺序**必须严格照 Spec Part 1「实现约束 2」执行，且**不接收 `kind` 参数**。

- [ ] **Step 1: 写失败测试**

在 `core/src/registry.rs` 末尾追加独立测试模块：

```rust
#[cfg(test)]
mod env_var_tests {
    use super::*;
    use crate::env_var::{EnvHive, EnvValueKind};
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_EXPAND_SZ, REG_SZ};
    use winreg::types::FromRegValue;

    /// RAII 隔离测试键：Drop 时递归删除，绝不触碰真实环境变量键。
    struct TempRegistryKey {
        root: winreg::HKEY,
        path: String,
    }

    impl TempRegistryKey {
        fn new(label: &str) -> Self {
            let parent = "Software\\PathEditor\\EnvVarTests";
            let unique = format!(
                "{}\\{}-{}-{}",
                parent,
                label,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("系统时间早于 UNIX_EPOCH")
                    .as_nanos()
            );
            let root = RegKey::predef(HKEY_CURRENT_USER);
            root.create_subkey(parent).expect("创建隔离测试父键失败");
            root.create_subkey(&unique).expect("创建隔离测试键失败");
            TempRegistryKey { root: HKEY_CURRENT_USER, path: unique }
        }

        fn key(&self) -> RegKey {
            let root = RegKey::predef(self.root);
            root.open_subkey_with_flags(&self.path, KEY_READ | KEY_WRITE)
                .expect("打开隔离测试键失败")
        }
    }

    impl Drop for TempRegistryKey {
        fn drop(&mut self) {
            let root = RegKey::predef(self.root);
            let _ = root.delete_subkey_all(&self.path);
        }
    }

    fn seed(key: &RegKey, name: &str, value: &str, vtype: RegType) {
        let mut raw = value.to_reg_value();
        raw.vtype = vtype;
        key.set_raw_value(name, &raw).expect("写入种子值失败");
    }

    #[test]
    fn read_env_var_returns_real_type_and_value() {
        let temp = TempRegistryKey::new("read");
        let key = temp.key();
        seed(&key, "JAVA_HOME", "C:\\Java", REG_EXPAND_SZ);

        let (vtype, value) = read_env_var(&key, "JAVA_HOME").expect("读取失败");
        assert_eq!(vtype, REG_EXPAND_SZ);
        assert_eq!(value, "C:\\Java");
    }

    #[test]
    fn write_env_var_preserves_expand_sz_type() {
        let temp = TempRegistryKey::new("preserve-expand");
        let key = temp.key();
        seed(&key, "GOPATH", "C:\\Old", REG_EXPAND_SZ);

        write_env_var(&key, "GOPATH", "C:\\New", REG_EXPAND_SZ).expect("写入失败");

        let raw = key.get_raw_value("GOPATH").expect("读取失败");
        assert_eq!(raw.vtype, REG_EXPAND_SZ);
        assert_eq!(String::from_reg_value(&raw).unwrap(), "C:\\New");
    }

    #[test]
    fn write_env_var_preserves_sz_type() {
        let temp = TempRegistryKey::new("preserve-sz");
        let key = temp.key();
        seed(&key, "JAVA_HOME", "C:\\Old", REG_SZ);

        write_env_var(&key, "JAVA_HOME", "C:\\New", REG_SZ).expect("写入失败");

        let raw = key.get_raw_value("JAVA_HOME").expect("读取失败");
        assert_eq!(raw.vtype, REG_SZ);
    }

    #[test]
    fn validate_env_name_rejects_invalid() {
        assert!(validate_env_name("JAVA_HOME").is_ok());
        assert!(validate_env_name("").is_err());
        assert!(validate_env_name("BAD\0NAME").is_err());
        assert!(validate_env_name("BAD=NAME").is_err());
    }

    #[test]
    fn validate_env_value_rejects_null_and_oversize() {
        assert!(validate_env_value("C:\\Java", "测试").is_ok());
        assert!(validate_env_value("bad\0value", "测试").is_err());
        let oversized = "x".repeat(32768);
        assert!(validate_env_value(&oversized, "测试").is_err());
    }

    #[test]
    fn reserved_names_are_never_writable_through_generic_path() {
        // Path 走专用通路，通用通路的写入口必须拒绝
        assert!(is_reserved("Path"));
        assert!(is_reserved("path"));
        let temp = TempRegistryKey::new("reserved");
        let key = temp.key();
        seed(&key, "Path", "C:\\Windows", REG_EXPAND_SZ);
        // 值仍在，但通用通路不展示它
        assert!(read_env_var(&key, "Path").is_ok());
    }

    #[test]
    fn revision_detects_external_change() {
        let temp = TempRegistryKey::new("revision");
        let key = temp.key();
        seed(&key, "MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&key, "MY_VAR").expect("读取失败");
        let revision = crate::env_var::revision_of("MY_VAR", vtype, &value);

        // 模拟外部进程修改
        seed(&key, "MY_VAR", "changed-by-other-process", REG_SZ);

        let (vtype2, value2) = read_env_var(&key, "MY_VAR").expect("读取失败");
        let current = crate::env_var::revision_of("MY_VAR", vtype2, &value2);
        assert_ne!(revision, current, "外部修改后 revision 必须不同");
    }

    #[test]
    fn unsupported_type_is_not_writable() {
        let temp = TempRegistryKey::new("dword");
        let key = temp.key();
        // REG_DWORD 需要 4 字节小端数据
        let raw = RegValue {
            bytes: vec![1, 0, 0, 0],
            vtype: REG_DWORD,
        };
        key.set_raw_value("MY_DWORD", &raw).expect("写入 DWORD 失败");

        let (vtype, _) = read_env_var(&key, "MY_DWORD").expect("读取失败");
        let kind = EnvValueKind::from_reg_type(vtype);
        assert_eq!(kind, EnvValueKind::Unsupported);
        assert!(!kind.is_writable());
    }

    #[test]
    fn hive_enum_serializes_camel_case() {
        let value = serde_json::to_value(EnvHive::System).expect("序列化失败");
        assert_eq!(value, serde_json::json!("system"));
        let value = serde_json::to_value(EnvHive::User).expect("序列化失败");
        assert_eq!(value, serde_json::json!("user"));
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cargo test -p path-editor-core env_var_tests 2>&1 | head -30
```

Expected: FAIL — `error[E0425]: cannot find function read_env_var` / `write_env_var` / `validate_env_name` / `validate_env_value`

- [ ] **Step 3: 写实现**

在 `core/src/registry.rs` 的 `clean_paths` 之后追加：

```rust
// ── 通用环境变量读写 ──

use crate::env_var::{
    capabilities_for, is_reserved, is_sensitive, revision_of, sanitize_preview, EnvHive,
    EnvValueKind, EnvVarMeta, EnvVarSnapshot,
};

fn env_key(root: winreg::HKEY, sub_path: &str, label: &str, write: bool) -> Result<RegKey, String> {
    let flags = if write { KEY_READ | KEY_WRITE } else { KEY_READ };
    let key = RegKey::predef(root);
    key.open_subkey_with_flags(sub_path, flags)
        .map_err(|e| format!("无法打开{}环境变量注册表项: {}", label, e))
}

fn hive_location(hive: EnvHive) -> (winreg::HKEY, &'static str, &'static str) {
    match hive {
        EnvHive::System => (HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统"),
        EnvHive::User => (HKEY_CURRENT_USER, USER_REG_PATH, "用户"),
    }
}

/// 读取单个值，返回 (vtype, value)。仅用于字符串类型。
fn read_env_var(key: &RegKey, name: &str) -> Result<(u32, String), String> {
    let raw = key
        .get_raw_value(name)
        .map_err(|e| format!("无法读取环境变量 {}: {}", name, e))?;
    let value = String::from_reg_value(&raw)
        .map_err(|e| format!("无法解码环境变量 {}: {}", name, e))?;
    Ok((raw.vtype, value))
}

/// 写入单个值，保持调用方给定的注册表类型。
fn write_env_var(key: &RegKey, name: &str, value: &str, vtype: RegType) -> Result<(), String> {
    let mut raw = value.to_reg_value();
    raw.vtype = vtype;
    key.set_raw_value(name, &raw)
        .map_err(|e| format!("无法写入环境变量 {}: {}", name, e))
}

/// 通用环境变量名校验。
pub fn validate_env_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("变量名不能为空".into());
    }
    if name.contains('\0') {
        return Err("变量名不能包含 null 字节".into());
    }
    if name.contains('=') {
        return Err("变量名不能包含等号".into());
    }
    if name.encode_utf16().count() > 32767 {
        return Err("变量名过长（超过 32767 字符）".into());
    }
    Ok(())
}

/// 通用环境变量值校验。不复用 `validate_and_join_paths`（那是 PATH 分号语义专用）。
pub fn validate_env_value(value: &str, label: &str) -> Result<(), String> {
    if value.contains('\0') {
        return Err(format!("{} 的值包含 null 字节", label));
    }
    let utf16_len = value.encode_utf16().count();
    if utf16_len > 32767 {
        return Err(format!(
            "{} 的值长度 {} 超出 Windows 限制 32767 字符",
            label, utf16_len
        ));
    }
    Ok(())
}

/// 读取单个 hive 的所有环境变量元数据。`Path` 在此被过滤。
fn list_hive_env_vars(hive: EnvHive) -> Result<Vec<EnvVarMeta>, String> {
    let (root, sub_path, label) = hive_location(hive);
    let key = env_key(root, sub_path, label, false)?;
    let mut metas = Vec::new();

    for name in key.enum_values().flatten().map(|(n, _)| n) {
        // 保留变量（Path）由专用通路拥有，通用通路完全不展示
        if is_reserved(&name) {
            continue;
        }

        let raw = match key.get_raw_value(&name) {
            Ok(raw) => raw,
            Err(e) => {
                log::warn!("跳过无法读取的环境变量 {}: {}", name, e);
                continue;
            }
        };
        let kind = EnvValueKind::from_reg_type(raw.vtype);
        let sensitive = is_sensitive(&name);

        let value = match kind {
            EnvValueKind::Unsupported => String::new(),
            _ => match String::from_reg_value(&raw) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("跳过无法解码的环境变量 {}: {}", name, e);
                    continue;
                }
            },
        };

        let preview = if sensitive || !kind.is_writable() {
            None
        } else {
            sanitize_preview(&value)
        };

        let (can_edit, can_delete) = capabilities_for(hive, &name, kind);

        metas.push(EnvVarMeta {
            revision: revision_of(&name, raw.vtype, &value),
            name,
            kind,
            hive,
            can_edit,
            can_delete,
            sensitive,
            preview,
        });
    }

    Ok(metas)
}

/// 一次读取两个 hive 的变量元数据（列表唯一入口）。
///
/// 单次调用内读两个 hive，保证快照一致 —— 若分两次调用，两次读取之间
/// 注册表可能变化，合并视图会出现 hive 来自不同时刻的不一致。
pub fn list_all_env_vars() -> Result<EnvVarSnapshot, String> {
    Ok(EnvVarSnapshot {
        system: list_hive_env_vars(EnvHive::System)?,
        user: list_hive_env_vars(EnvHive::User)?,
    })
}

/// 按需读取单个变量的明文（命中敏感规则的变量的唯一取值入口）。
///
/// `Unsupported` 类型返回 `Err`，不尝试转字符串。
pub fn reveal_env_var(hive: EnvHive, name: &str) -> Result<String, String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name));
    }
    let (root, sub_path, label) = hive_location(hive);
    let key = env_key(root, sub_path, label, false)?;
    let (vtype, value) = read_env_var(&key, name)?;
    match EnvValueKind::from_reg_type(vtype) {
        EnvValueKind::String | EnvValueKind::ExpandString => Ok(value),
        EnvValueKind::Unsupported => Err(format!(
            "{} 的注册表类型不受支持，无法读取其值",
            name
        )),
    }
}

/// 写入已有变量。类型从注册表读取，不由前端决定。
///
/// 在同一调用内完成「读 → 算 revision → 比对 → 校验 → 写」，消除 TOCTOU。
pub fn update_env_var(
    hive: EnvHive,
    name: &str,
    value: &str,
    expected_revision: &str,
) -> Result<(), String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name));
    }
    if is_protected(name) {
        return Err(format!("{} 是系统内置变量，不允许修改", name));
    }

    let (root, sub_path, label) = hive_location(hive);
    // 打开后不释放句柄，直到写入完成
    let key = env_key(root, sub_path, label, true)?;

    let (vtype, current) = read_env_var(&key, name)?;

    // 步骤 3：并发校验
    let current_revision = revision_of(name, vtype, &current);
    if current_revision != expected_revision {
        return Err("变量已被其他进程修改，请重新加载".into());
    }

    // 步骤 4：类型与值校验
    let kind = EnvValueKind::from_reg_type(vtype);
    if !kind.is_writable() {
        return Err(format!(
            "{} 的注册表类型不受支持，无法修改（仅可查看）",
            name
        ));
    }
    validate_env_value(value, name)?;

    // 步骤 5：写入，类型原样保留
    write_env_var(&key, name, value, vtype)?;
    crate::system::broadcast_env_change();
    Ok(())
}

/// 新建变量。`kind` 仅在此决定。
///
/// 在 Rust 内原子检查名称不存在 —— 不覆盖已有变量。
pub fn create_env_var(
    hive: EnvHive,
    name: &str,
    value: &str,
    kind: EnvValueKind,
) -> Result<(), String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!("{} 由专用 PATH 通路管理，无法通过通用通路创建", name));
    }
    if is_protected(name) {
        return Err(format!("{} 是系统内置变量，不允许覆盖", name));
    }
    if !kind.is_writable() {
        return Err("新建变量只支持 String 或 ExpandString 类型".into());
    }
    validate_env_value(value, name)?;

    let (root, sub_path, label) = hive_location(hive);
    let key = env_key(root, sub_path, label, true)?;

    // 原子检查：忽略大小写地确认该名不存在
    let existing = key
        .enum_values()
        .flatten()
        .any(|(n, _)| n.eq_ignore_ascii_case(name));
    if existing {
        return Err(format!("变量 {} 已存在，请使用编辑功能", name));
    }

    let vtype = match kind {
        EnvValueKind::String => REG_SZ,
        _ => REG_EXPAND_SZ,
    };
    write_env_var(&key, name, value, vtype)?;
    crate::system::broadcast_env_change();
    Ok(())
}

/// 删除变量。在同一调用内完成 revision 比对与删除。
pub fn delete_env_var(
    hive: EnvHive,
    name: &str,
    expected_revision: &str,
) -> Result<(), String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name));
    }
    if is_protected(name) {
        return Err(format!("{} 是系统内置变量，不允许删除", name));
    }

    let (root, sub_path, label) = hive_location(hive);
    let key = env_key(root, sub_path, label, true)?;

    let (vtype, current) = read_env_var(&key, name)?;
    let current_revision = revision_of(name, vtype, &current);
    if current_revision != expected_revision {
        return Err("变量已被其他进程修改，请重新加载".into());
    }

    if EnvValueKind::from_reg_type(vtype) == EnvValueKind::Unsupported {
        return Err(format!("{} 的注册表类型不受支持，无法删除", name));
    }

    key.delete_value(name)
        .map_err(|e| format!("无法删除环境变量 {}: {}", name, e))?;
    crate::system::broadcast_env_change();
    Ok(())
}
```

补齐 `registry.rs` 顶部的 `use`：

```rust
use crate::env_var::{is_protected, is_reserved, revision_of, sanitize_preview};
use winreg::types::FromRegValue;
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cargo test -p path-editor-core 2>&1 | tail -25
```

Expected: PASS — 新测试与全部既有测试通过

- [ ] **Step 5: 运行 clippy 与格式检查**

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 通过

- [ ] **Step 6: 提交**

```bash
git add core/src/registry.rs
git commit -m "feat(core): 新增通用环境变量读写函数（含 revision 并发校验）"
```

---

## Task 3: GUI command 层

**Files:**

- Create: `gui/src/commands/env_var.rs`
- Modify: `gui/src/commands/mod.rs`、`gui/src/lib.rs`

**Interfaces:**

- Consumes: Task 2 的 5 个 registry 函数
- Produces: 5 个 Tauri command，前端以 camelCase 调用：
  - `list_all_env_vars() -> Result<EnvVarSnapshot, String>`
  - `reveal_env_var(hive: EnvHive, name: String) -> Result<String, String>`
  - `update_env_var(hive, name, value, expectedRevision) -> Result<(), String>`
  - `create_env_var(hive, name, value, kind) -> Result<(), String>`
  - `delete_env_var(hive, name, expectedRevision) -> Result<(), String>`

- [ ] **Step 1: 写 command 实现**

创建 `gui/src/commands/env_var.rs`：

```rust
use path_editor_core::env_var::{EnvHive, EnvValueKind, EnvVarSnapshot};
use path_editor_core::registry;

#[tauri::command]
pub fn list_all_env_vars() -> Result<EnvVarSnapshot, String> {
    registry::list_all_env_vars()
}

#[tauri::command]
pub fn reveal_env_var(hive: EnvHive, name: String) -> Result<String, String> {
    registry::reveal_env_var(hive, &name)
}

#[tauri::command]
pub fn update_env_var(
    hive: EnvHive,
    name: String,
    value: String,
    expected_revision: String,
) -> Result<(), String> {
    registry::update_env_var(hive, &name, &value, &expected_revision)
}

#[tauri::command]
pub fn create_env_var(
    hive: EnvHive,
    name: String,
    value: String,
    kind: EnvValueKind,
) -> Result<(), String> {
    registry::create_env_var(hive, &name, &value, kind)
}

#[tauri::command]
pub fn delete_env_var(
    hive: EnvHive,
    name: String,
    expected_revision: String,
) -> Result<(), String> {
    registry::delete_env_var(hive, &name, &expected_revision)
}
```

在 `gui/src/commands/mod.rs` 追加：

```rust
pub mod env_var;
```

在 `gui/src/lib.rs` 的 `invoke_handler!` 列表中追加：

```rust
commands::env_var::list_all_env_vars,
commands::env_var::reveal_env_var,
commands::env_var::update_env_var,
commands::env_var::create_env_var,
commands::env_var::delete_env_var,
```

- [ ] **Step 2: 编译验证**

```bash
cargo check --workspace 2>&1 | tail -20
```

Expected: 编译通过，无 warning

- [ ] **Step 3: 运行 clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 通过

- [ ] **Step 4: 提交**

```bash
git add gui/src/commands/env_var.rs gui/src/commands/mod.rs gui/src/lib.rs
git commit -m "feat(gui): 注册通用环境变量读写 command"
```

---

## Task 4: 前端纯逻辑与 IPC 边界

**Files:**

- Create: `src/core/env-var.ts`
- Modify: `src/services/backend.ts`
- Test: `tests/unit/env-var.test.ts`

**Interfaces:**

- Consumes: Task 3 的 5 个 command
- Produces:
  - `export interface EnvVarMeta { name: string; kind: EnvValueKind; hive: EnvHive; canEdit: boolean; canDelete: boolean; sensitive: boolean; preview: string | null; revision: string }`
  - `export interface EnvVarSnapshot { system: EnvVarMeta[]; user: EnvVarMeta[] }`
  - `export type EnvValueKind = 'string' | 'expandString' | 'unsupported'`
  - `export type EnvHive = 'system' | 'user'`
  - `export function envVarKey(meta: EnvVarMeta): string`
  - `export function maskValue(): string`
  - `export function validateVarName(name: string): string | null`
  - `export function displayValue(meta: EnvVarMeta, revealedValue: string | null): string`
  - `export function filterEnvVars(snapshot, filter, query): EnvVarMeta[]`
  - `backend.listAllEnvVars()` / `revealEnvVar()` / `updateEnvVar()` / `createEnvVar()` / `deleteEnvVar()`

- [ ] **Step 1: 写失败测试**

创建 `tests/unit/env-var.test.ts`：

```typescript
import { describe, it, expect } from 'vitest';
import {
  displayValue,
  envVarKey,
  filterEnvVars,
  maskValue,
  validateVarName,
  type EnvVarMeta,
  type EnvVarSnapshot,
} from '@/core/env-var';

function meta(overrides: Partial<EnvVarMeta> = {}): EnvVarMeta {
  return {
    name: 'JAVA_HOME',
    kind: 'string',
    hive: 'user',
    canEdit: true,
    canDelete: true,
    sensitive: false,
    preview: 'C:\\Java',
    revision: 'rev-1',
    ...overrides,
  };
}

describe('envVarKey', () => {
  it('按 hive 与名称组合，区分同名跨 hive 变量', () => {
    expect(envVarKey(meta({ hive: 'user', name: 'PSModulePath' }))).toBe('user:PSModulePath');
    expect(envVarKey(meta({ hive: 'system', name: 'PSModulePath' }))).toBe('system:PSModulePath');
  });
});

describe('maskValue', () => {
  it('返回固定占位符，不泄露真实长度', () => {
    expect(maskValue()).toBe('••••••••');
    expect(maskValue()).toHaveLength(8);
  });
});

describe('validateVarName', () => {
  it('接受常规变量名', () => {
    expect(validateVarName('JAVA_HOME')).toBeNull();
    expect(validateVarName('Path')).toBeNull();
  });

  it('拒绝空名与仅空白', () => {
    expect(validateVarName('')).not.toBeNull();
    expect(validateVarName('   ')).not.toBeNull();
  });

  it('拒绝含等号的名字', () => {
    expect(validateVarName('BAD=NAME')).not.toBeNull();
  });

  it('拒绝含 null 字节的名字', () => {
    expect(validateVarName('BAD\0NAME')).not.toBeNull();
  });
});

describe('displayValue', () => {
  it('敏感且未 reveal 时返回占位符', () => {
    expect(displayValue(meta({ sensitive: true, preview: null }), null)).toBe('••••••••');
  });

  it('敏感但已 reveal 时返回明文', () => {
    expect(displayValue(meta({ sensitive: true, preview: null }), 'real-secret')).toBe(
      'real-secret',
    );
  });

  it('不敏感时返回 preview', () => {
    expect(displayValue(meta(), null)).toBe('C:\\Java');
  });

  it('不敏感但 preview 为 null 时返回空串', () => {
    expect(displayValue(meta({ preview: null }), null)).toBe('');
  });

  it('Unsupported 类型显示类型占位，不显示值', () => {
    expect(displayValue(meta({ kind: 'unsupported', preview: null }), null)).toBe(
      '(不支持的注册表类型)',
    );
  });
});

describe('filterEnvVars', () => {
  const snapshot: EnvVarSnapshot = {
    system: [
      meta({ name: 'windir', hive: 'system' }),
      meta({ name: 'MY_KEY', hive: 'system', sensitive: true, preview: null }),
    ],
    user: [meta({ name: 'JAVA_HOME', hive: 'user' })],
  };

  it('filter=all 返回两个 hive', () => {
    expect(filterEnvVars(snapshot, 'all', '')).toHaveLength(3);
  });

  it('filter=system 只返回系统变量', () => {
    const result = filterEnvVars(snapshot, 'system', '');
    expect(result).toHaveLength(2);
    expect(result.every((m) => m.hive === 'system')).toBe(true);
  });

  it('filter=user 只返回用户变量', () => {
    const result = filterEnvVars(snapshot, 'user', '');
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe('JAVA_HOME');
  });

  it('搜索忽略大小写且只匹配变量名', () => {
    expect(filterEnvVars(snapshot, 'all', 'java')).toHaveLength(1);
    expect(filterEnvVars(snapshot, 'all', 'WINDIR')).toHaveLength(1);
  });

  it('搜索不匹配值内容', () => {
    // preview 为 C:\Java，但按 "C:\\" 搜索不应命中
    expect(filterEnvVars(snapshot, 'all', 'C:\\')).toHaveLength(0);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

```bash
npx vitest run tests/unit/env-var.test.ts 2>&1 | tail -20
```

Expected: FAIL — `Failed to resolve import "@/core/env-var"`

- [ ] **Step 3: 写实现**

创建 `src/core/env-var.ts`：

```typescript
/**
 * 通用环境变量的纯展示逻辑。
 *
 * 安全判定（保留 / 保护 / 敏感 / 权限）全部在 Rust 侧计算并下发，
 * 本模块不重复实现 —— 避免双端各写一份判定规则而产生分歧。
 */

export type EnvValueKind = 'string' | 'expandString' | 'unsupported';
export type EnvHive = 'system' | 'user';
export type HiveFilter = 'system' | 'user' | 'all';

export interface EnvVarMeta {
  name: string;
  kind: EnvValueKind;
  hive: EnvHive;
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

const MASK_PLACEHOLDER = '••••••••';
const UNSUPPORTED_PLACEHOLDER = '(不支持的注册表类型)';

/** 唯一键：同名变量可能在两个 hive 同时存在。 */
export function envVarKey(meta: EnvVarMeta): string {
  return `${meta.hive}:${meta.name}`;
}

/**
 * 敏感值的固定占位符。
 *
 * 刻意不接受真实值作参数 —— 确保明文不会经过本函数（也就不会进入
 * 调用栈、日志或调试器）。
 */
export function maskValue(): string {
  return MASK_PLACEHOLDER;
}

/**
 * 前端预校验变量名，与 Rust `validate_env_name` 规则保持一致。
 * 返回错误文案，或 null 表示通过。
 */
export function validateVarName(name: string): string | null {
  if (name.trim().length === 0) return '变量名不能为空';
  if (name.includes('\0')) return '变量名不能包含 null 字节';
  if (name.includes('=')) return '变量名不能包含等号';
  return null;
}

function hiveOf(meta: EnvVarMeta): string {
  return meta.hive === 'system' ? 'system' : 'user';
}

/** 计算某变量在表格中的展示值。 */
export function displayValue(meta: EnvVarMeta, revealedValue: string | null): string {
  if (meta.kind === 'unsupported') return UNSUPPORTED_PLACEHOLDER;
  if (meta.sensitive) {
    return revealedValue === null ? MASK_PLACEHOLDER : revealedValue;
  }
  return meta.preview ?? '';
}

/**
 * 按来源筛选与关键字过滤变量列表。
 *
 * 搜索**只匹配变量名，绝不匹配值** —— 匹配值等于把密钥拿去比较，
 * 且命中与否本身就会泄露信息。
 */
export function filterEnvVars(
  snapshot: EnvVarSnapshot,
  filter: HiveFilter,
  query: string,
): EnvVarMeta[] {
  const source: EnvVarMeta[] =
    filter === 'system'
      ? snapshot.system
      : filter === 'user'
        ? snapshot.user
        : [...snapshot.system, ...snapshot.user];

  const trimmed = query.trim().toLowerCase();
  if (trimmed.length === 0) return source;

  return source.filter((meta) => meta.name.toLowerCase().includes(trimmed));
}

/** 筛选值是否属于已知范围（用于运行时校验）。 */
export function isHiveFilter(value: unknown): value is HiveFilter {
  return value === 'system' || value === 'user' || value === 'all';
}

export { hiveOf };
```

在 `src/services/backend.ts` 中追加导入与校验函数：

```typescript
import type { EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot } from '@/core/env-var';

const ENV_VALUE_KINDS: readonly EnvValueKind[] = ['string', 'expandString', 'unsupported'];
const ENV_HIVES: readonly EnvHive[] = ['system', 'user'];

function isEnvVarMeta(value: unknown): value is EnvVarMeta {
  if (!isRecord(value)) return false;
  return (
    typeof value.name === 'string' &&
    ENV_VALUE_KINDS.includes(value.kind as EnvValueKind) &&
    ENV_HIVES.includes(value.hive as EnvHive) &&
    typeof value.canEdit === 'boolean' &&
    typeof value.canDelete === 'boolean' &&
    typeof value.sensitive === 'boolean' &&
    (value.preview === null || typeof value.preview === 'string') &&
    typeof value.revision === 'string'
  );
}

function parseEnvVarMetas(value: unknown, label: string): EnvVarMeta[] {
  if (!Array.isArray(value) || !value.every(isEnvVarMeta)) {
    throw new Error(`${label} 返回了无效的 EnvVarMeta[] 契约`);
  }
  return value;
}

function parseEnvVarSnapshot(value: unknown): EnvVarSnapshot {
  if (!isRecord(value)) {
    throw new Error('list_all_env_vars 返回了无效的 EnvVarSnapshot 契约');
  }
  return {
    system: parseEnvVarMetas(value.system, 'list_all_env_vars.system'),
    user: parseEnvVarMetas(value.user, 'list_all_env_vars.user'),
  };
}
```

在 `backend` 对象中追加 5 个方法：

```typescript
  listAllEnvVars: async () => parseEnvVarSnapshot(await invoke<unknown>('list_all_env_vars')),
  revealEnvVar: (hive: EnvHive, name: string) =>
    invoke<string>('reveal_env_var', { hive, name }),
  updateEnvVar: (hive: EnvHive, name: string, value: string, expectedRevision: string) =>
    invoke<void>('update_env_var', { hive, name, value, expectedRevision }),
  createEnvVar: (hive: EnvHive, name: string, value: string, kind: EnvValueKind) =>
    invoke<void>('create_env_var', { hive, name, value, kind }),
  deleteEnvVar: (hive: EnvHive, name: string, expectedRevision: string) =>
    invoke<void>('delete_env_var', { hive, name, expectedRevision }),
```

- [ ] **Step 4: 运行测试确认通过**

```bash
npx vitest run tests/unit/env-var.test.ts 2>&1 | tail -20
```

Expected: PASS — 全部用例通过

- [ ] **Step 5: 类型检查**

```bash
npx tsc -b 2>&1 | tail -20
```

Expected: 无错误

- [ ] **Step 6: 提交**

```bash
git add src/core/env-var.ts src/services/backend.ts tests/unit/env-var.test.ts
git commit -m "feat(frontend): 新增环境变量纯逻辑与 IPC 边界契约"
```

---

## Task 5: 前端状态层

**Files:**

- Create: `src/store/env-store.ts`
- Test: `tests/unit/env-store.test.ts`

**Interfaces:**

- Consumes: Task 4 的 `backend.*` 方法与 `envVarKey` / `validateVarName`
- Produces: `export const useEnvStore`，字段与 action 见 Spec Part 2「状态层」

**关键约束：**

- `revealed` 是 `Map`，`load()` 与 `hide()` 都清除它 —— 刷新即恢复打码。
- **不做前端并发校验**。保存时原样回传 `meta.revision`；收到含「已被其他进程修改」的错误时提示并 `load()`。
- 全部 action 在失败时写入 `statusMessage`，不抛出未捕获异常。

- [ ] **Step 1: 写失败测试**

创建 `tests/unit/env-store.test.ts`：

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockBackend = {
  listAllEnvVars: vi.fn(),
  revealEnvVar: vi.fn(),
  updateEnvVar: vi.fn(),
  createEnvVar: vi.fn(),
  deleteEnvVar: vi.fn(),
};

vi.mock('@/services/backend', () => ({ backend: mockBackend }));

import { useEnvStore } from '@/store/env-store';
import type { EnvVarMeta } from '@/core/env-var';

function meta(overrides: Partial<EnvVarMeta> = {}): EnvVarMeta {
  return {
    name: 'JAVA_HOME',
    kind: 'string',
    hive: 'user',
    canEdit: true,
    canDelete: true,
    sensitive: false,
    preview: 'C:\\Java',
    revision: 'rev-1',
    ...overrides,
  };
}

const snapshot = {
  system: [meta({ name: 'windir', hive: 'system' })],
  user: [
    meta({ name: 'JAVA_HOME', hive: 'user' }),
    meta({ name: 'MY_TOKEN', hive: 'user', sensitive: true, preview: null }),
  ],
};

beforeEach(() => {
  vi.clearAllMocks();
  useEnvStore.setState({
    snapshot: null,
    revealed: new Map(),
    draft: new Map(),
    hiveFilter: 'all',
    isLoading: false,
    isSaving: false,
    statusMessage: '',
  });
});

describe('load', () => {
  it('载入两个 hive 的快照并清除全部明文', async () => {
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    useEnvStore.setState({ revealed: new Map([['user:MY_TOKEN', 'leaked']]) });

    await useEnvStore.getState().load();

    const state = useEnvStore.getState();
    expect(state.snapshot).toEqual(snapshot);
    expect(state.revealed.size).toBe(0);
    expect(state.isLoading).toBe(false);
  });

  it('载入失败时写 statusMessage 且不抛异常', async () => {
    mockBackend.listAllEnvVars.mockRejectedValue(new Error('boom'));

    await expect(useEnvStore.getState().load()).resolves.toBeUndefined();

    expect(useEnvStore.getState().statusMessage).toContain('boom');
    expect(useEnvStore.getState().isLoading).toBe(false);
  });
});

describe('reveal / hide', () => {
  it('reveal 存入明文，hide 清除', async () => {
    mockBackend.revealEnvVar.mockResolvedValue('real-secret');
    const target = meta({ name: 'MY_TOKEN', sensitive: true, preview: null });

    await useEnvStore.getState().reveal(target);
    expect(useEnvStore.getState().revealed.get('user:MY_TOKEN')).toBe('real-secret');
    expect(mockBackend.revealEnvVar).toHaveBeenCalledWith('user', 'MY_TOKEN');

    useEnvStore.getState().hide(target);
    expect(useEnvStore.getState().revealed.has('user:MY_TOKEN')).toBe(false);
  });

  it('reveal 失败时触发刷新并清除明文', async () => {
    mockBackend.revealEnvVar.mockRejectedValue(new Error('类型不受支持'));
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    const target = meta({ name: 'MY_TOKEN', sensitive: true, preview: null });

    await useEnvStore.getState().reveal(target);

    expect(useEnvStore.getState().revealed.has('user:MY_TOKEN')).toBe(false);
    expect(mockBackend.listAllEnvVars).toHaveBeenCalled();
  });
});

describe('save', () => {
  it('携带 meta.revision 调用 updateEnvVar', async () => {
    mockBackend.updateEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    const target = meta();
    useEnvStore.getState().setDraft(target, 'C:\\NewJava');

    await useEnvStore.getState().save(target);

    expect(mockBackend.updateEnvVar).toHaveBeenCalledWith(
      'user',
      'JAVA_HOME',
      'C:\\NewJava',
      'rev-1',
    );
    expect(useEnvStore.getState().draft.has('user:JAVA_HOME')).toBe(false);
  });

  it('revision 冲突时不重试、提示并刷新', async () => {
    mockBackend.updateEnvVar.mockRejectedValue(new Error('变量已被其他进程修改，请重新加载'));
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    const target = meta();
    useEnvStore.getState().setDraft(target, 'C:\\NewJava');

    await useEnvStore.getState().save(target);

    expect(mockBackend.updateEnvVar).toHaveBeenCalledTimes(1);
    expect(useEnvStore.getState().statusMessage).toContain('已被其他进程修改');
    expect(mockBackend.listAllEnvVars).toHaveBeenCalled();
    // 草稿保留，避免用户输入丢失
    expect(useEnvStore.getState().draft.has('user:JAVA_HOME')).toBe(true);
  });
});

describe('create / remove', () => {
  it('create 校验名称后调用 createEnvVar', async () => {
    mockBackend.createEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);

    await useEnvStore.getState().create('user', 'NEW_VAR', 'value', 'string');

    expect(mockBackend.createEnvVar).toHaveBeenCalledWith('user', 'NEW_VAR', 'value', 'string');
  });

  it('create 名称非法时直接拒绝，不调用 IPC', async () => {
    await useEnvStore.getState().create('user', 'BAD=NAME', 'value', 'string');

    expect(mockBackend.createEnvVar).not.toHaveBeenCalled();
    expect(useEnvStore.getState().statusMessage).not.toBe('');
  });

  it('remove 携带 meta.revision 调用 deleteEnvVar', async () => {
    mockBackend.deleteEnvVar.mockResolvedValue(undefined);
    mockBackend.listAllEnvVars.mockResolvedValue(snapshot);
    const target = meta();

    await useEnvStore.getState().remove(target);

    expect(mockBackend.deleteEnvVar).toHaveBeenCalledWith('user', 'JAVA_HOME', 'rev-1');
  });
});

describe('setHiveFilter', () => {
  it('切换筛选不触发 IPC', () => {
    useEnvStore.getState().setHiveFilter('system');

    expect(useEnvStore.getState().hiveFilter).toBe('system');
    expect(mockBackend.listAllEnvVars).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

```bash
npx vitest run tests/unit/env-store.test.ts 2>&1 | tail -20
```

Expected: FAIL — `Failed to resolve import "@/store/env-store"`

- [ ] **Step 3: 写实现**

创建 `src/store/env-store.ts`：

```typescript
import { create } from 'zustand';
import i18n from '@/i18n';
import {
  envVarKey,
  validateVarName,
  type EnvHive,
  type EnvValueKind,
  type EnvVarMeta,
  type EnvVarSnapshot,
  type HiveFilter,
} from '@/core/env-var';
import { backend } from '@/services/backend';

/** 冲突错误的稳定特征，用于决定是否刷新。 */
const CONFLICT_MARKER = '已被其他进程修改';

interface EnvState {
  snapshot: EnvVarSnapshot | null;
  revealed: Map<string, string>;
  draft: Map<string, string>;
  hiveFilter: HiveFilter;
  isLoading: boolean;
  isSaving: boolean;
  statusMessage: string;

  load: () => Promise<void>;
  setHiveFilter: (filter: HiveFilter) => void;
  setDraft: (meta: EnvVarMeta, value: string) => void;
  clearDraft: (meta: EnvVarMeta) => void;
  save: (meta: EnvVarMeta) => Promise<void>;
  create: (hive: EnvHive, name: string, value: string, kind: EnvValueKind) => Promise<void>;
  remove: (meta: EnvVarMeta) => Promise<void>;
  reveal: (meta: EnvVarMeta) => Promise<void>;
  hide: (meta: EnvVarMeta) => void;
  hasDrafts: () => boolean;
  setStatusMessage: (message: string) => void;
}

export const useEnvStore = create<EnvState>((set, get) => {
  /** 冲突或失败后统一刷新，并保留草稿避免用户输入丢失。 */
  const refreshAfterError = async (message: string) => {
    set({ statusMessage: message, isSaving: false });
    await get().load();
  };

  return {
    snapshot: null,
    revealed: new Map(),
    draft: new Map(),
    hiveFilter: 'all',
    isLoading: false,
    isSaving: false,
    statusMessage: '',

    load: async () => {
      set({ isLoading: true });
      try {
        const snapshot = await backend.listAllEnvVars();
        // 刷新即恢复打码：明文不跨次加载存活
        set({ snapshot, revealed: new Map(), isLoading: false });
      } catch (error) {
        set({
          isLoading: false,
          statusMessage: `${i18n.t('status.error')}: ${String(error)}`,
        });
      }
    },

    setHiveFilter: (filter) => set({ hiveFilter: filter }),

    setDraft: (meta, value) => {
      const draft = new Map(get().draft);
      draft.set(envVarKey(meta), value);
      set({ draft });
    },

    clearDraft: (meta) => {
      const draft = new Map(get().draft);
      draft.delete(envVarKey(meta));
      set({ draft });
    },

    save: async (meta) => {
      const value = get().draft.get(envVarKey(meta));
      if (value === undefined) return;
      set({ isSaving: true });
      try {
        await backend.updateEnvVar(meta.hive, meta.name, value, meta.revision);
        const draft = new Map(get().draft);
        draft.delete(envVarKey(meta));
        set({ draft, isSaving: false, statusMessage: i18n.t('status.saved') });
        await get().load();
      } catch (error) {
        const message = String(error);
        if (message.includes(CONFLICT_MARKER)) {
          // 不做前端重试或比对：Rust 已拒绝，只需提示并刷新
          await refreshAfterError(message);
        } else {
          set({ isSaving: false, statusMessage: `${i18n.t('status.error')}: ${message}` });
        }
      }
    },

    create: async (hive, name, value, kind) => {
      const invalid = validateVarName(name);
      if (invalid !== null) {
        set({ statusMessage: invalid });
        return;
      }
      set({ isSaving: true });
      try {
        await backend.createEnvVar(hive, name, value, kind);
        set({ isSaving: false, statusMessage: i18n.t('status.saved') });
        await get().load();
      } catch (error) {
        set({ isSaving: false, statusMessage: `${i18n.t('status.error')}: ${String(error)}` });
      }
    },

    remove: async (meta) => {
      set({ isSaving: true });
      try {
        await backend.deleteEnvVar(meta.hive, meta.name, meta.revision);
        set({ isSaving: false, statusMessage: i18n.t('status.saved') });
        await get().load();
      } catch (error) {
        const message = String(error);
        if (message.includes(CONFLICT_MARKER)) {
          await refreshAfterError(message);
        } else {
          set({ isSaving: false, statusMessage: `${i18n.t('status.error')}: ${message}` });
        }
      }
    },

    reveal: async (meta) => {
      try {
        const value = await backend.revealEnvVar(meta.hive, meta.name);
        const revealed = new Map(get().revealed);
        revealed.set(envVarKey(meta), value);
        set({ revealed });
      } catch (error) {
        const revealed = new Map(get().revealed);
        revealed.delete(envVarKey(meta));
        set({ revealed, statusMessage: `${i18n.t('status.error')}: ${String(error)}` });
        // 值可能已不存在或类型不受支持，刷新以同步真实状态
        await get().load();
      }
    },

    hide: (meta) => {
      const revealed = new Map(get().revealed);
      revealed.delete(envVarKey(meta));
      set({ revealed });
    },

    hasDrafts: () => get().draft.size > 0,

    setStatusMessage: (message) => set({ statusMessage: message }),
  };
});
```

请以该版本为准。

- [ ] **Step 4: 运行测试确认通过**

```bash
npx vitest run tests/unit/env-store.test.ts 2>&1 | tail -25
```

Expected: PASS — 全部用例通过

- [ ] **Step 5: 类型检查与格式检查**

```bash
npx tsc -b && npx prettier --check "src/store/env-store.ts" "src/core/env-var.ts" "tests/unit/env-store.test.ts" "tests/unit/env-var.test.ts"
```

Expected: 通过

- [ ] **Step 6: 提交**

```bash
git add src/store/env-store.ts tests/unit/env-store.test.ts
git commit -m "feat(frontend): 新增环境变量 store（revision 校验 + 敏感值按需 reveal）"
```

---

## Task 6: UI —— 表格与工具栏组件

**Files:**

- Create: `src/components/env-list/EnvVarTable.tsx`、`src/components/env-list/EnvVarToolbar.tsx`
- Modify: `src/i18n/locales/zh-CN.json`、`src/i18n/locales/en.json`
- Test: `tests/unit/env-var-table.test.tsx`

**Interfaces:**

- Consumes: Task 4 的 `displayValue` / `maskValue` / `filterEnvVars` / `envVarKey`；Task 5 的 `useEnvStore`
- Produces:
  - `export function EnvVarTable(): JSX.Element`
  - `export function EnvVarToolbar(props: { onCreate: () => void; onEdit: () => void; onDelete: () => void; onRefresh: () => void; selected: EnvVarMeta | null }): JSX.Element`

**i18n 新增键**（zh-CN 与 en 都要加，`en` 用英文值）：

```json
{
  "tab": {
    "system": "系统 PATH",
    "user": "用户 PATH",
    "allVars": "全部变量",
    "merged": "合并预览"
  },
  "envVar": {
    "name": "变量名",
    "value": "值",
    "type": "类型",
    "source": "来源",
    "actions": "操作",
    "search": "搜索变量名",
    "newVar": "新建变量",
    "refresh": "刷新",
    "show": "显示",
    "hide": "隐藏",
    "all": "全部",
    "typeString": "String",
    "typeExpand": "ExpandString",
    "typeUnsupported": "不支持的类型",
    "unsupportedValue": "(不支持的注册表类型)",
    "protectedHint": "系统内置变量，修改可能导致系统异常",
    "readonlyHint": "没有写入该 hive 的权限",
    "pathGuide": "Path 请在「系统 PATH」/「用户 PATH」中编辑",
    "pathGuideAction": "前往",
    "newVarTitle": "新建环境变量",
    "newVarName": "变量名",
    "newVarValue": "变量值",
    "newVarKind": "值类型",
    "revealFirstHint": "请先点「显示」查看当前值后再编辑"
  }
}
```

- [ ] **Step 1: 写失败测试**

创建 `tests/unit/env-var-table.test.tsx`：

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

const mockBackend = {
  listAllEnvVars: vi.fn(),
  revealEnvVar: vi.fn(),
  updateEnvVar: vi.fn(),
  createEnvVar: vi.fn(),
  deleteEnvVar: vi.fn(),
};

vi.mock('@/services/backend', () => ({ backend: mockBackend }));

import { EnvVarTable } from '@/components/env-list/EnvVarTable';
import { useEnvStore } from '@/store/env-store';
import type { EnvVarMeta } from '@/core/env-var';

function meta(overrides: Partial<EnvVarMeta> = {}): EnvVarMeta {
  return {
    name: 'JAVA_HOME',
    kind: 'string',
    hive: 'user',
    canEdit: true,
    canDelete: true,
    sensitive: false,
    preview: 'C:\\Java',
    revision: 'rev-1',
    ...overrides,
  };
}

const snapshot = {
  system: [
    meta({ name: 'windir', hive: 'system' }),
    meta({ name: 'SYS_BIN', hive: 'system', kind: 'unsupported', preview: null, canEdit: false, canDelete: false }),
  ],
  user: [
    meta({ name: 'JAVA_HOME', hive: 'user' }),
    meta({ name: 'MY_TOKEN', hive: 'user', sensitive: true, preview: null }),
  ],
};

beforeEach(() => {
  vi.clearAllMocks();
  useEnvStore.setState({
    snapshot,
    revealed: new Map(),
    draft: new Map(),
    hiveFilter: 'all',
    isLoading: false,
    isSaving: false,
    statusMessage: '',
  });
});

describe('EnvVarTable', () => {
  it('渲染两个 hive 的变量', () => {
    render(<EnvVarTable />);
    expect(screen.getByText('JAVA_HOME')).toBeInTheDocument();
    expect(screen.getByText('windir')).toBeInTheDocument();
  });

  it('不渲染 Path（Rust 侧已过滤，此处防御性验证）', () => {
    useEnvStore.setState({
      snapshot: { system: [meta({ name: 'Path', hive: 'system' })], user: [] },
    });
    render(<EnvVarTable />);
    expect(screen.queryByText('Path')).not.toBeInTheDocument();
  });

  it('敏感值默认打码，不出现明文', () => {
    render(<EnvVarTable />);
    expect(screen.getAllByText('••••••••').length).toBeGreaterThan(0);
    expect(screen.queryByText('real-secret')).not.toBeInTheDocument();
  });

  it('点击「显示」后触发 reveal 并渲染明文', async () => {
    mockBackend.revealEnvVar.mockResolvedValue('real-secret');
    render(<EnvVarTable />);

    const showButtons = screen.getAllByRole('button', { name: '显示' });
    fireEvent.click(showButtons[0]);

    expect(await screen.findByText('real-secret')).toBeInTheDocument();
    expect(mockBackend.revealEnvVar).toHaveBeenCalledWith('user', 'MY_TOKEN');
  });

  it('保护行与只读行的编辑按钮禁用', () => {
    render(<EnvVarTable />);
    const editButtons = screen.getAllByRole('button', { name: '编辑' });
    // system 两条（windir 保护、SYS_BIN 不支持）应禁用
    const disabled = editButtons.filter((b) => (b as HTMLButtonElement).disabled);
    expect(disabled.length).toBeGreaterThanOrEqual(2);
  });

  it('Unsupported 行显示类型占位而非值', () => {
    render(<EnvVarTable />);
    expect(screen.getByText('(不支持的注册表类型)')).toBeInTheDocument();
  });

  it('顶部显示 Path 引导提示', () => {
    render(<EnvVarTable />);
    expect(screen.getByText(/Path 请在/)).toBeInTheDocument();
  });

  it('切换 hiveFilter 后只显示对应 hive', () => {
    useEnvStore.setState({ hiveFilter: 'user' });
    render(<EnvVarTable />);
    expect(screen.getByText('JAVA_HOME')).toBeInTheDocument();
    expect(screen.queryByText('windir')).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

```bash
npx vitest run tests/unit/env-var-table.test.tsx 2>&1 | tail -20
```

Expected: FAIL — `Failed to resolve import "@/components/env-list/EnvVarTable"`

- [ ] **Step 3: 写实现**

先补 i18n（`src/i18n/locales/zh-CN.json` 与 `en.json` 各加上述键；`tab.system` / `tab.user` 改为 `系统 PATH` / `用户 PATH`）。

创建 `src/components/env-list/EnvVarToolbar.tsx`：

```typescript
import { useTranslation } from 'react-i18next';
import { useEnvStore } from '@/store/env-store';
import type { EnvVarMeta, HiveFilter } from '@/core/env-var';

interface EnvVarToolbarProps {
  onCreate: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onRefresh: () => void;
  selected: EnvVarMeta | null;
}

const FILTERS: HiveFilter[] = ['all', 'system', 'user'];

/**
 * 环境变量专用工具栏。
 *
 * 刻意不复用 PATH 工具栏 —— PATH 的上移/下移/一键清理/导入/导出
 * 对普通变量语义不成立。
 */
export function EnvVarToolbar({
  onCreate,
  onEdit,
  onDelete,
  onRefresh,
  selected,
}: EnvVarToolbarProps) {
  const { t } = useTranslation();
  const hiveFilter = useEnvStore((s) => s.hiveFilter);
  const setHiveFilter = useEnvStore((s) => s.setHiveFilter);

  const canEdit = selected !== null && selected.canEdit;
  const canDelete = selected !== null && selected.canDelete;

  return (
    <div className="flex items-center gap-2 flex-wrap">
      <button className="toolbar-btn" onClick={onCreate}>
        {t('envVar.newVar')}
      </button>
      <button className="toolbar-btn" onClick={onEdit} disabled={!canEdit}>
        {t('button.edit')}
      </button>
      <button className="toolbar-btn" onClick={onDelete} disabled={!canDelete}>
        {t('button.delete')}
      </button>
      <button className="toolbar-btn" onClick={onRefresh}>
        {t('envVar.refresh')}
      </button>
      <div className="ml-auto flex items-center gap-1">
        {FILTERS.map((filter) => (
          <button
            key={filter}
            className={`toolbar-btn ${hiveFilter === filter ? 'tab-active' : 'opacity-60'}`}
            onClick={() => setHiveFilter(filter)}
          >
            {filter === 'all' ? t('envVar.all') : t(`envVar.source${filter === 'system' ? 'System' : 'User'}`)}
          </button>
        ))}
      </div>
    </div>
  );
}
```

创建 `src/components/env-list/EnvVarTable.tsx`：

```typescript
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useRef } from 'react';
import { useEnvStore } from '@/store/env-store';
import { displayValue, envVarKey, filterEnvVars } from '@/core/env-var';
import type { EnvVarMeta } from '@/core/env-var';

interface EnvVarTableProps {
  searchQuery?: string;
  onSelect?: (meta: EnvVarMeta) => void;
  selectedKey?: string | null;
}

const TYPE_LABEL_KEY: Record<EnvVarMeta['kind'], string> = {
  string: 'envVar.typeString',
  expandString: 'envVar.typeExpand',
  unsupported: 'envVar.typeUnsupported',
};

export function EnvVarTable({ searchQuery = '', onSelect, selectedKey }: EnvVarTableProps) {
  const { t } = useTranslation();
  const snapshot = useEnvStore((s) => s.snapshot);
  const hiveFilter = useEnvStore((s) => s.hiveFilter);
  const revealed = useEnvStore((s) => s.revealed);
  const reveal = useEnvStore((s) => s.reveal);
  const hide = useEnvStore((s) => s.hide);
  const parentRef = useRef<HTMLDivElement>(null);

  // Path 由 Rust 侧过滤；此处防御性再滤一次，避免契约被破坏时泄漏到 UI
  const rows = useMemo(() => {
    if (snapshot === null) return [];
    return filterEnvVars(snapshot, hiveFilter, searchQuery).filter(
      (meta) => meta.name.toLowerCase() !== 'path',
    );
  }, [snapshot, hiveFilter, searchQuery]);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 34,
    overscan: 12,
  });

  return (
    <div className="flex flex-col h-full">
      <div className="px-3 py-1.5 text-xs opacity-70">
        {t('envVar.pathGuide')}
      </div>
      <div className="flex items-center gap-2 px-3 py-1 text-xs font-medium border-b" style={{ borderColor: 'var(--app-border)' }}>
        <span className="flex-1">{t('envVar.name')}</span>
        <span className="flex-[2]">{t('envVar.value')}</span>
        <span className="w-28">{t('envVar.type')}</span>
        <span className="w-16">{t('envVar.source')}</span>
        <span className="w-32">{t('envVar.actions')}</span>
      </div>
      <div ref={parentRef} className="flex-1 overflow-auto">
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {virtualizer.getVirtualItems().map((item) => {
            const meta = rows[item.index];
            const key = envVarKey(meta);
            const revealedValue = revealed.get(key) ?? null;
            const isUnsupported = meta.kind === 'unsupported';
            return (
              <div
                key={key}
                data-index={item.index}
                ref={virtualizer.measureElement}
                className={`flex items-center gap-2 px-3 text-sm border-b ${
                  selectedKey === key ? 'row-selected' : ''
                }`}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  transform: `translateY(${item.start}px)`,
                  borderColor: 'var(--app-border)',
                  height: 34,
                }}
                onClick={() => onSelect?.(meta)}
              >
                <span className="flex-1 truncate" title={meta.name}>
                  {meta.name}
                </span>
                <span className="flex-[2] truncate font-mono text-xs">
                  {displayValue(meta, revealedValue)}
                </span>
                <span className="w-28 text-xs opacity-70">{t(TYPE_LABEL_KEY[meta.kind])}</span>
                <span className="w-16 text-xs opacity-70">
                  {meta.hive === 'system' ? t('merge.system') : t('merge.user')}
                </span>
                <span className="w-32 flex items-center gap-2">
                  {meta.sensitive && (
                    <button
                      className="text-xs underline"
                      onClick={(e) => {
                        e.stopPropagation();
                        void (revealedValue === null ? reveal(meta) : hide(meta));
                      }}
                    >
                      {revealedValue === null ? t('envVar.show') : t('envVar.hide')}
                    </button>
                  )}
                  <button className="text-xs" disabled={!meta.canEdit} title={editHint(meta, t)}>
                    {t('button.edit')}
                  </button>
                  <button className="text-xs" disabled={!meta.canDelete} title={editHint(meta, t)}>
                    {t('button.delete')}
                  </button>
                </span>
                {isUnsupported && <span className="sr-only">{t('envVar.typeUnsupported')}</span>}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

/** 禁用原因提示；权限已由 Rust 算好，此处只做映射。 */
function editHint(meta: EnvVarMeta, t: (key: string) => string): string {
  if (meta.kind === 'unsupported') return t('envVar.typeUnsupported');
  if (!meta.canEdit) return t('envVar.protectedHint');
  return '';
}
```

**实现提示**：`editHint` 无法区分"保护变量"与"无写权限"——两者在契约上都只是 `canEdit === false`。若需要区分 tooltip，应在 Task 1 的 `EnvVarMeta` 增加 `denyReason` 字段；**本计划不增加该字段**，统一用 `envVar.protectedHint` 文案，并在文案中同时覆盖两种含义（如"该变量不可编辑（系统内置或权限不足）"）。

- [ ] **Step 4: 运行测试确认通过**

```bash
npx vitest run tests/unit/env-var-table.test.tsx 2>&1 | tail -25
```

Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/components/env-list/ src/i18n/locales/ tests/unit/env-var-table.test.tsx
git commit -m "feat(ui): 新增环境变量表格与专用工具栏"
```

---

## Task 7: 接入 AppShell

**Files:**

- Modify: `src/core/path-capabilities.ts`、`src/components/layout/AppShell.tsx`
- Test: `tests/unit/app-shell-env-vars.test.tsx`

**Interfaces:**

- Consumes: Task 5 的 `useEnvStore`；Task 6 的 `EnvVarTable` / `EnvVarToolbar`
- Produces: `TabId` 扩展为 `'system' | 'user' | 'allVars' | 'merged'`；`AppShell` 按 Tab 分支渲染

**四项必须落实的决策**（来自 Spec Part 3）：

1. `allVars` 是合并列表（`EnvVarTable` 内部已处理）
2. `allVars` 用**独立工具栏**，PATH 工具栏在其中不可见
3. drop handler 在 `allVars` 下早退
4. 关窗确认纳入 `env-store` 的未提交草稿

- [ ] **Step 1: 写失败测试**

创建 `tests/unit/app-shell-env-vars.test.tsx`：

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

const mockBackend = {
  listAllEnvVars: vi.fn().mockResolvedValue({ system: [], user: [] }),
  loadPathSnapshot: vi.fn().mockResolvedValue({ system: [], user: [] }),
  getPathCapabilities: vi.fn().mockResolvedValue({
    canReadSystem: true,
    canWriteSystem: true,
    canReadUser: true,
    canWriteUser: true,
  }),
  revealEnvVar: vi.fn(),
  updateEnvVar: vi.fn(),
  createEnvVar: vi.fn(),
  deleteEnvVar: vi.fn(),
  expandEnvVars: vi.fn().mockResolvedValue(''),
  validatePath: vi.fn().mockResolvedValue(true),
};

vi.mock('@/services/backend', () => ({ backend: mockBackend }));

import { AppShell } from '@/components/layout/AppShell';
import { useAppStore } from '@/store/app-store';
import { useEnvStore } from '@/store/env-store';

beforeEach(() => {
  vi.clearAllMocks();
  useAppStore.setState({ activeTab: 'system', isModified: false });
  useEnvStore.setState({ draft: new Map(), snapshot: { system: [], user: [] } });
});

describe('AppShell Tab 结构', () => {
  it('渲染 4 个 Tab 且「全部变量」可用', () => {
    render(<AppShell />);
    expect(screen.getByText('系统 PATH')).toBeInTheDocument();
    expect(screen.getByText('用户 PATH')).toBeInTheDocument();
    expect(screen.getByText('全部变量')).toBeInTheDocument();
    expect(screen.getByText('合并预览')).toBeInTheDocument();
  });

  it('切到「全部变量」后触发 env-store 加载', async () => {
    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await waitFor(() => expect(mockBackend.listAllEnvVars).toHaveBeenCalled());
  });

  it('「全部变量」下 PATH 专用按钮不可见', async () => {
    render(<AppShell />);
    fireEvent.click(screen.getByText('全部变量'));
    await waitFor(() => expect(mockBackend.listAllEnvVars).toHaveBeenCalled());

    expect(screen.queryByRole('button', { name: '上移' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '下移' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '一键清理' })).not.toBeInTheDocument();
  });

  it('PATH Tab 下环境变量工具栏不可见', () => {
    render(<AppShell />);
    expect(screen.queryByRole('button', { name: '新建变量' })).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

```bash
npx vitest run tests/unit/app-shell-env-vars.test.tsx 2>&1 | tail -25
```

Expected: FAIL — 找不到「全部变量」文本

- [ ] **Step 3: 写实现**

修改 `src/core/path-capabilities.ts`：

```typescript
export type TabId = 'system' | 'user' | 'allVars' | 'merged';
```

（`targetForTab` 已有 `null` 兜底分支，`allVars` 会走 `return null`，无需改动，但需确认实现是显式列举 `system` / `user` 后 `return null`，而非穷尽匹配。）

修改 `src/components/layout/AppShell.tsx`：

1. 导入新组件与 store：

```typescript
import { EnvVarTable } from '@/components/env-list/EnvVarTable';
import { EnvVarToolbar } from '@/components/env-list/EnvVarToolbar';
import { useEnvStore } from '@/store/env-store';
import { useEffect } from 'react';
import type { EnvVarMeta } from '@/core/env-var';
```

2. Tab 配置增加 `allVars`：

```typescript
const tabConfig: { id: TabId; label: string }[] = [
  { id: 'system', label: t('tab.system') },
  { id: 'user', label: t('tab.user') },
  { id: 'allVars', label: t('tab.allVars') },
  { id: 'merged', label: t('tab.merged') },
];
```

3. 组件内新增状态与副作用：

```typescript
const [newVarOpen, setNewVarOpen] = useState(false);
const [selectedVar, setSelectedVar] = useState<EnvVarMeta | null>(null);
const loadEnvVars = useEnvStore((s) => s.load);

// 首次进入「全部变量」时加载；切换筛选不做 IPC
useEffect(() => {
  if (activeTab === 'allVars') void loadEnvVars();
}, [activeTab, loadEnvVars]);
```

4. 工具栏按 Tab 分支渲染。把现有 `<ToolBar .../>` 包进条件：

```jsx
<div className="px-4 py-2">
  {activeTab === 'allVars' ? (
    <EnvVarToolbar
      onCreate={() => setNewVarOpen(true)}
      onEdit={() => {
        if (selectedVar) {
          useEnvStore.getState().setDraft(selectedVar, selectedVar.preview ?? '');
        }
      }}
      onDelete={() => {
        if (selectedVar) void useEnvStore.getState().remove(selectedVar);
      }}
      onRefresh={() => void useEnvStore.getState().load()}
      selected={selectedVar}
    />
  ) : (
    <ToolBar
      onNew={actions.handleNew}
      onEdit={actions.handleEdit}
      onBrowse={actions.handleBrowse}
      onDelete={actions.handleDelete}
      onMoveUp={actions.handleMoveUp}
      onMoveDown={actions.handleMoveDown}
      onClean={actions.handleClean}
      onImport={actions.handleImport}
      onExport={actions.handleExport}
      onSave={actions.handleSave}
      onCancel={() => {
        const state = useAppStore.getState();
        const hasEnvDrafts = useEnvStore.getState().hasDrafts();
        if ((state.isModified || hasEnvDrafts) && !window.confirm(t('dialog.unsavedConfirm')))
          return;
        window.close();
      }}
      onHelp={() => setHelpOpen(true)}
      onLanguage={() => {
        const current = localStorage.getItem('i18nextLng') || 'zh-CN';
        i18n.changeLanguage(current === 'zh-CN' ? 'en' : 'zh-CN');
      }}
      onProfiles={() => setProfilesOpen(true)}
      onAnalyze={() => setAnalyzeOpen(true)}
      onDarkMode={() => useThemeStore.getState().toggle()}
    />
  )}
</div>
```

5. drop handler 早退（现有已处理 `merged`，改为同时排除 `allVars`）：

```jsx
        onDrop={(e) => {
          e.preventDefault();
          if (activeTab === 'merged' || activeTab === 'allVars') return;
          // ...其余不变
        }}
```

6. 内容区增加 `allVars` 分支：

```jsx
{
  activeTab === 'merged' ? (
    <MergePreview />
  ) : activeTab === 'allVars' ? (
    <EnvVarTable
      searchQuery=""
      onSelect={setSelectedVar}
      selectedKey={selectedVar ? `${selectedVar.hive}:${selectedVar.name}` : null}
    />
  ) : (
    <PathTable tabId={activeTab} />
  );
}
```

7. 新增变量弹窗：复用现有 `Modal`（`src/components/ui/Modal.tsx`）：

```jsx
{
  newVarOpen && (
    <NewEnvVarDialog
      onCancel={() => setNewVarOpen(false)}
      onConfirm={(hive, name, value, kind) => {
        setNewVarOpen(false);
        void useEnvStore.getState().create(hive, name, value, kind);
      }}
    />
  );
}
```

新建 `src/components/dialogs/NewEnvVarDialog.tsx`（最小实现）：

```typescript
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { validateVarName, type EnvHive, type EnvValueKind } from '@/core/env-var';

interface NewEnvVarDialogProps {
  onCancel: () => void;
  onConfirm: (hive: EnvHive, name: string, value: string, kind: EnvValueKind) => void;
}

export function NewEnvVarDialog({ onCancel, onConfirm }: NewEnvVarDialogProps) {
  const { t } = useTranslation();
  const [hive, setHive] = useState<EnvHive>('user');
  const [name, setName] = useState('');
  const [value, setValue] = useState('');
  const [kind, setKind] = useState<EnvValueKind>('string');
  const [error, setError] = useState<string | null>(null);

  const submit = () => {
    const invalid = validateVarName(name);
    if (invalid !== null) {
      setError(invalid);
      return;
    }
    onConfirm(hive, name, value, kind);
  };

  return (
    <Modal title={t('envVar.newVarTitle')} onClose={onCancel}>
      <div className="flex flex-col gap-2 text-sm">
        <label className="flex items-center gap-2">
          <span className="w-20">{t('envVar.source')}</span>
          <select value={hive} onChange={(e) => setHive(e.target.value as EnvHive)}>
            <option value="user">{t('merge.user')}</option>
            <option value="system">{t('merge.system')}</option>
          </select>
        </label>
        <label className="flex items-center gap-2">
          <span className="w-20">{t('envVar.newVarName')}</span>
          <input value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <label className="flex items-center gap-2">
          <span className="w-20">{t('envVar.newVarValue')}</span>
          <input value={value} onChange={(e) => setValue(e.target.value)} />
        </label>
        <label className="flex items-center gap-2">
          <span className="w-20">{t('envVar.newVarKind')}</span>
          <select value={kind} onChange={(e) => setKind(e.target.value as EnvValueKind)}>
            <option value="string">String</option>
            <option value="expandString">ExpandString</option>
          </select>
        </label>
        {error && <p className="text-red-500">{error}</p>}
        <div className="flex justify-end gap-2">
          <button onClick={onCancel}>{t('button.cancel')}</button>
          <button onClick={submit}>{t('button.save')}</button>
        </div>
      </div>
    </Modal>
  );
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
npx vitest run tests/unit/app-shell-env-vars.test.tsx 2>&1 | tail -25
```

Expected: PASS

- [ ] **Step 5: 运行全量前端测试**

```bash
npx vitest run 2>&1 | tail -25
```

Expected: PASS — 无既有测试回归

- [ ] **Step 6: 提交**

```bash
git add src/core/path-capabilities.ts src/components/layout/AppShell.tsx src/components/dialogs/NewEnvVarDialog.tsx tests/unit/app-shell-env-vars.test.tsx
git commit -m "feat(ui): 接入「全部变量」Tab 与独立工具栏"
```

---

## Task 8: E2E 测试与 mock

**Files:**

- Modify: `e2e/mocks/ipc.ts`
- Create: `e2e/tests/env-vars.spec.ts`

**Interfaces:**

- Consumes: Task 3 的 5 个 command 名（mock 需覆盖）
- Produces: 可运行的 E2E 用例

**fixture 要求**（来自 Spec Part 4）：

- 一条普通变量、一条敏感变量、一条保护变量、一条 `Unsupported`、一条 `canEdit=false`
- `Path` 出现在 mock 的"注册表数据"中但**不出现在 `list_all_env_vars` 返回值中**
- **不构造** `canEdit=false` 但 `canDelete=true` 的 fixture

- [ ] **Step 1: 扩展 mock IPC**

在 `e2e/mocks/ipc.ts` 的 `switch (cmd)` 中追加：

```javascript
          case 'list_all_env_vars': return {
            system: [
              { name: 'windir', kind: 'string', hive: 'system', canEdit: false, canDelete: false, sensitive: false, preview: 'C:\\\\WINDOWS', revision: 'sys-windir' },
              { name: 'SYS_BINARY', kind: 'unsupported', hive: 'system', canEdit: false, canDelete: false, sensitive: false, preview: null, revision: 'sys-bin' },
              { name: 'ADMIN_ONLY', kind: 'string', hive: 'system', canEdit: false, canDelete: false, sensitive: false, preview: 'x', revision: 'sys-ro' }
            ],
            user: [
              { name: 'JAVA_HOME', kind: 'string', hive: 'user', canEdit: true, canDelete: true, sensitive: false, preview: 'C:\\\\Java', revision: 'usr-java' },
              { name: 'MY_TOKEN', kind: 'string', hive: 'user', canEdit: true, canDelete: true, sensitive: true, preview: null, revision: 'usr-token' }
            ]
          };
          case 'reveal_env_var': return 'plaintext-secret-value';
          case 'update_env_var': return undefined;
          case 'create_env_var': return undefined;
          case 'delete_env_var': return undefined;
```

**注意**：mock 中**故意不返回 `Path`** —— 对应 Rust 侧过滤后的契约。同时保留注释说明这一点：

```javascript
// list_all_env_vars 的契约：Path 已被 Rust 侧过滤，绝不会出现在返回值中
```

- [ ] **Step 2: 写 E2E 用例**

创建 `e2e/tests/env-vars.spec.ts`：

```typescript
import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
});

test('切到「全部变量」显示两个 hive 的变量', async ({ page }) => {
  await page.getByText('全部变量').click();

  await expect(page.getByText('JAVA_HOME')).toBeVisible();
  await expect(page.getByText('windir')).toBeVisible();
});

test('Path 不出现在「全部变量」列表中', async ({ page }) => {
  await page.getByText('全部变量').click();

  await expect(page.getByText('JAVA_HOME')).toBeVisible();
  await expect(page.getByText('Path', { exact: true })).toHaveCount(0);
});

test('敏感变量默认打码，点「显示」后出现明文', async ({ page }) => {
  await page.getByText('全部变量').click();
  await expect(page.getByText('JAVA_HOME')).toBeVisible();

  await expect(page.getByText('plaintext-secret-value')).toHaveCount(0);

  await page.getByRole('button', { name: '显示' }).first().click();
  await expect(page.getByText('plaintext-secret-value')).toBeVisible();
});

test('保护行与不支持类型的编辑按钮禁用', async ({ page }) => {
  await page.getByText('全部变量').click();
  await expect(page.getByText('windir')).toBeVisible();

  // 定位 windir 所在行内的编辑按钮
  const row = page.locator('div', { hasText: 'windir' }).last();
  await expect(row.getByRole('button', { name: '编辑' })).toBeDisabled();
});

test('Unsupported 类型显示占位而非值', async ({ page }) => {
  await page.getByText('全部变量').click();

  await expect(page.getByText('(不支持的注册表类型)')).toBeVisible();
});

test('「全部变量」下 PATH 专用按钮不可见', async ({ page }) => {
  await page.getByText('全部变量').click();
  await expect(page.getByText('JAVA_HOME')).toBeVisible();

  await expect(page.getByRole('button', { name: '上移' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: '下移' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: '一键清理' })).toHaveCount(0);
});

test('切换来源筛选只显示对应 hive', async ({ page }) => {
  await page.getByText('全部变量').click();
  await expect(page.getByText('JAVA_HOME')).toBeVisible();

  await page.getByRole('button', { name: '系统' }).click();
  await expect(page.getByText('windir')).toBeVisible();
  await expect(page.getByText('JAVA_HOME')).toHaveCount(0);
});

test('编辑普通变量触发 update_env_var 并携带 revision', async ({ page }) => {
  await page.getByText('全部变量').click();
  await expect(page.getByText('JAVA_HOME')).toBeVisible();

  // 选中 JAVA_HOME 行后点该行的「编辑」
  const row = page.locator('[data-env-var-key="user:JAVA_HOME"]');
  await row.click();
  await row.getByRole('button', { name: '编辑' }).click();

  // 输入新值并确认（对话框内第二个输入框为值）
  const dialogInputs = page.locator('.modal input[type="text"]');
  await dialogInputs.nth(1).fill('C:\\NewJava');
  await page.getByRole('button', { name: '确定' }).click();

  // 断言：update_env_var 被调用，且 expectedRevision 等于列表下发的 revision
  const calls = await page.evaluate(
    () =>
      (window as unknown as { __capturedCalls: { cmd: string; args: Record<string, unknown> }[] })
        .__capturedCalls,
  );
  const updateCall = calls.find((c) => c.cmd === 'update_env_var');
  expect(updateCall).toBeDefined();
  expect(updateCall?.args.hive).toBe('user');
  expect(updateCall?.args.name).toBe('JAVA_HOME');
  expect(updateCall?.args.value).toBe('C:\\NewJava');
  expect(updateCall?.args.expectedRevision).toBe('usr-java');
});

test('revision 冲突时显示错误并刷新，不静默覆盖', async ({ page }) => {
  // 覆盖 update_env_var，使其返回冲突错误
  await page.addInitScript(`
    window.__conflictOverride = true;
  `);
  await page.getByText('全部变量').click();
  await expect(page.getByText('JAVA_HOME')).toBeVisible();

  const row = page.locator('[data-env-var-key="user:JAVA_HOME"]');
  await row.click();
  await row.getByRole('button', { name: '编辑' }).click();

  const dialogInputs = page.locator('.modal input[type="text"]');
  await dialogInputs.nth(1).fill('C:\\NewJava');
  await page.getByRole('button', { name: '确定' }).click();

  await expect(page.getByText(/已被其他进程修改/)).toBeVisible();
});
```

**调用捕获机制（确定方案，非可选项）**

`e2e/mocks/ipc.ts` 的 `createIpcMock` 必须实现调用记录。在返回的模板字符串中，于 `invoke` 函数体最前面插入：

```javascript
      invoke: async (cmd, args) => {
        // E2E 调用捕获：供断言 expectedRevision 等参数使用
        window.__capturedCalls = window.__capturedCalls || [];
        window.__capturedCalls.push({ cmd, args });

        const overrides = ${JSON.stringify(overrides)};
        if (cmd in overrides) return overrides[cmd];
        switch (cmd) {
          // ...（其余不变）
```

并在 `update_env_var` 分支支持冲突注入：

```javascript
          case 'update_env_var':
            if (window.__conflictOverride) {
              throw new Error('变量已被其他进程修改，请重新加载');
            }
            return undefined;
```

同时，`EnvVarTable` 的每行必须带 `data-env-var-key` 属性（值为 `` `${hive}:${name}` ``），供上面的 `page.locator('[data-env-var-key="..."]')` 精确定位。**这是 Task 6 的实现要求** —— 若 Task 6 未加该属性，请回到 Task 6 补上，不要在 E2E 中用模糊文本定位替代。

**为什么不用"降级为行为断言"**：`expectedRevision` 是 P1 并发校验在 E2E 层的唯一验证点。降级为"编辑后列表刷新"会失去对"携带了正确的 revision"的验证，使该缺陷可在无察觉的情况下回归。因此本计划**只提供上述确定实现**。

- [ ] **Step 3: 运行 E2E**

```bash
npm run test:e2e 2>&1 | tail -30
```

Expected: PASS — 新用例与既有用例全部通过

- [ ] **Step 4: 提交**

```bash
git add e2e/mocks/ipc.ts e2e/tests/env-vars.spec.ts
git commit -m "test(e2e): 覆盖「全部变量」Tab、打码、只读锁定与筛选"
```

---

## Task 9: 文档同步与质量门

**Files:**

- Modify: `CLAUDE.md`、`AGENTS.md`、`README.md`

**Interfaces:**

- Consumes: 前 8 个任务的最终形态
- Produces: 与实现一致的文档；质量门全绿

- [ ] **Step 1: 更新 CLAUDE.md 的 IPC 表**

在「Tauri IPC 接口」表格中追加 5 行：

```markdown
| `list_all_env_vars` | `() -> Result<EnvVarSnapshot, String>` | 一次读取两个 hive 的全部环境变量元数据（不含敏感明文） |
| `reveal_env_var` | `(hive, name) -> Result<String, String>` | 按需读取单个变量明文；`Unsupported` 类型返回错误 |
| `update_env_var` | `(hive, name, value, expectedRevision)` | 写入已有变量；类型从注册表读取，revision 不匹配则拒绝 |
| `create_env_var` | `(hive, name, value, kind)` | 新建变量；Rust 内原子检查名称不存在 |
| `delete_env_var` | `(hive, name, expectedRevision)` | 删除变量；revision 不匹配则拒绝 |
```

- [ ] **Step 2: 更新 CLAUDE.md 目录树与关键约束**

在 `core/src/` 目录树中追加：

```text
│   ├── env_var.rs               # 通用环境变量契约与保留/保护/敏感判定
```

在「关键约束」列表中追加一条：

```markdown
- 通用环境变量通路（`EnvVar`）与 PATH 通路（`PathEntry`）并存：`Path` 列入 `RESERVED_NAMES`，`list_all_env_vars` 过滤掉它，Rust 写入口拒绝 —— **PATH 只能经专用通路编辑**，否则会绕过 `disabled.json` 与快照事务。`EnvVarMeta` 契约上不含 `value` 字段，命中敏感规则的变量明文只能经 `reveal_env_var` 获取。
```

- [ ] **Step 3: 同步 AGENTS.md**

```bash
cp CLAUDE.md AGENTS.md
```

验证两份一致：

```bash
diff CLAUDE.md AGENTS.md && echo "一致"
```

Expected: `一致`

- [ ] **Step 4: 更新 README.md**

在「功能」章节的「路径管理」之后增加：

```markdown
### 全环境变量管理

- 查看和管理系统 / 用户环境变量项下的**所有变量**（不止 PATH）
- 显示变量的真实注册表类型（`REG_SZ` / `REG_EXPAND_SZ`），不支持的类型只读展示
- 敏感变量（名称含 TOKEN / KEY / SECRET / 密码 / API）默认打码，需显式点击才显示明文
- 系统内置关键变量（`windir`、`ComSpec`、`PATHEXT` 等）硬锁定为只读，防止改坏系统
- `Path` 仍由专用 PATH 视图管理，保证启用/禁用状态与顺序不被绕过
```

- [ ] **Step 5: 运行完整质量门**

```bash
npm run verify 2>&1 | tail -30
```

Expected: PASS — Prettier / ESLint / `tsc -b` / 覆盖率 80% / `cargo fmt --check` / `cargo clippy -D warnings` / `cargo test --workspace` 全绿

若覆盖率低于 80%，为未覆盖的新分支补充 `tests/unit/env-var.test.ts` 与 `tests/unit/env-store.test.ts` 的用例，直到通过。

- [ ] **Step 6: 运行完整 E2E**

```bash
npm run verify:all 2>&1 | tail -30
```

Expected: PASS

- [ ] **Step 7: 提交**

```bash
git add CLAUDE.md AGENTS.md README.md
git commit -m "docs: 同步全环境变量管理到开发指南与 README"
```

---

## 自检结果

**Spec 覆盖检查：**

| Spec 章节                                                           | 对应任务                                             |
| ------------------------------------------------------------------- | ---------------------------------------------------- |
| Part 1 `EnvVarMeta` / `EnvValueKind` / `EnvHive` / `EnvVarSnapshot` | Task 1                                               |
| Part 1 保留/保护/敏感判定 + 权限矩阵 + preview 净化 + revision      | Task 1                                               |
| Part 1 五个 registry 函数 + 类型只从注册表读 + TOCTOU + 校验 + 广播 | Task 2                                               |
| Part 1 `Path` 过滤                                                  | Task 2（`list_hive_env_vars` 中 `is_reserved` 跳过） |
| Part 2 GUI command                                                  | Task 3                                               |
| Part 2 IPC 形状校验                                                 | Task 4                                               |
| Part 2 纯逻辑 `env-var.ts`                                          | Task 4                                               |
| Part 2 状态层 `env-store.ts`                                        | Task 5                                               |
| Part 3 UA 架构四项决策                                              | Task 7                                               |
| Part 3 Tab 结构与文案                                               | Task 6（i18n）+ Task 7（结构）                       |
| Part 3 `EnvVarTable`                                                | Task 6                                               |
| Part 3 敏感值交互                                                   | Task 6                                               |
| Part 3 只读锁定交互                                                 | Task 6                                               |
| Part 4 Rust 测试                                                    | Task 1 + Task 2                                      |
| Part 4 Vitest                                                       | Task 4 + Task 5 + Task 6 + Task 7                    |
| Part 4 E2E                                                          | Task 8                                               |
| Part 4 质量门                                                       | Task 9                                               |
| Part 4 文档同步                                                     | Task 9                                               |

**广播调用已内联**：Spec Part 1「实现约束 6」要求写操作成功后调用 `system::broadcast_env_change()`。Task 2 的 `update_env_var` / `create_env_var` / `delete_env_var` 三个函数体内**已直接包含**该调用，无需执行者跨章节补丁。

**执行者需要注意的实现细节（非错误，是设计取舍）**：

1. Task 6 的 `editHint` 无法区分"保护变量"与"无写权限" —— 两者在契约上都只是 `canEdit === false`。本计划选择**不增加 `denyReason` 字段**（避免为提示文案扩大契约），改用统一文案同时覆盖两种含义。
2. Task 6 的 `EnvVarTable` 每行**必须**带 `data-env-var-key` 属性（值为 `` `${hive}:${name}` ``）—— Task 8 的 E2E 依赖它精确定位行。这是跨任务的实现约束，不是可选优化。

**类型一致性检查**：`EnvVarMeta` 字段在 Task 1（Rust）、Task 4（TS）、Task 5（store）、Task 6（UI）中的命名与类型一致；`EnvValueKind` 的三个变体 `String`/`ExpandString`/`Unsupported` 在 Rust 侧与 TS 侧的 `'string'|'expandString'|'unsupported'` 通过 `#[serde(rename_all = "camelCase")]` 对齐；`envVarKey` 的 `${hive}:${name}` 格式在 Task 4、Task 5、Task 6、Task 7、Task 8 中一致。
