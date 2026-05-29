use path_editor_core::profiles;

#[tauri::command]
pub fn list_profiles() -> Result<Vec<profiles::ProfileMeta>, String> {
    profiles::list_profiles()
}
#[tauri::command]
pub fn save_profile(
    name: String,
    sys: Vec<profiles::ProfilePathEntry>,
    user: Vec<profiles::ProfilePathEntry>,
) -> Result<(), String> {
    profiles::save_profile(&name, sys, user)
}
#[tauri::command]
pub fn load_profile(name: String) -> Result<profiles::ProfileData, String> {
    profiles::load_profile(&name)
}
#[tauri::command]
pub fn delete_profile(name: String) -> Result<(), String> {
    profiles::delete_profile(&name)
}
#[tauri::command]
pub fn rename_profile(old_name: String, new_name: String) -> Result<(), String> {
    profiles::rename_profile(&old_name, &new_name)
}
