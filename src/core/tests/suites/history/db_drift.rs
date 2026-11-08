//! 注册表漂移检测：只记事件、同一外部状态去重、不建快照。
use path_editor_core::app::bootstrap::{ensure_baseline_from, record_drift};
use path_editor_core::domain::history::EventSource;
use path_editor_core::{PathEntry, PathSnapshot};

fn entry(path: &str, enabled: bool) -> PathEntry {
    PathEntry {
        path: path.into(),
        enabled,
    }
}

fn snapshot(system: Vec<PathEntry>, user: Vec<PathEntry>) -> PathSnapshot {
    PathSnapshot { system, user }
}

fn drift_events(db: &path_editor_core::db::Db) -> i64 {
    db.query_row_with(
        "SELECT COUNT(*) FROM events WHERE action = 'drift'",
        [],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn drift_records_once_per_external_state() {
    let (_dir, mut db) = crate::common::new_db("drift.db");
    assert!(ensure_baseline_from(
        &mut db,
        snapshot(vec![entry("C:\\A", true)], vec![entry("D:\\U", false)]),
    )
    .unwrap());

    // 无差异 → 不写事件
    let same = snapshot(vec![entry("C:\\A", true)], vec![entry("D:\\U", false)]);
    assert!(record_drift(&mut db, EventSource::Gui, &same)
        .unwrap()
        .is_none());
    assert_eq!(drift_events(&db), 0);

    // 大小写不同不算漂移（Windows 路径不区分大小写）
    let case_only = snapshot(vec![entry("c:\\a", true)], vec![entry("D:\\U", false)]);
    assert!(record_drift(&mut db, EventSource::Gui, &case_only)
        .unwrap()
        .is_none());

    // 外部新增一条 → 记一条 system 漂移
    let changed = snapshot(
        vec![entry("C:\\A", true), entry("C:\\B", true)],
        vec![entry("D:\\U", false)],
    );
    let outcome = record_drift(&mut db, EventSource::Gui, &changed)
        .unwrap()
        .expect("必须检测到漂移");
    assert_eq!(outcome.fields, vec!["system".to_string()]);
    assert!(outcome.recorded);
    assert_eq!(drift_events(&db), 1);
    let (action, result, target): (String, String, Option<String>) = db
        .query_row_with(
            "SELECT action, result, target FROM events WHERE action = 'drift'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(action, "drift");
    assert_eq!(result, "partial");
    assert_eq!(target.as_deref(), Some("system"));

    // 同一外部状态重复启动 → 不再重复记事件
    let again = record_drift(&mut db, EventSource::Gui, &changed)
        .unwrap()
        .expect("仍应报告漂移");
    assert!(!again.recorded, "同一状态不得重复写事件");
    assert_eq!(drift_events(&db), 1);

    // 用户 hive 的启用状态变化 → 新事件
    // system 与上一轮保持一致，只改 user，避免两个 hive 同时命中
    let user_changed = snapshot(vec![entry("C:\\A", true)], vec![entry("D:\\U", true)]);
    let outcome = record_drift(&mut db, EventSource::Gui, &user_changed)
        .unwrap()
        .expect("必须检测到 user 漂移");
    assert_eq!(outcome.fields, vec!["user".to_string()]);
    assert!(outcome.recorded);
    assert_eq!(drift_events(&db), 2);
}

#[test]
fn drift_without_current_snapshot_is_skipped() {
    let (_dir, mut db) = crate::common::new_db("drift-empty.db");
    let live = snapshot(vec![entry("C:\\A", true)], vec![]);
    assert!(record_drift(&mut db, EventSource::Gui, &live)
        .unwrap()
        .is_none());
    assert_eq!(drift_events(&db), 0);
}
