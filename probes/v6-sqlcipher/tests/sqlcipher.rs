// SQLCipher 在 Windows GNU 构建下需要较大栈：FTS5/虚拟表操作在 libtest 默认
// 线程栈上会 STATUS_STACK_OVERFLOW。实测 `--test-threads=1` **不能**避免溢出，
// 唯一可行方式是每个测试单独进程：`cargo test --exact <name>`。
// 见 M0 结果文档 §五。
use tempfile::TempDir;

#[test]
fn sqlcipher_builds_and_opens() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("probe.db");
    let key = "01".repeat(32); // 32 字节 = 64 hex
    let conn = v6_sqlcipher_probe::open_encrypted(&path, &key).expect("SQLCipher 打开失败");

    let version: String = conn
        .query_row("PRAGMA cipher_version", [], |row| row.get(0))
        .expect("cipher_version 查询失败");
    assert!(!version.is_empty(), "cipher_version 不能为空");

    conn.execute(
        "CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT NOT NULL)",
        [],
    )
    .expect("建表失败");
    conn.execute("INSERT INTO t (v) VALUES ('ok')", [])
        .expect("插入失败");
    let value: String = conn
        .query_row("SELECT v FROM t WHERE id = 1", [], |row| row.get(0))
        .expect("查询失败");
    assert_eq!(value, "ok");
}

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

#[test]
fn fts5_and_json1_are_available() {
    let dir = TempDir::new().unwrap();
    let conn =
        v6_sqlcipher_probe::open_encrypted(&dir.path().join("fts.db"), &"33".repeat(32)).unwrap();

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

    // 诊断：记录产物是否加密与 cipher 参数；不作为 CI 门，也不作为备份路径。
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
