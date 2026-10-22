// 一文件一测试（SQLCipher）。
//
// ** 时区约束**：`date(..., 'localtime')` 取 C 运行时时区，代码里没有任何地方
// 强制 `Asia/Shanghai`。本测试**不使用固定的日期字面量**，只断言「有且只有一天」
// 与「该天的值是确定的」——避免在 23:59 附近跑时因跨日而假失败。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::history_sink::DbHistory;
use path_editor_core::db::stats::{dashboard, rebuild_daily};
use path_editor_core::db::Db;
use path_editor_core::history::{EventAction, EventResult, EventSource, HistoryEvent, HistorySink};
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn daily_stats_aggregates_events_and_is_rebuildable() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("st.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let snap_uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![
                PathEntry {
                    path: "C:\\A".into(),
                    enabled: true,
                },
                PathEntry {
                    path: "C:\\BB".into(),
                    enabled: true,
                },
            ],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();

    // 把它设为当前快照——`current_*` 字段依赖 `app_state.current_snapshot_id`
    let version = path_editor_core::db::history::app_state_version(&db).unwrap();
    path_editor_core::db::history::set_current_snapshot(&mut db, &snap_uid, version).unwrap();

    for _ in 0..3 {
        DbHistory::new(&mut db)
            .record(HistoryEvent {
                source: EventSource::Gui,
                action: EventAction::Save,
                hive: None,
                target: Some("Path".into()),
                changed_fields: vec![],
                before_revision: None,
                after_revision: None,
                result: EventResult::Success,
                snapshot_uid: None,
            })
            .unwrap();
    }

    let n = rebuild_daily(&mut db).unwrap();
    assert_eq!(n, 1, "同一天只应有一行");

    let d = dashboard(&db, 7).unwrap();
    assert_eq!(d.days.len(), 1);
    assert_eq!(d.days[0].events, 3, "当天 3 条事件");
    assert_eq!(d.days[0].path_items, 2, "当日最后一条快照有 2 条 PATH");

    // ──：新增字段必须有断言，否则「看起来对、语义错」没人守 ──
    assert_eq!(d.current_path_items, 2, "当前快照 2 条 PATH");
    // C:\A 是 4 个字符、C:\BB 是 5 个字符，平均 4.5。
    // 用 AVG(length(path)) 算的是**字符数**（SQLite 的 length 对 TEXT 返回字符数）。
    assert!(
        (d.current_avg_len - 4.5).abs() < 1e-9,
        "C:\\A 长 4、C:\\BB 长 5，平均应为 4.5，实际 {}",
        d.current_avg_len
    );

    // 最常改变的变量：3 条 target=Path 的事件
    assert_eq!(d.top_targets.len(), 1, "只有一个 target");
    assert_eq!(d.top_targets[0].target, "Path");
    assert_eq!(d.top_targets[0].count, 3);

    // 配置应用次数：没有配置时为空，不报错
    assert!(d.profile_applies.is_empty());

    // 可重建
    db.with_transaction(|tx| {
        tx.execute("DELETE FROM daily_stats", []).map_err(|e| {
            path_editor_core::CoreError::new(
                path_editor_core::ErrorCode::Database,
                "clear",
                e.to_string(),
            )
        })?;
        Ok(())
    })
    .unwrap();
    assert!(dashboard(&db, 7).unwrap().days.is_empty());
    rebuild_daily(&mut db).unwrap();
    assert_eq!(dashboard(&db, 7).unwrap().days.len(), 1);
}
