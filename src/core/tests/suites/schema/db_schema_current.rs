//! schema：新库建到当前版本，且重复迁移幂等。
//!

#[test]
fn schema_current_adds_all_tables_and_is_idempotent() {
    let (_dir, mut db) = crate::common::new_db("v3.db");

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
