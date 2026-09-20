use crate::env_var::EnvHive;
use crate::error::{CoreError, ErrorCode};

/// 修订冲突统一错误消息。前端按 `[E_CONFLICT]` 前缀匹配，
/// 中文正文仅供人工阅读，修改时必须保留前缀原样。
pub(crate) const ERR_CONFLICT: &str = "[E_CONFLICT] 变量已被其他进程修改，请重新加载";

/// 冲突错误的完整文本。CLI / GUI 凭此前缀判定冲突，避免二次硬编码文案。
pub fn conflict_message() -> String {
    ERR_CONFLICT.to_string()
}

/// 构造 revision 冲突的结构化错误。新增判定一律走 `code == ErrorCode::Conflict`；
/// message 保留 `[E_CONFLICT]` 前缀仅为过渡期兼容（GUI/CLI 尚未完成 code 判定迁移）。
pub(crate) fn conflict_error(operation: &str, hive: EnvHive, name: &str) -> CoreError {
    CoreError::new(ErrorCode::Conflict, operation, ERR_CONFLICT).with_target(hive, name)
}
