//! schema：新库建到当前版本，且重复迁移幂等。
//!
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn schema_current_adds_all_tables_and_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let mut db = Db::open(&dir.path().join("v3.db")).unwrap();
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
