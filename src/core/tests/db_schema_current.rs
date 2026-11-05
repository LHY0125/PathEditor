//! schema：新库建到当前版本，且重复迁移幂等。
//!
//! 一文件一测试（SQLCipher）（同一进程内第二个 SQLCipher 测试会 STATUS_STACK_OVERFLOW）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn schema_current_adds_all_tables_and_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("v3.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    assert_eq!(
        db.schema_version().unwrap(),
        path_editor_core::db::schema::CURRENT_SCHEMA_VERSION,
        "版本必须是当前版本"
    );
    for t in ["changes", "sync_conflicts", "scan_runs", "search_fts"] {
        assert!(db.table_exists(t).unwrap(), "缺表: {t}");
    }
    db.migrate_to_current().unwrap();
    assert_eq!(
        db.schema_version().unwrap(),
        path_editor_core::db::schema::CURRENT_SCHEMA_VERSION,
        "幂等"
    );
}
