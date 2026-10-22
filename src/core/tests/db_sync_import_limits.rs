//! 导入的**边界**：已知类型是否真的落库、未知类型是否被拒绝。
//!
//! 本文件的历史：整改时为加过一条守卫——「`APPLICABLE_ENTITY_TYPES`
//! 之外的类型必须不计数、不记录」，当时 `profile` 正是被跳过的类型。
//! 起已知类型全部可落库：，那条口径已不适用，改为守两件事：
//!
//! - **`content = null` 的已知类型只记变更、不造实体**——它是墓碑的前置形态，
//!   若在这里凭空造实体，墓碑语义就无从区分。
//! - 未知 `entityType` 仍必须整包拒绝（`SyncFormat`）。
//!
//! 一文件一测试（SQLCipher）。
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

/// 手工封一个包——绕过 export，才能构造「`content = null` 的已知类型」与「未知类型」的 payload。
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
fn import_applies_known_types_and_rejects_unknown_ones() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "limits.db");

    // ── ① 含 1 条快照 + 1 条配置：两者都计入 applied，但配置 content 为 null ──
    //
    // `profile` 在 `APPLICABLE_ENTITY_TYPES` 内，所以它**不再**计入
    // `skipped_unsupported`；但这条 change 的 `content` 是 null，因此只记变更、
    // 不写 `profiles` 行——这正是「墓碑之前什么都没有」的形态。
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
    assert_eq!(report.applied, 2, "已知类型全部可落库");
    assert_eq!(
        report.skipped_unsupported, 0,
        "已知类型不得再计入 skipped_unsupported"
    );
    assert_eq!(report.skipped, 0);
    assert_eq!(report.conflicts, 0);

    // 快照确实写进去了
    let snaps: i64 = db
        .query_row_with("SELECT count(*) FROM snapshots", [], |r| r.get(0))
        .unwrap();
    assert_eq!(snaps, 1);

    // 配置的变更**必须**落进 changes：它将参与 LWW 与幂等判定
    let prof_changes: i64 = db
        .query_row_with(
            "SELECT count(*) FROM changes WHERE entity_type = 'profile'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(prof_changes, 1, "已知类型的变更必须记账");

    // 但 `content = null` 的配置**不得**凭空造出实体行
    let prof_rows: i64 = db
        .query_row_with("SELECT count(*) FROM profiles", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        prof_rows, 0,
        "content 为 null 的已知类型只记变更，不写实体（墓碑的前置形态）"
    );

    // 重复导入同一包：幂等——两条 uid 都已存在
    let again = import_bundle(&mut db, "pw", &mixed_path).unwrap();
    assert_eq!(again.applied, 0, "重复导入不得再次应用");
    assert_eq!(again.skipped, 2, "两条都已落库，全部走幂等跳过");
    assert_eq!(again.skipped_unsupported, 0);

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
