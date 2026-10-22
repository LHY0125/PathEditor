use serde::{Deserialize, Serialize};

/// PATH 路径条目 — Rust 与 TypeScript 共享的领域对象。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathEntry {
    pub path: String,
    pub enabled: bool,
}

/// 系统 PATH 与用户 PATH 的一致快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PathSnapshot {
    #[serde(default)]
    pub system: Vec<PathEntry>,
    #[serde(default)]
    pub user: Vec<PathEntry>,
}
