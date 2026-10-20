//! 共享应用服务层（F-08，Wave 2 Task 5）。
//!
//! 把 GUI（`path-session.ts`）与 CLI（`runtime.rs`）各自编排的
//! 「读 → 写注册表 → 广播 → 落 sidecar 快照 → 失败落 pending」
//! 用例编排下沉到此处，消除两端的策略分叉。
//!
//! 语义约定（核对轮 2026-09-20 裁决）：
//! - **best-effort 多 hive**：逐 hive 尝试，失败不阻断另一 hive；
//!   结果按 hive 分字段表达在 [`ApplyOutcome`] 中（W2-B1）。
//! - **诚实报告 vs 策略分层**（W2-N1）：本层 `Result` 是诚实的错误报告；
//!   CLI 策略层（如 `flush_pending_snapshot`）把 `Err` 降级为警告、不阻断。
//! - **pending 机制由服务层承担**：注册表写成功而 sidecar 落盘失败时，
//!   在此层落 pending；补写（[`retry_pending_path_state`]）**sidecar-only**，
//!   不触碰注册表（W2-B2）——pending 的产生前提就是注册表已写成功。
//! - **广播**：任一 hive 注册表写成功后由服务层广播（与 Wave 1 广播位一致）。

use crate::backup;
use crate::disabled;
use crate::env_var::EnvHive;
use crate::error::{CoreError, ErrorCode};
use crate::history::{
    EventAction, EventResult, EventSource, HistoryEvent, HistorySink, NoopHistory,
};
use crate::path_entry::PathEntry;
use crate::profiles;
use crate::registry;
use crate::system;

/// PATH 读写的可注入缝（仿 `EnvHiveStore` / `registry/env_var.rs` 的 `_with_store` 模式）。
///
/// 存在的理由：`save_path_with_sidecar` 的编排（备份 / 并发校验 / 顺序写 /
/// sidecar 事务）**必须能在不触碰真实注册表的前提下单测**（Global Constraints：
/// Task 1-7 不写真实注册表）。真实 IO 一路硬编码到 `HKEY_LOCAL_MACHINE` /
/// `HKEY_CURRENT_USER`（`registry::save_system_paths` 等），无法直接注入；
/// 故在服务层定义此缝，把「读写哪个 hive 的哪段存储」抽象掉。
///
/// 两个适配器 = 真实缝（`codebase-design` 判据）：
/// - [`RealPathHiveIo`]：转调真实注册表（生产）；
/// - 测试内的 fake：记录调用序列、不碰注册表。
///
/// **缝刻意定义在 `service.rs` 而非 `registry/path.rs`**：后者返回 `String`
/// 错误（F-3 未迁移领地），此处的缝用 `CoreError`，从而 `registry/path.rs` 零改动。
pub(crate) trait PathHiveIo {
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
                ErrorCode::Io,
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
                ErrorCode::Internal,
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
        // （N4 裁断：自定义目录不参与轮换，可能混有用户自己的文件）。
        backup::backup_registry(None)
            .map(|_| ())
            .map_err(|e| core_err(ErrorCode::Io, "backup", e))
    }
}

/// 单个 hive 的应用结果。
///
/// 需过 Tauri IPC（W2-B3），必须可序列化。
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

/// sidecar（disabled.json / PATH 快照 / pending）的落盘结果。
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

/// 一次 PATH 应用（注册表 + sidecar）的整体结果。
///
/// **按 hive 分字段**：多 hive 应用必须能同时表达「系统成功 / 用户失败」
/// 这种 partial 结果（评审 W2-B1），单个 `registry: HiveOutcome` 字段装不下。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    /// 系统 hive（HKLM）的结果
    pub system: HiveOutcome,
    /// 用户 hive（HKCU）的结果
    pub user: HiveOutcome,
    /// sidecar 快照的落盘结果
    pub sidecar: SidecarOutcome,
}

/// 构造带操作上下文的 `CoreError`（自由文本错误的过渡期收口）。
fn core_err(code: ErrorCode, operation: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::new(code, operation, message)
}

/// 单个 hive 的写入请求：待写条目 + 乐观并发的期望当前值。
///
/// 需过 Tauri IPC（W2-B3），必须可序列化。
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

