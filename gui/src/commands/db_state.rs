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
