// 一文件一测试（SQLCipher）。
//
// **CLI/DB 保存链路守卫**：`save_path_with_db_io` 必须在写注册表成功后：
// 1. 把当前快照写入 DB（`app_state.current_snapshot_id`）；
// 2. 为 CLI 的写入产生 `changes`（否则跨设备同步看不到 CLI 修改）；
// 3. 记录一条 `source = cli` 的事件。
//
// **本测试使用 SQLCipher**，因此整个文件只放一个 `#[test]`。
use path_editor_core::app::service::{self, PathHiveIo, PathHiveWrite};
use path_editor_core::db::history::{
    app_state_version, current_snapshot, insert_snapshot, set_current_snapshot, NewSnapshot,
    SnapshotKind,
};
use path_editor_core::db::Db;
use path_editor_core::domain::error::CoreError;
use path_editor_core::domain::history::EventSource;
use path_editor_core::{EnvHive, PathEntry};

fn entry(path: &str, enabled: bool) -> PathEntry {
    PathEntry {
        path: path.to_string(),
        enabled,
    }
}

struct FakeIo {
    sys: Vec<String>,
    usr: Vec<String>,
}

impl PathHiveIo for FakeIo {
    fn read_enabled(&self, hive: EnvHive) -> Result<Vec<String>, CoreError> {
        Ok(match hive {
            EnvHive::System => self.sys.clone(),
            EnvHive::User => self.usr.clone(),
        })
    }

    fn write_enabled(&self, _hive: EnvHive, _paths: &[String]) -> Result<(), CoreError> {
        Ok(())
    }

    fn backup(&self) -> Result<(), CoreError> {
        Ok(())
    }
}

#[test]
fn cli_save_updates_current_snapshot_changes_and_event() {
    let dir = tempfile::TempDir::new().unwrap();
    let key = "11".repeat(32);
    let mut db = Db::open_encrypted(&dir.path().join("state.db"), &key).unwrap();
    db.migrate_to_current().unwrap();

    let seed_uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![entry("C:\\A", true), entry("C:\\B", false)],
            user: vec![entry("C:\\U", true)],
            env: Vec::new(),
            parent_uid: None,
        },
    )
    .unwrap();
    let version = app_state_version(&db).unwrap();
    assert!(set_current_snapshot(&mut db, &seed_uid, version).unwrap());

    let changes_before: i64 = db
        .query_row_with(
            "SELECT COUNT(*) FROM changes WHERE entity_type = 'snapshot'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    let outcome = service::save_path_with_db_io(
        &FakeIo {
            sys: vec!["C:\\A".to_string(), "C:\\B".to_string()],
            usr: vec!["C:\\U".to_string()],
        },
        &mut db,
        EventSource::Cli,
        Some(PathHiveWrite {
            entries: vec![
                entry("C:\\A", true),
                entry("C:\\B", true),
                entry("C:\\C", true),
            ],
            expected: None,
        }),
        None,
        &dir.path().join("pending_path_snapshot.json"),
    )
    .expect("保存失败");

    assert!(matches!(
        outcome.sidecar,
        path_editor_core::app::service::SidecarOutcome::Saved
    ));

    let view = current_snapshot(&db).unwrap().expect("必须存在当前快照");
    let system: Vec<_> = view
        .system
        .iter()
        .map(|e| (e.path.as_str(), e.enabled))
        .collect();
    assert_eq!(
        system,
        vec![("C:\\A", true), ("C:\\B", true), ("C:\\C", true)],
        "当前快照必须反映本次写入的完整条目（含 enabled）"
    );
    assert_eq!(
        view.user,
        vec![entry("C:\\U", true)],
        "未写 hive 保留原状态"
    );

    let changes_after: i64 = db
        .query_row_with(
            "SELECT COUNT(*) FROM changes WHERE entity_type = 'snapshot'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        changes_after > changes_before,
        "CLI 写入必须产生 changes（{changes_before} -> {changes_after}）"
    );

    let (source, result): (String, String) = db
        .query_row_with(
            "SELECT source, result FROM events ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(source, "cli");
    assert_eq!(result, "success");
}