/// 乐观并发校验（只读，D5 / Wave 3.1）：`expected = Some(orig)` 时读当前启用路径比对。
///
/// - `None`（该 hive 未请求写入）→ `Ok(())`；
/// - `expected = None`（明确跳过校验）→ `Ok(())`；
/// - 读失败 → `Err`（透传 IO 错误码）；
/// - **内容**不匹配（多重集）→ `Err(code = Conflict)`，携带 `[E_CONFLICT]` 前缀消息。
///
/// **只读**：不触碰 `io.write_enabled`，故校验失败不产生写入、不产 pending。
///
/// **Wave 3.1（顺序敏感死锁修复）**：比较改为**多重集**（排序后比较）——
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
/// 均不会产生 pending（W2-B2 注入机制）。
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

/// 提交 sidecar 快照（仅落盘，不触碰注册表），失败时按 Wave 1 语义落 pending。
///
/// - 快照保存失败 → 先用当前快照把 `None` 侧填充为现有内容再落 pending
///   （保证补写是无损的整份覆盖），返回 [`SidecarOutcome::Pending`]；
///   连 pending 都记录失败则返回 [`SidecarOutcome::Failed`]。
/// - 快照保存成功 → 防御性清除任何陈旧 pending（其内容已被本快照取代）。
fn commit_sidecar(system: Option<Vec<PathEntry>>, user: Option<Vec<PathEntry>>) -> SidecarOutcome {
    if let Err(e) = disabled::save_path_snapshot(system.clone(), user.clone()) {
        // 「legacy」哨兵分支：为兼容旧错误形状保留的哨兵（operation == "legacy"
        // 表示 disabled.rs 旧通路产生的自由文本错误），Task 6 全面迁移后已是死代码，
        // 保留原因见 docs/审核和开发/2026.09.20/PathEditor-Wave2架构收口开发回执.md
        // 第 2 节 Task 6 表（「legacy 哨兵分支已死」条目）。
        let sidecar_err = if matches!(e.code, ErrorCode::Internal) && e.operation == "legacy" {
            core_err(ErrorCode::Io, "commit_sidecar", e.message)
        } else {
            e
        };
        // pending 快照是覆盖式整份落盘，`save_pending_path_snapshot` 的 None 语义
        // 是「空数组」而非「保留该 hive」。若把 None 原样落 pending，补写时会把
        // 未操作的 hive 清空。因此先用当前快照把 None 侧填充为现有内容。
        let pending = match disabled::load_path_snapshot() {
            Ok(snap) => {
                let sys = system.unwrap_or_else(|| snap.system.clone());
                let usr = user.unwrap_or_else(|| snap.user.clone());
                disabled::save_pending_path_snapshot(Some(sys), Some(usr))
            }
            // 当前快照读取失败时无法安全构造无损 pending，跳过落盘；
            // 调用方按 Failed 语义提示手工核对。失败详情不能只落在被吞掉的
            // `_pe` 上，必须记日志留存（终审 M-2）。
            Err(pe) => {
                log::error!("落 pending 前读取当前快照失败，跳过待补写记录: {pe}");
                Err(CoreError::new(
                    ErrorCode::Internal,
                    "commit_sidecar",
                    format!("待补写状态构造失败（读取当前快照出错）: {pe}"),
                ))
            }
        };
        return match pending {
            Ok(()) => SidecarOutcome::Pending(sidecar_err),
            Err(_) => SidecarOutcome::Failed(sidecar_err),
        };
    }

    // 防御性清理：本次快照已成功落盘，任何陈旧 pending 都已过时，必须清除，
    // 否则下次补写会用陈旧内容覆盖本次写入。磁盘此时已证明可写，失败概率极低；
    // 即便失败，下次补写也只是幂等重放已被取代的旧状态。
    if disabled::has_pending_path_snapshot() {
        if let Err(e) = disabled::clear_pending_path_snapshot() {
            log::warn!("清除待补写快照状态失败: {e}");
        }
    }
    SidecarOutcome::Saved
}

/// 把某 hive 的 PATH 写入注册表并落盘快照（含 pending 恢复语义）。
///
/// 未触及的另一 hive 置为 [`HiveOutcome::Skipped`]。内部先 best-effort 补写
/// 上次未落盘的 pending（服务层承担 Wave 1 的 pending 机制迁移），再写注册表、
/// 广播、提交 sidecar。
///
/// 注册表阶段失败（含写入前校验失败）以 `HiveOutcome::Failed` 在结果中诚实表达，
/// **不产生 pending**；`Err` 保留给无法产出结果的预置失败（当前实现不产生）。
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
        EnvHive::System => save_path_with_sidecar(Some(write), None, history),
        EnvHive::User => save_path_with_sidecar(None, Some(write), history),
    }
}

