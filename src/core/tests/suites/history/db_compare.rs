use path_editor_core::db::compare::compare;
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::domain::history::EventSource;
use path_editor_core::PathEntry;

#[test]
fn compare_reports_both_hives_added_removed_and_reorder() {
    let (_dir, mut db) = crate::common::new_db("c.db");

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

    // user 侧
    assert_eq!(r.user.added.len(), 1, "user 新增 C:\\U2");
    assert_eq!(r.user.added[0].path, "C:\\U2");
    assert!(r.user.removed.is_empty());
    assert!(r.user.reordered.is_empty(), "user 顺序没变");

    // env 三字段恒为空
    assert!(r.env_added.is_empty() && r.env_removed.is_empty() && r.env_modified.is_empty());

    // 自比对：两侧都为空
    let same = compare(&db, &a, &a).unwrap();
    assert!(
        same.system.is_empty() && same.user.is_empty(),
        "自比对必须两侧都无差异"
    );
}

#[test]
fn compare_reports_env_changes_without_values() {
    use path_editor_core::db::history::EnvValueRow;

    let (_dir, mut db) = crate::common::new_db("cenv.db");

    let row = |hive: &str, name: &str, revision: &str| EnvValueRow {
        hive: hive.into(),
        name: name.into(),
        kind: "String".into(),
        value: Some("secret-value".into()),
        revision: revision.into(),
        sensitive: false,
        restorable: true,
    };
    let mk = |env: Vec<EnvValueRow>| NewSnapshot {
        kind: SnapshotKind::History,
        source: EventSource::Gui,
        system: vec![],
        user: vec![],
        env,
        parent_uid: None,
    };

    let a = insert_snapshot(
        &mut db,
        mk(vec![row("user", "KEEP", "r1"), row("user", "GONE", "r1")]),
    )
    .unwrap();
    let b = insert_snapshot(
        &mut db,
        mk(vec![row("user", "KEEP", "r2"), row("user", "NEW", "r1")]),
    )
    .unwrap();

    let result = compare(&db, &a, &b).unwrap();
    assert_eq!(result.env_added, vec!["NEW"]);
    assert_eq!(result.env_removed, vec!["GONE"]);
    assert_eq!(result.env_modified, vec!["KEEP"]);
    assert_eq!(result.env_details.len(), 3);
    assert!(result.env_details.iter().all(|item| item.hive == "user"));

    // 对比结果不得泄露值
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("secret-value"), "对比结果不得包含变量值");
}
