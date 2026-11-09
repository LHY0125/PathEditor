//! 历史快照恢复：读快照 → 走保存编排（append-only）。
//!
//! PATH 与环境变量分开恢复：PATH 走注册表保存编排；环境变量复用 `.pebak`
//! 恢复的差异/冲突/删除逻辑。快照本身同时记录两者。

use super::*;
use crate::db::history::SnapshotView;
use crate::domain::env_var::EnvValueKind;
use crate::platform::backup::{EnvBackupHives, EnvBackupPayload, EnvBackupVar};

/// 把历史快照恢复回注册表。
///
/// 「读快照 → 写注册表与当前快照 → 记录恢复事件」是产品语义，GUI/CLI 共用。
/// 恢复是 **append-only**：成功后追加新快照与事件，旧快照原样保留——
/// 这样「恢复」本身也可被撤销。
pub fn restore_snapshot(
    db: &mut Db,
    uid: &str,
    source: EventSource,
) -> Result<ApplyOutcome, CoreError> {
    let env = collect_env_snapshot_rows().unwrap_or_else(|e| {
        log::warn!(
            "采集环境变量快照失败（恢复生成的快照只含 PATH）: {}",
            e.message
        );
        Vec::new()
    });
    restore_snapshot_io(
        &RealPathHiveIo,
        db,
        uid,
        source,
        &disabled::default_pending_path(),
        env,
    )
}

/// [`restore_snapshot`] 的可注入版本（测试用 fake 注册表与 `TempDir` pending 路径）。
pub fn restore_snapshot_io(
    io: &dyn PathHiveIo,
    db: &mut Db,
    uid: &str,
    source: EventSource,
    pending_path: &std::path::Path,
    env: Vec<EnvValueRow>,
) -> Result<ApplyOutcome, CoreError> {
    // 恢复的是 PATH（不含 env 值），因此读取时不需要 reveal。
    let view = history::get_snapshot(db, uid, false)?;
    let outcome = save_path_with_db_io(
        io,
        db,
        source,
        Some(PathHiveWrite {
            entries: view.system,
            expected: None,
        }),
        Some(PathHiveWrite {
            entries: view.user,
            expected: None,
        }),
        pending_path,
        env,
    )?;
    // 调用方只按 `Result` 判成败（GUI 的「恢复快照」成功后直接清空详情），
    // 逐 hive 失败必须在这里抬成 Err，否则「恢复」在注册表没改时也报成功。
    super::fail_on_hive_error(&outcome)?;
    Ok(outcome)
}

/// 把快照里的环境变量还原成备份载荷。
///
/// **空载荷拒绝恢复**：旧版本快照没有采集 env，空载荷按 `.pebak` 语义会把
/// 当前所有可写环境变量删光。这里必须把「没有数据」与「恢复到一个空环境」区分开。
fn snapshot_env_payload(view: &SnapshotView) -> Result<EnvBackupPayload, CoreError> {
    if view.env.is_empty() {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "restore_env_snapshot",
            "该快照不包含环境变量数据（可能由旧版本创建或采集失败）",
        ));
    }
    env_payload_from_rows(&view.env, view.created_at)
}

/// 把快照 env 行转换为备份载荷（配置应用与快照恢复共用）。
pub(crate) fn env_payload_from_rows(
    rows: &[EnvValueRow],
    captured_at: i64,
) -> Result<EnvBackupPayload, CoreError> {
    let mut system = Vec::new();
    let mut user = Vec::new();
    for row in rows {
        let Some(value) = row.value.clone() else {
            continue;
        };
        if !row.restorable {
            continue;
        }
        let kind = match row.kind.to_ascii_lowercase().as_str() {
            "expandstring" | "expand_string" => EnvValueKind::ExpandString,
            _ => EnvValueKind::String,
        };
        let var = EnvBackupVar {
            name: row.name.clone(),
            kind,
            value,
            revision: row.revision.clone(),
        };
        if row.hive == "system" {
            system.push(var);
        } else {
            user.push(var);
        }
    }

    if system.is_empty() && user.is_empty() {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "restore_env_snapshot",
            "该快照没有可恢复的环境变量值",
        ));
    }

    Ok(EnvBackupPayload {
        captured_at,
        hives: EnvBackupHives { system, user },
    })
}

/// 把历史快照里的环境变量恢复到注册表（生产入口）。
///
/// `force = false`：当前值自快照后被修改过则整批中止；
/// `force = true`：按快照覆盖，并删除快照中不存在的可写变量。
pub fn restore_env_snapshot(
    db: &mut Db,
    source: EventSource,
    uid: &str,
    force: bool,
) -> Result<backup::RestoreOutcome, CoreError> {
    let view = history::get_snapshot(db, uid, true)?;
    let payload = snapshot_env_payload(&view)?;
    let outcome = backup::restore_env_payload(&payload, force)?;
    record_env_restore_event(db, source, uid, &outcome);
    Ok(outcome)
}

/// [`restore_env_snapshot`] 的存储注入版本（测试不触碰真实注册表）。
pub fn restore_env_snapshot_in_stores(
    sys: &dyn crate::platform::registry::store::EnvHiveStore,
    usr: &dyn crate::platform::registry::store::EnvHiveStore,
    db: &mut Db,
    source: EventSource,
    uid: &str,
    force: bool,
) -> Result<backup::RestoreOutcome, CoreError> {
    let view = history::get_snapshot(db, uid, true)?;
    let payload = snapshot_env_payload(&view)?;
    let outcome = backup::restore_payload_in_stores(sys, usr, &payload, force)?;
    record_env_restore_event(db, source, uid, &outcome);
    Ok(outcome)
}

fn record_env_restore_event(
    db: &mut Db,
    source: EventSource,
    uid: &str,
    outcome: &backup::RestoreOutcome,
) {
    let result = if outcome.failures.is_empty() {
        EventResult::Success
    } else if outcome.applied == 0 {
        EventResult::Failure
    } else {
        EventResult::Partial
    };
    let event = crate::domain::history::HistoryEvent {
        source,
        action: EventAction::EnvRestore,
        hive: None,
        target: None,
        changed_fields: vec!["env_values".to_string()],
        before_revision: None,
        after_revision: None,
        result,
        snapshot_uid: Some(uid.to_string()),
    };
    if let Err(e) = DbHistory::new(db).record(event) {
        log::warn!("环境变量恢复审计写入失败（恢复已执行）: {}", e.message);
    }
}

/// 一次性恢复整份快照（PATH + env）。
///
/// 先恢复 env、再恢复 PATH：env 失败时整次中止，不会出现「PATH 已改、env 没改」；
/// PATH 失败时 env 已生效，错误原样返回（注册表跨 hive 无法做到真正原子）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullRestoreOutcome {
    /// PATH 恢复结果
    pub path: ApplyOutcome,
    /// env 恢复结果；快照没有 env 数据时为 `None`
    pub env: Option<backup::RestoreOutcome>,
}

/// 恢复整份快照：PATH 与 env 一起还原。
pub fn restore_snapshot_full(
    db: &mut Db,
    source: EventSource,
    uid: &str,
) -> Result<FullRestoreOutcome, CoreError> {
    let view = history::get_snapshot(db, uid, false)?;
    let env = if view.env.is_empty() {
        None
    } else {
        Some(restore_env_snapshot(db, source, uid, true)?)
    };
    let path = restore_snapshot(db, uid, source)?;
    Ok(FullRestoreOutcome { path, env })
}
