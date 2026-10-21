// 一文件一测试：见 M0 §5.3。
//
// 本测试守的是「M2 的派生表在新库上建得出来」与「迁移幂等」两件事。
// 版本号断言用 `CURRENT_SCHEMA_VERSION` 而非硬编码 `2`——schema 每升一版
// 都不该让它失败；「v1→v2 是否落版本号」由 `db_schema_v1_to_v2.rs` 专门守。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::{schema, Db};
use tempfile::TempDir;

#[test]
fn schema_v2_adds_derived_tables_and_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("v2.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    assert_eq!(
        db.schema_version().unwrap(),
        schema::CURRENT_SCHEMA_VERSION,
        "新库必须建到当前版本"
    );

    for t in ["scan_runs", "var_references", "daily_stats", "search_fts"] {
        assert!(db.table_exists(t).unwrap(), "缺表: {t}");
    }

    // 幂等：重复迁移不改版本、不报错
    db.migrate_to_current().unwrap();
    assert_eq!(db.schema_version().unwrap(), schema::CURRENT_SCHEMA_VERSION);
}
