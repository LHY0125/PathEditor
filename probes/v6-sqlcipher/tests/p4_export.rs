// M0 探针：sqlcipher_export 同密钥导出可读（一文件一测试，见 p1_open.rs 说明）。
use tempfile::TempDir;

#[test]
fn sqlcipher_export_roundtrip() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("main.db");
    let backup = dir.path().join("backup.db");
    let key = "44".repeat(32);

    let conn = v6_sqlcipher_probe::open_encrypted(&db, &key).unwrap();
    conn.execute_batch("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('x');")
        .unwrap();
    conn.execute_batch(&format!(
        "ATTACH DATABASE '{}' AS backup KEY \"x'{}'\";
         SELECT sqlcipher_export('backup');
         DETACH DATABASE backup;",
        backup.display(),
        key
    ))
    .unwrap();

    let restored = v6_sqlcipher_probe::open_encrypted(&backup, &key).unwrap();
    let value: String = restored
        .query_row("SELECT v FROM t", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, "x");
}
