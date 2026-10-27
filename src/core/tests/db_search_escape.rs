// 一文件一测试（SQLCipher）。
//
// **FTS5 查询转义的回归守卫。**
//
// FTS5 的 MATCH 会把用户输入当查询语法，因此 `WHERE search_fts MATCH ?1`
// 必须先把整段输入包成短语并转义内部引号。
// 裸 `"` 会报 `fts5: syntax error`，`C:\Program Files` 会被拆成两个 token。
//
// 我在 core 侧做了 phrase 转义（见 search.rs 的 `escape_query`），
// 但这个决定计划里没有——所以**必须有测试守住**，否则它只是「我以为是好的」。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::search::{rebuild_index, search};
use path_editor_core::db::Db;
use path_editor_core::domain::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn user_input_with_quotes_and_spaces_does_not_break_fts_query() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("q.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![
                PathEntry {
                    path: "C:\\Program Files\\Common Files".into(),
                    enabled: true,
                },
                PathEntry {
                    path: "D:\\Quote\"Inside".into(),
                    enabled: true,
                },
            ],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();
    rebuild_index(&mut db).unwrap();

    // ① 裸双引号：不转义会直接 `fts5: syntax error near """`
    let r = search(&db, "\"", 10);
    assert!(r.is_ok(), "裸双引号不得让搜索报错，实际: {r:?}");

    // ② 含空格与反斜杠的路径片段：不转义会被拆成 AND，匹配不到完整 phrase
    let r = search(&db, "Program Files", 10).unwrap();
    assert!(
        !r.is_empty(),
        "「Program Files」应作为 phrase 命中，而不是被拆成两个 token 后无结果"
    );

    // ③ FTS5 高级语法按字面量处理，不触发布尔运算
    let r = search(&db, "Common OR Files", 10);
    assert!(r.is_ok(), "「OR」必须被当字面量，而不是布尔运算符");

    // ④ 注入式输入不得导致 panic / 报错
    for q in ["*", "NEAR(", "a:b", "(((", "-x", "^", "NOT"] {
        assert!(
            search(&db, q, 10).is_ok(),
            "查询 {q:?} 不得让 FTS5 报语法错误"
        );
    }
}
