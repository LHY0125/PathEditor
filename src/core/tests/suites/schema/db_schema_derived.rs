//
// 守住「派生表在新库上建得出来」与「重复建库幂等」。
use path_editor_core::db::schema;

#[test]
fn schema_adds_derived_tables_and_is_idempotent() {
    let (_dir, mut db) = crate::common::new_db("v2.db");

    assert_eq!(
        db.schema_version().unwrap(),
        schema::CURRENT_SCHEMA_VERSION,
        "新库必须建到当前版本"
    );

    for t in ["scan_runs", "var_references", "daily_stats", "search_fts"] {
        assert!(db.table_exists(t).unwrap(), "缺表: {t}");
    }

    // 幂等：重复迁移不改版本、不报错
    db.ensure_schema().unwrap();
    assert_eq!(db.schema_version().unwrap(), schema::CURRENT_SCHEMA_VERSION);
}
