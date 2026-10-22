use crate::env_var::EnvHive;
use winreg::enums::*;
use winreg::RegKey;

use super::path::USER_REG_PATH;

/// 探测当前用户是否有权写入 HKCU 的 PATH 注册表项。
pub fn can_write_user() -> bool {
    let key = RegKey::predef(HKEY_CURRENT_USER);
    key.open_subkey_with_flags(USER_REG_PATH, KEY_WRITE).is_ok()
}

/// 返回指定 hive 的 (根键, 子路径, 显示标签)。
pub(crate) fn hive_location(hive: EnvHive) -> (winreg::HKEY, &'static str, &'static str) {
    match hive {
        EnvHive::System => (HKEY_LOCAL_MACHINE, super::path::SYS_REG_PATH, "系统"),
        EnvHive::User => (HKEY_CURRENT_USER, USER_REG_PATH, "用户"),
    }
}
