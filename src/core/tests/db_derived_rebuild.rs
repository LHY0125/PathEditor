// 一文件一测试（SQLCipher）。
//
// 派生表重建的验收判据：清空三张派生表 → 跑一遍重建 → 数据恢复。
//
// 启动序列的编排在 `core::app::bootstrap`（`search_fts` 仅在为空时构建、
// `daily_stats` / `var_references` 按水位判断）；编排的幂等性由
// `bootstrap_initialize` 覆盖。本测试守的是更根本的性质：三个 rebuild
// 函数能各自独立完成重建。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{
    app_state_version, insert_snapshot, set_current_snapshot, NewSnapshot, SnapshotKind,
};
use path_editor_core::db::refs::rebuild_refs;
use path_editor_core::db::search::{rebuild_index, search};
use path_editor_core::db::stats::{dashboard, rebuild_daily};
use path_editor_core::db::Db;
use path_editor_core::domain::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn all_three_derived_tables_recover_after_being_cleared() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("d.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    // 造一份「当前快照」，三张派生表都要从它/它相关的事件重建
    let uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![PathEntry {
                path: "%JAVA_HOME%\\bin".into(),
                enabled: true,
            }],
            user: vec![PathEntry {
                path: "C:\\USR\\MARKER".into(),
                enabled: true,
            }],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();
    let v = app_state_version(&db).unwrap();
    set_current_snapshot(&mut db, &uid, v).unwrap();

    // ── 第一次重建：建立基线 ──
    let fts_n = rebuild_index(&mut db).unwrap();
    assert!(fts_n >= 2, "至少索引两条 PATH，实际 {fts_n}");
    assert!(
        !search(&db, "MARKER", 10).unwrap().is_empty(),
        "能搜到 user 条目"
    );
    assert!(!search(&db, "JAVA_HOME", 10).unwrap().is_empty());

    let daily_n = rebuild_daily(&mut db).unwrap();
    assert!(daily_n >= 1, "至少有当天一行");

    let refs_n = rebuild_refs(&mut db).unwrap();
    assert!(refs_n >= 1, "至少有 PATH→JAVA_HOME 一条边");

    // 记录基线值，供后面比对
    let before_edges: i64 = db
        .query_row_with("SELECT count(*) FROM var_references", [], |r| r.get(0))
        .unwrap();
    let before_days = dashboard(&db, 400).unwrap().days.len();

    // ── 清空三张派生表（模拟「索引损坏 / 被清空」）──
    db.with_transaction(|tx| {
        tx.execute("DELETE FROM search_fts", []).map_err(|e| {
            path_editor_core::CoreError::new(
                path_editor_core::ErrorCode::Database,
                "clear_fts",
                e.to_string(),
            )
        })?;
        tx.execute("DELETE FROM daily_stats", []).map_err(|e| {
            path_editor_core::CoreError::new(
                path_editor_core::ErrorCode::Database,
                "clear_daily",
                e.to_string(),
            )
        })?;
        tx.execute("DELETE FROM var_references", []).map_err(|e| {
            path_editor_core::CoreError::new(
                path_editor_core::ErrorCode::Database,
                "clear_refs",
                e.to_string(),
            )
        })?;
        Ok(())
    })
    .unwrap();

    // 清空后确实都空了（否则下面的「恢复」断言没有意义）
    assert!(search(&db, "MARKER", 10).unwrap().is_empty(), "FTS 已清空");
    assert!(
        dashboard(&db, 400).unwrap().days.is_empty(),
        "daily_stats 已清空"
    );
    let empty_edges: i64 = db
        .query_row_with("SELECT count(*) FROM var_references", [], |r| r.get(0))
        .unwrap();
    assert_eq!(empty_edges, 0, "var_references 已清空");

    // ── 重建：数据必须完全恢复──
    rebuild_index(&mut db).unwrap();
    rebuild_daily(&mut db).unwrap();
    rebuild_refs(&mut db).unwrap();

    assert!(
        !search(&db, "MARKER", 10).unwrap().is_empty(),
        "FTS 重建后必须能搜到"
    );
    assert!(
        !search(&db, "JAVA_HOME", 10).unwrap().is_empty(),
        "FTS 重建后 system 条目也要在"
    );
    assert_eq!(
        dashboard(&db, 400).unwrap().days.len(),
        before_days,
        "daily_stats 重建后天数与清空前一致"
    );
    let after_edges: i64 = db
        .query_row_with("SELECT count(*) FROM var_references", [], |r| r.get(0))
        .unwrap();
    assert_eq!(after_edges, before_edges, "引用边数必须恢复");

    // ── 重建是幂等的：再跑一次，行数不变 ──
    let fts_again = rebuild_index(&mut db).unwrap();
    assert!(fts_again >= 2);
    let hits = search(&db, "MARKER", 10).unwrap();
    assert_eq!(
        hits.len(),
        1,
        "重复重建不得产生重复命中，实际 {} 条",
        hits.len()
    );
}
