// 一文件一测试（SQLCipher）。
//
// `index_snapshot_tx` 必须真的被写链路调用。
//
// 背景：首版只做了「启动时若 search_fts 为空则全量重建」，
// 没有在每次写入快照后增量索引。`index_snapshot_tx`
// 当时有实现、有单测，但**零生产调用者**——「函数存在但没人调」的缺陷形态。
//
// 本测试守的是那个语义：**新快照插入后（模拟写链路的两个动作：
// insert_snapshot + index_snapshot_tx），搜索必须能立刻找到它**，
// 不需要等到下次启动的全量重建。
//
// 注意：本测试直接调 core 的两个函数，而不是调 gui 的
// `with_history_and_snapshot`（那需要 Tauri State，core 集成测试拿不到）。
// 因此它守的是「这对调用放在一起能工作」，**不是**「gui 真的把它们放在一起了」——
// 后者的证据是 `gui/src/commands/service.rs` 里的两处 `index_snapshot_by_uid` 调用。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::search::{index_snapshot_tx, rebuild_index, search};
use path_editor_core::db::Db;
use path_editor_core::domain::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn newly_written_snapshot_is_searchable_without_waiting_for_restart() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("inc.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    // ── 基线：先有一条快照并被全量索引（模拟「启动时的首次构建」）──
    insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![PathEntry {
                path: "C:\\OLD_MARKER".into(),
                enabled: true,
            }],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();
    rebuild_index(&mut db).unwrap();
    assert!(!search(&db, "OLD_MARKER", 10).unwrap().is_empty());

    // ── 写链路：插入新快照 + 增量索引（与 gui 侧的两个动作一一对应）──
    let uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::History,
            source: EventSource::Gui,
            system: vec![PathEntry {
                path: "C:\\NEW_MARKER\\bin".into(),
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
    db.with_transaction(|tx| index_snapshot_tx(tx, sid))
        .unwrap();

    // ── 核心断言：不等重启，新内容立刻可搜 ──
    assert!(
        !search(&db, "NEW_MARKER", 10).unwrap().is_empty(),
        "新写入的快照必须立刻可搜——否则用户改完 PATH 去搜索会找不到刚保存的内容"
    );

    // 旧内容仍在（增量不是全量替换）
    assert!(
        !search(&db, "OLD_MARKER", 10).unwrap().is_empty(),
        "增量索引不得清掉其它快照的行"
    );
}
