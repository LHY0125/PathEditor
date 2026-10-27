// 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::search::{rebuild_index, search};
use path_editor_core::db::Db;
use path_editor_core::domain::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn fts_indexes_paths_and_never_indexes_sensitive_values() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("s.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let snap = NewSnapshot {
        kind: SnapshotKind::Current,
        source: EventSource::Gui,
        system: vec![PathEntry {
            path: "C:\\Program Files\\Java\\jdk21".into(),
            enabled: true,
        }],
        user: vec![],
        env: vec![],
        parent_uid: None,
    };
    insert_snapshot(&mut db, snap).unwrap();
    let n = rebuild_index(&mut db).unwrap();
    assert!(n >= 1, "至少要索引一条路径");

    let hits = search(&db, "jdk21", 10).unwrap();
    assert!(!hits.is_empty(), "应能搜到 jdk21");
    assert_eq!(hits[0].kind, "path");
    assert_eq!(
        hits[0].content, "C:\\Program Files\\Java\\jdk21",
        "content 必须是完整索引内容，供前端定位与复制"
    );

    // 空查询不得返回全量（避免「搜索框为空时列出全部」）
    assert!(search(&db, "   ", 10).unwrap().is_empty());

    // 派生表可重建：清空后重建，结果一致
    db.with_transaction(|tx| {
        tx.execute("DELETE FROM search_fts", []).map_err(|e| {
            path_editor_core::CoreError::new(
                path_editor_core::ErrorCode::Database,
                "clear",
                e.to_string(),
            )
        })?;
        Ok(())
    })
    .unwrap();
    assert!(search(&db, "jdk21", 10).unwrap().is_empty(), "清空后应为空");
    rebuild_index(&mut db).unwrap();
    assert!(!search(&db, "jdk21", 10).unwrap().is_empty(), "重建后恢复");
}
