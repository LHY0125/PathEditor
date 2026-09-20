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
