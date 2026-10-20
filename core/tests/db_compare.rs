// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::compare::compare;
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn compare_reports_both_hives_added_removed_and_reorder() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("c.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let mk = |sys: &[&str], usr: &[&str]| NewSnapshot {
        kind: SnapshotKind::History,
        source: EventSource::Gui,
        system: sys
            .iter()
            .map(|p| PathEntry {
                path: p.to_string(),
                enabled: true,
            })
            .collect(),
        user: usr
            .iter()
            .map(|p| PathEntry {
                path: p.to_string(),
                enabled: true,
            })
            .collect(),
        env: vec![],
        parent_uid: None,
    };

    let a = insert_snapshot(&mut db, mk(&["C:\\A", "C:\\B"], &["C:\\U1"])).unwrap();
    let b = insert_snapshot(
        &mut db,
        mk(&["C:\\B", "C:\\A", "C:\\C"], &["C:\\U1", "C:\\U2"]),
    )
    .unwrap();

    let r = compare(&db, &a, &b).unwrap();

    // system 侧
    assert_eq!(r.system.added.len(), 1, "system 新增 C:\\C");
    assert_eq!(r.system.added[0].path, "C:\\C");
    assert!(r.system.removed.is_empty(), "system 没有删除");
    assert!(!r.system.reordered.is_empty(), "system 的 A/B 顺序变了");

    // user 侧（N-8：必须分开断言，只比 system 时这里会全空而不报错）
    assert_eq!(r.user.added.len(), 1, "user 新增 C:\\U2");
    assert_eq!(r.user.added[0].path, "C:\\U2");
    assert!(r.user.removed.is_empty());
    assert!(r.user.reordered.is_empty(), "user 顺序没变");

    // env 三字段恒为空（T-3）
    assert!(r.env_added.is_empty() && r.env_removed.is_empty() && r.env_modified.is_empty());

    // 自比对：两侧都为空
    let same = compare(&db, &a, &a).unwrap();
    assert!(
        same.system.is_empty() && same.user.is_empty(),
        "自比对必须两侧都无差异"
    );
}
