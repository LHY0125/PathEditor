//! 通用环境变量契约与安全判定。
//!
//! 本模块的判定函数（保留 / 保护 / 敏感 / 权限）在 Rust 侧计算并下发，
//! 前端不重复实现 —— 判定规则是安全边界，单一实现处比双端各写一份可靠。

use serde::{Deserialize, Serialize};
use winreg::enums::{RegType, REG_EXPAND_SZ, REG_SZ};

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
    /// 从真实注册表类型映射。`RegType` 是 winreg 的枚举
    /// （`RegValue::vtype` 的类型），不是 `u32`。
    pub fn from_reg_type(vtype: RegType) -> Self {
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

/// FNV-1a 64 位散列。用于变更检测摘要，不用于防篡改。
///
/// 手写实现，零依赖 —— 摘要只需"任意改动都会变"，不需要密码学强度，
/// 因此不引入 sha2 / blake3。
fn fnv1a_64(input: &str) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET_BASIS;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// 计算并发校验摘要。任一组成部分变化都会改变结果。
///
/// 摘要基于 name + vtype + value 的散列，**不含明文** —— 该字段会经 IPC
/// 发到前端，因此绝不能携带敏感变量的值本身。散列使得任意改动（含等长
/// 改动）都能被检测到。
///
/// 分隔符用 \u{1} 而非 `|`，避免值本身含 `|` 时产生歧义。
///
/// 注意：这是变更检测摘要，不是密码学哈希。目的是发现"值被改动"，
/// 不用于防篡改。
pub fn revision_of(name: &str, vtype: RegType, value: &str) -> String {
    let material = format!(
        "{}\u{1}{}\u{1}{}",
        name.to_ascii_lowercase(),
        vtype as u32,
        value
    );
    format!("{:016x}", fnv1a_64(&material))
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
pub fn capabilities_for_with(writable: bool, name: &str, kind: EnvValueKind) -> (bool, bool) {
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
    fn revision_does_not_leak_value_and_detects_same_length_change() {
        let a = revision_of("MY_TOKEN", REG_SZ, "secret-A");
        let b = revision_of("MY_TOKEN", REG_SZ, "secret-B");
        // 等长不同值必须产生不同 revision
        assert_ne!(a, b);
        // 摘要中不得出现明文片段
        assert!(!a.contains("secret"));
        // 同值稳定
        assert_eq!(a, revision_of("MY_TOKEN", REG_SZ, "secret-A"));
    }

    #[test]
    fn revision_is_immune_to_separator_injection() {
        // \u{1} 分隔符注入：值内嵌分隔符不得与「名称拼接变体」产生同一 revision。
        // 注意 revision_of 会把名称转小写，拼接候选必须按小写形式构造，
        // 否则断言的是大小写差异而非分隔符歧义。
        assert_ne!(
            revision_of("A", REG_SZ, "B\u{1}C"),
            revision_of("A\u{1}B", REG_SZ, "C")
        );
        assert_ne!(
            revision_of("A", REG_SZ, "B\u{1}C"),
            revision_of("a\u{1}b", REG_SZ, "C")
        );
        // `|` 不是分隔符，含 | 的名称/值不得与拼接变体相撞
        assert_ne!(
            revision_of("A|B", REG_SZ, "C"),
            revision_of("A", REG_SZ, "B|C")
        );
        assert_ne!(
            revision_of("A|B", REG_SZ, "C"),
            revision_of("a|b", REG_SZ, "c")
        );
        // \u{1} 与 | 混合出现
        assert_ne!(
            revision_of("A", REG_SZ, "B|C\u{1}D"),
            revision_of("A\u{1}B|C", REG_SZ, "D")
        );
        // 分隔符候选与普通值之间也不得相撞
        assert_ne!(
            revision_of("A", REG_SZ, "B\u{1}C"),
            revision_of("A", REG_SZ, "BC")
        );
    }

    #[test]
    fn sanitize_preview_truncates_and_strips_control_chars() {
        assert_eq!(sanitize_preview("C:\\Java"), Some("C:\\Java".to_string()));
        assert_eq!(
            sanitize_preview("line1\nline2"),
            Some("line1line2".to_string())
        );
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
        let (can_edit, can_delete) =
            capabilities_for(EnvHive::User, "SOME_BINARY_VAR", EnvValueKind::Unsupported);
        assert!(!can_edit);
        assert!(!can_delete);
    }

    #[test]
    fn capabilities_deny_protected_even_when_writable() {
        let (can_edit, can_delete) =
            capabilities_for(EnvHive::User, "windir", EnvValueKind::String);
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
