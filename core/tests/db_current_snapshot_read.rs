// 一文件一测试（SQLCipher）。
//
// **DB 当前快照读取守卫**：`load_path_snapshot_db` 必须从
// `app_state.current_snapshot_id` 读取完整有序快照（含禁用项），而不是
// 回落到其它状态源。
//
// **本测试使用 SQLCipher**，因此整个文件只放一个 `#[test]`。
use path_editor_core::db::history::{
    app_state_version, insert_snapshot, set_current_snapshot, NewSnapshot, SnapshotKind,
};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::{service, PathEntry};

fn entry(path: &str, enabled: bool) -> PathEntry {
    PathEntry {
        path: path.to_string(),
        enabled,
    }
}

#[test]
fn load_snapshot_reads_db_current_snapshot_and_keeps_disabled_entries() {
    let dir = tempfile::TempDir::new().unwrap();
    let key = "00".repeat(32);
    let mut db = Db::open_encrypted(&dir.path().join("state.db"), &key).unwrap();
    db.migrate_to_current().unwrap();

    let uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![entry("C:\\WINDOWS", true)],
            user: vec![entry("C:\\WINDOWS\\system32", false)],
            env: Vec::new(),
            parent_uid: None,
        },
    )
    .unwrap();
    let version = app_state_version(&db).unwrap();
    assert!(set_current_snapshot(&mut db, &uid, version).unwrap());

    let snapshot = service::load_path_snapshot_db(&db).expect("读取 DB 当前快照失败");

    assert!(
        snapshot
            .system
            .iter()
            .any(|e| e.path.eq_ignore_ascii_case("C:\\WINDOWS")),
        "system 侧必须来自 DB 当前快照，实际: {:?}",
        snapshot.system.iter().map(|e| &e.path).collect::<Vec<_>>()
    );

    let usr = snapshot
        .user
        .iter()
        .find(|e| e.path.eq_ignore_ascii_case("C:\\WINDOWS\\system32"))
        .expect("user 侧必须来自 DB 当前快照");
    assert!(!usr.enabled, "禁用标记必须保留");
}
