mod commands;

use tauri::Manager;

/// 启动时的数据库初始化序列。
///
/// 拆成独立函数而不是塞进 `setup` 闭包：`setup` 的返回值是 `Result<(), Box<dyn Error>>`，
/// 把错误类型收窄成 `CoreError` 才能在这里做分步日志，也便于将来加启动自检。
fn setup_database(db: &mut path_editor_core::db::Db) -> Result<(), path_editor_core::CoreError> {
    // ① schema 建库/迁移
    db.migrate_to_current()?;

    // ② best-effort 补写上次未落盘的快照。失败只告警：它代表的是上一次操作
    //   遗留的「注册表已写、DB 当前快照未落盘」，补不上也不该阻止本次启动。
    if let Err(e) = path_editor_core::service::retry_pending_path_state_db(
        db,
        path_editor_core::history::EventSource::Gui,
    ) {
        log::warn!("启动时补写 pending 失败（继续启动）: {}", e.message);
    }

    // ③ 确保 DB 里有「当前状态」基线。
    //
    // 新装用户没有 current 快照；读路径没有持久化快照时只能看到注册表当前状态，
    // 被禁用的条目会显示成启用。这里用注册表当前状态建一条基线。
    ensure_current_snapshot_baseline(db);

    // ④ 备份索引：扫描 backups/ 写入 `backups` 表。
    // 没有这一步，`backup_list` 会永远返回空——命令在、数据恒空是最容易误导的形态。
    // 索引是派生数据，重建成本低，每次启动重扫一遍即可。
    let backup_dir = path_editor_core::backup::env_backup_dir();
    match path_editor_core::db::backups::index_backups(db, &backup_dir) {
        Ok(n) if n > 0 => log::info!("备份索引：{} 条", n),
        Ok(_) => {}
        Err(e) => log::warn!("备份索引失败（继续启动）: {}", e.message),
    }

    // ④b 保留策略清理：失败只告警，不阻断启动。
    //
    // 只删「已导出且超过 90 天」的 changes 与「已解决超过 30 天」的冲突；
    // 待导出队列永远不碰（`exported_at IS NULL`）。
    match path_editor_core::db::maintenance::prune_now(db) {
        Ok(r) if r.changes_deleted > 0 || r.conflicts_deleted > 0 => log::info!(
            "保留策略清理：changes {} 条，冲突 {} 条",
            r.changes_deleted,
            r.conflicts_deleted
        ),
        Ok(_) => {}
        Err(e) => log::warn!("保留策略清理失败（继续启动）: {}", e.message),
    }

    // ⑤ 派生表重建。
    //
    // 三张派生表都不是真相源，可随时重建：
    // - `search_fts`：只在为空时全量重建；写入路径会增量维护。
    // - `daily_stats`：每次启动全量重建，成本是一次全表扫描。
    // - `var_references`：每次启动全量重建，只保存当前快照的边。
    //
    // 全部失败只告警：DB 可用性比派生数据新鲜度重要，派生表下次启动会再算。
    rebuild_derived_tables(db);

    Ok(())
}

/// 重建三张派生表。
///
/// 抽成独立函数便于测试（`db_derived_rebuild.rs` 直接调它模拟启动序列）——
/// 若写在 `setup_database` 里，测试就得复现整个启动流程（含注册表读取）。
fn rebuild_derived_tables(db: &mut path_editor_core::db::Db) {
    // `search_fts`：仅当为空时才重建
    match db.query_row_with("SELECT count(*) FROM search_fts", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(0) => match path_editor_core::db::search::rebuild_index(db) {
            Ok(n) => log::info!("FTS 索引：首次构建 {} 条", n),
            Err(e) => log::warn!("FTS 索引重建失败（继续启动）: {}", e.message),
        },
        Ok(_) => {}
        Err(e) => log::warn!("FTS 索引计数失败（继续启动）: {}", e.message),
    }

    // `daily_stats`：每次启动全量重建（内部有耗时日志）
    match path_editor_core::db::stats::rebuild_daily(db) {
        Ok(n) => log::info!("daily_stats 重建：{} 天", n),
        Err(e) => log::warn!("daily_stats 重建失败（继续启动）: {}", e.message),
    }

    // `var_references`：每次启动全量重建
    match path_editor_core::db::refs::rebuild_refs(db) {
        Ok(n) => log::info!("引用图重建：{} 条边", n),
        Err(e) => log::warn!("引用图重建失败（继续启动）: {}", e.message),
    }
}

