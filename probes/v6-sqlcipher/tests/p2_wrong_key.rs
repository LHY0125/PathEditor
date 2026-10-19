// M0 探针：错误密钥必须被拒绝，且原库仍可用（一文件一测试，见 p1_open.rs 说明）。
use tempfile::TempDir;

#[test]
fn wrong_key_is_rejected_and_original_survives() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("wrong-key.db");
    let key_a = "11".repeat(32);
    let key_b = "22".repeat(32);

    {
        let conn = v6_sqlcipher_probe::open_encrypted(&path, &key_a).unwrap();
        conn.execute("CREATE TABLE t (v TEXT)", []).unwrap();
        conn.execute("INSERT INTO t VALUES ('keep')", []).unwrap();
    }

    // 只设 key，不应用会触碰页面的 PRAGMA；错误密钥在查询时被 HMAC 拒绝。
    let wrong = v6_sqlcipher_probe::open_with_key(&path, &key_b).unwrap();
    let result = wrong.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    });
    assert!(result.is_err(), "错误密钥必须无法查询");
    drop(wrong);

    // 错误尝试后，原库仍可用正确密钥打开并读到数据。
    let ok = v6_sqlcipher_probe::open_encrypted(&path, &key_a).unwrap();
    let value: String = ok
        .query_row("SELECT v FROM t", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, "keep");
}
