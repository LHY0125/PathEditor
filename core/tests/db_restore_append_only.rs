// 一文件一测试（SQLCipher）。
//
// 回归守卫：恢复快照必须是 **append-only**。
//
// 「restore_snapshot(uid)：把该快照交给既有注册表写入编排；
// 成功后追加新快照与事件（**append-only，不回滚 DB**）」。
// 缺口清单指出 `history_restore_snapshot` 在首版完全不存在。
//
// 本测试验证 DB 层的 append-only 语义：恢复 = 读旧快照 + 写新快照 + 记新事件，
// **旧快照与旧事件都还在**。注册表写入本身由既有编排负责（不在此测试范围，
// 因为它会碰真实注册表）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{
    get_snapshot, insert_snapshot, list_events, EventFilter, NewSnapshot, SnapshotKind,
};
use path_editor_core::db::history_sink::DbHistory;
use path_editor_core::db::Db;
use path_editor_core::history::{EventAction, EventResult, EventSource, HistoryEvent, HistorySink};
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn restore_is_append_only_and_keeps_old_snapshots() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("r.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let mk = |path: &str| NewSnapshot {
        kind: SnapshotKind::History,
        source: EventSource::Gui,
        system: vec![PathEntry {
            path: path.to_string(),
            enabled: true,
        }],
        user: vec![],
        env: vec![],
        parent_uid: None,
    };

    // ① 建两个历史快照（模拟两次保存）
    let old_uid = insert_snapshot(&mut db, mk("C:\\Old")).unwrap();
    let new_uid = insert_snapshot(&mut db, mk("C:\\New")).unwrap();
    let snapshots_before: i64 = db
        .query_row_with("SELECT count(*) FROM snapshots", [], |r| r.get(0))
        .unwrap();
    assert_eq!(snapshots_before, 2);

    // ② 模拟「恢复到 old_uid」：命令层会先建一份操作前快照，再写注册表，
    //   最后通过 sink 记事件。这里直接验证 DB 侧的 append 语义。
    let before_restore = insert_snapshot(&mut db, mk("C:\\New")).unwrap();
    {
        let sink = DbHistory::with_snapshot(&mut db, Some(before_restore.clone()));
        sink.record(HistoryEvent {
            source: EventSource::Gui,
            action: EventAction::Restore,
            hive: None,
            target: Some(old_uid.clone()),
            changed_fields: vec!["path_entries".into()],
            before_revision: None,
            after_revision: None,
            result: EventResult::Success,
            snapshot_uid: None,
        })
        .unwrap();
    }

    // ③ 旧快照**必须还在**（append-only 的核心断言）
    assert!(
        get_snapshot(&db, &old_uid, false).is_ok(),
        "恢复不得删除被恢复的那个快照"
    );
    assert!(
        get_snapshot(&db, &new_uid, false).is_ok(),
        "恢复不得删除恢复前的快照"
    );

    // ④ 快照数只增不减
    let snapshots_after: i64 = db
        .query_row_with("SELECT count(*) FROM snapshots", [], |r| r.get(0))
        .unwrap();
    assert_eq!(snapshots_after, 3, "恢复必须追加新快照，而不是替换");

    // ⑤ 恢复事件落库，且指向操作前快照
    let page = list_events(&db, &EventFilter::default()).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].action, EventAction::Restore);
    assert_eq!(page.items[0].target.as_deref(), Some(old_uid.as_str()));
    assert_eq!(
        page.items[0].snapshot_uid.as_deref(),
        Some(before_restore.as_str()),
        "恢复事件必须关联到操作前快照（这样恢复本身也能被撤销）"
    );

    // ⑥ 被恢复的快照内容可读，且是「操作前」的状态
    let view = get_snapshot(&db, &old_uid, false).unwrap();
    assert_eq!(view.system[0].path, "C:\\Old");
}
