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
