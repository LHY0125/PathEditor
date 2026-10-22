// 一文件一测试（SQLCipher）。
//
// **v1 → v2 升级路径的回归守卫。**
//
// 为什么必须有这个测试：把 `PRAGMA user_version` 写在 `if current < 1` **内部**。
// 照抄到后，v1 库升级到 v2 会执行 `DDL_V2` 但**不更新版本号**，
// 下次启动 `current` 仍是 1 → 再次执行 `DDL_V2` → `table scan_runs already exists`。
//
// 而 `db_schema_v2.rs` 只测了「新库直接建到 v2」，走的是 `current == 0 < 1` 分支，
// 两条路径不同——这正是「看起来对、语义错」的典型。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::{schema, Db};
use rusqlite::Connection;
use tempfile::TempDir;

#[test]
fn upgrading_a_v1_database_bumps_version_and_does_not_rerun_ddl_v2() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("upgrade.db");
    let key = load_or_create_key(dir.path()).unwrap();
    let key_hex = key_hex(&key);

    // ── 造一个真正的 v1 库：手动执行 DDL_V1 + 种子 + 写 user_version = 1 ──
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";"))
            .unwrap();
        conn.execute_batch(schema::DDL_V1).unwrap();
        conn.execute(
            schema::SEED_APP_STATE_SQL,
            rusqlite::params![uuid::Uuid::now_v7().to_string()],
        )
        .unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
    }

    // ── 用当前程序打开并迁移 ──
    let mut db = Db::open_encrypted(&path, &key_hex).unwrap();
    assert_eq!(db.schema_version().unwrap(), 1, "前置：确实是一个 v1 库");

    db.migrate_to_current().unwrap();
    assert_eq!(
        db.schema_version().unwrap(),
        schema::CURRENT_SCHEMA_VERSION,
        "升级后版本号必须落盘——写在 if 内部会漏掉这一步"
    );

    // 派生表确实建出来了
    for t in ["scan_runs", "var_references", "daily_stats", "search_fts"] {
        assert!(db.table_exists(t).unwrap(), "升级路径缺表: {t}");
    }

    // ── 关键：再迁移一次不得报错（若版本号没落盘，这里会 table already exists）──
    db.migrate_to_current().expect("二次迁移必须幂等");

    // ── v1 的既有数据必须还在（升级不能丢数据）──
    let device_id: String = db
        .query_row_with("SELECT device_id FROM app_state WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(!device_id.is_empty(), "升级不得清空 app_state");

    // 关掉再开，版本仍是当前版本（确认真的写进了文件，不是只改了内存）
    //
    // 断言用 `CURRENT_SCHEMA_VERSION` 而非硬编码数字：本测试守的是「持久化」，
    // 不是「等于 2」——每次 schema 递增都不该让它失败。
    drop(db);
    let reopened = Db::open_encrypted(&path, &key_hex).unwrap();
    assert_eq!(
        reopened.schema_version().unwrap(),
        schema::CURRENT_SCHEMA_VERSION,
        "版本号必须持久化"
    );
}
