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
use path_editor_core::db::sync::bundle::{seal, BundleHeader};
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::Db;
use path_editor_core::ErrorCode;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let mut db = Db::open(&dir.path().join(name)).unwrap();
    db.ensure_schema().unwrap();
    db
}

/// 手工封一个包——绕过 export，才能构造「`content = null` 的已知类型」与「未知类型」的 payload。
fn write_bundle(path: &std::path::Path, changes_json: &str, count: u32) {
    write_bundle_with(path, changes_json, count, 7, "dev-x", 5);
}

/// 指定 header 字段封包；用于构造版本/设备/时钟不匹配的畸形包。
fn write_bundle_with(
    path: &std::path::Path,
    changes_json: &str,
    count: u32,
    schema_version: u32,
    device_id: &str,
    base_lamport: i64,
) {
    let header = BundleHeader {
        schema_version,
        device_id: device_id.to_string(),
        exported_at: 0,
        base_lamport,
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
        "entityType": "profile", "entityUid": "p-1", "field": "name",
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

/// 最小快照变更：用于构造 header/时钟边界测试。
fn snapshot_change(uid: &str, device: &str, seq: i64, lamport: i64) -> String {
    format!(
        r#"{{ "uid": "{uid}", "deviceId": "{device}", "seq": {seq}, "lamport": {lamport},
             "entityType": "snapshot", "entityUid": "snap-{uid}", "field": null,
             "oldValue": null, "newValue": "{{}}", "createdAt": 0,
             "content": {{ "uid": "snap-{uid}", "createdAt": 1, "kind": "current",
                          "system": [], "user": [], "env": [] }} }}"#
    )
}

#[test]
fn header_schema_version_must_match_current() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "header.db");
    for version in [4, 5, 6, 8] {
        let path = dir.path().join(format!("header-{version}.pesync"));
        let change = snapshot_change("s1", "dev-x", 1, 1);
        write_bundle_with(&path, &format!("[{change}]"), 1, version, "dev-x", 10);
        let err = import_bundle(&mut db, "pw", &path).unwrap_err();
        assert_eq!(err.code, ErrorCode::SyncFormat);
        assert!(
            err.message.contains("结构版本"),
            "版本不符必须点明原因，实际: {}",
            err.message
        );
    }
}

#[test]
fn lamport_must_be_non_negative_and_not_above_base() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "lamport.db");
    for (name, lamport) in [("negative", -1), ("above-base", 11)] {
        let path = dir.path().join(format!("lamport-{name}.pesync"));
        let change = snapshot_change("s1", "dev-x", 1, lamport);
        write_bundle_with(&path, &format!("[{change}]"), 1, 7, "dev-x", 10);
        let err = import_bundle(&mut db, "pw", &path).unwrap_err();
        assert_eq!(err.code, ErrorCode::SyncFormat, "{name}");
        assert!(err.message.contains("lamport"), "{name}: {}", err.message);
    }
}

#[test]
fn seq_must_be_within_safe_range() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "seq.db");
    for (name, seq) in [("negative", -1), ("max", i64::MAX), ("max-1", i64::MAX - 1)] {
        let path = dir.path().join(format!("seq-{name}.pesync"));
        let change = snapshot_change("s1", "dev-x", seq, 1);
        write_bundle_with(&path, &format!("[{change}]"), 1, 7, "dev-x", 10);
        let err = import_bundle(&mut db, "pw", &path).unwrap_err();
        assert_eq!(err.code, ErrorCode::SyncFormat, "{name}");
        assert!(err.message.contains("seq"), "{name}: {}", err.message);
    }
}

