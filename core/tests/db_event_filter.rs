// 一文件一测试：见 M0 §5.3。
//
// **P1-1 回归守卫**：EventFilter 必须支持 spec §6.1 要求的全部 5 个筛选维度。
//
// 缺口清单 P1-1：首版只有 `action` / `source` 两个，缺
// `hive` / `target` / `result` / 时间范围。本测试逐个断言它们真的生效，
// 并确认 `total` 与分页用同一段 WHERE（旧代码的注释警告过这一点）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{list_events, EventFilter, NewSnapshot, SnapshotKind};
use path_editor_core::db::history_sink::DbHistory;
use path_editor_core::db::Db;
use path_editor_core::history::{EventAction, EventResult, EventSource, HistoryEvent, HistorySink};
use path_editor_core::{EnvHive, PathEntry};
use tempfile::TempDir;

#[test]
fn event_filter_supports_all_five_dimensions() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("f.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    // 建一个快照，让事件有 snapshot 可指（不影响本测试的筛选逻辑）
    let snap_uid = path_editor_core::db::history::insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::History,
            source: EventSource::Gui,
            system: vec![PathEntry {
                path: "C:\\X".into(),
                enabled: true,
            }],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();

    // 造 4 条覆盖不同维度的事件
    let record = |db: &mut Db, action, hive: Option<EnvHive>, target: Option<&str>, result| {
        let sink = DbHistory::with_snapshot(db, Some(snap_uid.clone()));
        sink.record(HistoryEvent {
            source: EventSource::Gui,
            action,
            hive,
            target: target.map(|s| s.to_string()),
            changed_fields: vec![],
            before_revision: None,
            after_revision: None,
            result,
            snapshot_uid: None,
        })
        .unwrap();
    };

    record(
        &mut db,
        EventAction::Save,
        Some(EnvHive::System),
        Some("Path"),
        EventResult::Success,
    );
    record(
        &mut db,
        EventAction::Save,
        Some(EnvHive::User),
        Some("JAVA_HOME"),
        EventResult::Partial,
    );
    record(
        &mut db,
        EventAction::Apply,
        Some(EnvHive::User),
        Some("JAVA_HOME"),
        EventResult::Failure,
    );
    record(
        &mut db,
        EventAction::Restore,
        None,
        None,
        EventResult::Success,
    );

    let all = list_events(&db, &EventFilter::default()).unwrap();
    assert_eq!(all.total, 4, "4 条事件都应存在");

    // ① hive
    let by_hive = list_events(
        &db,
        &EventFilter {
            hive: Some("user".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(by_hive.total, 2, "user hive 有 2 条");
    assert!(by_hive
        .items
        .iter()
        .all(|e| e.hive.as_deref() == Some("user")));

    // ② target
    let by_target = list_events(
        &db,
        &EventFilter {
            target: Some("JAVA_HOME".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(by_target.total, 2, "JAVA_HOME 有 2 条");

    // ③ result
    let by_result = list_events(
        &db,
        &EventFilter {
            result: Some("success".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(by_result.total, 2, "success 有 2 条");
    assert!(by_result
        .items
        .iter()
        .all(|e| e.result == EventResult::Success));

    // ④ 时间范围：全部事件都是「现在」，所以 from=0 命中全部、to=0 命中零条
    let from_epoch = list_events(
        &db,
        &EventFilter {
            from_ms: Some(0),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(from_epoch.total, 4, "from_ms=0 应命中全部");

    let before_epoch = list_events(
        &db,
        &EventFilter {
            to_ms: Some(0),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(before_epoch.total, 0, "to_ms=0 应命中零条（事件都在之后）");

    // ⑤ 多维度组合：user + JAVA_HOME + failure → 恰 1 条
    let combo = list_events(
        &db,
        &EventFilter {
            hive: Some("user".into()),
            target: Some("JAVA_HOME".into()),
            result: Some("failure".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(combo.total, 1);
    assert_eq!(combo.items[0].action, EventAction::Apply);

    // ⑥ total 与分页共用 WHERE：limit=1 时本页 1 条但 total 仍是 2
    let paged = list_events(
        &db,
        &EventFilter {
            limit: 1,
            hive: Some("user".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(paged.items.len(), 1, "limit 生效");
    assert_eq!(
        paged.total, 2,
        "total 必须是筛选后的全量计数，不受 limit 影响"
    );
}
