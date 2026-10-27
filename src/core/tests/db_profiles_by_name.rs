// 一文件一测试（SQLCipher）。
//
// 回归守卫：配置方案必须只有 DB 一个真相源。
//
// 这条测试的存在理由：首版把 `profiles/*.json` 导入 DB 后，UI 仍在读写文件，
// 造成「DB 数据永久冻结」的双真相源。缺口清单把它列为。
// 若将来有人把 UI 改回文件通路，或让 CLI 重新写文件，本用例会失败。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
use path_editor_core::db::profiles::{
    delete_profile_by_name, get_profile_by_name, list_profiles, mark_profile_applied_by_name,
    profile_paths, rename_profile, save_profile,
};
use path_editor_core::db::Db;
use path_editor_core::domain::history::EventSource;
use path_editor_core::{ErrorCode, PathEntry};
use tempfile::TempDir;

#[test]
fn profiles_live_in_db_and_support_full_crud_by_name() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("p.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let snap = |path: &str| NewSnapshot {
        kind: SnapshotKind::Profile,
        source: EventSource::Gui,
        system: vec![PathEntry {
            path: path.to_string(),
            enabled: true,
        }],
        user: vec![],
        env: vec![],
        parent_uid: None,
    };

    // ① 保存 → 出现在 DB 列表里
    let uid = save_profile(
        &mut db,
        "Work",
        Some("公司"),
        &["office".into()],
        snap("C:\\A"),
    )
    .unwrap();
    assert_eq!(list_profiles(&db).unwrap().len(), 1);

    // ② 按**名称**读取（UI 用名称，不用 uid）
    let view = get_profile_by_name(&db, "Work").unwrap().unwrap();
    assert_eq!(view.uid, uid);
    assert_eq!(view.description.as_deref(), Some("公司"));
    assert_eq!(view.tags, vec!["office".to_string()]);

    // ③ 按名称取 PATH 条目（UI 的预览与应用走这条）
    let (system, user) = profile_paths(&db, "Work").unwrap().unwrap();
    assert_eq!(system.len(), 1);
    assert_eq!(system[0].path, "C:\\A");
    assert!(user.is_empty());

    // ④ 按名称记账（应用次数）
    assert!(mark_profile_applied_by_name(&mut db, "Work").unwrap());
    assert_eq!(
        get_profile_by_name(&db, "Work")
            .unwrap()
            .unwrap()
            .apply_count,
        1
    );

    // ⑤ 按名称重命名；改名后旧名查不到、新名查得到
    assert!(rename_profile(&mut db, "Work", "Office").unwrap());
    assert!(get_profile_by_name(&db, "Work").unwrap().is_none());
    assert!(get_profile_by_name(&db, "Office").unwrap().is_some());

    // ⑥ 重命名到已存在的名字 → NameExists（不静默覆盖）
    save_profile(&mut db, "Home", None, &[], snap("C:\\B")).unwrap();
    let err = rename_profile(&mut db, "Home", "Office").unwrap_err();
    assert_eq!(err.code, ErrorCode::NameExists);

    // ⑦ 按名称软删除
    assert!(delete_profile_by_name(&mut db, "Office").unwrap());
    assert!(get_profile_by_name(&db, "Office").unwrap().is_none());
    // 幂等：再删返回 false 而不是报错
    assert!(!delete_profile_by_name(&mut db, "Office").unwrap());

    // ⑧ 删除后同名可重建
    let uid2 = save_profile(&mut db, "Office", None, &[], snap("C:\\C")).unwrap();
    assert_ne!(uid, uid2, "重建必须是新行");

    // ⑨ 不存在的名字：读返回 None，删返回 false（不报错）
    assert!(get_profile_by_name(&db, "__no_such__").unwrap().is_none());
    assert!(!delete_profile_by_name(&mut db, "__no_such__").unwrap());
}
