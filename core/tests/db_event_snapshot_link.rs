// 一文件一测试：见 M0 §5.3。
//
// **P0-2 回归守卫**：事件必须能关联到快照（`events.snapshot_id` 非 NULL）。
//
// 缺口清单 P0-2：首版 `service.rs:351` / `:486` 把 `snapshot_uid` 写死为 `None`，
// 导致历史面板的详情按钮恒禁用。修复方式是在命令层「先建操作前快照、再调服务」，
// 通过 `DbHistory::with_snapshot` 把 uid 绑定给该次操作的所有事件。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{list_events, EventFilter, NewSnapshot, SnapshotKind};
use path_editor_core::db::history_sink::DbHistory;
use path_editor_core::db::Db;
use path_editor_core::history::{EventAction, EventResult, EventSource, HistoryEvent, HistorySink};
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn events_recorded_through_bound_sink_point_at_the_snapshot() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("s.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    // 命令层的做法：先建「操作前」快照
    let before = path_editor_core::db::history::insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::History,
            source: EventSource::Gui,
            system: vec![PathEntry {
                path: "C:\\Before".into(),
                enabled: true,
            }],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();

    // 再把 uid 绑定给 sink，记一条事件
    {
        let sink = DbHistory::with_snapshot(&mut db, Some(before.clone()));
        sink.record(HistoryEvent {
            source: EventSource::Gui,
            action: EventAction::Save,
            hive: None,
            target: None,
            changed_fields: vec!["path_entries".into()],
            before_revision: None,
            after_revision: None,
            result: EventResult::Success,
            // 事件自身不带 uid —— 必须回落到 sink 预绑定的那个
            snapshot_uid: None,
        })
        .unwrap();
    }

    // ① 事件读回来必须带 snapshotUid（这就是 P0-2 的核心断言）
    let page = list_events(&db, &EventFilter::default()).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].snapshot_uid.as_deref(),
        Some(before.as_str()),
        "P0-2：事件必须指向命令层建的操作前快照"
    );

    // ② 落库的 snapshot_id 也必须非 NULL（读的是同一列的两条路径）
    let id_in_db: Option<i64> = db
        .query_row_optional("SELECT snapshot_id FROM events LIMIT 1", [], |r| r.get(0))
        .unwrap()
        .flatten();
    assert!(id_in_db.is_some(), "events.snapshot_id 不得为 NULL");

    // ③ 快照内容确实能读出来（面板点击后要展示的就是它）
    let view = path_editor_core::db::history::get_snapshot(&db, &before, false).unwrap();
    assert_eq!(view.system.len(), 1);
    assert_eq!(view.system[0].path, "C:\\Before");

    // ④ 事件自带的 snapshot_uid 优先于预绑定的
    let other = path_editor_core::db::history::insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::History,
            source: EventSource::Gui,
            system: vec![],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();
    {
        let sink = DbHistory::with_snapshot(&mut db, Some(before.clone()));
        sink.record(HistoryEvent {
            source: EventSource::Gui,
            action: EventAction::Apply,
            hive: None,
            target: None,
            changed_fields: vec![],
            before_revision: None,
            after_revision: None,
            result: EventResult::Success,
            snapshot_uid: Some(other.clone()),
        })
        .unwrap();
    }
    let page = list_events(&db, &EventFilter::default()).unwrap();
    let apply = page
        .items
        .iter()
        .find(|e| e.action == EventAction::Apply)
        .unwrap();
    assert_eq!(
        apply.snapshot_uid.as_deref(),
        Some(other.as_str()),
        "事件自带的 snapshot_uid 必须优先"
    );
}
