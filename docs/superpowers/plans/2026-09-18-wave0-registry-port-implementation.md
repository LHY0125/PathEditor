# 注册表端口与列表失败语义（Wave 0）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为环境变量读写引入 `EnvHiveStore` 端口——生产走 winreg、测试走内存，使环境变量测试不再写真实 HKCU；同时让列表枚举/读取/解码失败不再被静默吞掉（F-04）。

**Architecture:** 新增 `core/src/reg_store.rs`，定义 `EnvHiveStore` trait 与生产实现 `WinregHive`；`registry.rs` 内部的环境变量函数从收 `&RegKey` 改为收 `&dyn EnvHiveStore`；`broadcast_env_change()` 从 `*_in_store` 内部上移到公开包装函数，使测试路径不再触发系统广播。对外公开 API 的签名与语义**除 F-04 的失败语义外**保持不变。

**Tech Stack:** Rust workspace（`core` / `gui` / `cli`）、`winreg`、`serde`、`log`。

**Spec:** `docs/superpowers/specs/2026-09-18-consistency-and-architecture-design.md`

## Global Constraints

- **分支**：在 worktree 中开发（`.claude/worktrees/<name>`，分支 `worktree-<name>`），不在 main 直接改。worktree 内先 `npm install`。
- **不推送、不升级版本号**：提交落本地分支，推送时机由用户决定。
- **禁止真实注册表写入**：本计划所有测试**不得写** HKCU/HKLM。**只读**探测（`WinregHive::open(User, false)`、`enum_names()`）允许。完成后 `cargo test --workspace` 必须能在无注册表写入的环境跑通。既有的 `#[ignore]` 测试（`registry.rs:684`）保持 `#[ignore]` 不动。
- **文档注释**：所有 `pub` / `pub(crate)` 项必须有 `///` 文档注释（CONTRIBUTING.md 硬性要求）。
- **代码风格**：UTF-8、CRLF；Rust 4 空格缩进；rustfmt 默认 100 列。
- **质量门**：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 全绿。
- **提交规范**：Conventional Commits；每个 Task 结束提交一次。
- **不删除文件**，未经用户明确同意。
- **CLI 单测用 `cargo test -p patheditor-cli --bins`**（`patheditor-cli` 是 bin-only crate，`--lib` 会报 `no library targets found`）。

## File Structure

| 文件                    | 职责                                                                                                                                                 | 变更     |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| `core/src/reg_store.rs` | `EnvHiveStore` trait + `WinregHive` 生产实现 + `MemoryHive` 测试替身                                                                                 | **新建** |
| `core/src/lib.rs`       | 声明`pub mod reg_store;`                                                                                                                             | 修改     |
| `core/src/registry.rs`  | `hive_location` 改 `pub(crate)`；环境变量内部函数改收 `&dyn EnvHiveStore`；列表失败语义改为传播；删除已无用的 `env_key`；`broadcast_env_change` 上移 | 修改     |

---

### Task 1: `EnvHiveStore` 端口与 `WinregHive` 生产实现

**Files:**

- Create: `core/src/reg_store.rs`
- Modify: `core/src/lib.rs`（在模块声明区加 `pub mod reg_store;`）
- Modify: `core/src/registry.rs:227`（`fn hive_location` → `pub(crate) fn hive_location`）

**Interfaces:**

- Consumes: `crate::env_var::EnvHive`、`crate::system::check_admin`、`crate::registry::can_write_user`
- Produces:
  - `pub trait EnvHiveStore`，5 个方法（见下）
  - `pub struct WinregHive`，`pub fn open(hive: EnvHive, write: bool) -> Result<WinregHive, String>`

- [ ] **Step 1: 写 `reg_store.rs` 的端口与生产实现**

创建 `core/src/reg_store.rs`：

```rust
//! 环境变量 hive 的存储端口。
//!
//! 生产环境用 winreg 实现（[`WinregHive`]），测试用内存实现
//! （`memory::MemoryHive`），使环境变量读写测试不必写真实 HKCU。
//! 错误文案在此层统一格式化，调用方直接透传。

use winreg::enums::*;
use winreg::RegValue;

use crate::env_var::EnvHive;

/// 单个 hive 的环境变量键抽象。
///
/// 只覆盖环境变量读写所需的 5 个操作；PATH 通路仍走 `registry.rs` 的
/// `load_paths` / `save_paths`，本版不纳入端口。
pub trait EnvHiveStore {
    /// 当前用户对该 hive 是否可写（供 capabilities 计算，避免逐变量探测注册表）。
    fn writable(&self) -> bool;

    /// 枚举该 hive 下所有值名，保持注册表返回的原始大小写。
    fn enum_names(&self) -> Result<Vec<String>, String>;

    /// 读取值的原始字节与真实类型。
    fn get_raw(&self, name: &str) -> Result<RegValue, String>;

    /// 写入值，类型由调用方给定。
    fn set_raw(&self, name: &str, value: &RegValue) -> Result<(), String>;

    /// 删除值。
    fn delete_value(&self, name: &str) -> Result<(), String>;
}

/// 生产实现：把 winreg 的 `RegKey` 适配为 [`EnvHiveStore`]。
pub struct WinregHive {
    key: winreg::RegKey,
    writable: bool,
}

impl WinregHive {
    /// 打开指定 hive 的环境变量键。
    ///
    /// `write` 为 `true` 时请求 `KEY_READ | KEY_WRITE`，否则只请求 `KEY_READ`。
    pub fn open(hive: EnvHive, write: bool) -> Result<Self, String> {
        let (root, sub_path, label) = crate::registry::hive_location(hive);
        let flags = if write {
            KEY_READ | KEY_WRITE
        } else {
            KEY_READ
        };
        let key = winreg::RegKey::predef(root)
            .open_subkey_with_flags(sub_path, flags)
            .map_err(|e| format!("无法打开{}环境变量注册表项: {}", label, e))?;
        Ok(Self {
            key,
            writable: hive_writable(hive),
        })
    }
}

/// 该 hive 对当前用户是否可写。
fn hive_writable(hive: EnvHive) -> bool {
    match hive {
        EnvHive::System => crate::system::check_admin(),
        EnvHive::User => crate::registry::can_write_user(),
    }
}

impl EnvHiveStore for WinregHive {
    fn writable(&self) -> bool {
        self.writable
    }

    fn enum_names(&self) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        for item in self.key.enum_values() {
            let (name, _) = item.map_err(|e| format!("无法枚举环境变量: {}", e))?;
            names.push(name);
        }
        Ok(names)
    }

    fn get_raw(&self, name: &str) -> Result<RegValue, String> {
        self.key
            .get_raw_value(name)
            .map_err(|e| format!("无法读取环境变量 {}: {}", name, e))
    }

    fn set_raw(&self, name: &str, value: &RegValue) -> Result<(), String> {
        self.key
            .set_raw_value(name, value)
            .map_err(|e| format!("无法写入环境变量 {}: {}", name, e))
    }

    fn delete_value(&self, name: &str) -> Result<(), String> {
        self.key
            .delete_value(name)
            .map_err(|e| format!("无法删除环境变量 {}: {}", name, e))
    }
}
```

