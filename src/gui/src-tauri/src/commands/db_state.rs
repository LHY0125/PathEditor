//! 全局 DB 句柄与取用辅助。
//!
//! 所有需要 DB 的命令都经这里取连接：锁中毒与降级模式的判定只保留一份。

use path_editor_core::db::Db;
use path_editor_core::{CoreError, ErrorCode};
use std::sync::Mutex;
use tauri::State;

/// 全局 DB 句柄。`None` = 降级模式（打开失败）。
pub struct DbState(pub Mutex<Option<Db>>);

/// 从共享状态取 DB；降级模式返回结构化错误。
pub(crate) fn with_db<T>(
    state: &State<'_, DbState>,
    op: &'static str,
    f: impl FnOnce(&Db) -> Result<T, CoreError>,
) -> Result<T, CoreError> {
    let guard = state
        .0
        .lock()
        .map_err(|_| CoreError::new(ErrorCode::Internal, op, "数据库状态锁中毒"))?;
    let db = guard
        .as_ref()
        .ok_or_else(|| CoreError::new(ErrorCode::Database, op, "历史库不可用，当前处于降级模式"))?;
    f(db)
}

/// 从共享状态取可变 DB（写入路径）。
pub(crate) fn with_db_mut<T>(
    state: &State<'_, DbState>,
    op: &'static str,
    f: impl FnOnce(&mut Db) -> Result<T, CoreError>,
) -> Result<T, CoreError> {
    let mut guard = state
        .0
        .lock()
        .map_err(|_| CoreError::new(ErrorCode::Internal, op, "数据库状态锁中毒"))?;
    let db = guard
        .as_mut()
        .ok_or_else(|| CoreError::new(ErrorCode::Database, op, "历史库不可用，当前处于降级模式"))?;
    f(db)
}

/// 取 DB 的**可选**引用：降级模式传 `None`，由 core 决定回退行为。
///
/// 只用于「DB 不可用仍可继续」的读路径（如加载 PATH 快照）；
/// 需要 DB 的命令用 [`with_db`] / [`with_db_mut`]，降级时直接报错。
pub(crate) fn with_optional_db<T>(
    state: &State<'_, DbState>,
    op: &'static str,
    f: impl FnOnce(Option<&Db>) -> Result<T, CoreError>,
) -> Result<T, CoreError> {
    let guard = state
        .0
        .lock()
        .map_err(|_| CoreError::new(ErrorCode::Internal, op, "数据库状态锁中毒"))?;
    f(guard.as_ref())
}
