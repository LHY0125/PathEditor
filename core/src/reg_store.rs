//! 环境变量 hive 的存储端口。
//!
//! 生产环境用 winreg 实现（[`WinregHive`]），测试用内存实现
//! （`memory::MemoryHive`），使环境变量读写测试不必写真实 HKCU。
//! 错误文案在此层统一格式化，调用方直接透传。

use winreg::enums::*;
use winreg::RegValue;

use crate::env_var::EnvHive;
use crate::error::{CoreError, ErrorCode};

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

    /// 读取值的原始字节与真实类型，并携带结构化错误分类（#34）。
    ///
    /// 与 [`get_raw`] 的**唯一区别**是错误类型：本方法把 winreg 的
    /// `io::ErrorKind::NotFound` 映射为 [`ErrorCode::NotFound`]，其余为 [`ErrorCode::Io`]。
    /// `get_raw` 保持 `Result<_, String>` 不动（W2-N2 端口边界裁断，本文件 `WinregHive::open`
    /// 的文档注释）。
    ///
    /// 默认实现退回 `get_raw`（给不关心分类的实现用）；`WinregHive` 与 `MemoryHive` 各自覆写。
    /// 判定只认 `code`，**不得**匹配 message 文本（Global Constraints）。
    fn get_raw_kind(&self, name: &str) -> Result<RegValue, CoreError> {
        self.get_raw(name)
            .map_err(|m| CoreError::new(ErrorCode::Io, "read_env_var", m))
    }

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
    ///
    /// W2-N2（2026-09-20 裁断）：本方法是固有方法、不在 [`EnvHiveStore`] trait
    /// 上，可单独迁移到 `CoreError`。winreg 错误按 `io::ErrorKind` 诚实分类：
    /// `PermissionDenied` → [`ErrorCode::PermissionDenied`]，其余 → [`ErrorCode::Io`]。
    /// trait 的其他方法保持 `Result<_, String>`（Wave 0 端口边界不动）。
    pub fn open(hive: EnvHive, write: bool) -> Result<Self, CoreError> {
        let (root, sub_path, label) = crate::registry::hive_location(hive);
        let flags = if write {
            KEY_READ | KEY_WRITE
        } else {
            KEY_READ
        };
        let key = winreg::RegKey::predef(root)
            .open_subkey_with_flags(sub_path, flags)
            .map_err(|e| {
                let code = if e.kind() == std::io::ErrorKind::PermissionDenied {
                    ErrorCode::PermissionDenied
                } else {
                    ErrorCode::Io
                };
                CoreError::new(
                    code,
                    "open_env_key",
                    format!("无法打开{}环境变量注册表项: {}", label, e),
                )
                .with_hive(hive)
            })?;
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

    /// #34：与 [`get_raw`](Self::get_raw) 同源，但按 `io::ErrorKind` 分类错误码。
    ///
    /// 非 `NotFound` 分支的 message 与 `get_raw` **逐字节相同**（不变量 2）。
    fn get_raw_kind(&self, name: &str) -> Result<RegValue, CoreError> {
        self.key.get_raw_value(name).map_err(|e| {
            let code = if e.kind() == std::io::ErrorKind::NotFound {
                ErrorCode::NotFound
            } else {
                ErrorCode::Io
            };
            CoreError::new(
                code,
                "read_env_var",
                format!("无法读取环境变量 {}: {}", name, e),
            )
        })
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

/// 供单元测试使用的内存实现；不触碰真实注册表。
#[cfg(test)]
pub(crate) mod memory {
    use super::*;
    use std::cell::RefCell;
    use winreg::types::ToRegValue;

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

        /// #34：与 [`get_raw`](Self::get_raw) 同源，但把「找不到」分类为 `NotFound`、
        /// 注入失败分类为 `Io`。message 文本与 `get_raw` 逐字节相同（不变量 2）。
        fn get_raw_kind(&self, name: &str) -> Result<RegValue, CoreError> {
            if self
                .fail_get
                .as_deref()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
            {
                return Err(CoreError::new(
                    ErrorCode::Io,
                    "read_env_var",
                    format!("无法读取环境变量 {}: 注入的读取失败", name),
                ));
            }
            self.values
                .borrow()
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, v)| dup_reg_value(v))
                .ok_or_else(|| {
                    CoreError::new(
                        ErrorCode::NotFound,
                        "read_env_var",
                        format!("无法读取环境变量 {}: 找不到", name),
                    )
                })
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
        // brief 原文为 `let hive`，故障注入需要可变绑定，此处加 `mut`。
        let mut hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "v", REG_SZ);

        hive.fail_enum = true;
        assert!(hive.enum_names().is_err());
        hive.fail_enum = false;

        hive.fail_get = Some("my_var".into());
        assert!(hive.get_raw("MY_VAR").is_err());
        assert!(hive
            .get_raw("MY_VAR")
            .unwrap_err()
            .contains("注入的读取失败"));
    }

    /// #34：不存在的变量必须产出 `NotFound` 码，而不是笼统的 `Io`。
    ///
    /// 这是 `env get` 能给出「该变量不存在」而非「os error 2」的前提。
    #[test]
    fn get_raw_kind_missing_name_yields_not_found_code() {
        let hive = MemoryHive::new(true);
        hive.seed("PRESENT", "v", REG_SZ);

        let err = hive.get_raw_kind("ABSENT").expect_err("不存在应返回 Err");
        assert_eq!(
            err.code,
            crate::error::ErrorCode::NotFound,
            "不存在的变量必须是 NotFound 码，实际: {:?}",
            err.code
        );
    }

    /// #34 不变量 2：其它读取错误的 message 文本必须保持原样（不因新增方法而改变用户可见文案）。
    ///
    /// N1 裁断后：`fail_get` 注入场景下 `get_raw_kind` 默认实现直接复用 `get_raw`
    /// 的错误字符串，故 message 与 `get_raw` 逐字节相同。
    #[test]
    fn get_raw_kind_error_message_text_matches_get_raw() {
        // 既有测试 `memory_hive_injects_failures` 直接 `hive.fail_get = Some(...)`；
        // 该字段在测试模块内可见，本用例沿用同一手法（plan 初稿的 seed_fail_get 假设已删）。
        // 故障注入需要可变绑定（同既有测试的 `mut` 说明）。
        let mut hive = MemoryHive::new(true);
        hive.fail_get = Some("BROKEN".to_string());

        let err = hive.get_raw_kind("BROKEN").expect_err("注入失败应返回 Err");
        assert_eq!(
            err.message, "无法读取环境变量 BROKEN: 注入的读取失败",
            "message 文本不得因 #34 而改变"
        );
    }
}
