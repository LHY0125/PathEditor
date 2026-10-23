//! PATH 保存编排：注册表写入、备份、并发校验、广播与持久化。
//!
//! 两个入口共用本模块：
//! - [`save_path_with_db`]：DB 可用时使用；写注册表后提交 `current` 快照与事件。
//! - [`save_path_registry_only`]：DB 不可用时的降级路径；只写注册表。
//!
//! 语义：
//! - **best-effort 多 hive**：逐 hive 尝试，结果按 hive 分字段；
//! - **pending**：注册表已写成功但快照未落盘时记录，下次补写（只补写快照，不触碰注册表）；
//! - **广播**：任一 hive 注册表写成功后由服务层广播。

use crate::backup;
use crate::db::history::{self, NewSnapshot, SnapshotKind};
use crate::db::history_sink::DbHistory;
use crate::db::{search, Db};
use crate::disabled;
use crate::env_var::EnvHive;
use crate::error::{CoreError, ErrorCode};
use crate::history::{EventAction, EventResult, EventSource, HistoryEvent, HistorySink};
use crate::path_entry::{PathEntry, PathSnapshot};
use crate::registry;
use crate::system;

/// PATH 读写的可注入缝（仿 `EnvHiveStore` / `registry/env_var.rs` 的 `_with_store` 模式）。
///
/// 存在的理由：保存编排（备份 / 并发校验 / 顺序写 /
/// 快照事务）**必须能在不触碰真实注册表的前提下单测**（Global Constraints：
/// 本层测试不写真实注册表。真实 IO 一路硬编码到 `HKEY_LOCAL_MACHINE` /
/// `HKEY_CURRENT_USER`（`registry::save_system_paths` 等），无法直接注入；
/// 故在服务层定义此缝，把「读写哪个 hive 的哪段存储」抽象掉。
///
/// 两个适配器 = 真实缝（`codebase-design` 判据）：
/// - [`RealPathHiveIo`]：转调真实注册表（生产）；
/// - 测试内的 fake：记录调用序列、不碰注册表。
///
/// **缝刻意定义在 `service.rs` 而非 `registry/path.rs`**：后者返回 `String`
/// 错误，此处的缝用 `CoreError`，从而 `registry/path.rs` 零改动。
pub trait PathHiveIo {
    /// 读取某 hive 当前的**启用**路径（用于乐观并发比对）。
    fn read_enabled(&self, hive: EnvHive) -> Result<Vec<String>, CoreError>;

    /// 写入某 hive 的启用路径。
    fn write_enabled(&self, hive: EnvHive, paths: &[String]) -> Result<(), CoreError>;

    /// 写前备份（真实现 → `backup::backup_registry(None)`，走默认目录轮换）。
    fn backup(&self) -> Result<(), CoreError>;
}

/// [`PathHiveIo`] 的真实实现：转调 `registry` / `backup`。
pub(crate) struct RealPathHiveIo;

impl PathHiveIo for RealPathHiveIo {
    fn read_enabled(&self, hive: EnvHive) -> Result<Vec<String>, CoreError> {
        let result = match hive {
            EnvHive::System => registry::load_system_paths(),
            EnvHive::User => registry::load_user_paths(),
        };
        result.map_err(|e| {
            core_err(
                e.code,
                "read_enabled",
                format!(
                    "{} PATH 读取失败: {e}",
                    if hive == EnvHive::System {
                        "系统"
                    } else {
                        "用户"
                    }
                ),
            )
        })
    }

    fn write_enabled(&self, hive: EnvHive, paths: &[String]) -> Result<(), CoreError> {
        let result = match hive {
            EnvHive::System => registry::save_system_paths(paths.to_vec()),
            EnvHive::User => registry::save_user_paths(paths.to_vec()),
        };
        result.map_err(|e| {
            core_err(
                e.code,
                "write_enabled",
                format!(
                    "{} PATH 注册表写入失败: {e}",
                    if hive == EnvHive::System {
                        "系统"
                    } else {
                        "用户"
                    }
                ),
            )
        })
    }

    fn backup(&self) -> Result<(), CoreError> {
        // 传 None 走默认备份目录，从而沿用 `rotate_path_backups` 的默认目录限定
        // （自定义目录不参与轮换，可能混有用户自己的文件）。
        backup::backup_registry(None).map(|_| ())
    }
}

