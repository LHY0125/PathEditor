//! 检索 / 统计 / 对比 / 引用图 / 冲突历史的 Tauri 命令。
//!
//! 只做参数转换与错误呈现，业务规则全在 core。
//!
//! 降级语义：DB 不可用时 `DbState` 为 `None`，所有命令返回
//! `CoreError { code: database }`；注册表编辑命令不受影响。
//!
//! 分页契约：返回列表的命令都接受 `limit`，缺省 50、上限 500（与
//! `history_list_events` 一致）。搜索结果的上限是 200（由 `search::search`
//! 内部 clamp），因为搜索是「即时过滤」而非分页浏览。

use path_editor_core::db::{
    compare::{self, CompareResult},
    refs::{self, RefEdge},
    search::{self, SearchHit},
    stats::{self, Dashboard},
};
use path_editor_core::CoreError;
use tauri::State;

use super::db_state::{with_db, DbState};

/// 全文检索（FTS5）。
///
/// **空查询返回空数组**，不返回全量——搜索框为空时列出全部会让用户
/// 误以为「所有条目都匹配」。
///
/// 查询串由 core 侧做 FTS5 转义（见 `db::search::escape_query`），
/// 调用方直接传原始用户输入即可。
#[tauri::command]
pub fn search(
    query: String,
    limit: Option<u32>,
    state: State<'_, DbState>,
) -> Result<Vec<SearchHit>, CoreError> {
    with_db(&state, "search", |db| {
        search::search(db, &query, limit.unwrap_or(50))
    })
}

/// 统计面板数据（`days` 天的时间序列 + 当前状态 + 聚合榜）。
#[tauri::command]
pub fn stats_dashboard(
    days: Option<u32>,
    state: State<'_, DbState>,
) -> Result<Dashboard, CoreError> {
    with_db(&state, "stats_dashboard", |db| {
        stats::dashboard(db, days.unwrap_or(30))
    })
}

/// 比较两个快照（按 uid）。
///
/// 返回 `{ system: PathDiff, user: PathDiff, env_* }`——**双 hive 分开**。
/// env 三字段恒为空：**本版不支持环境变量对比**，UI 必须显式说明，
/// 不能让用户误以为「环境变量没有变化」。
#[tauri::command]
pub fn compare_snapshots(
    left_uid: String,
    right_uid: String,
    state: State<'_, DbState>,
) -> Result<CompareResult, CoreError> {
    with_db(&state, "compare_snapshots", |db| {
        compare::compare(db, &left_uid, &right_uid)
    })
}

/// 谁引用了 `name`（反向边）。
#[tauri::command]
pub fn refs_dependents(name: String, state: State<'_, DbState>) -> Result<Vec<RefEdge>, CoreError> {
    with_db(&state, "refs_dependents", |db| refs::dependents(db, &name))
}

/// `name` 传递依赖了谁（递归 CTE）。
#[tauri::command]
pub fn refs_dependencies(
    name: String,
    state: State<'_, DbState>,
) -> Result<Vec<RefEdge>, CoreError> {
    with_db(&state, "refs_dependencies", |db| {
        refs::dependencies(db, &name)
    })
}

/// 引用环。
#[tauri::command]
pub fn refs_cycles(state: State<'_, DbState>) -> Result<Vec<Vec<String>>, CoreError> {
    with_db(&state, "refs_cycles", refs::find_cycles)
}
