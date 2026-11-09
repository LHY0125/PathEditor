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
use crate::domain::error::CoreError;
use crate::domain::path_entry::PathEntry;
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
    /// 环境变量新增（仅变量名；同一名字在不同 hive 可能出现两次）
    pub env_added: Vec<String>,
    /// 环境变量删除（仅变量名）
    pub env_removed: Vec<String>,
    /// 环境变量修改（仅变量名）
    pub env_modified: Vec<String>,
    /// 带 hive 的明细；UI 用它区分 system/user，避免同名变量混淆。
    pub env_details: Vec<EnvNameChange>,
}

/// 一条环境变量差异（不含值）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvNameChange {
    /// `system` / `user`
    pub hive: String,
    /// 变量名
    pub name: String,
    /// `added` / `removed` / `modified`
    pub change: String,
}

/// 比较两个快照（按 uid）。
///
/// 敏感值在两侧都以 `None` 出现（`reveal=false`）；`modified` 因此按
/// `revision` 判定——它覆盖类型与值，且不含明文。
pub fn compare(db: &Db, left_uid: &str, right_uid: &str) -> Result<CompareResult, CoreError> {
    let left = history::get_snapshot(db, left_uid, false)?;
    let right = history::get_snapshot(db, right_uid, false)?;
    let (env_added, env_removed, env_modified, env_details) = compare_env(&left.env, &right.env);
    Ok(CompareResult {
        system: compare_paths(&left.system, &right.system),
        user: compare_paths(&left.user, &right.user),
        env_added,
        env_removed,
        env_modified,
        env_details,
    })
}

/// 纯函数：比较两个快照的环境变量。
///
/// 键是 `(hive, 小写变量名)`；`modified` 以 `revision` 为准（覆盖类型与值），
/// 因此敏感变量在掩码状态下也能被正确判定。
fn compare_env(
    left: &[super::history::EnvValueRow],
    right: &[super::history::EnvValueRow],
) -> (Vec<String>, Vec<String>, Vec<String>, Vec<EnvNameChange>) {
    use std::collections::HashMap;

    let key = |row: &super::history::EnvValueRow| (row.hive.clone(), row.name.to_lowercase());
    let left_map: HashMap<(String, String), &super::history::EnvValueRow> =
        left.iter().map(|row| (key(row), row)).collect();
    let right_map: HashMap<(String, String), &super::history::EnvValueRow> =
        right.iter().map(|row| (key(row), row)).collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    let mut details = Vec::new();

    for (k, row) in &right_map {
        match left_map.get(k) {
            None => {
                added.push(row.name.clone());
                details.push(EnvNameChange {
                    hive: row.hive.clone(),
                    name: row.name.clone(),
                    change: "added".into(),
                });
            }
            Some(old) if old.revision != row.revision => {
                modified.push(row.name.clone());
                details.push(EnvNameChange {
                    hive: row.hive.clone(),
                    name: row.name.clone(),
                    change: "modified".into(),
                });
            }
            Some(_) => {}
        }
    }
    for (k, row) in &left_map {
        if !right_map.contains_key(k) {
            removed.push(row.name.clone());
            details.push(EnvNameChange {
                hive: row.hive.clone(),
                name: row.name.clone(),
                change: "removed".into(),
            });
        }
    }

    added.sort();
    removed.sort();
    modified.sort();
    details.sort_by(|a, b| (&a.hive, &a.name, &a.change).cmp(&(&b.hive, &b.name, &b.change)));
    (added, removed, modified, details)
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

/// N 路对比：以第一个快照为基准，返回它与其余每个快照的差异。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiCompareEntry {
    /// 基准快照 uid
    pub base_uid: String,
    /// 被比较的快照 uid
    pub other_uid: String,
    /// 两者的差异
    pub diff: CompareResult,
}

/// 以 `uids[0]` 为基准做 N 路对比（至少 2 个 uid）。
pub fn compare_many(db: &Db, uids: &[String]) -> Result<Vec<MultiCompareEntry>, CoreError> {
    let Some((base, rest)) = uids.split_first() else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(rest.len());
    for other in rest {
        out.push(MultiCompareEntry {
            base_uid: base.clone(),
            other_uid: other.clone(),
            diff: compare(db, base, other)?,
        });
    }
    Ok(out)
}
