// 一文件一测试（SQLCipher）。
// `index_snapshot_tx` 此前只有实现没有测试。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::search::{index_snapshot_tx, rebuild_index, search};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn incremental_index_is_idempotent_and_matches_full_rebuild() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("i.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![PathEntry {
                path: "C:\\INCREMENTAL_MARK".into(),
                enabled: true,
            }],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();
    let sid: i64 = db
        .query_row_with("SELECT id FROM snapshots WHERE uid = ?1", [&uid], |r| {
            r.get(0)
        })
        .unwrap();

    // 增量索引两次：必须幂等（不产生重复行）
    db.with_transaction(|tx| index_snapshot_tx(tx, sid))
        .unwrap();
    db.with_transaction(|tx| index_snapshot_tx(tx, sid))
        .unwrap();

    let hits = search(&db, "INCREMENTAL_MARK", 10).unwrap();
    assert_eq!(hits.len(), 1, "重复索引必须幂等，实际 {} 条", hits.len());
    assert_eq!(hits[0].kind, "path");

    // 增量结果必须与全量重建一致（同一条快照，两种路径产出相同行数）
    rebuild_index(&mut db).unwrap();
    let after_full = search(&db, "INCREMENTAL_MARK", 10).unwrap();
    assert_eq!(
        after_full.len(),
        1,
        "全量重建后也必须是 1 条，实际 {} 条",
        after_full.len()
    );
}
