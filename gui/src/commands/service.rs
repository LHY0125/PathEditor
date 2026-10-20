//! 共享应用服务层（`core::service`）的 Tauri 命令封装。
//!
//! F-08（Wave 2 Task 5）：GUI 编排的注册表写入、广播、sidecar/pending
//! 事务语义下沉到 core；本文件只做参数转换与错误透传（自由文本过渡期）。
//!
//! v6 M1（V-2 整改）：写命令在 DB 可用时注入 [`DbHistory`]，**真正把事件落库**；
//! DB 不可用（降级模式）时回落 [`NoopHistory`] —— 编辑照常成功，只是不记历史（spec D16）。

use path_editor_core::db::history_sink::DbHistory;
use path_editor_core::history::{HistorySink, NoopHistory};
use path_editor_core::service::{self, ApplyOutcome, PathHiveWrite};
use path_editor_core::PathEntry;
use tauri::State;

use super::history::DbState;

/// 在 DB 可用时用真实 sink 执行 `f`，否则用 `NoopHistory`。
///
/// **为什么需要一个辅助函数**：`DbHistory<'a>` 借 `&'a mut Db`，而 `State` 的
/// `MutexGuard` 生命周期无法跨 `tauri::command` 的返回传播。把「加锁 → 构造 sink →
/// 调服务 → 释放锁」收在一处，调用点就不必各自处理生命周期。
///
/// 锁中毒时同样退化为 `NoopHistory`：**历史写不进去不该让注册表写入失败**
/// （spec D16 fail-open 的同一口径）。
fn with_history<T>(state: &State<'_, DbState>, f: impl FnOnce(&dyn HistorySink) -> T) -> T {
    let Ok(mut guard) = state.0.lock() else {
        log::warn!("历史库状态锁中毒，本次操作不记录历史");
        return f(&NoopHistory);
    };
    match guard.as_mut() {
        Some(db) => f(&DbHistory::new(db)),
        None => f(&NoopHistory),
    }
}

/// 同时写注册表与 sidecar（best-effort 多 hive，partial 语义按 hive 分字段）。
///
/// 每个 hive 传 [`PathHiveWrite`]（含 `expected` 用于乐观并发校验）。
#[tauri::command]
pub fn save_path_with_sidecar(
    system: Option<PathHiveWrite>,
    user: Option<PathHiveWrite>,
    state: State<'_, DbState>,
) -> Result<ApplyOutcome, String> {
    with_history(&state, |history| {
        service::save_path_with_sidecar(system, user, history)
    })
    .map_err(|e| e.message)
}

/// 把单 hive 的 PATH 写入注册表并落盘快照；未触及的另一 hive 为 `Skipped`。
#[tauri::command]
pub fn apply_path_snapshot(
    hive: path_editor_core::EnvHive,
    entries: Vec<PathEntry>,
    state: State<'_, DbState>,
) -> Result<ApplyOutcome, String> {
    with_history(&state, |history| {
        service::apply_path_snapshot(hive, entries, history)
    })
    .map_err(|e| e.message)
}

/// 补写上次未落盘的快照（sidecar-only，不触碰注册表）。
#[tauri::command]
pub fn retry_pending_path_state(state: State<'_, DbState>) -> Result<ApplyOutcome, String> {
    with_history(&state, |history| service::retry_pending_path_state(history))
        .map_err(|e| e.message)
}

/// 应用一个配置文件（多 hive best-effort）。
///
/// **J-5 整改**：成功后递增 `apply_count` / `last_applied_at`。记账失败只告警——
/// 注册表已写成功的事实优先于统计数字。
#[tauri::command]
pub fn apply_profile(name: String, state: State<'_, DbState>) -> Result<ApplyOutcome, String> {
    let outcome = with_history(&state, |history| service::apply_profile(&name, history))
        .map_err(|e| e.message)?;

    // 只在至少一个 hive 真的写入时记账
    let applied = matches!(outcome.system, service::HiveOutcome::Applied)
        || matches!(outcome.user, service::HiveOutcome::Applied);
    if applied {
        if let Ok(mut guard) = state.0.lock() {
            if let Some(db) = guard.as_mut() {
                // 按名字找到配置 uid 再记账（`apply_profile` 的入参是名字）
                match path_editor_core::db::profiles::list_profiles(db) {
                    Ok(list) => {
                        if let Some(p) = list.into_iter().find(|p| p.name == name) {
                            if let Err(e) =
                                path_editor_core::db::profiles::mark_profile_applied(db, &p.uid)
                            {
                                log::warn!("配置应用计数更新失败: {}", e.message);
                            }
                        }
                    }
                    Err(e) => log::warn!("配置列表读取失败，跳过应用计数: {}", e.message),
                }
            }
        }
    }

    Ok(outcome)
}
