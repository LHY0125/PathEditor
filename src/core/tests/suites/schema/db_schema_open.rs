// 数据库是普通 SQLite：验证「建库/校验/重开」与「拒绝旧版加密库」。
use path_editor_core::db::{schema::CURRENT_SCHEMA_VERSION, Db};
use std::io::Write as _;
use tempfile::TempDir;

#[test]
fn db_opens_as_plain_sqlite_and_rejects_legacy_encrypted_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.db");

    let mut db = Db::open(&path).expect("打开失败");
    assert_eq!(db.schema_version().unwrap(), 0, "新库版本应为 0");

    db.ensure_schema().expect("schema 初始化失败");
    assert_eq!(
        db.schema_version().unwrap(),
        CURRENT_SCHEMA_VERSION,
        "建库后版本应为当前版本"
    );

    // 重复校验必须幂等
    db.ensure_schema().expect("二次 schema 校验失败");
    assert_eq!(db.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);

    // 关键表存在
    for table in [
        "snapshots",
        "app_state",
        "path_entries",
        "env_values",
        "events",
        "profiles",
        "profile_revisions",
        "backups",
        "scan_runs",
        "changes",
        "sync_conflicts",
        "rules",
        "rule_hits",
    ] {
        assert!(db.table_exists(table).unwrap(), "缺少表 {table}");
    }
    drop(db);

    // 新库必须是明文 SQLite：文件头为 16 字节 magic
    let head: Vec<u8> = std::fs::read(&path).unwrap()[..16].to_vec();
    assert_eq!(head, b"SQLite format 3\0", "库文件必须是明文 SQLite");

    // 重开同一文件仍可读
    let reopened = Db::open(&path).expect("重开失败");
    assert_eq!(reopened.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    drop(reopened);

    // 旧版 SQLCipher 库：文件头不是 SQLite magic → 必须明确拒绝，
    // 且消息要给出「删除/重命名」的可操作指引（不做任何自动转换）。
    let legacy = dir.path().join("legacy.db");
    let mut f = std::fs::File::create(&legacy).unwrap();
    // 旧版库的真实文件头样本（本机实测取自 ~/.patheditor/patheditor.db）
    f.write_all(&[
        0x6a, 0x40, 0x7f, 0x68, 0x51, 0xc0, 0x29, 0xe4, 0xb2, 0x02, 0x00, 0x6a, 0xb6, 0xd9, 0xb3,
        0xaa,
    ])
    .unwrap();
    f.sync_all().unwrap();
    drop(f);

    let err = Db::open(&legacy)
        .map(|_| ())
        .expect_err("旧版加密库必须被拒绝");
    assert!(
        err.message.contains("旧版加密数据库"),
        "错误消息必须点明旧版加密库，实际: {}",
        err.message
    );
    assert!(
        err.message.contains("patheditor.db"),
        "错误消息必须给出文件名指引，实际: {}",
        err.message
    );

    // 拒绝时不得把旧文件改写成空库（大小不变）
    assert_eq!(
        std::fs::metadata(&legacy).unwrap().len(),
        16,
        "拒绝旧库时不得改动该文件"
    );
}

#[test]
fn old_schema_versions_are_rejected_without_mutation() {
    let dir = TempDir::new().unwrap();
    for version in [4, 5, 6] {
        let path = dir.path().join(format!("old-{version}.db"));
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(&format!("PRAGMA user_version = {version};"))
                .unwrap();
        }
        let mut db = Db::open(&path).unwrap();
        let err = db.ensure_schema().unwrap_err();
        assert_eq!(err.code, path_editor_core::ErrorCode::Migration);
        assert!(
            err.message.contains("不受支持"),
            "错误必须说明旧版本不受支持: {}",
            err.message
        );
        assert_eq!(
            db.schema_version().unwrap(),
            version,
            "拒绝迁移时不得改写版本号"
        );
    }
}

#[test]
fn non_empty_schema_6_is_rejected_without_touching_tables() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("non-empty-v6.db");
    {
        let mut db = Db::open(&path).unwrap();
        db.ensure_schema().unwrap();
        db.execute_batch("PRAGMA user_version = 6;").unwrap();
    }

    let mut db = Db::open(&path).unwrap();
    let err = db.ensure_schema().unwrap_err();
    assert_eq!(err.code, path_editor_core::ErrorCode::Migration);
    assert_eq!(
        db.schema_version().unwrap(),
        6,
        "拒绝旧库时不得改写 user_version"
    );
    // 表结构保持原样：拒绝发生在任何 DDL/表访问之前。
    let n: i64 = db
        .query_row_with("SELECT count(*) FROM snapshots", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
}
