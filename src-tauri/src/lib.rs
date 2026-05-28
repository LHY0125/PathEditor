mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::registry::load_system_paths,
            commands::registry::load_user_paths,
            commands::registry::save_system_paths,
            commands::registry::save_user_paths,
            commands::system::check_admin,
            commands::system::validate_path,
            commands::system::expand_env_vars,
            commands::system::broadcast_env_change,
            commands::backup::backup_registry,
            commands::backup::get_appdata_dir,
            commands::fs::read_text_file,
            commands::disabled::save_disabled_state,
            commands::disabled::load_disabled_state,
            commands::scanner::scan_conflicts,
            commands::scanner::scan_tools,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
