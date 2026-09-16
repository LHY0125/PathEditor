use path_editor_core::env_var::{EnvHive, EnvValueKind, EnvVarSnapshot};
use path_editor_core::registry;

#[tauri::command]
pub fn list_all_env_vars() -> Result<EnvVarSnapshot, String> {
    registry::list_all_env_vars()
}
#[tauri::command]
pub fn reveal_env_var(hive: EnvHive, name: String) -> Result<String, String> {
    registry::reveal_env_var(hive, &name)
}
#[tauri::command]
pub fn update_env_var(
    hive: EnvHive,
    name: String,
    value: String,
    expected_revision: String,
) -> Result<(), String> {
    registry::update_env_var(hive, &name, &value, &expected_revision)
}
#[tauri::command]
pub fn create_env_var(
    hive: EnvHive,
    name: String,
    value: String,
    kind: EnvValueKind,
) -> Result<(), String> {
    registry::create_env_var(hive, &name, &value, kind)
}
#[tauri::command]
pub fn delete_env_var(
    hive: EnvHive,
    name: String,
    expected_revision: String,
) -> Result<(), String> {
    registry::delete_env_var(hive, &name, &expected_revision)
}
