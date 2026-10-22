// 一文件一测试：同一进程内跑第二个 SQLCipher 测试会 STATUS_STACK_OVERFLOW
// （GNU/MSVC 均已复现，--test-threads=1 无效）。见测试约束说明。
use path_editor_core::db::{schema::CURRENT_SCHEMA_VERSION, Db};
use tempfile::TempDir;

#[test]
fn db_opens_creates_schema_and_reports_version() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.db");
    let key = "ab".repeat(32);

    let mut db = Db::open_encrypted(&path, &key).expect("打开失败");
    assert_eq!(db.schema_version().unwrap(), 0, "新库版本应为 0");

    db.migrate_to_current().expect("迁移失败");
    assert_eq!(
        db.schema_version().unwrap(),
        CURRENT_SCHEMA_VERSION,
        "迁移后版本应为当前版本"
    );

    // 重复迁移必须幂等
    db.migrate_to_current().expect("二次迁移失败");
    assert_eq!(db.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);

    // 关键表存在
    for table in [
        "snapshots",
        "app_state",
        "path_entries",
        "env_values",
        "events",
        "profiles",
        "profile_revisions",
        "backups",
        "migration_imports",
    ] {
        assert!(db.table_exists(table).unwrap(), "缺少表 {table}");
    }

    // 重开同一文件仍可读（密钥一致）
    let reopened = Db::open_encrypted(&path, &key).expect("重开失败");
    assert_eq!(reopened.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);

    // 错误密钥必须失败，且不产生半开库
    let bad = Db::open_encrypted(&path, &"cd".repeat(32));
    assert!(bad.is_err(), "错误密钥必须打不开");
}
