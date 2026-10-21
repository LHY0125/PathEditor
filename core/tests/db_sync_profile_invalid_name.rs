//! 同步包里的配置名称必须过与本地保存同一套校验（J-1）。
//!
//! 包虽然用口令加密，但发送端可能是更新版本、有 bug 的版本，或被分享口令的
//! 另一台机器——名称属于**跨信任边界的外部输入**，且会持久化并被 UI 渲染。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::Db;
use path_editor_core::ErrorCode;
use tempfile::TempDir;

fn write_bundle(dir: &TempDir, file: &str, name_json: &str) {
    let changes = format!(
        r#"[
          {{ "uid": "c-snap", "deviceId": "dev-x", "seq": 1, "lamport": 1,
             "entityType": "snapshot", "entityUid": "snap-1", "field": null,
             "oldValue": null, "newValue": "{{}}", "createdAt": 0,
             "content": {{ "uid": "snap-1", "createdAt": 1, "kind": "current",
                          "system": [], "user": [], "env": [] }} }},
          {{ "uid": "c-prof", "deviceId": "dev-x", "seq": 2, "lamport": 2,
             "entityType": "profile", "entityUid": "p-1", "field": null,
             "oldValue": null, "newValue": "{{}}", "createdAt": 0,
             "content": {{ "uid": "p-1", "name": {name_json}, "description": null,
                          "tags": [], "createdAt": 1, "updatedAt": 2, "version": 1,
                          "snapshotUid": "snap-1" }} }}
        ]"#
    );
    let header = BundleHeader {
        schema_version: 3,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 10,
        change_count: 2,
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
}

#[test]
fn invalid_profile_names_are_rejected_and_the_whole_bundle_rolls_back() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("bad_name.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    // 空名 / 纯空白名 / 超长名 / 含控制字符 —— 四种都必须整包拒绝
    let cases = [
        ("empty", "\"\""),
        ("blank", "\"   \""),
        (
            "toolong",
            "\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"",
        ),
        ("control", "\"bad\\u0007name\""),
    ];

    for (label, name_json) in cases {
        write_bundle(&dir, &format!("{label}.pesync"), name_json);
        let path = dir.path().join(format!("{label}.pesync"));

        let err = import_bundle(&mut db, "pw", &path)
            .err()
            .unwrap_or_else(|| panic!("{label}: 非法名称必须被拒绝"));
        assert_eq!(
            err.code,
            ErrorCode::SyncFormat,
            "{label}: 错误码必须是 SyncFormat（包格式错，不是用户输入错）"
        );
        assert!(
            err.message.contains("配置名称不合法"),
            "{label}: 错误文案必须点明原因，实际: {}",
            err.message
        );

        // 整包回滚：快照与变更都不得落库
        let profiles: i64 = db
            .query_row_with("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))
            .unwrap();
        let changes: i64 = db
            .query_row_with("SELECT COUNT(*) FROM changes", [], |r| r.get(0))
            .unwrap();
        let snaps: i64 = db
            .query_row_with("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            (profiles, changes, snaps),
            (0, 0, 0),
            "{label}: 拒绝时必须整包回滚，不得留下半应用状态"
        );
    }
}
