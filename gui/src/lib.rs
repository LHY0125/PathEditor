mod commands;

use tauri::Manager;

/// 启动时的数据库初始化序列（spec §7.1 / §7.2）。
///
/// 拆成独立函数而不是塞进 `setup` 闭包：`setup` 的返回值是 `Result<(), Box<dyn Error>>`，
/// 把错误类型收窄成 `CoreError` 才能在这里做分步日志，也便于将来加启动自检。
fn setup_database(db: &mut path_editor_core::db::Db) -> Result<(), path_editor_core::CoreError> {
    // ① best-effort 补写 pending。失败只告警：它代表的是上一次操作遗留的
    //    「注册表已写、快照未落盘」，补不上也不该阻止本次启动。
    if let Err(e) =
        path_editor_core::service::retry_pending_path_state(&path_editor_core::history::NoopHistory)
    {
        log::warn!("启动时补写 pending 失败（继续启动）: {}", e.message);
    }

    // ② schema 建库/迁移
    db.migrate_to_current()?;

    // ③ v5 遗留文件一次性导入（幂等：migration_imports 保证重跑不重复）
    let home = path_editor_core::db::crypto::config_dir();
    match path_editor_core::db::migrate_v5::migrate_legacy(db, &home) {
        Ok(report) => {
            if report.imported > 0 || report.failed > 0 {
                log::info!(
                    "v5 文件迁移完成：导入 {}，跳过 {}，失败 {}",
                    report.imported,
                    report.skipped,
                    report.failed
                );
            }
        }
        // 迁移失败不阻断启动：DB 仍可用于新记录，旧文件保持原样等下次重试
        Err(e) => log::warn!("v5 文件迁移失败（继续启动）: {}", e.message),
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

            // v6 M1：打开历史库。失败时进入降级模式（spec D16 fail-open）：
            // 注册表编辑照常，history_*/profile_*/backup_* 命令返回 database 错误。
            //
            // 启动序列（spec §7.1）：
            //   ① best-effort 补写 pending（注册表已写、快照未落盘的 gap）
            //   ② 建库/迁移 schema
            //   ③ 首次启动时把 v5 遗留文件导入 DB（幂等，可重跑）
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
            commands::disabled::save_disabled_state,
            commands::disabled::load_disabled_state,
            commands::disabled::save_path_snapshot,
            commands::disabled::load_path_snapshot,
            commands::scanner::scan_paths,
            commands::registry::clean_path_entries,
            commands::profiles::list_profiles,
            commands::profiles::save_profile,
            commands::profiles::load_profile,
            commands::profiles::delete_profile,
            commands::profiles::rename_profile,
            commands::env_var::list_all_env_vars,
            commands::env_var::reveal_env_var,
            commands::env_var::update_env_var,
            commands::env_var::create_env_var,
            commands::env_var::delete_env_var,
            commands::service::save_path_with_sidecar,
            commands::service::apply_path_snapshot,
            commands::service::retry_pending_path_state,
            commands::service::apply_profile,
            commands::history::history_list_events,
            commands::history::history_get_snapshot,
            commands::history::history_get_current,
            commands::history::profile_list,
            commands::history::profile_delete,
            commands::history::profile_mark_applied,
            commands::history::backup_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
