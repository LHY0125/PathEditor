//! 集成测试共用夹具。
//!
//! 只放「建库」这类纯样板；断言与业务数据留在各自用例里。

#![allow(dead_code)]

use path_editor_core::db::Db;
use tempfile::TempDir;

/// 在独立临时目录里打开一个空库并迁移到当前 schema。
///
/// 返回 `(TempDir, Db)`：`TempDir` 必须由调用方持有，drop 时目录才会被清理。
pub fn new_db(name: &str) -> (TempDir, Db) {
    let dir = TempDir::new().expect("创建临时目录失败");
    let mut db = Db::open(&dir.path().join(name)).expect("打开数据库失败");
    db.migrate_to_current().expect("迁移失败");
    (dir, db)
}
