// 一文件一测试（SQLCipher）。
// 的回归守卫：只有 user 侧变化时，compare 必须能发现。
// 若 compare() 只比 system（首版实现），本测试会失败——这正是它存在的理由。
use path_editor_core::db::compare::compare;
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn compare_detects_user_only_change() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("c2.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let mk = |usr: &[&str]| NewSnapshot {
        kind: SnapshotKind::History,
        source: EventSource::Gui,
        system: vec![PathEntry {
            path: "C:\\S".into(),
            enabled: true,
        }],
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

    let a = insert_snapshot(&mut db, mk(&["C:\\U1"])).unwrap();
    let b = insert_snapshot(&mut db, mk(&["C:\\U1", "C:\\U2"])).unwrap();

    let r = compare(&db, &a, &b).unwrap();
    assert!(r.system.is_empty(), "system 未变");
    assert_eq!(r.user.added.len(), 1, "user 的差异必须被发现");
    assert_eq!(r.user.added[0].path, "C:\\U2");
}
