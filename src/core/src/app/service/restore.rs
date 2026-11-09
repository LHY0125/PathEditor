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

    let mut system = Vec::new();
    let mut user = Vec::new();
    for row in &view.env {
        let Some(value) = row.value.clone() else {
            // 快照读取已传 reveal=true；仍为空说明该行不可恢复，跳过
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
        captured_at: view.created_at,
        hives: EnvBackupHives { system, user },
    })
}

/// 把历史快照里的环境变量恢复到注册表（生产入口）。
///
/// `force = false`：当前值自快照后被修改过则整批中止；
/// `force = true`：按快照覆盖，并删除快照中不存在的可写变量。
pub fn restore_env_snapshot(
    db: &Db,
    uid: &str,
    force: bool,
) -> Result<backup::RestoreOutcome, CoreError> {
    let view = history::get_snapshot(db, uid, true)?;
    let payload = snapshot_env_payload(&view)?;
    backup::restore_env_payload(&payload, force)
}

/// [`restore_env_snapshot`] 的存储注入版本（测试不触碰真实注册表）。
pub fn restore_env_snapshot_in_stores(
    sys: &dyn crate::platform::registry::store::EnvHiveStore,
    usr: &dyn crate::platform::registry::store::EnvHiveStore,
    db: &Db,
    uid: &str,
    force: bool,
) -> Result<backup::RestoreOutcome, CoreError> {
    let view = history::get_snapshot(db, uid, true)?;
    let payload = snapshot_env_payload(&view)?;
    backup::restore_payload_in_stores(sys, usr, &payload, force)
}
