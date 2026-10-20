// M0 探针：FTS5 与 JSON1 可用（一文件一测试，见 p1_open.rs 说明）。
use tempfile::TempDir;

#[test]
fn fts5_and_json1_are_available() {
    let dir = TempDir::new().unwrap();
    let conn = v6_sqlcipher_probe::open_encrypted(&dir.path().join("fts.db"), &"33".repeat(32))
        .unwrap();

    let fts: i64 = conn
        .query_row(
            "SELECT sqlite_compileoption_used('ENABLE_FTS5')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(fts, 1, "bundled SQLCipher 必须启用 FTS5");

    conn.execute_batch(
        "CREATE VIRTUAL TABLE ft USING fts5(content);
         INSERT INTO ft(content) VALUES ('hello path editor');",
    )
    .unwrap();
    let hits: i64 = conn
        .query_row("SELECT count(*) FROM ft WHERE ft MATCH 'path'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(hits, 1, "FTS5 查询应命中一条");

    // SQLite >= 3.38 内建 JSON，ENABLE_JSON1 编译选项可能返回 0，故用 json_valid 验收。
    let json: i64 = conn
        .query_row("SELECT json_valid('{\"a\":1}')", [], |row| row.get(0))
        .unwrap();
    assert_eq!(json, 1, "JSON1 函数必须可用");
}
