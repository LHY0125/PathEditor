// 一文件一测试（SQLCipher）。
//
// **本实现超出计划的维度：`enabled` 变化。**
//
// `PathEntry { path, enabled }` 是跨层契约（CLAUDE.md：「导入、配置、注册表和
// 撤销重做都必须保留 `enabled`」）。计划的 `compare_paths` 只比路径串，
// 「同一条路径从启用变为禁用」会显示成「没有差异」——与是同一类遗漏：
// 不报错、只给错误结论。
//
// 本测试是这个新增维度的唯一守卫。
use path_editor_core::db::compare::compare;
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn compare_detects_enabled_toggle_without_path_change() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("t.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let mk = |enabled: bool| NewSnapshot {
        kind: SnapshotKind::History,
        source: EventSource::Gui,
        system: vec![
            PathEntry {
                path: "C:\\SAME".into(),
                enabled,
            },
            PathEntry {
                path: "C:\\OTHER".into(),
                enabled: true,
            },
        ],
        user: vec![],
        env: vec![],
        parent_uid: None,
    };

    let a = insert_snapshot(&mut db, mk(true)).unwrap();
    let b = insert_snapshot(&mut db, mk(false)).unwrap();

    let r = compare(&db, &a, &b).unwrap();

    // 路径集合完全相同
    assert!(r.system.added.is_empty(), "没有新增");
    assert!(r.system.removed.is_empty(), "没有删除");
    assert!(r.system.reordered.is_empty(), "位置没变");

    // 但启用状态变了——这是真实差异，不能报「无变化」
    assert_eq!(
        r.system.toggled.len(),
        1,
        "启用状态变化必须被发现，实际 toggled={:?}",
        r.system.toggled
    );
    assert_eq!(r.system.toggled[0].path, "C:\\SAME");
    assert!(r.system.toggled[0].left_enabled);
    assert!(!r.system.toggled[0].right_enabled);

    // is_empty 必须把这个维度算进去，否则 UI 会显示「无变化」
    assert!(
        !r.system.is_empty(),
        "只有 enabled 变化时 is_empty 也必须是 false"
    );

    // 自比对：同一个快照，没有任何差异
    let same = compare(&db, &a, &a).unwrap();
    assert!(same.system.is_empty(), "自比对必须无差异");
}
