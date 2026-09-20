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

use crate::disabled;
use crate::env_var::EnvHive;
use crate::error::{CoreError, ErrorCode};
use crate::path_entry::PathEntry;
use crate::profiles;
use crate::registry;
use crate::system;

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

/// 一次 PATH ��用（注册表 + sidecar）的整体结果。
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

/// 把某 hive 的启用条目写入注册表；失败不 panic，转为 `HiveOutcome::Failed`。
///
/// 校验（null 字节 / 32767 上限）发生在 `registry::save_*_paths` 内部、
/// 触碰注册表之前，因此校验失败与注册表写失败走同一段「注册表阶段失败」路径，
/// 均不会产生 pending（W2-B2 注入机制）。
fn write_hive_registry(hive: EnvHive, entries: &[PathEntry]) -> HiveOutcome {
    let enabled: Vec<String> = entries
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| entry.path.clone())
        .collect();
    let result = match hive {
        EnvHive::System => registry::save_system_paths(enabled),
        EnvHive::User => registry::save_user_paths(enabled),
    };
    match result {
        Ok(()) => HiveOutcome::Applied,
        Err(e) => HiveOutcome::Failed(core_err(
            ErrorCode::Internal,
            "write_hive_registry",
            format!(
                "{} PATH 注册表写入失败: {e}",
                if hive == EnvHive::System {
                    "系统"
                } else {
                    "用户"
                }
            ),
        )),
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
            // 调用方按 Failed 语义提示手工核对。
            Err(_pe) => Err(CoreError::new(
                ErrorCode::Internal,
                "commit_sidecar",
                String::new(),
            )),
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
) -> Result<ApplyOutcome, CoreError> {
    match hive {
        EnvHive::System => save_path_with_sidecar(Some(entries), None),
        EnvHive::User => save_path_with_sidecar(None, Some(entries)),
    }
}

/// 同时写注册表与 sidecar，明确 partial 语义（best-effort，逐 hive 尝试）。
///
/// 编排顺序与 Wave 1 一致：
/// 1. best-effort 补写上次未落盘的 pending（失败仅告警，不阻断）；
/// 2. 逐 hive 写注册表（`None` 侧跳过），失败以 [`HiveOutcome::Failed`] 表达；
/// 3. 任一 hive 写成功后广播环境变更；
/// 4. 仅对注册表写成功的 hive 提交 sidecar 快照；快照失败落 pending。
///
/// **注意**：本函数不做乐观并发校验（读-比-写）。CLI 的 `verify_and_save`
/// 保留在 CLI 层（保守方案，见 Task 5 报告）；需要校验的调用方应自行先校验。
pub fn save_path_with_sidecar(
    system: Option<Vec<PathEntry>>,
    user: Option<Vec<PathEntry>>,
) -> Result<ApplyOutcome, CoreError> {
    if let Err(e) = retry_pending_path_state() {
        // pending 补写是 best-effort：失败不阻断本次写入，只告警（W2-N1 分层）。
        log::warn!("补写上次待落盘快照失败: {}", e.message);
    }

    let sys_outcome = system
        .as_ref()
        .map(|entries| write_hive_registry(EnvHive::System, entries))
        .unwrap_or(HiveOutcome::Skipped);
    let usr_outcome = user
        .as_ref()
        .map(|entries| write_hive_registry(EnvHive::User, entries))
        .unwrap_or(HiveOutcome::Skipped);

    let sys_applied = matches!(sys_outcome, HiveOutcome::Applied);
    let usr_applied = matches!(usr_outcome, HiveOutcome::Applied);

    if sys_applied || usr_applied {
        system::broadcast_env_change();
    }

    // sidecar 只记录注册表写成功的 hive；注册表是启用路径的真相来源。
    let sidecar = if sys_applied || usr_applied {
        let sys_snap = if sys_applied { system.clone() } else { None };
        let usr_snap = if usr_applied { user.clone() } else { None };
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
pub fn retry_pending_path_state() -> Result<ApplyOutcome, CoreError> {
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
pub fn apply_profile(name: &str) -> Result<ApplyOutcome, CoreError> {
    let data =
        profiles::load_profile(name).map_err(|e| core_err(e.code, "apply_profile", e.message))?;
    save_path_with_sidecar(Some(data.sys), Some(data.user))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disabled;
    use crate::path_entry::PathEntry;

    fn entry(path: &str, enabled: bool) -> PathEntry {
        PathEntry {
            path: path.to_string(),
            enabled,
        }
    }

    /// pending / disabled.json 在测试下共享固定 temp 路径（Wave 1 约定），
    /// 与 disabled.rs 的生命周期测试存在并行竞态窗口（F-11 起由
    /// `persist::test_persist_lock` 序列化，见 persist.rs）。
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
    #[test]
    fn registry_stage_failure_reports_failed_without_pending() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        let entries = vec![entry("C:\\ok", true), entry("C:\0bad", true)];
        let outcome = save_path_with_sidecar(Some(entries), None).unwrap();
        assert!(matches!(outcome.system, HiveOutcome::Failed(_)));
        assert!(matches!(outcome.user, HiveOutcome::Skipped));
        assert!(matches!(outcome.sidecar, SidecarOutcome::Saved));
        assert!(
            !disabled::has_pending_path_snapshot(),
            "注册表阶段失败不得产生 pending"
        );
    }

    // 同类注入：超长（>32767 UTF-16）条目，走用户 hive，验证 partial 形态
    // （system Skipped / user Failed，W2-B1 可表达性）。
    #[test]
    fn oversized_user_entry_yields_partial_outcome_without_pending() {
        let _guard = crate::persist::test_persist_lock();
        let _ = disabled::clear_pending_path_snapshot();
        let long = format!("D:\\{}", "a".repeat(32767));
        let outcome = save_path_with_sidecar(None, Some(vec![entry(&long, true)])).unwrap();
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

        let outcome = retry_pending_path_state().unwrap();
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
        let result = apply_profile("__svc_no_such_profile__");
        assert!(result.is_err());
    }
}
