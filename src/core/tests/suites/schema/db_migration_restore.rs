//! 迁移前备份的列举与替换。
use path_editor_core::db::schema;
use path_editor_core::db::{list_migration_backups, restore_migration_backup, Db};
use tempfile::TempDir;

#[test]
fn migration_backups_can_be_listed_and_restored() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("patheditor.db");
    let mut db = Db::open(&db_path).unwrap();
    db.migrate_to_current().unwrap();
    drop(db);

    let backup = dir.path().join("patheditor.db.pre-v4.bak");
    std::fs::copy(&db_path, &backup).unwrap();

    let list = list_migration_backups(&db_path).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].version, 4);

    // 未确认时拒绝，主文件不变
    assert!(restore_migration_backup(&db_path, &backup, false).is_err());

    // 确认后替换，并留下 pre-restore 保底副本
    restore_migration_backup(&db_path, &backup, true).unwrap();
    assert!(dir.path().join("patheditor.db.pre-restore.bak").is_file());
    let reopened = Db::open(&db_path).unwrap();
    assert_eq!(
        reopened.schema_version().unwrap(),
        schema::CURRENT_SCHEMA_VERSION
    );

    // 非 SQLite 文件拒绝
    let fake = dir.path().join("patheditor.db.pre-v9.bak");
    std::fs::write(&fake, b"not sqlite").unwrap();
    assert!(restore_migration_backup(&db_path, &fake, true).is_err());
}
