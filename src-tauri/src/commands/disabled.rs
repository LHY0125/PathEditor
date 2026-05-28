use path_editor_core::disabled;

#[tauri::command]
pub fn save_disabled_state(system: Vec<String>, user: Vec<String>) -> Result<(), String> { disabled::save_disabled_state(system, user) }
#[tauri::command]
pub fn load_disabled_state() -> Result<(Vec<String>, Vec<String>), String> { disabled::load_disabled_state() }
