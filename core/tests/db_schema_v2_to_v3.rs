//! v2 → v3 升级路径的回归守卫。
//!
//! 首版把 `PRAGMA user_version` 写在 `if current < 1` 内部，
//! 导致 v1→v2 执行了 DDL 但不更新版本号。 必须避免同样的错误。
//!
//! 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::{schema, Db};
use rusqlite::Connection;
use tempfile::TempDir;

#[test]
fn upgrading_a_v2_database_bumps_version_and_preserves_data() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("up.db");
    let key = load_or_create_key(dir.path()).unwrap();
    let kh = key_hex(&key);

    // 造一个真 v2 库：DDL_V1 + 种子 + DDL_V2 + user_version = 2
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(&format!("PRAGMA key = \"x'{kh}'\";"))
            .unwrap();
        conn.execute_batch(schema::DDL_V1).unwrap();
        conn.execute(
            schema::SEED_APP_STATE_SQL,
            rusqlite::params![uuid::Uuid::now_v7().to_string()],
        )
        .unwrap();
        conn.execute_batch(schema::DDL_V2).unwrap();
        conn.execute_batch("PRAGMA user_version = 2;").unwrap();
        // 造一条 v2 时期的数据
        conn.execute(
            "INSERT INTO snapshots (uid, created_at, source, kind, content_hash, updated_at)
             VALUES ('v2-snap', 1, 'gui', 'current', 'h', 1)",
            [],
        )
        .unwrap();
    }

    let mut db = Db::open_encrypted(&path, &kh).unwrap();
    assert_eq!(db.schema_version().unwrap(), 2, "前置：确实是 v2 库");

    db.migrate_to_current().unwrap();
    assert_eq!(db.schema_version().unwrap(), 3, "升级后版本必须落盘");

    for t in ["changes", "sync_conflicts"] {
        assert!(db.table_exists(t).unwrap(), "升级路径缺表: {t}");
    }
    db.migrate_to_current().expect("二次迁移必须幂等");

    // v2 数据必须还在
    let n: i64 = db
        .query_row_with(
            "SELECT count(*) FROM snapshots WHERE uid = 'v2-snap'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1, "升级不得丢数据");
}