/// 同时写注册表与 sidecar，明确 partial 语义（best-effort，逐 hive 尝试）。
///
/// 编排见 [`save_path_with_sidecar_io`]（本函数只是注入真实 IO 的薄壳）。
///
/// 乐观并发：`PathHiveWrite.expected = Some(orig)` 时先在 core 内完成
/// 「读-比-写」校验，不匹配则该 hive 以 `HiveOutcome::Failed(code = Conflict)`
/// 表达，**不写、不产 pending**（CLI 侧 `verify_and_save` 已由此下沉）。
pub fn save_path_with_sidecar(
    system: Option<PathHiveWrite>,
    user: Option<PathHiveWrite>,
    history: &dyn HistorySink,
) -> Result<ApplyOutcome, CoreError> {
    let outcome = save_path_with_sidecar_io(&RealPathHiveIo, system, user)?;
    // 上报失败不阻断保存（历史是 sidecar 能力，spec D16 fail-open）。
    if let Err(e) = history.record(HistoryEvent {
        source: EventSource::Gui,
        action: EventAction::Save,
        hive: None,
        target: None,
        changed_fields: vec!["path_entries".to_string()],
        before_revision: None,
        after_revision: None,
        result: match (&outcome.system, &outcome.user) {
            (HiveOutcome::Failed(_), _) | (_, HiveOutcome::Failed(_)) => EventResult::Partial,
            _ => EventResult::Success,
        },
        // 保存操作的快照由调用方（GUI/CLI）在拿到 uid 后另行关联；
        // 本服务函数不知道 DB，因此留 None。
        snapshot_uid: None,
    }) {
        log::warn!("历史写入失败（保存已成功）: {}", e.message);
    }
    Ok(outcome)
}

/// [`save_path_with_sidecar`] 的核心编排：读写经 `io` 注入（生产传 [`RealPathHiveIo`]，
/// 测试传 fake，**全程不碰真实注册表**）。
///
/// 编排顺序：
/// 1. best-effort 补写上次未落盘的 pending（失败仅告警，不阻断）；
/// 2. 对需校验的 hive 先做**乐观并发校验**（只读，不写）；校验失败以
///    [`HiveOutcome::Failed`]（`code = Conflict`）表达，该 hive 不写、不产 pending；
/// 3. **至少一个 hive 确实将写入**时，先写前备份（失败仅告警，不阻断）；
/// 4. 逐 hive 写注册表（`None` 侧跳过）；
/// 5. 任一 hive 写成功后广播环境变更；
/// 6. 仅对注册表写成功的 hive 提交 sidecar 快照；快照失败落 pending。
///
/// **备份时机**：在**校验通过、确实将写入**之后、真正写入之前。这样校验失败的
/// 调用**不产生无谓备份文件**——否则一次并发冲突就会留下一份没人需要的备份。
pub(crate) fn save_path_with_sidecar_io(
    io: &dyn PathHiveIo,
    system: Option<PathHiveWrite>,
    user: Option<PathHiveWrite>,
) -> Result<ApplyOutcome, CoreError> {
    // pending 补写是本次保存的前置内部步骤，不再单独记事件——否则一次保存
    // 会产生两条事件（Task 1 的 B2 口径：事件由最外层操作负责）。
    if let Err(e) = retry_pending_path_state(&NoopHistory) {
        // pending 补写是 best-effort：失败不阻断本次写入，只告警（W2-N1 分层）。
        log::warn!("补写上次待落盘快照失败: {}", e.message);
    }

    // 步骤 2：乐观并发校验（只读）。校验通过则返回 ()，失败返回 Conflict。
    // 单独出来做，是为了让「是否将写入」在备份之前就已知。
    let sys_check = check_expected(io, EnvHive::System, system.as_ref());
    let usr_check = check_expected(io, EnvHive::User, user.as_ref());
    let sys_will_write = system.is_some() && sys_check.is_ok();
    let usr_will_write = user.is_some() && usr_check.is_ok();

    // 步骤 3：确认将发生写入时备份（K3）。
    //
    // 真实现经 `RealPathHiveIo::backup` → `backup::backup_registry(None)`，
    // 走默认备份目录，从而沿用 `rotate_path_backups` 的默认目录限定（N4 裁断：
    // 自定义目录不参与轮换，可能混有用户自己的文件）。
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

    // sidecar 只记录注册表写成功的 hive；注册表是启用路径的真相来源。
    let sidecar = if sys_applied || usr_applied {
        let sys_snap = if sys_applied {
            system.map(|w| w.entries)
        } else {
            None
        };
        let usr_snap = if usr_applied {
            user.map(|w| w.entries)
        } else {
            None
        };
        commit_sidecar(sys_snap, usr_snap)
    } else {
        SidecarOutcome::Saved
    };

    Ok(ApplyOutcome {
        system: sys_outcome,
        user: usr_outcome,
        sidecar,
    })
}