/// 单个 hive 的应用结果。
///
/// 需过 Tauri IPC，必须可序列化。
/// - `Applied`：该 hive 注册表写入成功；
/// - `Skipped`：本次调用未触及该 hive（如单 hive 应用的另一侧）；
/// - `Failed`：该 hive 注册表阶段失败（含写入前校验失败）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HiveOutcome {
    /// 注册表写入成功
    Applied,
    /// 未触及该 hive
    Skipped,
    /// 注册表阶段失败，携带结构化错误
    Failed(CoreError),
}

/// 快照持久化的落盘结果。
///
/// - `Saved`：快照已落盘；
/// - `Pending`：快照落盘失败，但待补写状态已成功记录，下次运行可补写；
/// - `Failed`：快照落盘失败，且待补写状态记录也失败，需人工核对。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SidecarOutcome {
    /// 快照已成功落盘
    Saved,
    /// 快照未落盘，已记录待补写状态；携带快照保存的原始错误
    Pending(CoreError),
    /// 快照未落盘且待补写状态记录失败；携带快照保存的原始错误
    Failed(CoreError),
}

/// 一次 PATH 应用（注册表 + 快照）的整体结果。
///
/// **按 hive 分字段**：多 hive 应用必须能同时表达「系统成功 / 用户失败」
/// 这种 partial 结果，单个 `registry:HiveOutcome` 字段装不下。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    /// 系统 hive（HKLM）的结果
    pub system: HiveOutcome,
    /// 用户 hive（HKCU）的结果
    pub user: HiveOutcome,
    /// 快照的落盘结果
    pub sidecar: SidecarOutcome,
}

/// 构造带操作上下文的 `CoreError`（自由文本错误的统一收口）。
fn core_err(code: ErrorCode, operation: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::new(code, operation, message)
}

/// 单个 hive 的写入请求：待写条目 + 乐观并发的期望当前值。
///
/// 需过 Tauri IPC，必须可序列化。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathHiveWrite {
    /// 待写入的完整条目列表（含 `enabled`）。
    pub entries: Vec<PathEntry>,
    /// 期望的注册表现值（**启用**路径，即 `enabled` 条目）。
    ///
    /// - `Some(orig)`：写前读当前启用路径比对，不匹配则以
    ///   [`HiveOutcome::Failed`]（`code = Conflict`）表达，**不写、不产 pending**；
    /// - `None`：跳过并发校验（如 `apply_path_snapshot` / `apply_profile`）。
    pub expected: Option<Vec<String>>,
}

/// 乐观并发校验：`expected = Some(orig)` 时读当前启用路径比对。
///
/// - `None`（该 hive 未请求写入）→ `Ok(())`；
/// - `expected = None`（明确跳过校验）→ `Ok(())`；
/// - 读失败 → `Err`（透传 IO 错误码）；
/// - **内容**不匹配（多重集）→ `Err(code = Conflict)`，携带 `[E_CONFLICT]` 前缀消息。
///
/// **只读**：不触碰 `io.write_enabled`，故校验失败不产生写入、不产 pending。
///
/// 顺序敏感死锁修复：比较改为**多重集**（排序后比较）——
/// 忽略顺序、保留重复计数。理由：
/// - `expected` 来自 `load_path_snapshot` 的 `merge_hive` 输出，其**成员忠实于
///   注册表、唯独顺序取自快照**（`disabled.rs:137`，golden `merge_snapshot_order_wins`）；
///   而 `current` 是**注册表顺序**。二者在**任何外部重排**后即不相等。
/// - 顺序敏感比较因此**永久拒绝保存**（存不了 → 快照永不更新 → 一直拒绝），
///   且「重新加载后重试」是假解药（reload 走同一合并，产出同一顺序）。
/// - PathEditor **拥有 PATH 顺序**（快照是顺序真相源），外部纯重排不是
///   lost-update，不应据此拒绝。
///
/// ⚠️ **不放宽成员冲突**：外部增删仍拒绝（排序后仍不等）。
fn check_expected(
    io: &dyn PathHiveIo,
    hive: EnvHive,
    write: Option<&PathHiveWrite>,
) -> Result<(), CoreError> {
    let Some(w) = write else {
        return Ok(());
    };
    let Some(expected) = &w.expected else {
        return Ok(());
    };
    let current = io.read_enabled(hive)?;
    // 内容比较（多重集）：忽略顺序、保留重复计数。
    let mut current_sorted = current;
    let mut expected_sorted = expected.clone();
    current_sorted.sort();
    expected_sorted.sort();
    if current_sorted != expected_sorted {
        return Err(core_err(
            ErrorCode::Conflict,
            "check_expected",
            crate::registry::conflict_message(),
        ));
    }
    Ok(())
}

