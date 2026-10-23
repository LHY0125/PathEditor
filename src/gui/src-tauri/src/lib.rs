mod commands;

use tauri::Manager;

/// 启动时的数据库初始化序列。
///
/// 迁移、pending 补写、当前状态基线、备份索引、保留策略、派生表重建
/// 全部在 `core::bootstrap`；这里只负责把结果写进 GUI 的日志。
/// 引导失败（只有迁移失败会返回 `Err`）由调用方决定是否进入降级模式。
fn setup_database(db: &mut path_editor_core::db::Db) -> Result<(), path_editor_core::CoreError> {
    let report = path_editor_core::bootstrap::initialize(
        db,
        path_editor_core::history::EventSource::Gui,
        &path_editor_core::bootstrap::BootstrapOptions::default(),
    )?;

    for warning in &report.warnings {
        log::warn!("{}", warning);
    }
    if report.baseline_created {
        log::info!("已用注册表当前状态建立 current 快照基线");
    }
    if report.backups_indexed > 0 {
        log::info!("备份索引：{} 条", report.backups_indexed);
    }
    if report.changes_pruned > 0 || report.conflicts_pruned > 0 {
        log::info!(
            "保留策略清理：changes {} 条，冲突 {} 条",
            report.changes_pruned,
            report.conflicts_pruned
        );
    }
    if report.fts_indexed > 0 {
        log::info!("FTS 索引：首次构建 {} 条", report.fts_indexed);
    }
    if let Some((days, edges)) = report.derived_rebuilt {
        log::info!("派生表重建：daily_stats {} 天，引用图 {} 条边", days, edges);
    }

    Ok(())
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

            // 打开历史库。失败时进入降级模式：
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
            app.manage(commands::db_state::DbState(std::sync::Mutex::new(db)));
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
            commands::profiles::profile_list,
            commands::profiles::profile_delete,
            commands::profiles::profile_apply,
            commands::backup::backup_list,
            commands::profiles::profile_save,
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
            commands::profiles::profile_paths,
            commands::profiles::profile_rename,
            commands::profiles::profile_delete_by_name,
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