/// 补写上次未落盘的快照（**sidecar-only**，不触碰注册表）。
///
/// pending 的产生前提就是注册表已写成功，补写只把完整有序快照写回
/// `disabled.json` 并清除 pending 文件（W2-B2 裁决）。无 pending 时返回
/// 全 `Skipped` + `Saved` 的空结果。
///
/// 本函数返回 `Result` 是诚实报告（W2-N1）；CLI 策略层把 `Err` 映射为
/// 警告、不阻断当前命令。
pub fn retry_pending_path_state(history: &dyn HistorySink) -> Result<ApplyOutcome, CoreError> {
    let pending = disabled::load_pending_path_snapshot()
        .map_err(|e| core_err(e.code, "retry_pending_path_state", e.message))?;
    let Some(pending) = pending else {
        return Ok(ApplyOutcome {
            system: HiveOutcome::Skipped,
            user: HiveOutcome::Skipped,
            sidecar: SidecarOutcome::Saved,
        });
    };

    disabled::save_path_snapshot(Some(pending.system), Some(pending.user))
        .map_err(|e| core_err(e.code, "retry_pending_path_state", e.message))?;
    // 清除失败不阻断：pending 是幂等整份快照，下次补写重放同样内容（Wave 1 语义）。
    let _ = disabled::clear_pending_path_snapshot();

    // 补写也是一次可追溯的操作，记一条事件（失败不阻断）。
    if let Err(e) = history.record(HistoryEvent {
        source: EventSource::Gui,
        action: EventAction::Save,
        hive: None,
        target: None,
        changed_fields: vec!["pending_retry".to_string()],
        before_revision: None,
        after_revision: None,
        result: EventResult::Success,
        snapshot_uid: None,
    }) {
        log::warn!("补写历史失败（pending 已补写）: {}", e.message);
    }

    Ok(ApplyOutcome {
        system: HiveOutcome::Skipped,
        user: HiveOutcome::Skipped,
        sidecar: SidecarOutcome::Saved,
    })
}

/// 提交 sidecar 快照（仅落盘、不触碰注册表），供「注册表已另行写入」的调用方使用。
///
/// 这是 Wave 1 CLI `persist_snapshot` 的服务层化：注册表由调用方（带乐观并发
/// 校验的路径）写入后，调用本函数提交快照并处理 pending 语义。失败不返回
/// `Err`——注册表已写成功的事实必须完整表达，故以 [`SidecarOutcome::Pending`]
/// / [`SidecarOutcome::Failed`] 携带错误在 `Ok` 中诚实报告。
pub fn commit_sidecar_snapshot(
    system: Option<Vec<PathEntry>>,
    user: Option<Vec<PathEntry>>,
) -> Result<ApplyOutcome, CoreError> {
    let sidecar = commit_sidecar(system, user);
    Ok(ApplyOutcome {
        system: HiveOutcome::Skipped,
        user: HiveOutcome::Skipped,
        sidecar,
    })
}

