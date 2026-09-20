use path_editor_core::env_var::{EnvHive, EnvValueKind, EnvVarSnapshot, RevealedValue};
use path_editor_core::error::CoreError;
use path_editor_core::registry;

/// 一次读取两个 hive 的全部环境变量元数据（不含敏感明文）。
///
/// F-06（Wave 2 Task 2）：env 通路错误迁移为结构化 `CoreError`（serde 序列化为
/// `{code, message, ...}` 对象）；Task 3 接手前端按 `code` 判定的完整迁移。
///
/// # Returns
/// - `Ok(EnvVarSnapshot)` — system / user 两个 hive 的变量元数据
/// - `Err(CoreError)` — 注册表读取失败（code=`Io`/`Parse` 等）
#[tauri::command]
pub fn list_all_env_vars() -> Result<EnvVarSnapshot, CoreError> {
    registry::list_all_env_vars()
}

/// 按需读取单个变量的明文及读取时的 revision；`Unsupported` 类型返回 `Err`。
///
/// # Returns
/// - `Ok(RevealedValue)` — 变量完整值与读取时的 revision
/// - `Err(CoreError)` — 名称非法（`InvalidName`）、保留名（`ReservedName`）、
///   类型不受支持（`UnsupportedType`）或读取失败（`Io`）
#[tauri::command]
pub fn reveal_env_var(hive: EnvHive, name: String) -> Result<RevealedValue, CoreError> {
    registry::reveal_env_var(hive, &name)
}

/// 写入已有变量；类型从注册表读取，revision 不匹配则拒绝（code=`Conflict`）。
///
/// # Returns
/// - `Ok(())` — 写入成功并广播环境变更
/// - `Err(CoreError)` — 校验失败、修订冲突或类型不受支持
#[tauri::command]
pub fn update_env_var(
    hive: EnvHive,
    name: String,
    value: String,
    expected_revision: String,
) -> Result<(), CoreError> {
    registry::update_env_var(hive, &name, &value, &expected_revision)
}

/// 新建变量，`kind` 决定写入的注册表类型；同名或保护名单变量会被拒绝。
///
/// # Returns
/// - `Ok(())` — 创建成功并广播环境变更
/// - `Err(CoreError)` — 名称非法、已存在（`NameExists`）、保护名单（`Protected`）
///   或类型不受支持
#[tauri::command]
pub fn create_env_var(
    hive: EnvHive,
    name: String,
    value: String,
    kind: EnvValueKind,
) -> Result<(), CoreError> {
    registry::create_env_var(hive, &name, &value, kind)
}

/// 删除变量；revision 不匹配则拒绝（code=`Conflict`），变量保持原样。
///
/// # Returns
/// - `Ok(())` — 删除成功并广播环境变更
/// - `Err(CoreError)` — 校验失败、修订冲突或类型不受支持
#[tauri::command]
pub fn delete_env_var(
    hive: EnvHive,
    name: String,
    expected_revision: String,
) -> Result<(), CoreError> {
    registry::delete_env_var(hive, &name, &expected_revision)
}
