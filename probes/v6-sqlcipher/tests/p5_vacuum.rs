// M0 探针：VACUUM INTO 的加密性与 cipher 参数（诊断项，不是 CI 门）。
// 一文件一测试，见 p1_open.rs 说明。
use tempfile::TempDir;

#[test]
fn vacuum_into_parameters_are_recorded() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("v.db");
    let out = dir.path().join("v-out.db");
    let key = "55".repeat(32);

    let conn = v6_sqlcipher_probe::open_encrypted(&db, &key).unwrap();
    conn.execute_batch("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('x');")
        .unwrap();
    conn.execute_batch(&format!("VACUUM INTO '{}'", out.display()))
        .unwrap();

    let header = std::fs::read(&out).unwrap();
    let encrypted = !header.starts_with(b"SQLite format 3\0");
    let page_size: i64 = conn
        .query_row("PRAGMA cipher_page_size", [], |row| row.get(0))
        .unwrap_or(0);
    let kdf_iter: i64 = conn
        .query_row("PRAGMA cipher_kdf_iter", [], |row| row.get(0))
        .unwrap_or(0);
    println!(
        "VACUUM INTO diagnostic: encrypted={encrypted} header={:02x?} cipher_page_size={page_size} cipher_kdf_iter={kdf_iter}",
        &header[..16.min(header.len())]
    );

    if let Ok(copy) = v6_sqlcipher_probe::open_encrypted(&out, &key) {
        let n: i64 = copy
            .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))
            .unwrap();
        assert!(n >= 1);
    }
}
