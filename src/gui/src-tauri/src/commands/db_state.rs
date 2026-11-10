//! 全局 DB 句柄与取用辅助。
//!
//! 所有需要 DB 的命令都经这里取连接：锁中毒与降级模式的判定只保留一份。

use path_editor_core::db::Db;
use path_editor_core::{CoreError, ErrorCode};
use std::sync::Mutex;
use tauri::State;

/// 全局 DB 状态。
///
/// `db = None` 表示降级模式；`init_error` 保存导致降级的原始错误。
/// 只保留通用文案会让前端无法区分「版本不兼容」「旧加密库」「磁盘错误」，
/// 用户也就无法按正确路径恢复。
pub struct DbState(pub Mutex<DbStateInner>);

/// [`DbState`] 的内部状态。
pub struct DbStateInner {
    /// 可用数据库；`None` = 降级模式。
    pub db: Option<Db>,
    /// 初始化失败原因；仅在 `db = None` 时有值。
    pub init_error: Option<CoreError>,
}

impl DbState {
    /// 构造全局 DB 状态。
    pub fn new(db: Option<Db>, init_error: Option<CoreError>) -> Self {
        Self(Mutex::new(DbStateInner { db, init_error }))
    }

    /// 降级模式下返回保存的根因；没有记录时回落到通用文案。
    fn unavailable(guard: &DbStateInner, op: &'static str) -> CoreError {
        guard.init_error.clone().unwrap_or_else(|| {
            CoreError::new(ErrorCode::Database, op, "历史库不可用，当前处于降级模式")
        })
    }
}

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
        .db
        .as_ref()
        .ok_or_else(|| DbState::unavailable(&guard, op))?;
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
    if guard.db.is_none() {
        return Err(DbState::unavailable(&guard, op));
    }
    let db = guard.db.as_mut().expect("已检查 db 存在");
    f(db)
}

/// 取 DB 的**可选可变**引用：写入仍可在 DB 降级时继续（历史副作用跳过）。
pub(crate) fn with_optional_db_mut<T>(
    state: &State<'_, DbState>,
    op: &'static str,
    f: impl FnOnce(Option<&mut Db>) -> Result<T, CoreError>,
) -> Result<T, CoreError> {
    let mut guard = state
        .0
        .lock()
        .map_err(|_| CoreError::new(ErrorCode::Internal, op, "数据库状态锁中毒"))?;
    f(guard.db.as_mut())
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
    f(guard.db.as_ref())
}
