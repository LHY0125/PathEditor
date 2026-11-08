// 数据库是普通 SQLite：验证「建库/迁移/重开」与「拒绝旧版加密库」。
use path_editor_core::db::{schema::CURRENT_SCHEMA_VERSION, Db};
use std::io::Write as _;
use tempfile::TempDir;

#[test]
fn db_opens_as_plain_sqlite_and_rejects_legacy_encrypted_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.db");

    let mut db = Db::open(&path).expect("打开失败");
    assert_eq!(db.schema_version().unwrap(), 0, "新库版本应为 0");

    db.migrate_to_current().expect("迁移失败");
    assert_eq!(
        db.schema_version().unwrap(),
        CURRENT_SCHEMA_VERSION,
        "迁移后版本应为当前版本"
    );

    // 重复迁移必须幂等
    db.migrate_to_current().expect("二次迁移失败");
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
        "migration_imports",
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
