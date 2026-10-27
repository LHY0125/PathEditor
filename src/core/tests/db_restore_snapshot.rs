// 一文件一测试（SQLCipher）。
//
// 恢复快照：读快照 → 写注册表 → 当前快照更新 → 事件 source=restore；
// 旧快照保留（append-only），保证「恢复」本身可被撤销。
use path_editor_core::app::service::{self, HiveOutcome, PathHiveIo};
use path_editor_core::db::history::{current_snapshot, insert_snapshot, NewSnapshot, SnapshotKind};
use path_editor_core::db::Db;
use path_editor_core::domain::history::EventSource;
use path_editor_core::{app::bootstrap, CoreError, EnvHive, PathEntry, PathSnapshot};
use std::cell::RefCell;
use tempfile::TempDir;

fn entry(path: &str, enabled: bool) -> PathEntry {
    PathEntry {
        path: path.to_string(),
        enabled,
    }
}

/// 记录写入的 fake 注册表。
struct FakeIo {
    sys: RefCell<Vec<String>>,
    usr: RefCell<Vec<String>>,
}

impl FakeIo {
    fn new(sys: &[&str], usr: &[&str]) -> Self {
        Self {
            sys: RefCell::new(sys.iter().map(|s| s.to_string()).collect()),
            usr: RefCell::new(usr.iter().map(|s| s.to_string()).collect()),
        }
    }
}

impl PathHiveIo for FakeIo {
    fn read_enabled(&self, hive: EnvHive) -> Result<Vec<String>, CoreError> {
        Ok(match hive {
            EnvHive::System => self.sys.borrow().clone(),
            EnvHive::User => self.usr.borrow().clone(),
        })
    }

    fn write_enabled(&self, hive: EnvHive, paths: &[String]) -> Result<(), CoreError> {
        match hive {
            EnvHive::System => *self.sys.borrow_mut() = paths.to_vec(),
            EnvHive::User => *self.usr.borrow_mut() = paths.to_vec(),
        }
        Ok(())
    }

    fn backup(&self) -> Result<(), CoreError> {
        Ok(())
    }
}

#[test]
fn restore_snapshot_writes_registry_updates_current_and_keeps_old_snapshot() {
    let dir = TempDir::new().unwrap();
    let key = "44".repeat(32);
    let mut db = Db::open_encrypted(&dir.path().join("restore.db"), &key).unwrap();
    db.migrate_to_current().unwrap();

    // 当前基线：C:\A
    bootstrap::ensure_baseline_from(
        &mut db,
        PathSnapshot {
            system: vec![entry("C:\\A", true)],
            user: vec![],
        },
    )
    .unwrap();

    // 历史快照：C:\B + D:\U
    let uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::History,
            source: EventSource::Gui,
            system: vec![entry("C:\\B", true)],
            user: vec![entry("D:\\U", true)],
            env: Vec::new(),
            parent_uid: None,
        },
    )
    .unwrap();

    let snapshots_before: i64 = db
        .query_row_with("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0))
        .unwrap();

    let io = FakeIo::new(&["C:\\A"], &[]);
    let outcome = service::restore_snapshot_io(
        &io,
        &mut db,
        &uid,
        EventSource::Restore,
        &dir.path().join("pending.json"),
    )
    .expect("恢复失败");

    assert!(matches!(outcome.system, HiveOutcome::Applied));
    assert!(matches!(outcome.user, HiveOutcome::Applied));
    assert_eq!(*io.sys.borrow(), vec!["C:\\B".to_string()]);
    assert_eq!(*io.usr.borrow(), vec!["D:\\U".to_string()]);

    let current = current_snapshot(&db).unwrap().expect("必须有当前快照");
    assert_eq!(current.system, vec![entry("C:\\B", true)]);
    assert_eq!(current.user, vec![entry("D:\\U", true)]);

    let snapshots_after: i64 = db
        .query_row_with("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0))
        .unwrap();
    assert!(
        snapshots_after > snapshots_before,
        "恢复必须追加新快照，旧快照保留（append-only）"
    );

    let (source, action): (String, String) = db
        .query_row_with(
            "SELECT source, action FROM events ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(source, "restore", "恢复事件必须标记来源");
    assert_eq!(action, "save", "保存编排统一记 save 动作");
}
