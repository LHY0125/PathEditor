//! 导入：两个独立库之间搬数据，幂等，且篡改的包整体拒绝。
//!
//! 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::sync::export::export_bundle;
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join(name), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    db
}

fn make_snapshot(db: &mut Db, paths: &[&str]) {
    insert_snapshot(
        db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: paths
                .iter()
                .map(|p| PathEntry {
                    path: (*p).to_string(),
                    enabled: true,
                })
                .collect(),
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();
}

#[test]
fn import_is_idempotent_transfers_entities_and_rejects_tampering() {
    let dir = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();

    let mut a = open_db(&src, "a.db");
    let mut b = open_db(&dst, "b.db");

    // ── A 端：建两个快照并导出 ──
    make_snapshot(&mut a, &["C:\\Tools", "C:\\Bin"]);
    make_snapshot(&mut a, &["C:\\Only"]);
    let bundle_path = dir.path().join("a.pesync");
    let report = export_bundle(&mut a, "pw", &bundle_path).unwrap();
    assert_eq!(report.change_count, 2, "两个快照 → 两条变更");

    // ── B 端：导入 → 实体必须真的落库（不是只记变更） ──
    let imported = import_bundle(&mut b, "pw", &bundle_path).unwrap();
    assert_eq!(imported.applied, 2, "两条都应应用");
    assert_eq!(imported.skipped, 0);
    assert_eq!(imported.conflicts, 0);

    let snap_count: i64 = b
        .query_row_with("SELECT count(*) FROM snapshots", [], |r| r.get(0))
        .unwrap();
    assert_eq!(snap_count, 2, "快照必须真的写进 B 的库，不能只记变更");

    let entry_count: i64 = b
        .query_row_with("SELECT count(*) FROM path_entries", [], |r| r.get(0))
        .unwrap();
    assert_eq!(entry_count, 3, "PATH 条目必须跟着快照一起导入（2 + 1）");

    // 内容正确性：C:\Tools 与 C:\Only 都要在
    let tools: i64 = b
        .query_row_with(
            "SELECT count(*) FROM path_entries WHERE path = 'C:\\Tools'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tools, 1, "条目内容必须逐字还原");

    // ── 幂等：再导入同一包 → 全部跳过，库不变 ──
    let again = import_bundle(&mut b, "pw", &bundle_path).unwrap();
    assert_eq!(again.applied, 0, "重复导入不得再次应用");
    assert_eq!(again.skipped, 2, "重复导入必须全部跳过");
    let snap_count2: i64 = b
        .query_row_with("SELECT count(*) FROM snapshots", [], |r| r.get(0))
        .unwrap();
    assert_eq!(snap_count2, 2, "重复导入不得产生重复实体");

    // ── 篡改的包必须整体拒绝，库内无任何变化 ──
    let mut tampered = std::fs::read(&bundle_path).unwrap();
    let n = tampered.len();
    tampered[n - 20] ^= 0xFF;
    let bad_path = dir.path().join("bad.pesync");
    std::fs::write(&bad_path, &tampered).unwrap();

    let before = b
        .query_row_with("SELECT count(*) FROM changes", [], |r| r.get::<_, i64>(0))
        .unwrap();
    let err = import_bundle(&mut b, "pw", &bad_path).unwrap_err();
    assert_eq!(err.code, path_editor_core::ErrorCode::Crypto);
    let after = b
        .query_row_with("SELECT count(*) FROM changes", [], |r| r.get::<_, i64>(0))
        .unwrap();
    assert_eq!(before, after, "篡改包不得产生任何写入");

    // ── 错误口令同样拒绝 ──
    let err2 = import_bundle(&mut b, "wrong", &bundle_path).unwrap_err();
    assert_eq!(err2.code, path_editor_core::ErrorCode::Crypto);
}
