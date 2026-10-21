//! 冲突记录与解决：同 lamport 不同设备改同一字段 → 记冲突，用户选择后追加新变更。
//!
//! 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::sync::change_log::{
    current_lamport, pending_export, record_change_tx, NewChange,
};
use path_editor_core::db::sync::conflict::{
    list_conflicts, resolve_conflict, unresolved_count, Resolution,
};
use path_editor_core::db::sync::export::export_bundle;
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::Db;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join(name), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    db
}

/// 在同一实体同一字段上写一条变更。
/// 在**支持落库**的实体类型上写一条变更。
///
/// 用 `snapshot` 而不是 `profile`：`profile` 属于「格式合法但当前版本不落库」
/// 的类型，导入时会被 `APPLICABLE_ENTITY_TYPES` 提前跳过，走不到冲突判定。
/// 快照的整实体变更没有 `field`，因此 `field = None`。
fn write_entity(db: &mut Db, entity_uid: &str, value: &str) -> String {
    db.with_transaction(|tx| {
        record_change_tx(
            tx,
            NewChange {
                entity_type: "snapshot".into(),
                entity_uid: entity_uid.into(),
                field: None,
                old_value: String::new(),
                new_value: value.into(),
            },
        )
    })
    .unwrap()
}

#[test]
fn concurrent_same_lamport_edits_record_conflict_and_resolution_appends_newer_change() {
    let dir = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();

    // 两个库必须有不同的 device_id（否则同 lamport 会被判为同源）
    let mut a = open_db(&src, "a.db");
    let mut b = open_db(&dst, "b.db");
    let dev_a: String = a
        .query_row_with("SELECT device_id FROM app_state WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    let dev_b: String = b
        .query_row_with("SELECT device_id FROM app_state WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_ne!(dev_a, dev_b, "前置：两个库的 device_id 必须不同");

    // ── 两边各自在同一个 lamport 上改同一实体 ──
    write_entity(&mut a, "snap-shared", "\"from-A\"");
    write_entity(&mut b, "snap-shared", "\"from-B\"");
    assert_eq!(
        current_lamport(&a).unwrap(),
        current_lamport(&b).unwrap(),
        "前置：两边 lamport 相同（都是 1）"
    );

    let bundle = dir.path().join("a.pesync");
    export_bundle(&mut a, "pw", &bundle).unwrap();

    // ── B 导入 → 必须记一条冲突，且不改实体 ──
    let report = import_bundle(&mut b, "pw", &bundle).unwrap();
    assert_eq!(
        report.conflicts, 1,
        "同 lamport 不同设备改同一实体必须记冲突"
    );
    assert_eq!(report.applied, 0, "冲突项不得应用");
    assert_eq!(unresolved_count(&b).unwrap(), 1);

    let list = list_conflicts(&b, false).unwrap();
    assert_eq!(list.len(), 1);
    let c = &list[0];
    assert_eq!(c.field, None, "整实体变更没有 field");
    assert_eq!(c.entity_uid, "snap-shared");
    assert_eq!(
        c.local_value.as_deref(),
        Some("\"from-B\""),
        "本地值是 B 自己写的"
    );
    assert_eq!(
        c.remote_value.as_deref(),
        Some("\"from-A\""),
        "远端值是 A 写的"
    );
    assert_eq!(c.remote_device, dev_a);
    assert!(c.resolved_at.is_none(), "新冲突必须未解决");

    // ── 解决：采纳远端 → 追加一条 lamport 更高的 change ──
    let lamport_before = current_lamport(&b).unwrap();
    resolve_conflict(&mut b, c.id, Resolution::Remote).unwrap();

    let after = list_conflicts(&b, false).unwrap();
    assert!(after.is_empty(), "解决后不该再出现在未解决列表");
    assert_eq!(
        list_conflicts(&b, true).unwrap().len(),
        1,
        "含已解决时应能看到"
    );

    let resolved = &list_conflicts(&b, true).unwrap()[0];
    assert_eq!(resolved.resolution.as_deref(), Some("remote"));
    assert!(resolved.resolved_at.is_some());

    // 追加的 change：lamport 必须**严格大于**解决前，且值已改为胜方
    assert!(
        current_lamport(&b).unwrap() > lamport_before,
        "解决冲突必须推进 lamport"
    );
    let pending = pending_export(&b).unwrap();
    // 按 lamport 取最高的那条：B 自己早先写的变更也在待导出队列里，
    // 按 seq 取会拿到旧的。追加的变更必然是 lamport 最大的那条。
    let added = pending
        .iter()
        .filter(|x| x.entity_uid == "snap-shared" && x.field.is_none())
        .max_by_key(|x| x.lamport)
        .expect("解决冲突必须追加一条新变更");
    assert!(
        added.lamport > lamport_before,
        "新变更的 lamport 必须高于冲突双方——否则第三台设备会重新判一次"
    );
    assert_eq!(
        added.new_value.as_deref(),
        Some("\"from-A\""),
        "胜方值（远端）必须写进新变更"
    );
    assert_eq!(
        added.old_value.as_deref(),
        Some("\"from-B\""),
        "败方值（本地）必须记在 old_value"
    );

    // ── 重复解决同一条 → 拒绝 ──
    let err = resolve_conflict(&mut b, c.id, Resolution::Local).unwrap_err();
    assert_eq!(err.code, path_editor_core::ErrorCode::Conflict);

    // ── 不存在的冲突 → NotFound ──
    let err2 = resolve_conflict(&mut b, 9999, Resolution::Local).unwrap_err();
    assert_eq!(err2.code, path_editor_core::ErrorCode::NotFound);
}
