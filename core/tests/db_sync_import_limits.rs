//! 导入的**边界**：不支持的类型如何处理、未知类型是否被拒绝。
//!
//! 对应审查报告的两条：
//! - **R-1**：`APPLICABLE_ENTITY_TYPES` 之外的类型必须**不计数、不记录**，
//!   且单独计入 `skipped_unsupported`——不能报「成功」。
//! - **白名单缺测试**：未知 `entityType` 必须整包拒绝（`SyncFormat`）。
//!
//! 一文件一测试：见 M0 §5.3。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::Db;
use path_editor_core::ErrorCode;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join(name), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    db
}

/// 手工封一个包——绕过 export，才能构造「含不支持类型」与「未知类型」的 payload。
fn write_bundle(path: &std::path::Path, changes_json: &str, count: u32) {
    let header = BundleHeader {
        schema_version: 3,
        device_id: "dev-x".to_string(),
        exported_at: 0,
        base_lamport: 5,
        change_count: count,
    };
    let payload = format!("{{\"changes\":{changes_json}}}");
    let bytes = seal("pw", &header, payload.as_bytes()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn import_reports_unsupported_types_and_rejects_unknown_ones() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "limits.db");

    // ── ① 含 1 条快照 + 1 条配置：快照落库，配置**不计数、不记录** ──
    let mixed = r#"[
      {
        "uid": "c-snap", "deviceId": "dev-x", "seq": 1, "lamport": 1,
        "entityType": "snapshot", "entityUid": "snap-1", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0,
        "content": { "uid": "snap-1", "createdAt": 1, "kind": "current",
                     "system": [{"path": "C:\\A", "enabled": true}], "user": [], "env": [] }
      },
      {
        "uid": "c-prof", "deviceId": "dev-x", "seq": 2, "lamport": 2,
        "entityType": "profile", "entityUid": "p-1", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0, "content": null
      }
    ]"#;
    let mixed_path = dir.path().join("mixed.pesync");
    write_bundle(&mixed_path, mixed, 2);

    let report = import_bundle(&mut db, "pw", &mixed_path).unwrap();
    assert_eq!(report.applied, 1, "只有快照真正落库");
    assert_eq!(
        report.skipped_unsupported, 1,
        "配置变更必须单独计数，不能计入 applied（R-1）"
    );
    assert_eq!(report.skipped, 0);
    assert_eq!(report.conflicts, 0);

    // 快照确实写进去了
    let snaps: i64 = db
        .query_row_with("SELECT count(*) FROM snapshots", [], |r| r.get(0))
        .unwrap();
    assert_eq!(snaps, 1);

    // **配置变更不得留下 changes 行**：留下会占住 uid，
    // 将来支持配置同步时同一个包再也无法重放。
    let prof_changes: i64 = db
        .query_row_with(
            "SELECT count(*) FROM changes WHERE entity_type = 'profile'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(prof_changes, 0, "未落库的类型不得写变更日志");

    // 重复导入同一包：幂等——快照那条 uid 已存在 → skipped=1，
    // 配置那条仍按 unsupported 计（因为压根没记过）。
    let again = import_bundle(&mut db, "pw", &mixed_path).unwrap();
    assert_eq!(again.applied, 0, "重复导入不得再次应用");
    assert_eq!(again.skipped, 1, "已落库的快照走幂等跳过");
    assert_eq!(again.skipped_unsupported, 1);

    // ── ② 未知实体类型：整包拒绝，且不产生任何写入 ──
    let unknown = r#"[
      {
        "uid": "c-bad", "deviceId": "dev-x", "seq": 1, "lamport": 1,
        "entityType": "backups", "entityUid": "b-1", "field": null,
        "oldValue": null, "newValue": "{}", "createdAt": 0, "content": null
      }
    ]"#;
    let bad_path = dir.path().join("bad.pesync");
    write_bundle(&bad_path, unknown, 1);

    let before: i64 = db
        .query_row_with("SELECT count(*) FROM changes", [], |r| r.get(0))
        .unwrap();
    let err = import_bundle(&mut db, "pw", &bad_path).unwrap_err();
    assert_eq!(
        err.code,
        ErrorCode::SyncFormat,
        "未知实体类型必须是 SyncFormat"
    );
    assert!(
        err.message.contains("未知的实体类型"),
        "错误文案必须点明原因，实际: {}",
        err.message
    );
    let after: i64 = db
        .query_row_with("SELECT count(*) FROM changes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after, "整包拒绝时不得产生任何写入");
}
