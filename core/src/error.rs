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
    /// 数据库打开/查询/事务失败（v6）
    Database,
    /// 密钥/解密失败（v6）
    Crypto,
    /// 同步包/合并冲突（v6）：需要人工选择 local/remote
    Sync,
    /// 同步包格式错误（v6）：magic/版本/AEAD/未知实体类型/悬空引用
    ///
    /// **与 [`Self::Sync`] 分开的原因**：两者的脚本处理完全不同——
    /// 合并冲突应提示用户解决，格式错误应报告文件损坏、重试无用。
    /// 共用 exit 4 会让自动化无法区分。
    SyncFormat,
    /// v5 迁移与库版本问题（v6）
    Migration,
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

    /// 仅附加 hive 上下文（无单一目标名，如列表/打开键级别的错误）。
    pub fn with_hive(mut self, hive: EnvHive) -> Self {
        self.hive = Some(hive);
        self
    }

    /// CLI 退出码映射：冲突 3，加密/密钥 5，其余 1。
    ///
    /// spec §8：4 预留给同步/合并冲突（M3），当前 `Sync` 仍映射为 1。
    pub fn exit_code(&self) -> i32 {
        match self.code {
            ErrorCode::Conflict => 3,
            // 4 留给「需要人工介入的合并冲突」；包格式错误重试无用，归入一般错误
            ErrorCode::Sync => 4,
            ErrorCode::Crypto => 5,
            _ => 1,
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
