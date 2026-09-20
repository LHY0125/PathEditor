use crate::env_var::EnvHive;
use crate::error::{CoreError, ErrorCode};

/// 修订冲突统一错误消息。判定按 `CoreError.code == Conflict`（Task 3 迁移后），
/// message 的 `[E_CONFLICT]` 前缀仅为过渡期展示文本，中文正文仅供人工阅读。
pub(crate) const ERR_CONFLICT: &str = "[E_CONFLICT] 变量已被其他进程修改，请重新加载";

/// 冲突错误的完整文本。判定一律走 `code == ErrorCode::Conflict`，不依赖前缀或正文。
pub fn conflict_message() -> String {
    ERR_CONFLICT.to_string()
}

/// 构造 revision 冲突的结构化错误。新增判定一律走 `code == ErrorCode::Conflict`；
/// message 保留 `[E_CONFLICT]` 前缀仅为过渡期展示文本（供旧文本路径），非判定机制。
pub(crate) fn conflict_error(operation: &str, hive: EnvHive, name: &str) -> CoreError {
    CoreError::new(ErrorCode::Conflict, operation, ERR_CONFLICT).with_target(hive, name)
}
