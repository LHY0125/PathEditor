//! 迁移前备份：每个旧版本留一份 `.pre-v{n}.bak`，最多保留 3 份。
use path_editor_core::db::{schema, Db};
use rusqlite::Connection;
use tempfile::TempDir;

fn build_db(path: &std::path::Path, version: u32) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(schema::DDL_V1).unwrap();
    conn.execute(
        schema::SEED_APP_STATE_SQL,
        rusqlite::params![uuid::Uuid::now_v7().to_string()],
    )
    .unwrap();
    if version >= 2 {
        conn.execute_batch(schema::DDL_V2).unwrap();
    }
    if version >= 3 {
        conn.execute_batch(schema::DDL_V3).unwrap();
    }
    if version >= 4 {
        conn.execute_batch(schema::DDL_V4).unwrap();
    }
    conn.execute_batch(&format!("PRAGMA user_version = {version};"))
        .unwrap();
}

#[test]
fn migration_backs_up_old_schema_and_rotates_to_three() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("patheditor.db");

    for version in 1..=4u32 {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
        build_db(&path, version);

        let mut db = Db::open(&path).unwrap();
        db.migrate_to_current().unwrap();
        assert_eq!(db.schema_version().unwrap(), schema::CURRENT_SCHEMA_VERSION);

        let bak = path.with_file_name(format!("patheditor.db.pre-v{version}.bak"));
        assert!(bak.is_file(), "缺少 v{version} 的迁移前备份");
        // 备份必须是**迁移前**的版本，且能独立打开
        let conn = Connection::open(&bak).unwrap();
        let v: u32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, version, "备份的 user_version 必须是迁移前版本");
    }

    let backups = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("patheditor.db.pre-v") && name.ends_with(".bak")
        })
        .count();
    assert_eq!(backups, 3, "迁移前备份最多保留 3 份");
}

#[test]
fn fresh_database_does_not_create_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let mut db = Db::open(&path).unwrap();
    db.migrate_to_current().unwrap();
    let backups = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains(".pre-v"))
        .count();
    assert_eq!(backups, 0, "全新库没有可备份的数据");
}