/// 确保 DB 里有一条 `kind=current` 快照；没有就用注册表当前状态建一条。
///
/// 幂等：已有 `current_snapshot_id` 时什么都不做。
/// 失败只告警——它是降级可容忍的（读路径会回落到文件），不该阻断启动。
fn ensure_current_snapshot_baseline(db: &mut path_editor_core::db::Db) {
    // 已有基线：检查 `app_state.current_snapshot_id` 是否指向一条 kind=current
    let has_current: Result<i64, _> = db.query_row_with(
        "SELECT count(*) FROM app_state a
           JOIN snapshots s ON s.id = a.current_snapshot_id
          WHERE a.id = 1 AND s.kind = 'current'",
        [],
        |r| r.get::<_, i64>(0),
    );
    if matches!(has_current, Ok(n) if n > 0) {
        return;
    }

    // 没有基线：用注册表当前状态建一条
    let (system, user) = match path_editor_core::service::load_path_snapshot_db(db) {
        Ok(s) => (s.system, s.user),
        Err(e) => {
            log::warn!("建立当前状态基线失败（读注册表出错）: {}", e.message);
            return;
        }
    };
    let snap = path_editor_core::db::history::NewSnapshot {
        kind: path_editor_core::db::history::SnapshotKind::Current,
        source: path_editor_core::history::EventSource::Migration,
        system,
        user,
        env: Vec::new(),
        parent_uid: None,
    };
    let uid = match path_editor_core::db::history::insert_snapshot(db, snap) {
        Ok(uid) => uid,
        Err(e) => {
            log::warn!("建立当前状态基线失败（写入失败）: {}", e.message);
            return;
        }
    };
    let version = match path_editor_core::db::history::app_state_version(db) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("建立当前状态基线失败（读 version）: {}", e.message);
            return;
        }
    };
    match path_editor_core::db::history::set_current_snapshot(db, &uid, version) {
        Ok(true) => log::info!("已用注册表当前状态建立 current 快照基线"),
        Ok(false) => log::warn!("建立基线的 CAS 冲突（有并发写），读路径将回落到文件"),
        Err(e) => log::warn!("建立当前状态基线失败: {}", e.message),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let level = if cfg!(debug_assertions) {
                log::LevelFilter::Info
            } else {
                log::LevelFilter::Warn
            };
            app.handle()
                .plugin(tauri_plugin_log::Builder::default().level(level).build())?;

            // v6：打开历史库。失败时进入降级模式：
            // 注册表编辑照常，history_*/profile_*/backup_* 命令返回 database 错误。
            //
            // 启动序列：
            //  ① 建库/迁移 schema
            //  ② 补写上次未落盘的 pending 快照
            //  ③ 建立当前状态基线
            //  ④ 备份索引 / 保留策略 / 派生表重建
            let db = match path_editor_core::db::open_default() {
                Ok(mut db) => match setup_database(&mut db) {
                    Ok(()) => Some(db),
                    Err(e) => {
                        log::error!("历史库初始化失败，进入降级模式: {}", e.message);
                        None
                    }
                },
                Err(e) => {
                    log::warn!("历史库打开失败，进入降级模式: {}", e.message);
                    None
                }
            };
            app.manage(commands::history::DbState(std::sync::Mutex::new(db)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::registry::load_system_paths,
            commands::registry::load_user_paths,
            commands::system::check_admin,
            commands::system::get_path_capabilities,
            commands::system::validate_path,
            commands::system::expand_env_vars,
            commands::system::broadcast_env_change,
            commands::backup::backup_registry,
            commands::backup::backup_env_vars,
            commands::backup::list_env_backups,
            commands::backup::preview_env_backup,
            commands::backup::restore_env_backup,
            commands::fs::read_text_file,
            commands::fs::import_file,
            commands::fs::export_path_entries,
            commands::disabled::load_path_snapshot,
            commands::scanner::scan_paths,
            commands::registry::clean_path_entries,
            commands::env_var::list_all_env_vars,
            commands::env_var::reveal_env_var,
            commands::env_var::update_env_var,
            commands::env_var::create_env_var,
            commands::env_var::delete_env_var,
            commands::service::save_path_with_sidecar,
            commands::history::history_list_events,
            commands::history::history_get_snapshot,
            commands::history::history_get_current,
            commands::history::history_reveal_env_value,
            commands::history::profile_list,
            commands::history::profile_delete,
            commands::history::profile_mark_applied,
            commands::history::backup_list,
            commands::history::profile_save,
            // ── 检索 / 统计 / 对比 / 引用图 / 冲突历史 ──
            commands::analytics::search,
            commands::analytics::stats_dashboard,
            commands::analytics::stats_rebuild,
            commands::analytics::compare_snapshots,
            commands::analytics::refs_dependents,
            commands::analytics::refs_dependencies,
            commands::analytics::refs_cycles,
            commands::analytics::refs_rebuild,
            commands::analytics::search_rebuild,
            commands::analytics::scan_history_list,
            commands::analytics::scan_history_diff,
            commands::analytics::scan_history_record,
            commands::history::profile_paths,
            commands::history::profile_rename,
            commands::history::profile_delete_by_name,
            commands::history::profile_mark_applied_by_name,
            commands::history::history_restore_snapshot,
            // ── 同步中心 ──
            commands::sync::sync_status,
            commands::sync::sync_export,
            commands::sync::sync_import,
            commands::sync::sync_list_conflicts,
            commands::sync::sync_resolve_conflict,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
