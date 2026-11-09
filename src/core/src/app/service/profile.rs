//! 配置方案应用：读配置 → 只写变化的 hive → 记账。

use super::*;

/// 应用一个配置方案：读配置内容 → 写注册表与当前快照 → 记录应用次数。
///
/// GUI 与 CLI 共用同一编排；记账失败只告警，不影响保存结果
/// （注册表已写成功的事实优先于统计）。
///
/// 配置不存在或没有内容时返回 `NotFound`，不触碰注册表。
pub fn apply_profile(
    db: &mut Db,
    name: &str,
    source: EventSource,
) -> Result<ApplyOutcome, CoreError> {
    let env = super::collect_env_snapshot_rows().unwrap_or_else(|e| {
        log::warn!(
            "采集环境变量快照失败（本次配置应用快照只含 PATH）: {}",
            e.message
        );
        Vec::new()
    });
    apply_profile_io(
        &RealPathHiveIo,
        db,
        name,
        source,
        &disabled::default_pending_path(),
        env,
    )
}

/// [`apply_profile`] 的可注入版本（测试用 fake 注册表与 `TempDir` pending 路径）。
pub fn apply_profile_io(
    io: &dyn PathHiveIo,
    db: &mut Db,
    name: &str,
    source: EventSource,
    pending_path: &std::path::Path,
    env: Vec<EnvValueRow>,
) -> Result<ApplyOutcome, CoreError> {
    let Some((system, user)) = crate::db::profiles::profile_paths(db, name)? else {
        return Err(CoreError::new(
            ErrorCode::NotFound,
            "apply_profile",
            format!("配置不存在或没有内容: {name}"),
        ));
    };

    let outcome = {
        // 只提交确实变化的 hive：避免无谓备份，也避免把「未触及」记成 `Applied`。
        let before = load_path_snapshot_db_io(io, db)?;
        let system_changed = system != before.system;
        let user_changed = user != before.user;

        if system_changed || user_changed {
            save_path_with_db_io(
                io,
                db,
                source,
                system_changed.then_some(PathHiveWrite {
                    entries: system,
                    expected: None,
                }),
                user_changed.then_some(PathHiveWrite {
                    entries: user,
                    expected: None,
                }),
                pending_path,
                env,
            )?
        } else {
            ApplyOutcome {
                system: HiveOutcome::Skipped,
                user: HiveOutcome::Skipped,
                sidecar: SidecarOutcome::Saved,
                backup: None,
            }
        }
    };

    // 顺序要紧：先判失败、再记账。反过来的话，某个 hive 写失败仍会被算作
    // 一次成功应用（applyCount +1），用户看到「失败」却在配置列表里看到计数增长。
    //
    // 调用方只按 `Result` 判成败（CLI 打印「已写入注册表」、GUI 关闭弹窗），
    // 逐 hive 的 `Failed` 必须在这里抬成 Err，否则注册表没改也报成功。
    super::fail_on_hive_error(&outcome)?;

    if let Err(e) = crate::db::profiles::mark_profile_applied_by_name(db, name) {
        log::warn!("配置应用计数更新失败（保存已成功）: {}", e.message);
    }

    Ok(outcome)
}
