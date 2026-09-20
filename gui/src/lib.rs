mod commands;

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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::registry::load_system_paths,
            commands::registry::load_user_paths,
            commands::registry::save_system_paths,
            commands::registry::save_user_paths,
            commands::system::check_admin,
            commands::system::get_path_capabilities,
            commands::system::validate_path,
            commands::system::expand_env_vars,
            commands::system::broadcast_env_change,
            commands::backup::backup_registry,
            commands::backup::get_appdata_dir,
            commands::fs::read_text_file,
            commands::fs::import_file,
            commands::fs::export_path_entries,
            commands::disabled::save_disabled_state,
            commands::disabled::load_disabled_state,
            commands::disabled::save_path_snapshot,
            commands::disabled::load_path_snapshot,
            commands::scanner::scan_conflicts,
            commands::scanner::scan_tools,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