#[test]
fn change_device_must_match_header_and_not_be_local() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "device.db");

    // ① change 的 deviceId 与 header 不一致
    let mismatch = dir.path().join("device-mismatch.pesync");
    let change = snapshot_change("s1", "dev-y", 1, 1);
    write_bundle_with(&mismatch, &format!("[{change}]"), 1, 7, "dev-x", 10);
    let err = import_bundle(&mut db, "pw", &mismatch).unwrap_err();
    assert_eq!(err.code, ErrorCode::SyncFormat);
    assert!(err.message.contains("deviceId") || err.message.contains("设备"));

    // ② header 与 change 都伪造成接收方的本地 device_id
    let local: String = db
        .query_row_with("SELECT device_id FROM app_state WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    let local_path = dir.path().join("device-local.pesync");
    let change = snapshot_change("s1", &local, 1, 1);
    write_bundle_with(&local_path, &format!("[{change}]"), 1, 7, &local, 10);
    let err = import_bundle(&mut db, "pw", &local_path).unwrap_err();
    assert_eq!(err.code, ErrorCode::SyncFormat);
    assert!(
        err.message.contains("本机设备"),
        "必须拒绝伪造成本机的包: {}",
        err.message
    );
}

#[test]
fn profile_tombstone_must_not_carry_content() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "tomb-content.db");
    let change = r#"{ "uid": "c-tomb", "deviceId": "dev-x", "seq": 1, "lamport": 1,
             "entityType": "profile", "entityUid": "p-1", "field": "deleted_at",
             "oldValue": "null", "newValue": "1", "createdAt": 0,
             "content": { "uid": "p-1", "name": "Work", "description": null,
                          "tags": [], "createdAt": 1, "updatedAt": 2,
                          "version": 1, "snapshotUid": null } }"#;
    let path = dir.path().join("tomb-content.pesync");
    write_bundle_with(&path, &format!("[{change}]"), 1, 7, "dev-x", 10);
    let err = import_bundle(&mut db, "pw", &path).unwrap_err();
    assert_eq!(err.code, ErrorCode::SyncFormat);
    assert!(
        err.message.contains("墓碑"),
        "错误必须点明墓碑不能带 content: {}",
        err.message
    );
}

#[test]
fn local_seq_overflow_is_reported_not_silently_saturated() {
    use path_editor_core::db::profiles::rename_profile;
    use path_editor_core::ErrorCode as Code;

    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "seq-overflow.db");
    db.execute_batch(
        "INSERT INTO profiles (uid, name, description, tags, created_at, updated_at, apply_count, version)
         VALUES ('p-1', 'Work', NULL, '[]', 1, 1, 0, 1);",
    )
    .unwrap();
    let local: String = db
        .query_row_with("SELECT device_id FROM app_state WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    db.execute_batch(&format!(
        "INSERT INTO changes
           (uid, device_id, seq, lamport, origin, entity_type, entity_uid,
            field, old_value, new_value, created_at, applied_at, exported_at)
         VALUES ('poison', '{local}', {}, 0, 'local', 'profile', 'p-1',
                 'name', 'Work', 'Work', 1, 1, NULL);",
        i64::MAX
    ))
    .unwrap();

    let err = rename_profile(&mut db, "Work", "Renamed").unwrap_err();
    assert_eq!(err.code, Code::Database);
    assert!(
        err.message.contains("序号已耗尽"),
        "必须显式报告序号耗尽: {}",
        err.message
    );
}

#[test]
fn header_base_above_half_is_clamped_without_overflow() {
    let dir = TempDir::new().unwrap();
    let mut db = open_db(&dir, "base-clamp.db");
    let change = snapshot_change("s1", "dev-x", 1, 1);
    let path = dir.path().join("base-clamp.pesync");
    write_bundle_with(&path, &format!("[{change}]"), 1, 7, "dev-x", i64::MAX);

    let report = import_bundle(&mut db, "pw", &path).unwrap();
    assert_eq!(
        report.applied, 1,
        "钳制后的 baseLamport 仍应允许低 lamport 变更"
    );
    let lamport: i64 = db
        .query_row_with("SELECT lamport FROM app_state WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        lamport,
        i64::MAX / 2,
        "本地时钟只能抬到钳制后的 baseLamport"
    );
}
