//! 数据库自检与清理时间元数据。
use path_editor_core::db::health;
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::maintenance::{last_prune_time, record_prune_time};
use path_editor_core::db::schema;
use path_editor_core::domain::history::EventSource;

#[test]
fn health_reports_schema_counts_and_integrity() {
    let (_dir, mut db) = crate::common::new_db("health.db");
    insert_snapshot(
        &mut db,
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

    let report = health::health(&db).unwrap();
    assert!(report.integrity_ok, "integrity={}", report.integrity);
    assert_eq!(report.integrity, "ok");
    assert_eq!(report.schema_version, schema::CURRENT_SCHEMA_VERSION);
    assert_eq!(
        report.expected_schema_version,
        schema::CURRENT_SCHEMA_VERSION
    );
    assert!(report.file_bytes > 0);
    assert_eq!(report.path, db.path().display().to_string());
    let snapshots = report
        .table_rows
        .iter()
        .find(|row| row.name == "snapshots")
        .expect("必须统计 snapshots");
    assert_eq!(snapshots.rows, 1);
    assert!(
        report.table_rows.iter().any(|row| row.name == "events"),
        "必须统计 events"
    );
}

#[test]
fn prune_time_metadata_roundtrips_and_ignores_corruption() {
    let dir = tempfile::TempDir::new().unwrap();
    let meta_dir = dir.path().join("meta");

    assert_eq!(last_prune_time(&meta_dir), None, "文件不存在时返回 None");
    record_prune_time(&meta_dir, 1_700_000_000_000).unwrap();
    assert_eq!(last_prune_time(&meta_dir), Some(1_700_000_000_000));

    // 覆盖写入后读到的必须是最新值
    record_prune_time(&meta_dir, 1_800_000_000_000).unwrap();
    assert_eq!(last_prune_time(&meta_dir), Some(1_800_000_000_000));

    // 损坏内容不影响启动，只当没有记录
    std::fs::write(meta_dir.join("maintenance.json"), "{not json").unwrap();
    assert_eq!(last_prune_time(&meta_dir), None);
}
