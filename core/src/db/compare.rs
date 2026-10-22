//! N 路快照对比。
//!
//! **敏感值恒掩码**：对比结果里 env 的 `value` 只对非敏感项填充。
//!
//! ## 为什么 system 与 user 必须分开返回
//!
//! 首版实现只比 `left.system`，用户 PATH 的变化**完全不可见**——不报错、
//! 测试也能过，只有「只有用户 PATH 变了」时会显示「没有差异」。
//! 这是「给出错误结论而不报错」的典型，比编译错误危险得多。
//!
//! ## 为什么同时比较 `enabled`（本实现超出计划的部分）
//!
//! `PathEntry { path, enabled }` 是跨层契约（`CLAUDE.md`：「导入、配置、注册表和
//! 撤销重做都必须保留 `enabled`」）。计划只比较路径串，会让「同一条路径从启用
//! 变为禁用」显示成「没有差异」——与是同一类遗漏。
//! 因此 `PathDiff` 增加 `toggled` 维度。

use super::{history, Db};
use crate::error::CoreError;
use crate::path_entry::PathEntry;
use std::collections::HashMap;

/// 顺序变化的一条。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderEntry {
    /// 路径
    pub path: String,
    /// 在 left 里的位置
    pub left_index: usize,
    /// 在 right 里的位置
    pub right_index: usize,
}

/// 启用状态变化的一条。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleEntry {
    /// 路径
    pub path: String,
    /// left 里是否启用
    pub left_enabled: bool,
    /// right 里是否启用
    pub right_enabled: bool,
}

/// 单侧（system 或 user）的 PATH 差异。
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathDiff {
    /// right 有、left 无
    pub added: Vec<PathEntry>,
    /// left 有、right 无
    pub removed: Vec<PathEntry>,
    /// 两边都有但位置变了
    pub reordered: Vec<ReorderEntry>,
    /// 两边都有、位置未变，但 `enabled` 不同
    pub toggled: Vec<ToggleEntry>,
}

impl PathDiff {
    /// 该侧是否完全无差异。UI 用它决定是否显示「无变化」。
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.reordered.is_empty()
            && self.toggled.is_empty()
    }
}

/// 对比结果。
///
/// system 与 user **必须分开返回**。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareResult {
    /// 系统 PATH 的差异
    pub system: PathDiff,
    /// 用户 PATH 的差异
    pub user: PathDiff,
    /// 环境变量对比尚未实现，字段恒为空。
    ///
    /// UI 必须显式显示「本版本不支持环境变量对比」，不能留白让用户以为
    /// 「环境变量没有变化」。
    pub env_added: Vec<String>,
    /// 同上
    pub env_removed: Vec<String>,
    /// 同上
    pub env_modified: Vec<String>,
}

/// 比较两个快照（按 uid）。
pub fn compare(db: &Db, left_uid: &str, right_uid: &str) -> Result<CompareResult, CoreError> {
    // 双 hive 都对比；环境变量对比涉及掩码语义，尚未实现（三个 env 字段恒为空）
    let left = history::get_snapshot(db, left_uid, false)?;
    let right = history::get_snapshot(db, right_uid, false)?;
    Ok(CompareResult {
        system: compare_paths(&left.system, &right.system),
        user: compare_paths(&left.user, &right.user),
        env_added: Vec::new(),
        env_removed: Vec::new(),
        env_modified: Vec::new(),
    })
}

/// 纯函数：对比单侧 PATH。
///
/// 以**小写去空白后的路径**为键（Windows 路径大小写不敏感）。
/// 同一份快照内若出现重复路径，`HashMap` 会保留**最后一个**——这与
/// 「后写入的生效」语义一致。
pub fn compare_paths(left: &[PathEntry], right: &[PathEntry]) -> PathDiff {
    let key = |p: &str| p.trim().to_lowercase();

    let left_map: HashMap<String, (usize, &PathEntry)> = left
        .iter()
        .enumerate()
        .map(|(i, e)| (key(&e.path), (i, e)))
        .collect();
    let right_map: HashMap<String, (usize, &PathEntry)> = right
        .iter()
        .enumerate()
        .map(|(i, e)| (key(&e.path), (i, e)))
        .collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut reordered = Vec::new();
    let mut toggled = Vec::new();

    for (k, (ri, e)) in &right_map {
        match left_map.get(k) {
            None => added.push((*e).clone()),
            Some((li, le)) => {
                if li != ri {
                    reordered.push(ReorderEntry {
                        path: e.path.clone(),
                        left_index: *li,
                        right_index: *ri,
                    });
                }
                // `enabled` 是跨层契约的一部分，不能只看路径串
                if le.enabled != e.enabled {
                    toggled.push(ToggleEntry {
                        path: e.path.clone(),
                        left_enabled: le.enabled,
                        right_enabled: e.enabled,
                    });
                }
            }
        }
    }
    for (k, (_, e)) in &left_map {
        if !right_map.contains_key(k) {
            removed.push((*e).clone());
        }
    }

    // 稳定排序，避免 HashMap 迭代顺序影响输出。
    // 用 `sort_by_cached_key` 而不是 `sort_by_key`：`key()` 返回 `String`，
    // 后者每次比较都会重新分配（N log N 次），前者每个元素只算一次。
    added.sort_by_cached_key(|a| key(&a.path));
    removed.sort_by_cached_key(|a| key(&a.path));
    reordered.sort_by_key(|r| r.right_index);
    toggled.sort_by_cached_key(|a| key(&a.path));

    PathDiff {
        added,
        removed,
        reordered,
        toggled,
    }
}
