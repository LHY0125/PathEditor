//! 配置同步：profile content 落库，且 profile 在 bundle 中先于 snapshot 时仍能应用。
//!
//! 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::Db;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join(name), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    db
}

fn write_bundle(path: &std::path::Path, changes_json: &str, count: u32) {
    let header = BundleHeader {
        schema_version: 3,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 9,
        change_count: count,
    };
    let payload = format!("{{\"changes\":{changes_json}}}");
    std::fs::write(path, seal("pw", &header, payload.as_bytes()).unwrap()).unwrap();
}

#[test]
fn profile_is_applied_after_its_snapshot_even_if_bundle_order_is_reversed() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "profile_order.db");

    // **profile 在前、snapshot 在后**：不按依赖排序时，profile 的 snapshotUid
    // 查不到本地快照，必须报 SyncFormat 并整包回滚。
    let changes = r#"[
      {
        "uid": "c-prof", "deviceId": "dev-x", "seq": 1, "lamport": 1,
        "entityType": "profile", "entityUid": "p-1", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": {
          "uid": "p-1", "name": "Work", "description": null, "tags": ["a"],
          "createdAt": 1, "updatedAt": 2, "version": 1, "snapshotUid": "snap-1"
        }
      },
      {
        "uid": "c-snap", "deviceId": "dev-x", "seq": 2, "lamport": 2,
        "entityType": "snapshot", "entityUid": "snap-1", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": {
          "uid": "snap-1", "createdAt": 1, "kind": "current",
          "system": [{"path": "C:\\A", "enabled": true}], "user": [], "env": []
        }
      }
    ]"#;
    let path = dir.path().join("profile_order.pesync");
    write_bundle(&path, changes, 2);

    let report = import_bundle(&mut db, "pw", &path).unwrap();
    assert_eq!(report.applied, 2, "快照与配置都必须落库");
    assert_eq!(report.skipped_unsupported, 0, "已知类型全部可落库");

    let name: String = db
        .query_row_with("SELECT name FROM profiles WHERE uid = 'p-1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(name, "Work");

    let revisions: i64 = db
        .query_row_with(
            "SELECT count(*) FROM profile_revisions r
               JOIN snapshots s ON s.id = r.snapshot_id
              WHERE s.uid = 'snap-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(revisions, 1, "profile_revision 必须指向导入的快照");
}
