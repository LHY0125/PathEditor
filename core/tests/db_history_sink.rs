// 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{list_events, EventFilter};
use path_editor_core::db::history_sink::DbHistory;
use path_editor_core::db::Db;
use path_editor_core::history::{EventAction, EventResult, EventSource, HistoryEvent, HistorySink};
use path_editor_core::EnvHive;
use tempfile::TempDir;

#[test]
fn db_history_records_events_and_counts_total() {
    let dir = TempDir::new().unwrap();

    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("s.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let make = |action: EventAction| HistoryEvent {
        source: EventSource::Gui,
        action,
        hive: Some(EnvHive::User),
        target: Some("Path".into()),
        changed_fields: vec!["path_entries".into()],
        before_revision: None,
        after_revision: Some("rev1".into()),
        result: EventResult::Success,
        snapshot_uid: None,
    };

    // 逐条记录（每次借用同一 Db）
    for action in [EventAction::Save, EventAction::Apply, EventAction::Restore] {
        let sink = DbHistory::new(&mut db);
        sink.record(make(action)).unwrap();
    }

    let page = list_events(&db, &EventFilter::default()).unwrap();
    assert_eq!(page.items.len(), 3, "应记录 3 条事件");
    assert_eq!(page.total, 3);

    // 事件里必须带 hive 与 target（历史面板的筛选价值依赖它们）
    assert_eq!(page.items[0].hive.as_deref(), Some("user"));
    assert_eq!(page.items[0].target.as_deref(), Some("Path"));

    // H1：limit=2 时本页 2 条，但 total 仍是全量 3
    let paged = list_events(
        &db,
        &EventFilter {
            limit: 2,
            offset: 0,
            action: None,
            source: None,
        },
    )
    .unwrap();
    assert_eq!(paged.items.len(), 2, "limit 生效");
    assert_eq!(paged.total, 3, "H1：total 必须独立计数，不等于本页条数");

    // 筛选与 total 用同一 WHERE
    let filtered = list_events(
        &db,
        &EventFilter {
            limit: 50,
            offset: 0,
            action: Some("apply".into()),
            source: None,
        },
    )
    .unwrap();
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.items[0].action, EventAction::Apply);
}
