// M0 探针：SQLCipher 可构建、可加密打开。
//
// **为什么一个文件只放一个测试**：SQLCipher 在 Windows 上对栈/虚拟地址空间的需求
// 使同一进程内第二个 SQLCipher 测试会 STATUS_STACK_OVERFLOW（GNU 与 MSVC 均复现，
// `--test-threads=1` 也不能避免）。每个测试独立进程（独立文件）实测 18/18 稳定通过。
// 见 M0 结果文档 §五。M1 的 DB 测试必须沿用此约定。
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
