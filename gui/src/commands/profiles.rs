use path_editor_core::profiles;

#[tauri::command]
pub fn list_profiles() -> Result<Vec<profiles::ProfileMeta>, String> {
    // F-11：core 侧已迁移为 CoreError；IPC 契约仍是自由文本，透传 message。
    profiles::list_profiles().map_err(|e| e.message)
}
#[tauri::command]
pub fn save_profile(
    name: String,
    sys: Vec<profiles::ProfilePathEntry>,
    user: Vec<profiles::ProfilePathEntry>,
) -> Result<(), String> {
    profiles::save_profile(&name, sys, user).map_err(|e| e.message)
}
#[tauri::command]
pub fn load_profile(name: String) -> Result<profiles::ProfileData, String> {
    profiles::load_profile(&name).map_err(|e| e.message)
}
#[tauri::command]
pub fn delete_profile(name: String) -> Result<(), String> {
    profiles::delete_profile(&name).map_err(|e| e.message)
}
#[tauri::command]
pub fn rename_profile(old_name: String, new_name: String) -> Result<(), String> {
    profiles::rename_profile(&old_name, &new_name).map_err(|e| e.message)
}
