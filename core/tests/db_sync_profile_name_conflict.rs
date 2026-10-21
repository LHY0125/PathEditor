//! 同名不同 uid 的配置：自动改名导入，本地配置不受影响。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::{profiles, Db};
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use rusqlite::params;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join(name), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    db
}

#[test]
fn same_name_different_uid_is_imported_with_device_suffix() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "name_conflict.db");

    // 本地已有 "Work"
    let snap = NewSnapshot {
        kind: SnapshotKind::Current,
        source: EventSource::Gui,
        system: vec![PathEntry {
            path: "C:\\LOCAL".to_string(),
            enabled: true,
        }],
        user: vec![],
        env: vec![],
        parent_uid: None,
    };
    let local_uid = profiles::save_profile(&mut db, "Work", None, &[], snap).unwrap();

    // 远端也有一个 "Work"，uid 不同
    let changes = r#"[
      {
        "uid": "c-snap", "deviceId": "dev-x", "seq": 1, "lamport": 10,
        "entityType": "snapshot", "entityUid": "snap-remote", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": {
          "uid": "snap-remote", "createdAt": 1, "kind": "current",
          "system": [{"path": "C:\\REMOTE", "enabled": true}], "user": [], "env": []
        }
      },
      {
        "uid": "c-prof", "deviceId": "dev-x", "seq": 2, "lamport": 11,
        "entityType": "profile", "entityUid": "p-remote", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": {
          "uid": "p-remote", "name": "Work", "description": null, "tags": [],
          "createdAt": 1, "updatedAt": 2, "version": 1, "snapshotUid": "snap-remote"
        }
      }
    ]"#;
    let header = BundleHeader {
        schema_version: 3,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 10,
        change_count: 2,
    };
    let path = dir.path().join("conflict.pesync");
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
    assert_eq!(report.renamed_profiles, 1, "必须报告改名数量");
    assert_eq!(report.applied, 2);

    let local_name: String = db
        .query_row_with(
            "SELECT name FROM profiles WHERE uid = ?1",
            params![local_uid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(local_name, "Work", "本地配置不得被改名");

    let imported_name: String = db
        .query_row_with(
            "SELECT name FROM profiles WHERE uid = 'p-remote'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(imported_name, "Work (来自 dev-x)");
}
