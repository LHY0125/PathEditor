// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn schema_v2_adds_derived_tables_and_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("v2.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    assert_eq!(db.schema_version().unwrap(), 2, "版本必须是 2");

    for t in ["scan_runs", "var_references", "daily_stats", "search_fts"] {
        assert!(db.table_exists(t).unwrap(), "缺表: {t}");
    }

    // 幂等：重复迁移不改版本、不报错
    db.migrate_to_current().unwrap();
    assert_eq!(db.schema_version().unwrap(), 2);
}
