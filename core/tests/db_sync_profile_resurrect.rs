//! 墓碑与旧内容：删除之后，低 lamport 的内容变更不得让配置「复活」。
//!
//! 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::sync::ImportReport;
use path_editor_core::db::Db;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join(name), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    db
}

fn import_changes(
    db: &mut Db,
    dir: &TempDir,
    file: &str,
    changes: &str,
    count: u32,
) -> ImportReport {
    let header = BundleHeader {
        schema_version: 3,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 100,
        change_count: count,
    };
    let path = dir.path().join(file);
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
    import_bundle(db, "pw", &path).unwrap()
}

const SNAPSHOT: &str = r#"{
  "uid": "c-snap", "deviceId": "dev-x", "seq": 1, "lamport": 1,
  "entityType": "snapshot", "entityUid": "snap-1", "field": null,
  "oldValue": null, "newValue": "{}", "createdAt": 0,
  "content": { "uid": "snap-1", "createdAt": 1, "kind": "current",
               "system": [], "user": [], "env": [] }
}"#;

#[test]
fn older_content_does_not_resurrect_a_deleted_profile() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "resurrect.db");

    // 1) 建立配置（lamport 1）
    let create = format!(
        r#"[{SNAPSHOT}, {{
          "uid": "c-prof-1", "deviceId": "dev-x", "seq": 2, "lamport": 1,
          "entityType": "profile", "entityUid": "p-1", "field": null,
          "oldValue": null, "newValue": "{{}}", "createdAt": 0,
          "content": {{ "uid": "p-1", "name": "Work", "description": null, "tags": [],
                       "createdAt": 1, "updatedAt": 2, "version": 1, "snapshotUid": "snap-1" }}
        }}]"#
    );
    import_changes(&mut db, &dir, "create.pesync", &create, 2);

    // 2) 删除（lamport 5）
    let tomb = r#"[{
      "uid": "c-tomb", "deviceId": "dev-x", "seq": 3, "lamport": 5,
      "entityType": "profile", "entityUid": "p-1", "field": "deleted_at",
      "oldValue": "null", "newValue": "2000000000000", "createdAt": 0, "content": null
    }]"#;
    import_changes(&mut db, &dir, "tomb.pesync", tomb, 1);

    // 3) 旧的 profile 内容（lamport 3）必须被 KeepLocal 跳过。
    //
    // 这是实体级 LWW 的**唯一**判据：若改回字段级（`field = null` 那条线），
    // 本地看到的最新内容变更仍是 lamport 1，3 > 1 → TakeRemote → 配置复活。
    let older = r#"[{
      "uid": "c-prof-old", "deviceId": "dev-x", "seq": 4, "lamport": 3,
      "entityType": "profile", "entityUid": "p-1", "field": null,
      "oldValue": null, "newValue": "{}", "createdAt": 0,
      "content": { "uid": "p-1", "name": "Work", "description": null, "tags": [],
                   "createdAt": 1, "updatedAt": 2, "version": 1, "snapshotUid": "snap-1" }
    }]"#;
    let report = import_changes(&mut db, &dir, "older.pesync", older, 1);
    assert_eq!(report.skipped, 1, "低 lamport 的旧内容必须跳过");

    let deleted: Option<i64> = db
        .query_row_with(
            "SELECT deleted_at FROM profiles WHERE uid = 'p-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(deleted.is_some(), "删除状态不得被旧内容覆盖");
}