- [ ] **Step 2: 注册模块**

在 `core/src/lib.rs` 的模块声明区加入（与既有 `pub mod registry;` 并列）：

```rust
pub mod reg_store;
```

在 `core/src/registry.rs:227` 把 `hive_location` 提升可见性：

```rust
pub(crate) fn hive_location(hive: EnvHive) -> (winreg::HKEY, &'static str, &'static str) {
```

- [ ] **Step 3: 写只读冒烟测试**

在 `reg_store.rs` 末尾追加（**只读**，不写注册表）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winreg_hive_opens_user_hive_read_only() {
        // 只断言「只读打开 + 枚举可用」，**不绑定任何具体值名** ——
        // 很多机器的 HKCU\Environment 下没有用户级 Path（PATH 常只在 HKLM），
        // 绑定 Path 会让测试在那些机器上误红。
        let store = WinregHive::open(EnvHive::User, false).expect("打开 HKCU 环境变量键失败");
        store.enum_names().expect("枚举用户环境变量失败");
    }

    #[test]
    fn winreg_hive_reports_writability_as_bool() {
        let store = WinregHive::open(EnvHive::User, false).expect("打开 HKCU 环境变量键失败");
        // 只断言能取到布尔值，不断言具体权限（取决于运行账户）
        let _ = store.writable();
    }
}
```

- [ ] **Step 4: 运行测试**

Run: `cargo test -p path-editor-core reg_store`

Expected: PASS（2 passed）

- [ ] **Step 5: 质量门 + 提交**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p path-editor-core
git add core/src/reg_store.rs core/src/lib.rs core/src/registry.rs
git commit -m "feat(core): 新增环境变量存储端口 EnvHiveStore 与 WinregHive"
```

---

### Task 2: `MemoryHive` 内存测试替身

**Files:**

- Modify: `core/src/reg_store.rs`（追加 `#[cfg(test)] mod memory`）

**Interfaces:**

- Consumes: `EnvHiveStore`
- Produces:
  - `memory::MemoryHive`，`new(writable: bool)`、`seed(name, value, vtype)`、`seed_raw(name, raw)`、`contains(name)`
  - 故障注入字段：`fail_enum: bool`、`fail_get: Option<String>`（Wave 1 的 F-04 测试会用）

- [ ] **Step 1: 实现 `MemoryHive`**

在 `core/src/reg_store.rs` 追加：

```rust
/// 供单元测试使用的内存实现；不触碰真实注册表。
#[cfg(test)]
pub(crate) mod memory {
    use super::*;
    use std::cell::RefCell;

    /// 复制一个 `RegValue`。
    ///
    /// **winreg 0.52.0 的 `RegValue` 只 derive 了 `PartialEq`，没有 `Clone`**
    /// （`winreg-0.52.0/src/reg_value.rs:11`），因此只能按字段复制。
    fn dup_reg_value(raw: &RegValue) -> RegValue {
        // `RegType` 是 Clone 但不是 Copy（`winreg-0.52.0/src/enums.rs:20`，
        // derive 只有 Debug/Clone/PartialEq），所以这里必须 `.clone()`。
        RegValue {
            bytes: raw.bytes.clone(),
            vtype: raw.vtype.clone(),
        }
    }

    /// 内存版环境变量 hive。
    ///
    /// 值名按 Windows 语义**大小写不敏感**。`fail_*` 字段用于故障注入，
    /// 让 F-04 的「枚举/读取失败不得静默」可被测试。
    pub(crate) struct MemoryHive {
        values: RefCell<Vec<(String, RegValue)>>,
        writable: bool,
        /// 为 `true` 时 `enum_names` 返回 `Err`。
        pub(crate) fail_enum: bool,
        /// 命中的值名在 `get_raw` 时返回 `Err`。
        pub(crate) fail_get: Option<String>,
    }

    impl MemoryHive {
        /// 新建空 hive。`writable` 决定 `capabilities_for_with` 的写权限分支。
        pub(crate) fn new(writable: bool) -> Self {
            Self {
                values: RefCell::new(Vec::new()),
                writable,
                fail_enum: false,
                fail_get: None,
            }
        }

        /// 写入一个字符串种子值。
        pub(crate) fn seed(&self, name: &str, value: &str, vtype: RegType) {
            let mut raw = value.to_reg_value();
            raw.vtype = vtype;
            self.set_raw(name, &raw).expect("内存写入不应失败");
        }

        /// 写入一个原始值（用于构造非法字节/不支持类型）。
        pub(crate) fn seed_raw(&self, name: &str, raw: RegValue) {
            self.set_raw(name, &raw).expect("内存写入不应失败");
        }

        /// 判断是否存在某值名（大小写不敏感）。
        pub(crate) fn contains(&self, name: &str) -> bool {
            self.values
                .borrow()
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case(name))
        }
    }

    impl EnvHiveStore for MemoryHive {
        fn writable(&self) -> bool {
            self.writable
        }

        fn enum_names(&self) -> Result<Vec<String>, String> {
            if self.fail_enum {
                return Err("无法枚举环境变量: 注入的枚举失败".into());
            }
            Ok(self
                .values
                .borrow()
                .iter()
                .map(|(n, _)| n.clone())
                .collect())
        }

        fn get_raw(&self, name: &str) -> Result<RegValue, String> {
            if self
                .fail_get
                .as_deref()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
            {
                return Err(format!("无法读取环境变量 {}: 注入的读取失败", name));
            }
            self.values
                .borrow()
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, v)| dup_reg_value(v))
                .ok_or_else(|| format!("无法读取环境变量 {}: 找不到", name))
        }

        fn set_raw(&self, name: &str, value: &RegValue) -> Result<(), String> {
            let mut values = self.values.borrow_mut();
            if let Some(slot) = values
                .iter_mut()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
            {
                slot.1 = dup_reg_value(value);
            } else {
                values.push((name.to_string(), dup_reg_value(value)));
            }
            Ok(())
        }

        fn delete_value(&self, name: &str) -> Result<(), String> {
            let mut values = self.values.borrow_mut();
            let before = values.len();
            values.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
            if values.len() == before {
                return Err(format!("无法删除环境变量 {}: 找不到", name));
            }
            Ok(())
        }
    }
}
```

