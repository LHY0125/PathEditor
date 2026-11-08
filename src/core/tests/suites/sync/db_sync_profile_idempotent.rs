//! 同一包重复导入：配置与 revision 不得重复，第二次全部走幂等跳过。
//!
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;

#[test]
fn reimporting_the_same_bundle_does_not_duplicate_profiles_or_revisions() {
    let (dir, mut db) = crate::common::new_db("idem.db");

    let changes = r#"[
      { "uid": "c-snap", "deviceId": "dev-x", "seq": 1, "lamport": 1,
        "entityType": "snapshot", "entityUid": "snap-1", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": { "uid": "snap-1", "createdAt": 1, "kind": "current",
                     "system": [], "user": [], "env": [] } },
      { "uid": "c-prof", "deviceId": "dev-x", "seq": 2, "lamport": 2,
        "entityType": "profile", "entityUid": "p-1", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": { "uid": "p-1", "name": "Work", "description": null, "tags": [],
                     "createdAt": 1, "updatedAt": 2, "version": 1, "snapshotUid": "snap-1" } }
    ]"#;
    let header = BundleHeader {
        schema_version: 3,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 10,
        change_count: 2,
    };
    let path = dir.path().join("idem.pesync");
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

    let first = import_bundle(&mut db, "pw", &path).unwrap();
    assert_eq!(first.applied, 2);

    let second = import_bundle(&mut db, "pw", &path).unwrap();
    assert_eq!(second.applied, 0);
    assert_eq!(second.skipped, 2);
    assert_eq!(second.renamed_profiles, 0);

    let profiles: i64 = db
        .query_row_with("SELECT COUNT(*) FROM profiles WHERE uid = 'p-1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(profiles, 1);

    let revisions: i64 = db
        .query_row_with(
            "SELECT COUNT(*) FROM profile_revisions WHERE profile_id =
               (SELECT id FROM profiles WHERE uid = 'p-1')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(revisions, 1, "重复导入不得生成第二个 revision");
}
