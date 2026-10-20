mod commands;

use tauri::Manager;

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
            let db = match path_editor_core::db::open_default() {
                Ok(mut db) => {
                    if let Err(e) = db.migrate_to_current() {
                        log::error!("历史库 schema 迁移失败，进入降级模式: {}", e.message);
                        None
                    } else {
                        Some(db)
                    }
                }
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
