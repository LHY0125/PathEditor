//! 的回归守卫：写操作必须真的产生 `changes` 行。
//!
//! 首版计划里 `record_change_tx` 只有测试调用者，这个测试守的是「生产路径也在调」。
//! 注意本测试**不手写 `record_change_tx`**——它调 core 的写函数，然后断言
//! `changes` 表自己多出了行。
//!
//! 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::profiles;
use path_editor_core::db::sync::change_log::pending_export;
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

fn entry(path: &str) -> PathEntry {
    PathEntry {
        path: path.into(),
        enabled: true,
    }
}

#[test]
fn write_paths_populate_change_log_without_manual_recording() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("w.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    // 前置：空库没有任何待导出变更
    assert!(pending_export(&db).unwrap().is_empty());

    // ① 存一个配置 → 必须产生变更
    profiles::save_profile(
        &mut db,
        "Work",
        None,
        &[],
        NewSnapshot {
            kind: SnapshotKind::Profile,
            source: EventSource::Gui,
            system: vec![entry("C:\\X")],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();
    let after_profile = pending_export(&db).unwrap();
    assert!(!after_profile.is_empty(), "保存配置必须产生 changes 行");
    assert!(after_profile.iter().any(|c| c.entity_type == "profile"));
    assert!(
        after_profile.iter().any(|c| c.entity_type == "snapshot"),
        "配置保存内含快照，也必须记"
    );

    // ② 单独插入快照 → 必须再产生变更
    let before = pending_export(&db).unwrap().len();
    insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![entry("C:\\Y")],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();
    let after_snap = pending_export(&db).unwrap().len();
    assert!(after_snap > before, "插入快照必须产生 changes 行");
    assert!(
        pending_export(&db)
            .unwrap()
            .iter()
            .any(|c| c.entity_type == "snapshot"),
        "快照变更的 entity_type 必须是 snapshot"
    );

    // ③ 迁移来源的快照**不得**产生变更（否则首次同步会把陈年数据全推出去）
    let before_migration = pending_export(&db).unwrap().len();
    insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Backup,
            source: EventSource::Migration,
            system: vec![entry("C:\\Old")],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();
    assert_eq!(
        pending_export(&db).unwrap().len(),
        before_migration,
        "v5 迁移产生的快照不得进入待导出队列"
    );
}
