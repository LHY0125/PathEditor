// 一文件一测试（SQLCipher）。
//
// 水位未变时跳过派生表重建；事件/快照增删后必须重新重建。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::derived::rebuild_if_stale;
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::Db;
use path_editor_core::domain::history::EventSource;
use tempfile::TempDir;

#[test]
fn rebuild_is_skipped_until_event_or_snapshot_watermark_changes() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("d.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    let watermark_path = dir.path().join("derived.watermark");

    // 首次：无水位文件，必须重建并写水位。
    assert!(
        rebuild_if_stale(&mut db, &watermark_path)
            .unwrap()
            .is_some(),
        "首次启动必须重建"
    );
    // 水位未变：跳过。
    assert!(
        rebuild_if_stale(&mut db, &watermark_path)
            .unwrap()
            .is_none(),
        "水位未变必须跳过"
    );

    // 新增快照：快照计数/最大 id 变化 → 重建。
    let uid = insert_snapshot(
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
    assert!(
        rebuild_if_stale(&mut db, &watermark_path)
            .unwrap()
            .is_some(),
        "新增快照后必须重建"
    );
    assert!(
        rebuild_if_stale(&mut db, &watermark_path)
            .unwrap()
            .is_none(),
        "重建后水位必须更新"
    );

    // 删除快照：计数变化（即使最大 id 不变）→ 重建。
    db.with_transaction(|tx| {
        tx.execute("DELETE FROM snapshots WHERE uid = ?1", [&uid])
            .map_err(|e| {
                path_editor_core::CoreError::new(
                    path_editor_core::ErrorCode::Database,
                    "delete_snapshot",
                    e.to_string(),
                )
            })?;
        Ok(())
    })
    .unwrap();
    assert!(
        rebuild_if_stale(&mut db, &watermark_path)
            .unwrap()
            .is_some(),
        "删除快照后必须重建"
    );

    // 水位文件损坏：回落到一次重建，而不是永久跳过。
    std::fs::write(&watermark_path, "broken").unwrap();
    assert!(
        rebuild_if_stale(&mut db, &watermark_path)
            .unwrap()
            .is_some(),
        "水位损坏必须触发重建"
    );
}