/// 把某 hive 的启用条目经 `io` 写入注册表；失败不 panic，转为 `HiveOutcome::Failed`。
///
/// **前置条件**：调用方已通过 [`check_expected`] 完成并发校验。本函数只负责写入。
///
/// 校验（null 字节 / 32767 上限）发生在 `registry::save_*_paths` 内部、
/// 触碰注册表之前，因此校验失败与注册表写失败走同一段「注册表阶段失败」路径，
/// 均不会产生 pending。
fn write_hive_registry(io: &dyn PathHiveIo, hive: EnvHive, write: &PathHiveWrite) -> HiveOutcome {
    let enabled: Vec<String> = write
        .entries
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| entry.path.clone())
        .collect();
    match io.write_enabled(hive, &enabled) {
        Ok(()) => HiveOutcome::Applied,
        Err(e) => HiveOutcome::Failed(core_err(e.code, "write_hive_registry", e.message)),
    }
}

/// 只写注册表（DB 不可用时的降级路径）。
///
/// 禁用状态无法持久化，因此只要有一个 hive 写入成功，`sidecar` 就是
/// `Failed(database)`——诚实报告，而不是假装保存成功。
pub fn save_path_registry_only(
    system: Option<PathHiveWrite>,
    user: Option<PathHiveWrite>,
    source: EventSource,
    history: &dyn HistorySink,
) -> Result<ApplyOutcome, CoreError> {
    let mut outcome = save_path_registry_io(&RealPathHiveIo, system, user)?;
    let applied = matches!(outcome.system, HiveOutcome::Applied)
        || matches!(outcome.user, HiveOutcome::Applied);
    if applied {
        outcome.sidecar = SidecarOutcome::Failed(CoreError::new(
            ErrorCode::Database,
            "save_path",
            "数据库不可用，当前 PATH 快照未保存",
        ));
    }

    if let Err(e) = history.record(HistoryEvent {
        source,
        action: EventAction::Save,
        hive: None,
        target: None,
        changed_fields: vec!["path_entries".to_string()],
        before_revision: None,
        after_revision: None,
        result: event_result(&outcome.system, &outcome.user),
        snapshot_uid: None,
    }) {
        log::warn!("历史写入失败（保存已成功）: {}", e.message);
    }
    Ok(outcome)
}

/// 把某 hive 的 PATH 写入注册表（降级路径，不持久化快照）。
pub fn apply_path_snapshot(
    hive: EnvHive,
    entries: Vec<PathEntry>,
    history: &dyn HistorySink,
) -> Result<ApplyOutcome, CoreError> {
    let write = PathHiveWrite {
        entries,
        expected: None,
    };
    match hive {
        EnvHive::System => save_path_registry_only(Some(write), None, EventSource::Gui, history),
        EnvHive::User => save_path_registry_only(None, Some(write), EventSource::Gui, history),
    }
}

