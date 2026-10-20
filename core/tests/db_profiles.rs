// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
use path_editor_core::db::profiles::{
    delete_profile, list_profiles, mark_profile_applied, save_profile,
};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn profile_create_delete_recreate_with_same_name() {
    let dir = TempDir::new().unwrap();

    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("p.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let snap = || NewSnapshot {
        kind: SnapshotKind::Profile,
        source: EventSource::Gui,
        system: vec![PathEntry {
            path: "C:\\A".into(),
            enabled: true,
        }],
        user: vec![],
        env: vec![],
        parent_uid: None,
    };

    let uid1 = save_profile(
        &mut db,
        "Work",
        Some("公司"),
        &["office".into(), "daily".into()],
        snap(),
    )
    .unwrap();

    let list = list_profiles(&db).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Work");
    assert_eq!(list[0].description.as_deref(), Some("公司"));
    assert_eq!(
        list[0].tags,
        vec!["office".to_string(), "daily".to_string()]
    );
    assert_eq!(list[0].apply_count, 0);
    assert!(list[0].snapshot_uid.is_some(), "配置必须关联到快照版本");

    // 同名重复保存 → NameExists（不静默覆盖）
    let dup = save_profile(&mut db, "Work", None, &[], snap());
    assert_eq!(
        dup.unwrap_err().code,
        path_editor_core::ErrorCode::NameExists
    );

    // 应用计数
    assert!(mark_profile_applied(&mut db, &uid1).unwrap());
    let list = list_profiles(&db).unwrap();
    assert_eq!(list[0].apply_count, 1);
    assert!(list[0].last_applied_at.is_some());

    // 软删除后不出现在列表
    assert!(delete_profile(&mut db, &uid1).unwrap());
    assert!(
        list_profiles(&db).unwrap().is_empty(),
        "软删除后不应出现在列表"
    );

    // 软删除后允许同名重建（B5 的部分唯一索引）
    let uid2 = save_profile(&mut db, "Work", None, &[], snap()).unwrap();
    assert_ne!(uid1, uid2, "重建必须是新行");
    assert_eq!(list_profiles(&db).unwrap().len(), 1);

    // 幂等删除：已删除再删返回 false 而不是报错
    assert!(!delete_profile(&mut db, &uid1).unwrap());
}
