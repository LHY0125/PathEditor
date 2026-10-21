//! v5 配置导入：把 `profiles/*.json` 读回 DB，并保留 PATH 条目。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::v5_import::import_profile_v5;
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn imports_a_v5_profile_json_into_the_database() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("v5import.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let file = dir.path().join("Work.json");
    std::fs::write(
        &file,
        r#"{
          "schemaVersion": 1,
          "name": "Work",
          "sys": [{"path": "C:\\A", "enabled": true}],
          "user": [{"path": "D:\\B", "enabled": false}],
          "created": "2026-01-01T00:00:00",
          "modified": "2026-01-02T00:00:00"
        }"#,
    )
    .unwrap();

    let uid = import_profile_v5(&mut db, &file).unwrap();
    assert!(!uid.is_empty());

    let name: String = db
        .query_row_with(
            "SELECT name FROM profiles WHERE uid = ?1",
            rusqlite::params![uid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(name, "Work");

    let entries: i64 = db
        .query_row_with(
            "SELECT COUNT(*) FROM path_entries pe
               JOIN profile_revisions r ON r.snapshot_id = pe.snapshot_id
              WHERE r.profile_id = (SELECT id FROM profiles WHERE uid = ?1)",
            rusqlite::params![uid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(entries, 2, "两个 hive 的条目都必须落库");
}