/// 保存 PATH 的核心编排：读写经 `io` 注入（生产传 [`RealPathHiveIo`]，
/// 测试传 fake，**全程不碰真实注册表**）。
///
/// 编排顺序：
/// 1. best-effort 补写上次未落盘的 pending（失败仅告警，不阻断）；
/// 2. 对需校验的 hive 先做**乐观并发校验**（只读，不写）；校验失败以
///    [`HiveOutcome::Failed`]（`code = Conflict`）表达，该 hive 不写、不产 pending；
/// 3. **至少一个 hive 确实将写入**时，先写前备份（失败仅告警，不阻断）；
/// 4. 逐 hive 写注册表（`None` 侧跳过）；
/// 5. 任一 hive 写成功后广播环境变更。
///
/// **备份时机**：在**校验通过、确实将写入**之后、真正写入之前。这样校验失败的
/// 调用**不产生无谓备份文件**——否则一次并发冲突就会留下一份没人需要的备份。
pub(crate) fn save_path_registry_io(
    io: &dyn PathHiveIo,
    system: Option<PathHiveWrite>,
    user: Option<PathHiveWrite>,
) -> Result<ApplyOutcome, CoreError> {
    // 步骤 2：乐观并发校验（只读）。校验通过则返回 ()，失败返回 Conflict。
    // 单独出来做，是为了让「是否将写入」在备份之前就已知。
    let sys_check = check_expected(io, EnvHive::System, system.as_ref());
    let usr_check = check_expected(io, EnvHive::User, user.as_ref());
    let sys_will_write = system.is_some() && sys_check.is_ok();
    let usr_will_write = user.is_some() && usr_check.is_ok();

    // 步骤 3：确认将发生写入时备份。
    //
    // 真实现经 `RealPathHiveIo::backup` → `backup::backup_registry(None)`，
    // 走默认备份目录，从而沿用 `rotate_path_backups` 的默认目录限定（自定义目录不参与轮换，可能混有用户自己的文件）。
    //
    // 备份失败**不阻断**写入（与 env 侧 `WriteOutcome.backup` 同语义）：
    // 只记警告并继续，写入结果照常返回。
    if sys_will_write || usr_will_write {
        if let Err(e) = io.backup() {
            log::warn!("PATH 写前备份失败（写入将继续）: {}", e.message);
        }
    }

    // 步骤 4：写入。校验结论已算出，此处直接映射为结果。
    let sys_outcome = match sys_check {
        Err(e) => HiveOutcome::Failed(e),
        Ok(()) => match &system {
            Some(w) => write_hive_registry(io, EnvHive::System, w),
            None => HiveOutcome::Skipped,
        },
    };
    let usr_outcome = match usr_check {
        Err(e) => HiveOutcome::Failed(e),
        Ok(()) => match &user {
            Some(w) => write_hive_registry(io, EnvHive::User, w),
            None => HiveOutcome::Skipped,
        },
    };

    let sys_applied = matches!(sys_outcome, HiveOutcome::Applied);
    let usr_applied = matches!(usr_outcome, HiveOutcome::Applied);

    if sys_applied || usr_applied {
        system::broadcast_env_change();
    }

    // 当前快照由调用方（save_path_with_db）提交；这里只负责注册表。
    let sidecar = SidecarOutcome::Saved;

    Ok(ApplyOutcome {
        system: sys_outcome,
        user: usr_outcome,
        sidecar,
    })
}

/// 读取完整 PATH 快照：DB 可用时读 `current_snapshot`，否则只读注册表。
///
/// GUI 与 CLI 共用的降级语义：DB 不可用不等于读不到 PATH。
pub fn load_path_snapshot_any(db: Option<&Db>) -> Result<PathSnapshot, CoreError> {
    match db {
        Some(db) => load_path_snapshot_db(db),
        None => disabled::load_path_snapshot(),
    }
}

/// 从 DB 的 `current_snapshot` 加载完整 PATH 快照，并与注册表合并。
///
/// DB 没有当前快照时只读注册表；这与降级模式的行为一致。
pub fn load_path_snapshot_db(db: &Db) -> Result<PathSnapshot, CoreError> {
    let persisted = history::current_snapshot(db)?.map(|view| PathSnapshot {
        system: view.system,
        user: view.user,
    });
    disabled::load_path_snapshot_with(persisted)
}