`reg_store.rs` 顶部需要 `use winreg::types::ToRegValue;`（`seed` 用到）。

- [ ] **Step 2: 写一致性测试**

```rust
#[cfg(test)]
mod memory_tests {
    use super::memory::MemoryHive;
    use super::*;
    use winreg::enums::REG_SZ;
    use winreg::types::FromRegValue;

    #[test]
    fn memory_hive_roundtrip_is_case_insensitive() {
        let hive = MemoryHive::new(true);
        hive.seed("JAVA_HOME", "C:\\Java", REG_SZ);

        assert!(hive.contains("java_home"));
        let raw = hive.get_raw("java_Home").expect("读取失败");
        assert_eq!(String::from_reg_value(&raw).unwrap(), "C:\\Java");
        assert_eq!(hive.enum_names().unwrap(), vec!["JAVA_HOME".to_string()]);
    }

    #[test]
    fn memory_hive_overwrites_in_place_preserving_original_name() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "old", REG_SZ);
        hive.seed("my_var", "new", REG_SZ);

        assert_eq!(hive.enum_names().unwrap(), vec!["MY_VAR".to_string()]);
        let raw = hive.get_raw("MY_VAR").unwrap();
        assert_eq!(String::from_reg_value(&raw).unwrap(), "new");
    }

    #[test]
    fn memory_hive_delete_reports_missing() {
        let hive = MemoryHive::new(true);
        assert!(hive.delete_value("NOPE").is_err());
        hive.seed("MY_VAR", "v", REG_SZ);
        assert!(hive.delete_value("my_var").is_ok());
        assert!(!hive.contains("MY_VAR"));
    }

    #[test]
    fn memory_hive_injects_failures() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "v", REG_SZ);

        hive.fail_enum = true;
        assert!(hive.enum_names().is_err());
        hive.fail_enum = false;

        hive.fail_get = Some("my_var".into());
        assert!(hive.get_raw("MY_VAR").is_err());
        assert!(hive.get_raw("MY_VAR").unwrap_err().contains("注入的读取失败"));
    }
}
```

- [ ] **Step 3: 运行测试**

Run: `cargo test -p path-editor-core reg_store`

Expected: PASS（Task 1 的 2 个 + Task 2 的 4 个）

