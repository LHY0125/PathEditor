//! 字段级 LWW：profile 按 name/description/tags/content 四个字段合并。
use path_editor_core::db::profiles::{get_profile, list_profiles, rename_profile};
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::conflict::{resolve_conflict, Resolution};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::Db;
use tempfile::TempDir;

fn import(
    db: &mut Db,
    dir: &TempDir,
    file: &str,
    changes: &str,
    count: u32,
    base_lamport: i64,
) -> path_editor_core::db::sync::ImportReport {
    let header = BundleHeader {
        schema_version: 7,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport,
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

const BASE_PROFILE: &str = r#"{
  "uid": "c-prof", "deviceId": "dev-x", "seq": 2, "lamport": 1,
  "entityType": "profile", "entityUid": "p-1", "field": "name",
  "oldValue": null, "newValue": "{}", "createdAt": 0,
  "content": { "uid": "p-1", "name": "Work", "description": "BaseDesc", "tags": [],
               "createdAt": 1, "updatedAt": 2, "version": 1, "snapshotUid": "snap-1" }
}"#;

fn profile_content(name: &str, description: &str) -> String {
    format!(
        r#"{{ "uid": "p-1", "name": "{name}", "description": "{description}", "tags": [],
             "createdAt": 1, "updatedAt": 20, "version": 2, "snapshotUid": "snap-1" }}"#
    )
}

fn field_change(
    uid: &str,
    field: &str,
    new_value: &str,
    lamport: i64,
    device: &str,
    content: String,
) -> String {
    format!(
        r#"{{ "uid": "{uid}", "deviceId": "{device}", "seq": 3, "lamport": {lamport},
             "entityType": "profile", "entityUid": "p-1", "field": "{field}",
             "oldValue": null, "newValue": "{new_value}", "createdAt": 0,
             "content": {content} }}"#
    )
}

#[test]
fn different_fields_from_two_devices_are_both_kept() {
    let dir = TempDir::new().unwrap();
    let mut db = Db::open(&dir.path().join("fields.db")).unwrap();
    db.ensure_schema().unwrap();

    // 建立基线（name 字段，lamport 1）
    import(
        &mut db,
        &dir,
        "base.pesync",
        &format!("[{SNAPSHOT},{BASE_PROFILE}]"),
        2,
        10,
    );
    // 本地改名 → 追加 name 字段变更（lamport 11）
    assert!(rename_profile(&mut db, "Work", "LocalName").unwrap());

    // 远端只改 description（lamport 12，高于本地 name）
    let remote = field_change(
        "c-desc",
        "description",
        "RemoteDesc",
        12,
        "dev-x",
        profile_content("RemoteName", "RemoteDesc"),
    );
    let report = import(&mut db, &dir, "desc.pesync", &format!("[{remote}]"), 1, 20);

    assert_eq!(report.conflicts, 0, "不同字段不应产生冲突");
    assert_eq!(report.applied, 1);
    let p = get_profile(&db, "p-1").unwrap().unwrap();
    assert_eq!(p.name, "LocalName", "本地 name 必须保留");
    assert_eq!(
        p.description.as_deref(),
        Some("RemoteDesc"),
        "远端 description 必须落地"
    );
    assert_eq!(list_profiles(&db).unwrap().len(), 1);
}

#[test]
fn same_field_conflict_resolves_only_that_field() {
    let dir = TempDir::new().unwrap();
    let mut db = Db::open(&dir.path().join("conflict.db")).unwrap();
    db.ensure_schema().unwrap();

    import(
        &mut db,
        &dir,
        "base.pesync",
        &format!("[{SNAPSHOT},{BASE_PROFILE}]"),
        2,
        10,
    );
    assert!(rename_profile(&mut db, "Work", "LocalName").unwrap());

    // 远端改同一字段 name，lamport 与本地 rename 相同（11）→ 冲突
    let remote = field_change(
        "c-name",
        "name",
        "RemoteName",
        11,
        "dev-x",
        profile_content("RemoteName", "RemoteDesc"),
    );
    let report = import(&mut db, &dir, "name.pesync", &format!("[{remote}]"), 1, 20);
    assert_eq!(report.conflicts, 1, "同字段同 lamport 必须记冲突");
    let p = get_profile(&db, "p-1").unwrap().unwrap();
    assert_eq!(p.name, "LocalName", "冲突未解决前保留本地");

    let conflict = path_editor_core::db::sync::conflict::list_conflicts(&db, false)
        .unwrap()
        .into_iter()
        .next()
        .expect("应有一条未解决冲突");
    assert_eq!(conflict.field.as_deref(), Some("name"));
    resolve_conflict(&mut db, conflict.id, Resolution::Remote).unwrap();

    let p = get_profile(&db, "p-1").unwrap().unwrap();
    assert_eq!(p.name, "RemoteName", "采纳远端后 name 更新");
    assert_eq!(
        p.description.as_deref(),
        Some("BaseDesc"),
        "只应用 name，不得用远端整实体覆盖 description"
    );
}

