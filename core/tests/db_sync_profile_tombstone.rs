//! 墓碑：`content = null` 且 `field = "deleted_at"` → 本地软删除。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::{profiles, Db};
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join(name), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    db
}

#[test]
fn tombstone_soft_deletes_profile_and_frees_the_name() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "tombstone.db");

    let snap = NewSnapshot {
        kind: SnapshotKind::Current,
        source: EventSource::Gui,
        system: vec![PathEntry {
            path: "C:\\A".to_string(),
            enabled: true,
        }],
        user: vec![],
        env: vec![],
        parent_uid: None,
    };
    profiles::save_profile(&mut db, "Work", None, &[], snap).unwrap();
    let uid: String = db
        .query_row_with("SELECT uid FROM profiles WHERE name = 'Work'", [], |r| {
            r.get(0)
        })
        .unwrap();

    let changes = format!(
        r#"[{{
          "uid": "c-tomb", "deviceId": "dev-x", "seq": 9, "lamport": 999,
          "entityType": "profile", "entityUid": "{uid}", "field": "deleted_at",
          "oldValue": "null", "newValue": "2000000000000", "createdAt": 0,
          "content": null
        }}]"#
    );
    let header = BundleHeader {
        schema_version: 3,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 999,
        change_count: 1,
    };
    let path = dir.path().join("tomb.pesync");
    std::fs::write(
        &path,
        seal(
            "pw",
            &header,
            format!("{{\"changes\":{changes}}}").as_bytes(),
        )
        .unwrap(),
    )
    .unwrap();

    let report = import_bundle(&mut db, "pw", &path).unwrap();
    assert_eq!(report.applied, 1);

    let deleted: Option<i64> = db
        .query_row_with(
            "SELECT deleted_at FROM profiles WHERE uid = ?1",
            rusqlite::params![uid],
            |r| r.get(0),
        )
        .unwrap();
    assert!(deleted.is_some(), "墓碑必须写入 deleted_at");

    let active: i64 = db
        .query_row_with(
            "SELECT COUNT(*) FROM profiles WHERE name = 'Work' AND deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(active, 0, "名称必须被释放");
}