- [ ] **Step 4: 质量门 + 提交**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add core/src/reg_store.rs
git commit -m "test(core): 新增内存 hive 测试替身与故障注入能力"
```

---

### Task 3: 单值读写函数改走端口（`read_env_var` / `write_env_var`）

**Files:**

- Modify: `core/src/registry.rs:241-259`

**Interfaces:**

- Consumes: `EnvHiveStore`（`crate::reg_store::EnvHiveStore`）
- Produces:
  - `fn read_env_var(store: &dyn EnvHiveStore, name: &str) -> Result<(RegType, String), String>`
  - `fn write_env_var(store: &dyn EnvHiveStore, name: &str, value: &str, vtype: RegType) -> Result<(), String>`

**为什么先改这两个**：`reveal` / `update` / `create` / `delete` 四个核心函数都调用它们；先把最底层换成端口，后续四个函数只需替换形参。

- [ ] **Step 1: 替换 `read_env_var`（`registry.rs:241-251`）**

```rust
/// 读取单个值，返回 (vtype, value)。仅用于字符串类型。
///
/// `vtype` 是 winreg 的 `RegType`，不是 `u32` —— 类型判定与 revision
/// 计算都基于它。先判定类型再做字符串解码：`REG_DWORD` 等不支持类型
/// 必须返回「类型不受支持」，而不是误导性的解码失败。
fn read_env_var(store: &dyn EnvHiveStore, name: &str) -> Result<(RegType, String), String> {
    let raw = store.get_raw(name)?;
    // `RegType` 非 Copy（`winreg-0.52.0/src/enums.rs:20`），必须先
    // `.clone()` 取值再借用 `&raw`，否则 `from_reg_type(raw.vtype)` 会部分
    // 移出 `raw`，后面 `&raw` 与 `Ok((raw.vtype, …))` 都会编译失败。
    // 原实现此处即 `raw.vtype.clone()`（`registry.rs:245`）。
    if !EnvValueKind::from_reg_type(raw.vtype.clone()).is_writable() {
        return Err(format!("环境变量 {} 的注册表类型不受支持", name));
    }
    let value = String::from_reg_value(&raw)
        .map_err(|e| format!("无法解码环境变量 {}: {}", name, e))?;
    Ok((raw.vtype, value))
}
```

- [ ] **Step 2: 替换 `write_env_var`（`registry.rs:254-259`）**

```rust
/// 写入单个值，保持调用方给定的注册表类型。错误由端口层格式化。
fn write_env_var(
    store: &dyn EnvHiveStore,
    name: &str,
    value: &str,
    vtype: RegType,
) -> Result<(), String> {
    let mut raw = value.to_reg_value();
    raw.vtype = vtype;
    store.set_raw(name, &raw)
}
```

- [ ] **Step 3: 加 import**

`registry.rs` 顶部 `use` 区加入：

```rust
use crate::reg_store::EnvHiveStore;
```

- [ ] **Step 4: 编译（预期报错，尚未更新的调用点）**

Run: `cargo check -p path-editor-core`

Expected: FAIL —— `reveal_env_var_in_key` / `update_env_var_in_key` / `create_env_var_in_key` / `delete_env_var_in_key` 里的 `read_env_var(&key, …)` / `write_env_var(&key, …)` 类型不匹配。这是预期，Task 4 修复。

---

### Task 4: 四个环境变量核心函数改收 `&dyn EnvHiveStore`

**Files:**

- Modify: `core/src/registry.rs`（`reveal` / `update` / `create` / `delete` 的 `_in_key` 系列 → `_in_store`）

**Interfaces:**

- Consumes: `read_env_var` / `write_env_var`（Task 3）
- Produces（内部函数，测试直接调用）：
  - `fn reveal_env_var_in_store(store: &dyn EnvHiveStore, name: &str) -> Result<String, String>`
  - `fn update_env_var_in_store(store: &dyn EnvHiveStore, name: &str, value: &str, expected_revision: &str) -> Result<(), String>`
  - `fn create_env_var_in_store(store: &dyn EnvHiveStore, name: &str, value: &str, kind: EnvValueKind) -> Result<(), String>`
  - `fn delete_env_var_in_store(store: &dyn EnvHiveStore, name: &str, expected_revision: &str) -> Result<(), String>`

**关键设计**：`broadcast_env_change()` 从 `*_in_store` 内部**上移到公开包装函数**。理由：测试调用 `_in_store` 时不应向系统发 `WM_SETTINGCHANGE`；真实调用方仍走公开 API，行为不变。

- [ ] **Step 1: 改写 `reveal_env_var_in_key` → `reveal_env_var_in_store`**

```rust
/// `reveal_env_var` 的核心逻辑，存储可注入（测试用内存实现）。
fn reveal_env_var_in_store(store: &dyn EnvHiveStore, name: &str) -> Result<String, String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name));
    }
    let (_vtype, value) = read_env_var(store, name)?;
    Ok(value)
}
```

- [ ] **Step 2: 改写 `update_env_var_in_key` → `update_env_var_in_store`**

```rust
/// `update_env_var` 的核心逻辑，存储可注入。
///
/// 在同一调用内完成「读 → 算 revision → 比对 → 校验 → 写」。
/// 读与写是两次独立存储调用，仍有竞态窗口；revision 校验缩小影响，
/// 不能完全消除 TOCTOU。
fn update_env_var_in_store(
    store: &dyn EnvHiveStore,
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

    let (vtype, current) = read_env_var(store, name)?;

    let current_revision = revision_of(name, vtype.clone(), &current);
    if current_revision != expected_revision {
        return Err(ERR_CONFLICT.into());
    }

    let kind = EnvValueKind::from_reg_type(vtype.clone());
    if !kind.is_writable() {
        return Err(format!("{} 的注册表类型不受支持，无法修改（仅可查看）", name));
    }
    validate_env_value(value, name)?;

    write_env_var(store, name, value, vtype)
}
```

（注意：`broadcast_env_change()` 已从本函数移除。）

- [ ] **Step 3: 改写 `create_env_var_in_key` → `create_env_var_in_store`**

```rust
/// `create_env_var` 的核心逻辑，存储可注入。
///
/// 写入前先确认同名变量不存在（忽略大小写），不覆盖已有变量。
/// 枚举检查与写入是两次独立调用，存在竞态窗口，当前未做原子 CAS。
///
/// **有意的语义变更（评审裁断 O-2）**：查重用的 `store.enum_names()?` 会传播
/// 枚举错误。原 `create_env_var_in_key` 用 `enum_values().flatten()` 吞掉枚举
/// 错误 —— 枚举失败会被当作「同名不存在」而继续写入，有冒险覆盖的隐患。
/// 新行为是「失败响亮优于冒险覆盖」，与 F-04 同源，但**超出 F-04 只针对
/// list 的字面范围**。若不愿引入该变更，改 `store.enum_names().unwrap_or_default()`
/// 即可完全保持现状。
fn create_env_var_in_store(
    store: &dyn EnvHiveStore,
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

    let existing = store
        .enum_names()?
        .iter()
        .any(|n| n.eq_ignore_ascii_case(name));
    if existing {
        return Err(format!("变量 {} 已存在，请使用编辑功能", name));
    }

    let vtype = match kind {
        EnvValueKind::String => REG_SZ,
        _ => REG_EXPAND_SZ,
    };
    write_env_var(store, name, value, vtype)
}
```

- [ ] **Step 4: 改写 `delete_env_var_in_key` → `delete_env_var_in_store`**

```rust
/// `delete_env_var` 的核心逻辑，存储可注入。
///
/// 读与删除是两次独立调用，revision 校验缩小竞态影响，不能完全消除 TOCTOU。
fn delete_env_var_in_store(
    store: &dyn EnvHiveStore,
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

    let (vtype, current) = read_env_var(store, name)?;
    let current_revision = revision_of(name, vtype.clone(), &current);
    if current_revision != expected_revision {
        return Err(ERR_CONFLICT.into());
    }

    if EnvValueKind::from_reg_type(vtype) == EnvValueKind::Unsupported {
        // 防御性、当前不可达（评审裁断 O-3）：`read_env_var` 已对 Unsupported
        // 类型提前返回 Err，上面的 `?` 会先短路。保留以显式表达删除的前置条件，
        // 行为与现状一致。
        return Err(format!("{} 的注册表类型不受支持，无法删除", name));
    }

    store.delete_value(name)?;
    Ok(())
}
```

- [ ] **Step 5: 更新四个公开包装函数，并在此处广播**

```rust
/// 按需读取单个变量的明文（命中敏感规则的变量的唯一取值入口）。
///
/// `Unsupported` 类型返回 `Err`，不尝试转字符串。
pub fn reveal_env_var(hive: EnvHive, name: &str) -> Result<String, String> {
    let store = WinregHive::open(hive, false)?;
    reveal_env_var_in_store(&store, name)
}

/// 写入已有变量。类型从注册表读取，不由前端决定。
pub fn update_env_var(
    hive: EnvHive,
    name: &str,
    value: &str,
    expected_revision: &str,
) -> Result<(), String> {
    let store = WinregHive::open(hive, true)?;
    update_env_var_in_store(&store, name, value, expected_revision)?;
    crate::system::broadcast_env_change();
    Ok(())
}

