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

/// 与 [`with_history`] 相同，但**先建一份「操作前」快照**并把 uid 绑定给事件（P0-2）。
///
/// 为什么快照记录的是**操作前**的状态：spec §6.1 规定「撤销/重做以『新快照 + 事件』
/// 表示」——恢复某个快照就等于回到那一刻。若快照记的是「即将写入的内容」，
/// 恢复它等于什么都不做，撤销语义不成立。
///
/// `snapshot_of` 返回 `(system, user)` 的当前快照；返回 `None` 表示拿不到
/// （例如注册表不可读），此时**照常执行操作**，只是事件不带快照 uid——
/// 历史记录的价值不该因为快照建不出来而丢失。
///
/// **事务边界**：`insert_snapshot` 自带事务并先提交，服务层内部的 `record`
/// 再开自己的事务。两者是**先后**关系而非嵌套（SQLite 不支持嵌套事务）。
pub(super) fn with_history_and_snapshot<T>(
    state: &State<'_, DbState>,
    snapshot_of: impl FnOnce() -> Option<(Vec<PathEntry>, Vec<PathEntry>)>,
    snapshot_of_after: impl FnOnce() -> Option<(Vec<PathEntry>, Vec<PathEntry>)>,
    f: impl FnOnce(&dyn HistorySink) -> T,
) -> T {
    let Ok(mut guard) = state.0.lock() else {
        log::warn!("历史库状态锁中毒，本次操作不记录历史");
        return f(&NoopHistory);
    };
    let Some(db) = guard.as_mut() else {
        return f(&NoopHistory);
    };

    // 建「操作前」快照。失败不阻断操作——历史是 sidecar 能力（spec D16）。
    let snapshot_uid = match snapshot_of() {
        Some((system, user)) => {
            let snap = path_editor_core::db::history::NewSnapshot {
                kind: path_editor_core::db::history::SnapshotKind::History,
                source: path_editor_core::history::EventSource::Gui,
                system,
                user,
                env: Vec::new(),
                parent_uid: None,
            };
            match path_editor_core::db::history::insert_snapshot(db, snap) {
                Ok(uid) => Some(uid),
                Err(e) => {
                    log::warn!("建快照失败（操作继续，事件将不带快照）: {}", e.message);
                    None
                }
            }
        }
        None => None,
    };

    let result = f(&DbHistory::with_snapshot(db, snapshot_uid));

    // 操作成功后，把「操作后」的状态记为**当前快照**（P1-4 的前置）。
    //
    // 与上面的「操作前」快照职责不同：
    // - `snapshot_uid`（操作前）绑给事件 → 撤销时恢复它；
    // - `current_snapshot_id`（操作后）→ 表示「现在 PATH 是什么」，是 `disabled.json`
    //   停写后（spec §7.8 H7）DB 侧唯一的状态真相。
    //
    // 失败不阻断：当前状态建不出来不该让用户的操作失败（sidecar 语义）。
    if let Some((system, user)) = snapshot_of_after() {
        update_current_snapshot(db, system, user);
    }

    result
}

/// 把操作后的状态写成新快照并设为当前快照。
///
/// CAS 语义（spec D14）：先读 `app_state.version`，再带 version 更新；
/// 失败说明有并发写——此时**放弃**而不是强行覆盖：历史仍然正确，
/// 只是 `current_snapshot_id` 这一次没跟上。下一次操作会重新对齐。
fn update_current_snapshot(
    db: &mut path_editor_core::db::Db,
    system: Vec<PathEntry>,
    user: Vec<PathEntry>,
) {
    // 先留一份给读缓存（`system`/`user` 随后会被移进 `NewSnapshot`）
    let system_for_cache = system.clone();
    let user_for_cache = user.clone();
    let snap = path_editor_core::db::history::NewSnapshot {
        kind: path_editor_core::db::history::SnapshotKind::Current,
        source: path_editor_core::history::EventSource::Gui,
        system,
        user,
        env: Vec::new(),
        parent_uid: None,
    };
    let uid = match path_editor_core::db::history::insert_snapshot(db, snap) {
        Ok(uid) => uid,
        Err(e) => {
            log::warn!("建当前快照失败（不影响已完成的写入）: {}", e.message);
            return;
        }
    };
    let version = match path_editor_core::db::history::app_state_version(db) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("读取 app_state.version 失败: {}", e.message);
            return;
        }
    };
    match path_editor_core::db::history::set_current_snapshot(db, &uid, version) {
        Ok(true) => {}
        Ok(false) => log::warn!("current_snapshot_id 的 CAS 冲突（有并发写），本次不更新当前状态"),
        Err(e) => log::warn!("更新 current_snapshot_id 失败: {}", e.message),
    }

    // 同步刷新 `disabled.rs` 的读缓存（D-1 方案 A）。
    // 停写之后 `load_path_snapshot()` 从 DB 读，而加载器是 `Fn()` 且拿不到 `Db`，
    // 靠进程级缓存桥接；写完不刷新会让读路径停留在旧状态。
    crate::refresh_read_snapshot(system_for_cache, user_for_cache);
}

/// 读当前 PATH 快照（供建「操作前」快照用）。
///
/// 走 `disabled::load_path_snapshot`（注册表 + disabled.json 合并），
/// 与 `save_path_with_sidecar` 写入的 sidecar 口径一致。
pub(super) fn current_path_snapshot() -> Option<(Vec<PathEntry>, Vec<PathEntry>)> {
    match path_editor_core::disabled::load_path_snapshot() {
        Ok(s) => Some((s.system, s.user)),
        Err(e) => {
            log::warn!("读取当前 PATH 快照失败，本次事件不带快照: {}", e.message);
            None
        }
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
    with_history_and_snapshot(
        &state,
        current_path_snapshot,
        current_path_snapshot,
        |history| service::save_path_with_sidecar(system, user, history),
    )
    .map_err(|e| e.message)
}

/// 把单 hive 的 PATH 写入注册表并落盘快照；未触及的另一 hive 为 `Skipped`。
#[tauri::command]
pub fn apply_path_snapshot(
    hive: path_editor_core::EnvHive,
    entries: Vec<PathEntry>,
    state: State<'_, DbState>,
) -> Result<ApplyOutcome, String> {
    with_history_and_snapshot(
        &state,
        current_path_snapshot,
        current_path_snapshot,
        |history| service::apply_path_snapshot(hive, entries, history),
    )
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
