use path_editor_core::env_var::{EnvHive, EnvValueKind, EnvVarSnapshot};
use path_editor_core::registry;

/// 一次读取两个 hive 的全部环境变量元数据（不含敏感明文）。
///
/// # Returns
/// - `Ok(EnvVarSnapshot)` — system / user 两个 hive 的变量元数据
/// - `Err(String)` — 注册表读取失败
#[tauri::command]
pub fn list_all_env_vars() -> Result<EnvVarSnapshot, String> {
    registry::list_all_env_vars()
}

/// 按需读取单个变量的完整明文；`Unsupported` 类型返回 `Err`。
///
/// # Returns
/// - `Ok(String)` — 变量完整值
/// - `Err(String)` — 名称非法、保留名或类型不受支持
#[tauri::command]
pub fn reveal_env_var(hive: EnvHive, name: String) -> Result<String, String> {
    registry::reveal_env_var(hive, &name)
}

/// 写入已有变量；类型从注册表读取，revision 不匹配则拒绝并返回 `[E_CONFLICT]`。
///
/// # Returns
/// - `Ok(())` — 写入成功并广播环境变更
/// - `Err(String)` — 校验失败、修订冲突或类型不受支持
#[tauri::command]
pub fn update_env_var(
    hive: EnvHive,
    name: String,
    value: String,
    expected_revision: String,
) -> Result<(), String> {
    registry::update_env_var(hive, &name, &value, &expected_revision)
}

/// 新建变量，`kind` 决定写入的注册表类型；同名或保护名单变量会被拒绝。
///
/// # Returns
/// - `Ok(())` — 创建成功并广播环境变更
/// - `Err(String)` — 名称非法、已存在、保护名单或类型不受支持
#[tauri::command]
pub fn create_env_var(
    hive: EnvHive,
    name: String,
    value: String,
    kind: EnvValueKind,
) -> Result<(), String> {
    registry::create_env_var(hive, &name, &value, kind)
}

/// 删除变量；revision 不匹配则拒绝并返回 `[E_CONFLICT]`，变量保持原样。
///
/// # Returns
/// - `Ok(())` — 删除成功并广播环境变更
/// - `Err(String)` — 校验失败、修订冲突或类型不受支持
#[tauri::command]
pub fn delete_env_var(
    hive: EnvHive,
    name: String,
    expected_revision: String,
) -> Result<(), String> {
    registry::delete_env_var(hive, &name, &expected_revision)
}
