//! 保留策略：只删「已导出且超过 90 天」的 changes 与「已解决超过 30 天」的冲突。
//!
//! 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::maintenance::{prune, CHANGES_RETENTION_MS, CONFLICTS_RETENTION_MS};
use path_editor_core::db::Db;
use rusqlite::params;
use tempfile::TempDir;

#[test]
fn prune_deletes_only_expired_and_exported_rows() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("prune.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let now: i64 = 1_800_000_000_000;
    let old = now - CHANGES_RETENTION_MS - 1;
    let recent = now - 1_000;

    db.with_transaction(|tx| {
        let insert = |uid: &str, applied: i64, exported: Option<i64>| {
            tx.execute(
                "INSERT INTO changes
                   (uid, device_id, seq, lamport, origin, entity_type, entity_uid,
                    field, old_value, new_value, created_at, applied_at, exported_at)
                 VALUES (?1, 'd', ?2, ?2, 'remote', 'snapshot', 's-1', NULL, NULL, NULL, ?2, ?2, ?3)",
                params![uid, applied, exported],
            )
            .unwrap();
        };
        // 已导出且到期 → 删
        insert("old-exported", old, Some(old));
        // 未导出（待导出队列）→ 必须保留，删了就永久丢变更
        insert("old-pending", old, None);
        // 已导出但未到期 → 保留
        insert("recent-exported", recent, Some(recent));

        tx.execute(
            "INSERT INTO sync_conflicts
               (entity_type, entity_uid, field, local_value, remote_value, remote_device,
                created_at, resolved_at, resolution)
             VALUES ('snapshot', 's-1', NULL, NULL, NULL, 'd', ?1, ?2, 'local')",
            params![old, Some(now - CONFLICTS_RETENTION_MS - 1)],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO sync_conflicts
               (entity_type, entity_uid, field, local_value, remote_value, remote_device,
                created_at, resolved_at, resolution)
             VALUES ('snapshot', 's-1', NULL, NULL, NULL, 'd', ?1, NULL, NULL)",
            params![old],
        )
        .unwrap();
        Ok::<_, path_editor_core::CoreError>(())
    })
    .unwrap();

    let report = prune(&mut db, now).unwrap();
    assert_eq!(report.changes_deleted, 1);
    assert_eq!(report.conflicts_deleted, 1);

    let remaining: i64 = db
        .query_row_with("SELECT COUNT(*) FROM changes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(remaining, 2, "待导出与未到期记录必须保留");

    let unresolved: i64 = db
        .query_row_with(
            "SELECT COUNT(*) FROM sync_conflicts WHERE resolved_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(unresolved, 1, "未解决的冲突必须保留");
}