/// 与 [`load_path_snapshot_db`] 相同，但注册表读取经可注入的 [`PathHiveIo`]。
///
/// 单个 hive 读取失败时只告警，并用 DB 当前快照里该 hive 的启用项兜底——
/// 保存一个 hive 不应该因为另一个 hive 不可读而整体失败。
fn load_path_snapshot_db_io(io: &dyn PathHiveIo, db: &Db) -> Result<PathSnapshot, CoreError> {
    let persisted = history::current_snapshot(db)?.map(|view| PathSnapshot {
        system: view.system,
        user: view.user,
    });
    let fallback = |entries: Option<&Vec<PathEntry>>| {
        entries
            .map(|list| {
                list.iter()
                    .filter(|entry| entry.enabled)
                    .map(|entry| entry.path.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let registry_system = io.read_enabled(EnvHive::System).unwrap_or_else(|e| {
        log::warn!("读取系统 PATH 失败，使用 DB 快照兜底: {}", e.message);
        fallback(persisted.as_ref().map(|p| &p.system))
    });
    let registry_user = io.read_enabled(EnvHive::User).unwrap_or_else(|e| {
        log::warn!("读取用户 PATH 失败，使用 DB 快照兜底: {}", e.message);
        fallback(persisted.as_ref().map(|p| &p.user))
    });
    disabled::merge_path_snapshot(registry_system, registry_user, persisted)
}

/// 把 hive 结果映射为事件结果。
///
/// 两个 hive 都失败必须是 `Failure`；只有一个成功才是 `Partial`。
fn event_result(system: &HiveOutcome, user: &HiveOutcome) -> EventResult {
    let applied = matches!(system, HiveOutcome::Applied) || matches!(user, HiveOutcome::Applied);
    let failed = matches!(system, HiveOutcome::Failed(_)) || matches!(user, HiveOutcome::Failed(_));
    match (applied, failed) {
        (true, true) => EventResult::Partial,
        (false, true) => EventResult::Failure,
        _ => EventResult::Success,
    }
}

/// 把给定快照写成新的 `kind=current` 快照并更新 `app_state`。
fn persist_current_snapshot(
    db: &mut Db,
    source: EventSource,
    snapshot: PathSnapshot,
) -> Result<String, CoreError> {
    let uid = history::insert_snapshot(
        db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source,
            system: snapshot.system,
            user: snapshot.user,
            env: Vec::new(),
            parent_uid: None,
        },
    )?;
    let version = history::app_state_version(db)?;
    if !history::set_current_snapshot(db, &uid, version)? {
        return Err(CoreError::new(
            ErrorCode::Conflict,
            "save_current",
            "当前快照 version CAS 冲突，本次不更新",
        ));
    }
    Ok(uid)
}

/// 增量索引一份快照；失败只告警（派生表不是真相）。
fn index_snapshot_by_uid(db: &mut Db, uid: &str) {
    let id: Result<i64, CoreError> =
        db.query_row_with("SELECT id FROM snapshots WHERE uid = ?1", [uid], |row| {
            row.get(0)
        });
    let Ok(id) = id else {
        log::warn!("增量索引：解析快照 id 失败，跳过（下次启动会全量重建）");
        return;
    };
    if let Err(e) = db.with_transaction(|tx| search::index_snapshot_tx(tx, id)) {
        log::warn!("增量索引失败（下次启动会全量重建）: {}", e.message);
    }
}

/// 保存 PATH 并把当前状态提交到 DB（不做文件持久化）。
///
/// 与 [`save_path_with_db`] 的唯一区别是注册表 IO 与 pending 路径可注入，
/// 供集成测试使用。
pub fn save_path_with_db_io(
    io: &dyn PathHiveIo,
    db: &mut Db,
    source: EventSource,
    system: Option<PathHiveWrite>,
    user: Option<PathHiveWrite>,
    pending_path: &std::path::Path,
) -> Result<ApplyOutcome, CoreError> {
    // 先 best-effort 补写上次失败留下的 pending；失败不阻断本次保存。
    if let Err(e) = retry_pending_path_state_db_at(db, source, pending_path) {
        log::warn!("补写上次待落盘的当前快照失败: {}", e.message);
    }

    // 操作前快照用于撤销；失败不阻断保存。
    let before = load_path_snapshot_db_io(io, db)?;
    let before_uid = match history::insert_snapshot(
        db,
        NewSnapshot {
            kind: SnapshotKind::History,
            source,
            system: before.system.clone(),
            user: before.user.clone(),
            env: Vec::new(),
            parent_uid: None,
        },
    ) {
        Ok(uid) => Some(uid),
        Err(e) => {
            log::warn!("建操作前快照失败（保存继续）: {}", e.message);
            None
        }
    };

    let sys_entries = system.as_ref().map(|w| w.entries.clone());
    let usr_entries = user.as_ref().map(|w| w.entries.clone());
    let mut outcome = save_path_registry_io(io, system, user)?;
    let applied = matches!(&outcome.system, HiveOutcome::Applied)
        || matches!(&outcome.user, HiveOutcome::Applied);

    if applied {
        // 当前快照以「本次实际写入的条目」为准；未写/失败的 hive 保留操作前状态。
        let current = PathSnapshot {
            system: if matches!(&outcome.system, HiveOutcome::Applied) {
                sys_entries.unwrap_or_default()
            } else {
                before.system.clone()
            },
            user: if matches!(&outcome.user, HiveOutcome::Applied) {
                usr_entries.unwrap_or_default()
            } else {
                before.user.clone()
            },
        };
        let pending_snapshot = current.clone();
        match persist_current_snapshot(db, source, current) {
            Ok(uid) => {
                index_snapshot_by_uid(db, &uid);
                // 本次 current 已落库，任何陈旧 pending 都被取代。
                let _ = disabled::clear_pending_path_snapshot_at(pending_path);
                outcome.sidecar = SidecarOutcome::Saved;
            }
            Err(e) => {
                // 注册表已写成功但 DB 当前快照没跟上：落 pending，下次补写。
                outcome.sidecar = match disabled::save_pending_path_snapshot_at(
                    pending_path,
                    Some(pending_snapshot.system),
                    Some(pending_snapshot.user),
                ) {
                    Ok(()) => SidecarOutcome::Pending(e),
                    Err(_) => SidecarOutcome::Failed(e),
                };
            }
        }
    }

    let event = HistoryEvent {
        source,
        action: EventAction::Save,
        hive: None,
        target: None,
        changed_fields: vec!["path_entries".to_string()],
        before_revision: None,
        after_revision: None,
        result: event_result(&outcome.system, &outcome.user),
        snapshot_uid: before_uid.clone(),
    };
    if let Err(e) = DbHistory::with_snapshot(db, before_uid).record(event) {
        log::warn!("历史写入失败（保存已成功）: {}", e.message);
    }

    Ok(outcome)
}

/// 保存 PATH 并把当前状态提交到 DB（生产入口）。
pub fn save_path_with_db(
    db: &mut Db,
    source: EventSource,
    system: Option<PathHiveWrite>,
    user: Option<PathHiveWrite>,
) -> Result<ApplyOutcome, CoreError> {
    save_path_with_db_io(
        &RealPathHiveIo,
        db,
        source,
        system,
        user,
        &disabled::default_pending_path(),
    )
}

mod profile;
mod restore;

pub use profile::{apply_profile, apply_profile_io};
pub use restore::{restore_snapshot, restore_snapshot_io};

/// 把文件 pending 快照补写进 DB 的当前快照（DB 模式）。
pub fn retry_pending_path_state_db(
    db: &mut Db,
    source: EventSource,
) -> Result<ApplyOutcome, CoreError> {
    retry_pending_path_state_db_at(db, source, &disabled::default_pending_path())
}

/// [`retry_pending_path_state_db`] 的显式 pending 路径版本（测试注入 `TempDir`）。
pub fn retry_pending_path_state_db_at(
    db: &mut Db,
    source: EventSource,
    pending_path: &std::path::Path,
) -> Result<ApplyOutcome, CoreError> {
    let Some(pending) = disabled::load_pending_path_snapshot_at(pending_path)? else {
        return Ok(ApplyOutcome {
            system: HiveOutcome::Skipped,
            user: HiveOutcome::Skipped,
            sidecar: SidecarOutcome::Saved,
        });
    };

    let uid = history::insert_snapshot(
        db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source,
            system: pending.system,
            user: pending.user,
            env: Vec::new(),
            parent_uid: None,
        },
    )?;
    let version = history::app_state_version(db)?;
    if !history::set_current_snapshot(db, &uid, version)? {
        return Err(CoreError::new(
            ErrorCode::Conflict,
            "retry_pending",
            "当前快照 version CAS 冲突，pending 未清除",
        ));
    }
    index_snapshot_by_uid(db, &uid);
    let _ = disabled::clear_pending_path_snapshot_at(pending_path);

    Ok(ApplyOutcome {
        system: HiveOutcome::Skipped,
        user: HiveOutcome::Skipped,
        sidecar: SidecarOutcome::Saved,
    })
}

#[cfg(test)]
mod tests;
