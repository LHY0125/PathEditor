//! 保留策略扩展：快照/事件清理与保护位。
use path_editor_core::db::history::{
    insert_snapshot, set_snapshot_pinned, NewSnapshot, SnapshotKind,
};
use path_editor_core::db::maintenance::{prune, prune_plan};
use path_editor_core::db::Db;
use path_editor_core::domain::history::EventSource;
use path_editor_core::ErrorCode;
use tempfile::TempDir;

const DAY: i64 = 24 * 60 * 60 * 1000;

/// 造一条快照并把 `created_at` 改成指定时间（`insert_snapshot` 只用当前时间）。
fn snapshot_at(db: &mut Db, at_ms: i64) -> String {
    let uid = insert_snapshot(
        db,
        NewSnapshot {
            kind: SnapshotKind::History,
            source: EventSource::Cli,
            system: Vec::new(),
            user: Vec::new(),
            env: Vec::new(),
            parent_uid: None,
        },
    )
    .unwrap();
    db.execute_batch(&format!(
        "UPDATE snapshots SET created_at = {at_ms} WHERE uid = '{uid}'"
    ))
    .unwrap();
    uid
}

fn count(db: &Db, sql: &str) -> i64 {
    db.query_row_with(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn retention_protects_current_pinned_profile_and_backup_references() {
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("config.ini");
    std::fs::write(
        &config,
        "snapshot_keep_days = 365\nsnapshot_keep_max = 500\nevent_keep_days = 30\n",
    )
    .unwrap();
    // 同一测试二进制里只有本测试会读配置；单个测试内设置可避免并发写环境变量。
    std::env::set_var("PATHEDITOR_CONFIG_FILE", &config);

    let mut db = Db::open(&dir.path().join("retention.db")).unwrap();
    db.migrate_to_current().unwrap();
    let now = 1_800_000_000_000i64;
    let old = now - 800 * DAY;

    let expired = snapshot_at(&mut db, old);
    let pinned = snapshot_at(&mut db, old);
    let profile_ref = snapshot_at(&mut db, old);
    let backup_ref = snapshot_at(&mut db, old);
    let current = snapshot_at(&mut db, old);
    let fresh = snapshot_at(&mut db, now);

    set_snapshot_pinned(&mut db, &pinned, true).unwrap();

    // profiles + profile_revisions 引用
    db.execute_batch(&format!(
        "INSERT INTO profiles (uid, name, created_at, updated_at) VALUES ('prof-1', '工作', 1, 1);
         INSERT INTO profile_revisions (uid, profile_id, version, created_at, snapshot_id)
         SELECT 'rev-1', id, 1, 1, (SELECT id FROM snapshots WHERE uid = '{profile_ref}') FROM profiles WHERE uid = 'prof-1';"
    ))
    .unwrap();
    // backups 引用
    db.execute_batch(&format!(
        "INSERT INTO backups (uid, kind, created_at, content_hash, size_bytes, file_path, snapshot_id)
         VALUES ('bak-1', 'env', 1, 'h', 1, 'x', (SELECT id FROM snapshots WHERE uid = '{backup_ref}'));"
    ))
    .unwrap();
    // current 指针指向一条**很旧**的快照：它也必须受保护
    assert!(path_editor_core::db::history::set_current_snapshot(&mut db, &current, 1).unwrap());

    // 事件：一条到期、一条较新且引用将被删除的快照
    db.execute_batch(&format!(
        "INSERT INTO events (uid, created_at, source, action, result, snapshot_id, device_id)
         VALUES ('e-old', {}, 'cli', 'apply', 'ok', NULL, 'dev'),
                ('e-link', {}, 'cli', 'restore', 'ok',
                 (SELECT id FROM snapshots WHERE uid = '{expired}'), 'dev');",
        now - 60 * DAY,
        now - DAY
    ))
    .unwrap();

    let plan = prune_plan(&db, now).unwrap();
    assert_eq!(plan.snapshots_deleted, 1, "只有一条未被保护且到期的快照");
    assert_eq!(plan.events_deleted, 1, "只有过期事件被删");

    let report = prune(&mut db, now).unwrap();
    assert_eq!(report.snapshots_deleted, 1);
    assert_eq!(report.events_deleted, 1);
    assert_eq!(report, plan, "dry-run 与实际删除数必须一致");

    // 保护项全部还在
    for uid in [&pinned, &profile_ref, &backup_ref, &current, &fresh] {
        let n = count(
            &db,
            &format!("SELECT COUNT(*) FROM snapshots WHERE uid = '{uid}'"),
        );
        assert_eq!(n, 1, "受保护快照被误删: {uid}");
    }
    let n = count(
        &db,
        &format!("SELECT COUNT(*) FROM snapshots WHERE uid = '{expired}'"),
    );
    assert_eq!(n, 0, "未受保护的旧快照应被删除");

    // 事件审计记录保留，但与被删快照解链
    let link: Option<i64> = db
        .query_row_with(
            "SELECT snapshot_id FROM events WHERE uid = 'e-link'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(link, None, "被删快照的事件必须在 snapshot_id 上解链");

    // 外键完整性
    let integrity: String = db
        .query_row_with("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");

    // ── 条数上限：只保留最新 2 条未保护快照 ──
    std::fs::write(
        &config,
        "snapshot_keep_days = 3650\nsnapshot_keep_max = 2\nevent_keep_days = 30\n",
    )
    .unwrap();
    let n1 = snapshot_at(&mut db, now + DAY);
    let n2 = snapshot_at(&mut db, now + 2 * DAY);
    let n3 = snapshot_at(&mut db, now + 3 * DAY);
    prune(&mut db, now).unwrap();
    // 保护项不计入上限；最新两条（n3、n2）保留，n1 与更早的 fresh 被清掉
    for uid in [&n3, &n2] {
        assert_eq!(
            count(
                &db,
                &format!("SELECT COUNT(*) FROM snapshots WHERE uid = '{uid}'")
            ),
            1,
            "最新快照应保留: {uid}"
        );
    }
    for uid in [&n1, &fresh] {
        assert_eq!(
            count(
                &db,
                &format!("SELECT COUNT(*) FROM snapshots WHERE uid = '{uid}'")
            ),
            0,
            "超出条数上限的旧快照应被删除: {uid}"
        );
    }
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM snapshots WHERE pinned = 1"),
        1,
        "固定快照不受条数上限影响"
    );

    // 不存在的快照做 pin/note 仍返回 NotFound（写路径口径一致）
    let err =
        path_editor_core::db::history::set_snapshot_pinned(&mut db, "nope", true).unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
}