/// 新建变量。`kind` 仅在此决定。
pub fn create_env_var(
    hive: EnvHive,
    name: &str,
    value: &str,
    kind: EnvValueKind,
) -> Result<(), String> {
    let store = WinregHive::open(hive, true)?;
    create_env_var_in_store(&store, name, value, kind)?;
    crate::system::broadcast_env_change();
    Ok(())
}

/// 删除变量。在同一调用内完成 revision 比对与删除。
pub fn delete_env_var(hive: EnvHive, name: &str, expected_revision: &str) -> Result<(), String> {
    let store = WinregHive::open(hive, true)?;
    delete_env_var_in_store(&store, name, expected_revision)?;
    crate::system::broadcast_env_change();
    Ok(())
}
```

- [ ] **Step 6: 删除已无用的 `env_key`（`registry.rs:216-225`）**

`env_key` 现在只被这些函数使用，全部改走端口后不再被引用；留着会触发 `dead_code` 的 clippy 警告。删除该函数。

同时确认文件头部需补的 use：

```rust
use crate::reg_store::{EnvHiveStore, WinregHive};
```

- [ ] **Step 7: 编译**

Run: `cargo check -p path-editor-core`

Expected: FAIL —— 还剩 `list_env_vars_in_key` 与测试模块引用旧符号；Task 5、Task 6 处理。

---

### Task 5: 列表路径改走端口，并让失败不再静默（F-04）

**Files:**

- Modify: `core/src/registry.rs:294-359`（`list_hive_env_vars` / `list_env_vars_in_key`）

**Interfaces:**

- Consumes: `EnvHiveStore`、`WinregHive`
- Produces:
  - `fn list_env_vars_in_store(hive: EnvHive, store: &dyn EnvHiveStore) -> Result<Vec<EnvVarMeta>, String>`

**F-04 用户裁决（2026-09-18）**：**整个 hive 报错**——同一 hive 内枚举 / 读取 / 解码任一失败即返回 `Err`，不再 `warn + continue`。

- [ ] **Step 1: 改写 `list_hive_env_vars`**

```rust
/// 读取单个 hive 的所有环境变量元数据。`Path` 在此被过滤。
fn list_hive_env_vars(hive: EnvHive) -> Result<Vec<EnvVarMeta>, String> {
    let store = WinregHive::open(hive, false)?;
    list_env_vars_in_store(hive, &store)
}
```

- [ ] **Step 2: 改写 `list_env_vars_in_key` → `list_env_vars_in_store`（F-04 语义）**

```rust
/// `list_hive_env_vars` 的核心逻辑，存储可注入（测试用内存实现）。
///
/// F-04：同一 hive 内任一枚举 / 读取 / 解码失败即返回 `Err`。
/// 「成功但不完整」的列表是错误的成功语义，用户无法区分「变量不存在」
/// 与「枚举/读取失败」。
fn list_env_vars_in_store(hive: EnvHive, store: &dyn EnvHiveStore) -> Result<Vec<EnvVarMeta>, String> {
    let mut metas = Vec::new();

    // 写权限探测按 hive 只做一次（端口在 open 时已探测，避免逐变量重复探测）。
    let writable = store.writable();

    for name in store.enum_names()? {
        // 保留变量（Path）由专用通路拥有，通用通路完全不展示
        if is_reserved(&name) {
            continue;
        }

        let raw = store.get_raw(&name)?;
        let kind = EnvValueKind::from_reg_type(raw.vtype.clone());
        let sensitive = is_sensitive(&name);

        let value = match kind {
            EnvValueKind::Unsupported => String::new(),
            _ => String::from_reg_value(&raw)
                .map_err(|e| format!("无法解码环境变量 {}: {}", name, e))?,
        };

        let preview = if sensitive || !kind.is_writable() {
            None
        } else {
            sanitize_preview(&value)
        };

        let (can_edit, can_delete) = capabilities_for_with(writable, &name, kind);

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
```

- [ ] **Step 3: 更新 `list_all_env_vars` 的文档注释（F-05 的措辞部分，本计划只改注释）**

```rust
/// 一次读取两个 hive 的变量元数据（列表唯一入口）。
///
/// 两个 hive 是**先后两次独立读取**，没有跨键事务，因此返回的是
/// 「两个接近时刻的快照」，不是原子一致快照。外部进程可能在两次读取
/// 之间修改任一侧。若需强一致，应在单 hive 维度用 revision 做提交检查。
pub fn list_all_env_vars() -> Result<EnvVarSnapshot, String> {
    Ok(EnvVarSnapshot {
        system: list_hive_env_vars(EnvHive::System)?,
        user: list_hive_env_vars(EnvHive::User)?,
    })
}
```

- [ ] **Step 4: 编译**

Run: `cargo check -p path-editor-core`

Expected: FAIL —— 只剩测试模块引用旧符号；Task 6 处理。

---

### Task 6: 测试改用 `MemoryHive`，删除写 HKCU 的测试脚手架

**Files:**

- Modify: `core/src/registry.rs`（`env_var_tests` 模块，730-1101）

**目标**：`env_var_tests` 不再用 `TempRegistryKey` 建真实 HKCU 键；改用 `MemoryHive`。`issue26_tests`（627-728）里那个 `#[ignore]` 的真实写入测试**保持不动**（它已 `#[ignore]`，不参与默认测试）。

- [ ] **Step 1: 删除 `env_var_tests` 里的 `TempRegistryKey` 与 `seed`**

删除 `registry.rs:737-783` 的 `TempRegistryKey`（struct + `new` + `key` + `Drop`）与 `seed` 函数。

保留 `TEST_PATH_SUBKEY` 常量？它现在只被 `list_test_helper_uses_isolated_prefix`（1090-1093）使用——那个测试是「防御性回归：测试键路径与隔离父键一致」，端口化后已无隔离键，**一并删除该测试与常量**。

- [ ] **Step 2: 改写 `use` 与每个测试**

`env_var_tests` 模块头改为：

```rust
#[cfg(test)]
mod env_var_tests {
    use super::*;
    use crate::env_var::{EnvHive, EnvValueKind};
    use crate::reg_store::memory::MemoryHive;
    use winreg::enums::{REG_DWORD, REG_EXPAND_SZ, REG_SZ};
    use winreg::types::FromRegValue;
```

各测试改写（保留原意，只换存储）：

```rust
    #[test]
    fn read_env_var_returns_real_type_and_value() {
        let hive = MemoryHive::new(true);
        hive.seed("JAVA_HOME", "C:\\Java", REG_EXPAND_SZ);

        let (vtype, value) = read_env_var(&hive, "JAVA_HOME").expect("读取失败");
        assert_eq!(vtype, REG_EXPAND_SZ);
        assert_eq!(value, "C:\\Java");
    }

    #[test]
    fn write_env_var_preserves_expand_sz_type() {
        let hive = MemoryHive::new(true);
        hive.seed("GOPATH", "C:\\Old", REG_EXPAND_SZ);

        write_env_var(&hive, "GOPATH", "C:\\New", REG_EXPAND_SZ).expect("写入失败");

        let raw = hive.get_raw("GOPATH").expect("读取失败");
        assert_eq!(raw.vtype, REG_EXPAND_SZ);
        assert_eq!(String::from_reg_value(&raw).unwrap(), "C:\\New");
    }

    #[test]
    fn write_env_var_preserves_sz_type() {
        let hive = MemoryHive::new(true);
        hive.seed("JAVA_HOME", "C:\\Old", REG_SZ);

        write_env_var(&hive, "JAVA_HOME", "C:\\New", REG_SZ).expect("写入失败");

        let raw = hive.get_raw("JAVA_HOME").expect("读取失败");
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
        assert!(is_reserved("Path"));
        assert!(is_reserved("path"));
        let hive = MemoryHive::new(true);
        hive.seed("Path", "C:\\Windows", REG_EXPAND_SZ);
        assert!(read_env_var(&hive, "Path").is_ok());
    }

    #[test]
    fn revision_detects_external_change() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let revision = crate::env_var::revision_of("MY_VAR", vtype, &value);

        hive.seed("MY_VAR", "changed-by-other-process", REG_SZ);

        let (vtype2, value2) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let current = crate::env_var::revision_of("MY_VAR", vtype2, &value2);
        assert_ne!(revision, current, "外部修改后 revision 必须不同");
    }

    #[test]
    fn unsupported_type_is_not_writable() {
        let hive = MemoryHive::new(true);
        hive.seed_raw(
            "MY_DWORD",
            winreg::RegValue {
                bytes: vec![1, 0, 0, 0],
                vtype: REG_DWORD,
            },
        );

        assert!(read_env_var(&hive, "MY_DWORD").is_err());

        let stored = hive.get_raw("MY_DWORD").expect("读取原始值失败");
        let kind = EnvValueKind::from_reg_type(stored.vtype);
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
```

公开 API 行为测试（原 896-1086 段）改为：

```rust
    #[test]
    fn list_path_is_filtered_and_name_case_preserved() {
        let hive = MemoryHive::new(true);
        hive.seed("Path", "C:\\Windows", REG_EXPAND_SZ);
        hive.seed("path", "C:\\ShouldNotAppear", REG_EXPAND_SZ);
        hive.seed("MyApp_Home", "C:\\MyApp", REG_SZ);
        hive.seed("another_var", "C:\\another", REG_EXPAND_SZ);

        let metas = list_env_vars_in_store(EnvHive::User, &hive).expect("列表失败");

        let names: Vec<&str> = metas.iter().map(|m| m.name.as_str()).collect();
        assert!(!names.iter().any(|n| n.eq_ignore_ascii_case("path")), "Path 必须被过滤");
        assert!(names.contains(&"MyApp_Home"));
        assert!(names.contains(&"another_var"));
        assert!(metas.iter().all(|m| !m.preview.as_deref().unwrap_or("").contains("ShouldNotAppear")));
    }

    #[test]
    fn update_env_var_rejects_revision_mismatch_and_keeps_value() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let stale_revision = revision_of("MY_VAR", vtype, &value);

        hive.seed("MY_VAR", "changed-by-other", REG_SZ);

        let result = update_env_var_in_store(&hive, "MY_VAR", "mine", &stale_revision);
        assert!(result.is_err(), "revision 不匹配必须 Err");
        assert_eq!(result.unwrap_err(), ERR_CONFLICT);

        let raw = hive.get_raw("MY_VAR").expect("读取失败");
        assert_eq!(String::from_reg_value(&raw).unwrap(), "changed-by-other");
    }

    #[test]
    fn update_env_var_rejects_dword_and_keeps_value() {
        let hive = MemoryHive::new(true);
        hive.seed_raw(
            "MY_DWORD",
            winreg::RegValue { bytes: vec![7, 0, 0, 0], vtype: REG_DWORD },
        );
        let stored = hive.get_raw("MY_DWORD").expect("读取失败");
        let revision = revision_of("MY_DWORD", stored.vtype, "");

        let result = update_env_var_in_store(&hive, "MY_DWORD", "1", &revision);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("类型不受支持"));

        let after = hive.get_raw("MY_DWORD").expect("读取失败");
        assert_eq!(after.vtype, REG_DWORD);
        assert_eq!(after.bytes, vec![7, 0, 0, 0]);
    }

    #[test]
    fn update_env_var_succeeds_when_revision_matches() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let revision = revision_of("MY_VAR", vtype, &value);

        update_env_var_in_store(&hive, "MY_VAR", "updated", &revision)
            .expect("revision 匹配必须成功");

        let raw = hive.get_raw("MY_VAR").expect("读取失败");
        assert_eq!(String::from_reg_value(&raw).unwrap(), "updated");
        assert_eq!(raw.vtype, REG_SZ, "成功写入也必须保持原类型");
    }

    #[test]
    fn delete_env_var_succeeds_when_revision_matches() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let revision = revision_of("MY_VAR", vtype, &value);

        delete_env_var_in_store(&hive, "MY_VAR", &revision).expect("revision 匹配必须成功");

        assert!(!hive.contains("MY_VAR"), "成功删除后变量必须消失");
    }

    #[test]
    fn create_env_var_rejects_protected_and_duplicate() {
        let hive = MemoryHive::new(true);
        hive.seed("Existing", "already-here", REG_SZ);

        let protected =
            create_env_var_in_store(&hive, "windir", "C:\\evil", EnvValueKind::String);
        assert!(protected.is_err());
        assert!(protected.unwrap_err().contains("系统内置"));

        let duplicate = create_env_var_in_store(&hive, "EXISTING", "dup", EnvValueKind::String);
        assert!(duplicate.is_err());
        assert!(duplicate.unwrap_err().contains("已存在"));

        let raw = hive.get_raw("Existing").expect("读取失败");
        assert_eq!(String::from_reg_value(&raw).unwrap(), "already-here");
    }

    #[test]
    fn reveal_env_var_returns_plaintext_and_sensitive_preview_is_none() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_PLAIN", "C:\\plain value", REG_SZ);
        hive.seed("MY_API_TOKEN", "super-secret-plaintext", REG_SZ);

        let revealed = reveal_env_var_in_store(&hive, "MY_API_TOKEN").expect("reveal 失败");
        assert_eq!(revealed, "super-secret-plaintext");

        let metas = list_env_vars_in_store(EnvHive::User, &hive).expect("列表失败");
        let token_meta = metas.iter().find(|m| m.name == "MY_API_TOKEN").expect("敏感变量应在列表");
        assert!(token_meta.sensitive);
        assert_eq!(token_meta.preview, None);

        let plain_meta = metas.iter().find(|m| m.name == "MY_PLAIN").expect("普通变量应在列表");
        assert_eq!(plain_meta.preview.as_deref(), Some("C:\\plain value"));
    }

    #[test]
    fn delete_env_var_rejects_revision_mismatch_and_keeps_var() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let stale_revision = revision_of("MY_VAR", vtype, &value);

        hive.seed("MY_VAR", "changed-by-other", REG_SZ);

        let result = delete_env_var_in_store(&hive, "MY_VAR", &stale_revision);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ERR_CONFLICT);
        assert!(hive.contains("MY_VAR"), "冲突时不得删除变量");
    }
```

- [ ] **Step 3: 新增 F-04 的故障注入测试**

```rust
    #[test]
    fn list_fails_when_enum_fails() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "v", REG_SZ);
        hive.fail_enum = true;

        assert!(list_env_vars_in_store(EnvHive::User, &hive).is_err(), "枚举失败必须让整个 hive 报错");
    }

    #[test]
    fn list_fails_when_single_read_fails() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "v", REG_SZ);
        hive.seed("BAD_VAR", "v", REG_SZ);
        hive.fail_get = Some("BAD_VAR".into());

        assert!(list_env_vars_in_store(EnvHive::User, &hive).is_err(), "单值读取失败必须让整个 hive 报错");
    }

    #[test]
    fn list_fails_when_decode_fails() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "v", REG_SZ);
        // 奇数字节不是合法 UTF-16，解码必然失败
        hive.seed_raw(
            "BAD_STR",
            winreg::RegValue { bytes: vec![0x41], vtype: REG_SZ },
        );

        assert!(list_env_vars_in_store(EnvHive::User, &hive).is_err(), "解码失败必须让整个 hive 报错");
    }
```

- [ ] **Step 4: 运行全部 core 测试**

Run: `cargo test -p path-editor-core`

Expected: PASS，且**测试过程不写真实 HKCU**（除 `#[ignore]` 的那个）。

- [ ] **Step 5: 验证没有遗留的 HKCU 写测试**

Run: `grep -n "TempRegistryKey\|delete_subkey_all\|create_subkey" core/src/registry.rs`

Expected: 仅 `issue26_tests` 内的 `TempRegistryKey`（627-728）与那一个 `#[ignore]` 测试仍在；`env_var_tests` 已无匹配。

- [ ] **Step 6: 质量门 + 提交**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add core/src/registry.rs
git commit -m "refactor(core): 环境变量读写改走 EnvHiveStore 端口，列表失败不再静默"
```

---

### Task 7: 全工作区质量门与去 HKCU 验证

**Files:** 无（验证任务）

- [ ] **Step 1: 全量质量门**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p patheditor-cli --bins
npm test
```

Expected: 全绿；`cargo test --workspace` 在**不写 HKCU 的前提下**通过。

- [ ] **Step 2: 确认 gui / cli 调用点未受影响**

Run: `grep -rn "update_env_var\|create_env_var\|delete_env_var\|reveal_env_var\|list_all_env_vars" gui/src cli/src`

Expected: 全部调用的是公开 API（签名未变），无编译错误。

- [ ] **Step 3: 记录偏差**

在 spec 的 `## Execution Notes`（若不存在则新建）如实记录执行中与计划不符之处。

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "chore(core): Wave 0 质量门收口"
```

---

## 开发窗口异议与裁断（2026-09-18）

开发窗口逐行核对后提出 5 条异议，审核窗口裁断如下（结论已并入正文）：

| #   | 异议                                                                                                                            | 裁断                 | 处置                                                                                                                                                                                        |
| --- | ------------------------------------------------------------------------------------------------------------------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| O-1 | Task 1 冒烟测试断言「用户 hive 含 Path」，在无用户 Path 的机器误红                                                              | **成立**             | 改为只断言「只读打开 + `enum_names()` 可用」，不绑定值名（Task 1 Step 3）                                                                                                                   |
| O-2 | `create_env_var_in_store` 用 `enum_names()?` 传播枚举错误，超出 F-04（只针对 list）范围                                         | **成立**             | 保留传播（失败响亮优于冒险覆盖），但在 doc 注释标注为**有意的语义变更**并给出回退写法（Task 4 Step 3）                                                                                      |
| O-3 | `delete` 里 `Unsupported` 检查不可达（`read_env_var` 已提前返回）                                                               | **成立（非阻塞）**   | 保留但加注释说明不可达，行为与现状一致（Task 4 Step 4）                                                                                                                                     |
| O-4 | `update`/`delete` 缺成功路径测试                                                                                                | **成立**             | 各补一条 revision 匹配 → 成功写入/删除的测试（Task 6 Step 2）                                                                                                                               |
| O-5 | 需确认 `winreg::RegValue: Clone`                                                                                                | **成立且比预期严重** | **实测 winreg 0.52.0 的 `RegValue` 只 derive `PartialEq`、无 `Clone`**（`winreg-0.52.0/src/reg_value.rs:11`）——原计划的 `.clone()` 会编译失败。已改为按字段复制的 `dup_reg_value`（Task 2） |
| O-6 | O-5 的修复本身漏了 `.clone()`：`dup_reg_value` 里 `vtype: raw.vtype`，而 `RegType` 非 `Copy` → E0507 cannot move out            | **成立**             | `vtype: raw.vtype.clone()`（Task 2）                                                                                                                                                        |
| O-7 | Task 3 `read_env_var` 删掉了原实现的 `raw.vtype.clone()`（`registry.rs:245`），`RegType` 非 `Copy` → 部分移出 + `&raw` 的 E0382 | **成立**             | 恢复 `EnvValueKind::from_reg_type(raw.vtype.clone())`（Task 3 Step 1）                                                                                                                      |

O-5/O-6/O-7 是同一条线：O-5 的「验证项」实测**不成立**（`RegValue` 无 `Clone`），开发窗口顺着 `RegType` 的 derive（`winreg-0.52.0/src/enums.rs:20`，只有 `Debug/Clone/PartialEq`、**无 `Copy`**）追出我修复本身的两处漏 `.clone()`。若未核对，Wave 0 会在 Task 2（`dup_reg_value`）与 Task 3（`read_env_var`）两处编译失败。

**O-2 已由开发窗口拍板保留**：`flatten()` 吞枚举错误 → 枚举失败被当「同名不存在」→ 继续写入，是真实的覆盖隐患；计划本就为改这一行（`&RegKey` → `&dyn EnvHiveStore`），在同一处切口收紧是恰当的。`unwrap_or_default()` 等于重新引入一条静默失败路径，与 Wave 0 的立意相悖。开发回执里将把它列为**规格外补全**（CLAUDE.md 要求）。

**开发窗口核对的其他结论（确认无误，不改）**：`hive_location@227`、`env_key@216`、`read_env_var@241`、`write_env_var@254`、`list_hive_env_vars@294`、`list_env_vars_in_key@301`、`#[ignore]@684`、`env_var_tests@730-1101`、`TEST_PATH_SUBKEY@899`、`list_test_helper_uses_isolated_prefix@1090-1093` 全部属实；内部函数引用仅限 `registry.rs` 内部，Wave 0 改造自包含，不波及 `gui`/`cli`；`broadcast_env_change` 确在 `_in_key` 内（`451`/`510`/`556`），上移到公开包装函数是真实且正确的改动。

---

## Self-Review

**1. Spec coverage**

| Spec 条目                                     | 对应任务                                             |
| --------------------------------------------- | ---------------------------------------------------- |
| F-09 端口 trait + 生产/内存 adapter           | Task 1、Task 2                                       |
| F-09 测试不再写真实 HKCU                      | Task 6、Task 7                                       |
| F-04 枚举/读取/解码失败不静默、整个 hive 报错 | Task 5（实现）、Task 6（测试）                       |
| F-09 端口作为 F-04 故障注入与 F-07 拆分的地基 | Task 5 + Task 6 的注入测试                           |
| F-05 快照契约注释措辞                         | Task 5 Step 3（仅注释，字段与 capturedAt 留 Wave 1） |

**2. Placeholder scan**

无 TBD / TODO / 「适当处理」；每个代码步骤给出完整函数体，每个测试步骤给出完整测试函数。

**3. Type consistency**

| 符号                                                    | 定义   | 使用               | 一致 |
| ------------------------------------------------------- | ------ | ------------------ | ---- |
| `EnvHiveStore`（5 方法）                                | Task 1 | Task 2、3、4、5、6 | ✓    |
| `WinregHive::open(EnvHive, bool)`                       | Task 1 | Task 4、5          | ✓    |
| `MemoryHive::new/seed/seed_raw/contains`                | Task 2 | Task 6             | ✓    |
| `read_env_var(&dyn EnvHiveStore, &str)`                 | Task 3 | Task 4、5、6       | ✓    |
| `write_env_var(&dyn EnvHiveStore, &str, &str, RegType)` | Task 3 | Task 4、6          | ✓    |
| `*_in_store` 四个函数签名                               | Task 4 | Task 6             | ✓    |
| `list_env_vars_in_store(EnvHive, &dyn EnvHiveStore)`    | Task 5 | Task 6             | ✓    |

## Execution Notes

| 计划原文                                                                  | 实际                                                                                                                                                    | 处理                                                                          |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Task 3-6 各设中间态编译步骤（brief「Expected: FAIL」）                    | 合并为单次提交 `65e1d13`，仅 T6 末尾要求编译全绿                                                                                                        | 控制者裁决 P-2/P-3（`new_notebook.py` 不可满足，以编译单元为提交单元）        |
| Task 4 Step 6「删除 env_key」                                             | 延后到 T5 list 通路改走端口之后                                                                                                                         | 控制者裁决 P-1（list_hive_env_vars 仍在使用）                                 |
| Task 6 测试 `list_fails_when_decode_fails` 的前提「奇数字节解码必然失败」 | 实测不成立：winreg 0.52 的 `String::from_reg_value` 用 `from_utf16_lossy`（winreg-0.52.0/src/types.rs:38），解码失败分支对字符串类型当前不可达          | 测试保留为休眠用例（`#[ignore]` + 根因注释），枚举/读取失败有活测试           |
| Task 2 brief 代码可按原样编译                                             | 3 处编译修复：`let hive`→`let mut hive`（E0594）；`ToRegValue` import 移入 memory 模块（生产构建 unused import）；`seed_raw` 曾加 `#[allow(dead_code)]` | `#[allow(dead_code)]` 已在 Task 7 移除（方法现被 3 处测试使用），其余两处保留 |
| Task 1 冒烟测试断言绑定具体值名                                           | 按评审裁决 O-1 改为「只读打开 + enum_names() 可用」                                                                                                     | 不绑定值名                                                                    |
| Task 7 Step 3 回填 Execution Notes                                        | 延迟到最终复审后                                                                                                                                        | 本条即该回填                                                                  |
| F-04 的 log::warn! 删除                                                   | 最终复审指出 spec 原文要求「warning 保留，但不再是唯一反馈」                                                                                            | 已在 list 错误传播处恢复（本修复波次）                                        |

以上裁决记录同时存在于 SDD ledger（不入库），本表是入库版本。