/// 应用一个配置文件（多 hive；语义为 best-effort，逐 hive 结果可见）。
///
/// 内部完成：best-effort 补写 pending → 读取配置 → 写注册表 → 广播 →
/// 落 sidecar。配置不存在或名称非法返回 `Err`（诚实报告）。
pub fn apply_profile(name: &str, history: &dyn HistorySink) -> Result<ApplyOutcome, CoreError> {
    let data =
        profiles::load_profile(name).map_err(|e| core_err(e.code, "apply_profile", e.message))?;
    save_path_with_sidecar(
        Some(PathHiveWrite {
            entries: data.sys,
            expected: None,
        }),
        Some(PathHiveWrite {
            entries: data.user,
            expected: None,
        }),
        history,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disabled;
    use crate::path_entry::PathEntry;
    use std::cell::RefCell;

    fn entry(path: &str, enabled: bool) -> PathEntry {
        PathEntry {
            path: path.to_string(),
            enabled,
        }
    }

    /// 构造「无并发校验」的写入请求（`expected: None`），简化测试调用。
    fn write(entries: Vec<PathEntry>) -> PathHiveWrite {
        PathHiveWrite {
            entries,
            expected: None,
        }
    }

    /// fake `PathHiveIo` 记录的一次调用事件。
    ///
    /// 顺序类契约（`writes_are_sequential_system_then_user`）**无法从终态反推**——
    /// 两次写入都成功时，终态看不出谁先谁后。故 fake 必须记录**有序事件序列**，
    /// 而非布尔标志。
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum IoCall {
        Read(EnvHive),
        Write(EnvHive),
        Backup,
    }

    /// 可注入的 fake `PathHiveIo`：记录调用序列，**不碰真实注册表**。
    ///
    /// 用 `RefCell` 而非 `Mutex`：契约测试单线程，`&dyn PathHiveIo` 的 `&self`
    /// 方法需要可变记录，`RefCell` 足够且无锁开销。
    struct FakePathIo {
        calls: RefCell<Vec<IoCall>>,
        /// `read_enabled` 返回的「当前启用路径」；用于并发比对。
        ///
        /// 用有序 `Vec` 而非 `HashMap`：`EnvHive` **未派生 `Hash`**，而契约测试
        /// 的自用结构**不得反向要求生产类型加派生**（那会扩进 `env_var.rs`）。
        current: RefCell<Vec<(EnvHive, Vec<String>)>>,
        /// 置位后 `write_enabled` 返回错误（模拟注册表写失败）。
        fail_write: bool,
        /// 置位后 `backup` 返回错误（模拟备份失败）。
        fail_backup: bool,
    }

    impl FakePathIo {
        fn new() -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                current: RefCell::new(Vec::new()),
                fail_write: false,
                fail_backup: false,
            }
        }

        /// 预置某 hive 的「当前启用路径」（供并发比对用）。
        fn with_current(self, hive: EnvHive, paths: &[&str]) -> Self {
            self.current
                .borrow_mut()
                .push((hive, paths.iter().map(|p| p.to_string()).collect()));
            self
        }

        /// 读某 hive 的当前值（未预置则视为空）。
        fn current_of(&self, hive: EnvHive) -> Vec<String> {
            self.current
                .borrow()
                .iter()
                .find(|(h, _)| *h == hive)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        }

        fn calls(&self) -> Vec<IoCall> {
            self.calls.borrow().clone()
        }

        /// 提取「写入事件」的 hive 序列（供顺序断言）。
        fn write_order(&self) -> Vec<EnvHive> {
            self.calls()
                .into_iter()
                .filter_map(|c| match c {
                    IoCall::Write(h) => Some(h),
                    _ => None,
                })
                .collect()
        }

        fn backup_count(&self) -> usize {
            self.calls()
                .iter()
                .filter(|c| matches!(c, IoCall::Backup))
                .count()
        }
    }

    impl PathHiveIo for FakePathIo {
        fn read_enabled(&self, hive: EnvHive) -> Result<Vec<String>, CoreError> {
            self.calls.borrow_mut().push(IoCall::Read(hive));
            Ok(self.current_of(hive))
        }

        fn write_enabled(&self, hive: EnvHive, paths: &[String]) -> Result<(), CoreError> {
            self.calls.borrow_mut().push(IoCall::Write(hive));
            if self.fail_write {
                return Err(CoreError::new(
                    ErrorCode::Io,
                    "write_enabled",
                    "注入的写失败",
                ));
            }
            let mut cur = self.current.borrow_mut();
            if let Some(slot) = cur.iter_mut().find(|(h, _)| *h == hive) {
                slot.1 = paths.to_vec();
            } else {
                cur.push((hive, paths.to_vec()));
            }
            Ok(())
        }

        fn backup(&self) -> Result<(), CoreError> {
            self.calls.borrow_mut().push(IoCall::Backup);
            if self.fail_backup {
                return Err(CoreError::new(
                    ErrorCode::Io,
                    "backup",
                    "注入的备份失败（磁盘已满）",
                ));
            }
            Ok(())
        }
    }

    // pending / disabled.json 在测试下共享固定 temp 路径（Wave 1 约定），
    // 与 disabled.rs 的生命周期测试存在并行竞态窗口（F-11 起由
    // `persist::test_persist_lock` 序列化，见 persist.rs）。

    // ── Wave 3.0-B 契约测试（K7）：经 fake `PathHiveIo`，零真实注册表 ──

    /// 契约 3：system 先于 user（顺序写，D3）。
    ///
    /// 顺序**无法从终态反推**，故断言 fake 记录的写入事件序列。
    #[test]
    fn writes_are_sequential_system_then_user() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        let io = FakePathIo::new();
        let sys = vec![entry("C:\\svc_seq_sys", true)];
        let usr = vec![entry("D:\\svc_seq_usr", true)];

        save_path_with_sidecar_io(&io, Some(write(sys)), Some(write(usr))).unwrap();

        assert_eq!(
            io.write_order(),
            vec![EnvHive::System, EnvHive::User],
            "两个 hive 都写时必须是 system → user 顺序（D3）"
        );
    }

    /// 契约 1：确实发生写入时 `backup()` 被调用**恰好一次**。
    ///
    /// Task 2 之前此测试 RED（尚无备份调用）。
    #[test]
    fn backup_called_once_when_write_happens() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        let io = FakePathIo::new();
        let sys = vec![entry("C:\\svc_bak_sys", true)];

        save_path_with_sidecar_io(&io, Some(write(sys)), None).unwrap();

        assert_eq!(
            io.backup_count(),
            1,
            "有写入时 backup() 必须被调用恰好一次（Task 2）"
        );
    }

    /// 契约 1 的一半：**无变更时不备份**。
    ///
    /// 与上一条互为补集——只断言「有写入则备份」不够，还要钉住「无写入则不备份」，
    /// 否则「无条件备份」的实现也能让上一条通过。
    #[test]
    fn no_backup_when_both_hives_skipped() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        let io = FakePathIo::new();

        save_path_with_sidecar_io(&io, None, None).unwrap();

        assert_eq!(
            io.backup_count(),
            0,
            "两个 hive 都 Skipped 时不得备份（无写入发生）"
        );
    }

    /// 契约 2：并发不匹配 → 该 hive **不写、不备份、不产 pending**（D5 / N1 不变量 2）。
    ///
    /// 三条「不」缺一不可：只断言「不写」不够——若实现仍备份，等于并发冲突还留下
    /// 一份无用备份；若仍走 `commit_sidecar`，则污染 sidecar。
    #[test]
    fn concurrent_mismatch_writes_nothing_and_no_backup() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        // 注册表当前是 ["C:\\orig"]，但期望是 ["C:\\stale"]（模拟外部已改动）。
        let io = FakePathIo::new().with_current(EnvHive::System, &["C:\\orig"]);
        let req = PathHiveWrite {
            entries: vec![entry("C:\\new", true)],
            expected: Some(vec!["C:\\stale".to_string()]),
        };

        let outcome = save_path_with_sidecar_io(&io, Some(req), None).unwrap();

        // 不写该 hive，且错误码为 Conflict
        match outcome.system {
            HiveOutcome::Failed(e) => {
                assert_eq!(e.code, ErrorCode::Conflict, "并发不匹配必须是 Conflict");
            }
            other => panic!("期望 HiveOutcome::Failed(Conflict)，实为 {other:?}"),
        }
        assert!(
            io.write_order().is_empty(),
            "并发不匹配时不得发生任何 write_enabled（不写）"
        );
        assert_eq!(io.backup_count(), 0, "并发不匹配时不得备份");
        assert!(
            !disabled::has_pending_path_snapshot(),
            "并发不匹配时不得产生 pending"
        );
    }

    /// 契约 2 的补集：`expected` **匹配**时正常写入（证明校验不是恒拒绝）。
    #[test]
    fn concurrent_match_proceeds_with_write() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        let io = FakePathIo::new().with_current(EnvHive::System, &["C:\\orig"]);
        let req = PathHiveWrite {
            entries: vec![entry("C:\\new", true)],
            expected: Some(vec!["C:\\orig".to_string()]),
        };

        let outcome = save_path_with_sidecar_io(&io, Some(req), None).unwrap();

        assert!(matches!(outcome.system, HiveOutcome::Applied));
        assert_eq!(io.write_order(), vec![EnvHive::System]);
        assert_eq!(io.backup_count(), 1, "匹配且写入时应有备份");
    }

    // ── Wave 3.1 契约测试：共存校验的顺序敏感修复（K1）──
    //
    // 背景：`expected` 来自 `merge_hive` 输出，其**成员忠实于注册表、顺序取自快照**；
    // `current` 是**注册表顺序**。二者在外部重排后必然不等 → 顺序敏感比较会**永久
    // 拒绝保存**（死锁）。修复：改为**多重集比较**（忽略顺序、保留重复计数）。
    //
    // ⚠️ 三条测试分工：第 1 条驱动修复（现状 RED）；第 2/3 条**守护**——现状即 GREEN，
    // 用来防止修复被误实现成「关掉校验」。

    /// 契约 W3.1-1：**仅顺序不同**（同集合）→ 校验**通过**（现状 RED）。
    #[test]
    fn reorder_only_does_not_conflict() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        let io = FakePathIo::new().with_current(EnvHive::System, &["C:\\a", "C:\\b", "C:\\c"]);
        // 期望值与当前值同集合，但顺序不同（模拟外部重排 / 快照顺序陈旧）。
        let req = PathHiveWrite {
            entries: vec![entry("C:\\new", true)],
            expected: Some(vec!["C:\\c".into(), "C:\\a".into(), "C:\\b".into()]),
        };

        let outcome = save_path_with_sidecar_io(&io, Some(req), None).unwrap();

        assert!(
            matches!(outcome.system, HiveOutcome::Applied),
            "仅顺序不同不得判冲突（死锁修复），实为 {:?}",
            outcome.system
        );
    }

    /// 契约 W3.1-2【守护】：**成员不同**（增/删）→ **仍拒绝**。
    ///
    /// 防止修复被误实现成「恒通过」——真实的成员变化必须仍被检出。
    #[test]
    fn membership_change_still_conflicts() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        let io = FakePathIo::new().with_current(EnvHive::System, &["C:\\a", "C:\\b"]);
        // 期望值少一条（模拟外部删除）；即使排序后也不相等。
        let req = PathHiveWrite {
            entries: vec![entry("C:\\new", true)],
            expected: Some(vec!["C:\\a".into()]),
        };

        let outcome = save_path_with_sidecar_io(&io, Some(req), None).unwrap();

        match outcome.system {
            HiveOutcome::Failed(e) => assert_eq!(e.code, ErrorCode::Conflict),
            other => panic!("成员变化必须判 Conflict，实为 {other:?}"),
        }
        assert!(io.write_order().is_empty(), "冲突时不得写入");
    }

    /// 契约 W3.1-3【守护】：同集合但**重复计数不同** → **拒绝**（多重集而非集合）。
    ///
    /// 防止修复被误实现成「`HashSet` 集合比较」——那会吞掉重复项。
    #[test]
    fn duplicate_count_matters() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        let io = FakePathIo::new().with_current(EnvHive::System, &["C:\\a", "C:\\a", "C:\\b"]);
        // 期望值：`C:\a` 只出现一次（去重后的集合相同，但多重集不同）。
        let req = PathHiveWrite {
            entries: vec![entry("C:\\new", true)],
            expected: Some(vec!["C:\\a".into(), "C:\\b".into()]),
        };

        let outcome = save_path_with_sidecar_io(&io, Some(req), None).unwrap();

        match outcome.system {
            HiveOutcome::Failed(e) => assert_eq!(e.code, ErrorCode::Conflict),
            other => panic!("重复计数不同必须判 Conflict（多重集语义），实为 {other:?}"),
        }
    }

    #[test]
    fn apply_outcome_serializes_camel_case() {
        let outcome = ApplyOutcome {
            system: HiveOutcome::Applied,
            user: HiveOutcome::Skipped,
            sidecar: SidecarOutcome::Saved,
        };
        let v = serde_json::to_value(&outcome).unwrap();
        assert_eq!(v["system"], serde_json::json!("applied"));
        assert_eq!(v["user"], serde_json::json!("skipped"));
        assert_eq!(v["sidecar"], serde_json::json!("saved"));

        // Failed 变体携带结构化错误，且错误码为 camelCase
        let failed = ApplyOutcome {
            system: HiveOutcome::Skipped,
            user: HiveOutcome::Failed(CoreError::new(ErrorCode::Io, "op", "磁盘已满")),
            sidecar: SidecarOutcome::Pending(CoreError::new(ErrorCode::Io, "op", "磁盘已满")),
        };
        let v = serde_json::to_value(&failed).unwrap();
        assert_eq!(v["user"]["failed"]["code"], serde_json::json!("io"));
        assert_eq!(
            v["sidecar"]["pending"]["message"],
            serde_json::json!("磁盘已满")
        );

        // 反序列化 roundtrip（W2-B3 要求 Deserialize）
        let back: ApplyOutcome = serde_json::from_value(v).unwrap();
        assert!(matches!(back.user, HiveOutcome::Failed(_)));
        assert!(matches!(back.sidecar, SidecarOutcome::Pending(_)));
    }

    #[test]
    fn sidecar_outcome_enum_semantics() {
        // 三态语义可区分：Saved / Pending（可补写）/ Failed（需人工核对）
        let saved: SidecarOutcome = SidecarOutcome::Saved;
        let pending = SidecarOutcome::Pending(CoreError::new(ErrorCode::Io, "op", "a"));
        let failed = SidecarOutcome::Failed(CoreError::new(ErrorCode::Io, "op", "b"));
        assert!(matches!(saved, SidecarOutcome::Saved));
        assert!(matches!(pending, SidecarOutcome::Pending(_)));
        assert!(matches!(failed, SidecarOutcome::Failed(_)));
    }

    // W2-B2 注入机制：输入校验（null 字节）使 validate_and_join_paths 在触碰
    // 注册表前失败 —— 与「注册表写失败」走同一段注册表阶段失败路径。
    // 断言：HiveOutcome::Failed、无 pending 产生、sidecar 未被触及。
    //
    // **用 fake IO**（而非真实 `RealPathHiveIo`）：Task 3 起 `save_path_with_sidecar`
    // 会在写入前调 `io.backup()`，真实现会落真实备份文件到 `~/.patheditor/backups/`
    // —— 违反「Task 1-7 不写真实注册表/不产生真实副作用」。fake 记录调用且零副作用。
    #[test]
    fn registry_stage_failure_reports_failed_without_pending() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        let mut io = FakePathIo::new();
        io.fail_write = true; // 模拟注册表写失败（等价于输入校验失败）
        let entries = vec![entry("C:\\ok", true), entry("C:\0bad", true)];
        let outcome = save_path_with_sidecar_io(&io, Some(write(entries)), None).unwrap();
        assert!(matches!(outcome.system, HiveOutcome::Failed(_)));
        assert!(matches!(outcome.user, HiveOutcome::Skipped));
        assert!(matches!(outcome.sidecar, SidecarOutcome::Saved));
        assert!(
            !disabled::has_pending_path_snapshot(),
            "注册表阶段失败不得产生 pending"
        );
    }

    // 同类注入：超长（>32767 UTF-16）条目，走用户 hive，验证 partial 形态
    // （system Skipped / user Failed，W2-B1 可表达性）。同样经 fake IO。
    #[test]
    fn oversized_user_entry_yields_partial_outcome_without_pending() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        let mut io = FakePathIo::new();
        io.fail_write = true;
        let long = format!("D:\\{}", "a".repeat(32767));
        let outcome =
            save_path_with_sidecar_io(&io, None, Some(write(vec![entry(&long, true)]))).unwrap();
        assert!(matches!(outcome.system, HiveOutcome::Skipped));
        assert!(matches!(outcome.user, HiveOutcome::Failed(_)));
        assert!(!disabled::has_pending_path_snapshot());
    }

    // flush 重放（sidecar-only）：注入 pending 文件 → retry 补写并清除。
    // 不触碰注册表：system/user 恒为 Skipped（W2-B2 裁断）。
    #[test]
    fn retry_replays_pending_sidecar_only_and_clears_it() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        let sys = vec![
            entry("C:\\svc_test_sys", true),
            entry("C:\\svc_test_disabled", false),
        ];
        let usr = vec![entry("D:\\svc_test_usr", false)];
        disabled::save_pending_path_snapshot(Some(sys), Some(usr)).unwrap();
        assert!(disabled::has_pending_path_snapshot());

        let outcome = retry_pending_path_state(&NoopHistory).unwrap();
        assert!(matches!(outcome.system, HiveOutcome::Skipped));
        assert!(matches!(outcome.user, HiveOutcome::Skipped));
        assert!(matches!(outcome.sidecar, SidecarOutcome::Saved));
        assert!(
            !disabled::has_pending_path_snapshot(),
            "补写成功后 pending 必须被清除"
        );
    }

    #[test]
    fn apply_profile_missing_reports_error() {
        // 配置不存在 → Err（诚实报告），不触碰注册表
        let result = apply_profile("__svc_no_such_profile__", &NoopHistory);
        assert!(result.is_err());
    }
}