#[test]
fn exported_field_changes_recreate_profile_on_a_fresh_device() {
    use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
    use path_editor_core::db::profiles::save_profile;
    use path_editor_core::db::sync::export::export_bundle;
    use path_editor_core::domain::history::EventSource;

    let dir = TempDir::new().unwrap();
    let mut src = Db::open(&dir.path().join("src.db")).unwrap();
    src.ensure_schema().unwrap();
    let mut dst = Db::open(&dir.path().join("dst.db")).unwrap();
    dst.ensure_schema().unwrap();

    save_profile(
        &mut src,
        "Work",
        Some("公司"),
        &["office".into()],
        NewSnapshot {
            kind: SnapshotKind::Profile,
            source: EventSource::Gui,
            system: vec![],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();

    let bundle = dir.path().join("profile.pesync");
    let report = export_bundle(&mut src, "pw", &bundle).unwrap();
    assert!(report.change_count >= 4, "四个字段 + revision 都应导出");
    let raw = std::fs::read(&bundle).unwrap();
    let version = u32::from_le_bytes(raw[8..12].try_into().unwrap());
    assert_eq!(version, 2, "带字段变更的包必须写 FORMAT_VERSION = 2");
    let imported = import_bundle(&mut dst, "pw", &bundle).unwrap();
    assert_eq!(imported.applied, report.change_count);
    assert_eq!(imported.conflicts, 0);

    let p = list_profiles(&dst).unwrap();
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].name, "Work");
    assert_eq!(p[0].description.as_deref(), Some("公司"));
    assert_eq!(p[0].tags, vec!["office".to_string()]);
    assert!(p[0].snapshot_uid.is_some(), "内容字段必须恢复快照引用");

    // 幂等：同一 v2 包再导入一次不得产生新实体或字段覆盖。
    let again = import_bundle(&mut dst, "pw", &bundle).unwrap();
    assert_eq!(again.applied, 0);
    assert_eq!(again.skipped, report.change_count);
    assert_eq!(list_profiles(&dst).unwrap().len(), 1);
}

#[test]
fn newer_tombstone_blocks_a_field_edit_from_resurrecting_the_profile() {
    let dir = TempDir::new().unwrap();
    let mut db = Db::open(&dir.path().join("tomb-field.db")).unwrap();
    db.ensure_schema().unwrap();

    import(
        &mut db,
        &dir,
        "base.pesync",
        &format!("[{SNAPSHOT},{BASE_PROFILE}]"),
        2,
        10,
    );

    let tomb = r#"{
      "uid": "c-tomb", "deviceId": "dev-x", "seq": 3, "lamport": 50,
      "entityType": "profile", "entityUid": "p-1", "field": "deleted_at",
      "oldValue": "null", "newValue": "2000000000000", "createdAt": 0, "content": null
    }"#;
    import(&mut db, &dir, "tomb.pesync", &format!("[{tomb}]"), 1, 60);

    let edit = field_change(
        "c-edit",
        "name",
        "AfterDelete",
        40,
        "dev-x",
        profile_content("AfterDelete", "BaseDesc"),
    );
    let report = import(&mut db, &dir, "edit.pesync", &format!("[{edit}]"), 1, 60);
    assert_eq!(report.skipped, 1, "晚于编辑的墓碑必须保留");
    assert!(
        get_profile(&db, "p-1").unwrap().is_none(),
        "配置不得被旧编辑复活"
    );
}

#[test]
fn profile_change_without_field_is_rejected() {
    let dir = TempDir::new().unwrap();
    let mut db = Db::open(&dir.path().join("no-field.db")).unwrap();
    db.ensure_schema().unwrap();

    let legacy = format!(
        r#"{{ "uid": "c-prof", "deviceId": "dev-x", "seq": 1, "lamport": 1,
             "entityType": "profile", "entityUid": "p-1", "field": null,
             "oldValue": null, "newValue": "{{}}", "createdAt": 0,
             "content": {} }}"#,
        profile_content("Work", "BaseDesc")
    );
    let header = BundleHeader {
        schema_version: 7,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 10,
        change_count: 1,
    };
    let path = dir.path().join("no-field.pesync");
    std::fs::write(
        &path,
        seal(
            "pw",
            &header,
            format!("{{\"changes\":[{legacy}]}}").as_bytes(),
        )
        .unwrap(),
    )
    .unwrap();
    let err = import_bundle(&mut db, "pw", &path).unwrap_err();
    assert_eq!(err.code, path_editor_core::ErrorCode::SyncFormat);
    assert!(
        err.message.contains("字段名"),
        "错误必须说明缺少字段名: {}",
        err.message
    );
}
