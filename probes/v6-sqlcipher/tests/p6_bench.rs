// M0 探针：cipher_memory_security ON/OFF 的性能诊断（不设时间断言）。
// 一文件一测试，见 p1_open.rs 说明。
use tempfile::TempDir;

#[test]
fn cipher_memory_security_benchmark_is_recorded() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bench.db");
    let key = "77".repeat(32);
    {
        let conn = v6_sqlcipher_probe::open_encrypted(&path, &key).unwrap();
        conn.execute_batch("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('x');")
            .unwrap();
    }
    for (label, on) in [("ON", true), ("OFF", false)] {
        let conn = v6_sqlcipher_probe::open_encrypted_with(&path, &key, on).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _: String = conn
                .query_row("SELECT v FROM t", [], |row| row.get(0))
                .unwrap();
        }
        println!(
            "cipher_memory_security={label}: 100 queries in {:?}",
            start.elapsed()
        );
    }
    // 诊断，不设时间断言（CI 噪声）。
}
