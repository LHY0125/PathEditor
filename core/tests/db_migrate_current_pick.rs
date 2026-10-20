// 一文件一测试：见 M0 §5.3。
//
// **D-2 回归守卫**：迁移后 `current_snapshot_id` 必须指向 `kind='current'`，
// 不是「最后插入的那条」。
//
// 缺口补齐轮复审报告 D-2：原实现用 `ORDER BY created_at DESC, id DESC LIMIT 1`
// 取 `current_snapshot_id`，而导入顺序是 disabled → profiles → env_backups →
// path_backups，最后插入的是**备份**快照。实现已改为按 `kind='current'` 取，
// 但**当时没有任何测试**——这正是「看起来对、语义错」的典型。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::current_snapshot;
use path_editor_core::db::migrate_v5::migrate_legacy;
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn migrated_current_snapshot_points_at_disabled_not_backup() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();

    // ① disabled.json —— 这是「当前状态」的来源（kind=current）
    std::fs::write(
        home.join("disabled.json"),
        r#"{"schemaVersion":1,"system":[],"user":[],
            "systemSnapshot":[{"path":"C:\\WINDOWS","enabled":true}],
            "userSnapshot":[]}"#,
    )
    .unwrap();

    // ② path_backup_*.txt —— 这是**备份**（kind=backup），且导入顺序在最后。
    //    文件名用比 disabled.json 更晚的时间戳，确保「按时间取最后一条」会取到它。
    std::fs::create_dir_all(home.join("backups")).unwrap();
    std::fs::write(
        home.join("backups").join("path_backup_20991231_235959_999.txt"),
        "PathEditor Backup - 2099-12-31 23:59:59\n\n[System PATH]\nC:\\BACKUP_MARKER\n\n[User PATH]\n",
    )
    .unwrap();

    // ③ 迁移
    let key = load_or_create_key(home).unwrap();
    let mut db = Db::open_encrypted(&home.join("m.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    let report = migrate_legacy(&mut db, home).unwrap();
    assert_eq!(report.imported, 2, "disabled.json + path_backup 各一份");

    // ④ 核心断言：current_snapshot_id 必须指向 kind='current' 的那条
    let kind: String = db
        .query_row_with(
            "SELECT s.kind FROM app_state a JOIN snapshots s ON s.id = a.current_snapshot_id
              WHERE a.id = 1",
            [],
            |r| r.get(0),
        )
        .expect("迁移后 current_snapshot_id 必须非 NULL 且指向存在的快照");
    assert_eq!(
        kind, "current",
        "D-2：current_snapshot_id 必须指向 kind='current'，实际指向 {kind}"
    );

    // ⑤ 内容校验：必须是 disabled.json 的那份，不是备份
    let snap = current_snapshot(&db).unwrap().expect("应有当前快照");
    assert!(
        snap.system.iter().any(|e| e.path == "C:\\WINDOWS"),
        "当前快照应来自 disabled.json"
    );
    assert!(
        !snap.system.iter().any(|e| e.path == "C:\\BACKUP_MARKER"),
        "D-2：当前快照不得来自 path_backup（那是备份，不是当前状态）"
    );
}
