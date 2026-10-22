//! `changes` 变更日志：seq/lamport 单调推进、待导出查询、标记已导出。
//!
//! 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::sync::change_log::{
    current_lamport, mark_exported, pending_export, record_change_tx, NewChange,
};
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn change_log_records_assigns_seq_and_marks_exported() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("cl.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let uid1 = db
        .with_transaction(|tx| {
            record_change_tx(
                tx,
                NewChange {
                    entity_type: "profile".into(),
                    entity_uid: "p1".into(),
                    field: Some("name".into()),
                    old_value: "\"old\"".into(),
                    new_value: "\"new\"".into(),
                },
            )
        })
        .unwrap();
    let _uid2 = db
        .with_transaction(|tx| {
            record_change_tx(
                tx,
                NewChange {
                    entity_type: "profile".into(),
                    entity_uid: "p1".into(),
                    field: Some("tags".into()),
                    old_value: "[]".into(),
                    new_value: "[\"a\",\"b\"]".into(),
                },
            )
        })
        .unwrap();

    // 两条 uid 必须不同（UUIDv7 冲突概率可忽略，但真发出来就说明生成有问题）
    assert_ne!(uid1, _uid2, "变更 uid 必须唯一");

    let pending = pending_export(&db).unwrap();
    assert_eq!(pending.len(), 2, "两条都待导出");
    assert!(pending[0].seq < pending[1].seq, "seq 必须单调递增");
    assert_eq!(pending[0].origin, "local");
    assert!(pending[0].exported_at.is_none(), "新变更必须待导出");

    // lamport 必须与 seq 同步推进（LWW 依赖它）
    assert!(current_lamport(&db).unwrap() >= 2, "lamport 必须随写入推进");

    let marked = mark_exported(&mut db, std::slice::from_ref(&uid1), 999).unwrap();
    assert_eq!(marked, 1);
    assert_eq!(
        pending_export(&db).unwrap().len(),
        1,
        "标记后只剩一条待导出"
    );

    // 重复标记同一条不得改变行数（幂等：只影响 exported_at 非空的行）
    let again = mark_exported(&mut db, &[_uid2.clone(), uid1], 1000).unwrap();
    assert_eq!(again, 1, "已导出的不应被再次计入");
    assert!(pending_export(&db).unwrap().is_empty());
}
