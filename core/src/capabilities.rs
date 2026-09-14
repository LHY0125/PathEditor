use crate::registry;
use crate::system;
use serde::Serialize;

/// 按注册表 hive 拆分读写能力，避免用单一的“管理员”状态锁死用户 PATH。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathCapabilities {
    pub can_read_system: bool,
    pub can_write_system: bool,
    pub can_read_user: bool,
    pub can_write_user: bool,
}

/// 探测当前进程对系统/用户注册表的实际读写能力。
pub fn detect() -> PathCapabilities {
    PathCapabilities {
        can_read_system: registry::load_system_paths().is_ok(),
        can_write_system: system::check_admin(),
        can_read_user: registry::load_user_paths().is_ok(),
        can_write_user: registry::can_write_user(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn serializes_camel_case_contract() {
        let actual = serde_json::to_value(PathCapabilities {
            can_read_system: true,
            can_write_system: false,
            can_read_user: true,
            can_write_user: true,
        })
        .expect("序列化能力对象失败");
        let expected: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/path-capabilities.json"))
                .expect("解析共享契约 fixture 失败");

        assert_eq!(actual, expected);
        assert!(actual.get("canWriteSystem").is_some());
        assert!(actual.get("can_write_system").is_none());
    }

    #[test]
    fn detected_capabilities_use_camel_case_keys() {
        let detected = serde_json::to_value(detect()).expect("序列化真实探测结果失败");

        for key in [
            "canReadSystem",
            "canWriteSystem",
            "canReadUser",
            "canWriteUser",
        ] {
            assert!(detected.get(key).is_some(), "缺少能力字段: {key}");
        }
        assert!(detected.get("can_write_system").is_none());
    }
}
