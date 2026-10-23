// 一文件一测试（SQLCipher）。
//
// 配置应用编排：读配置 → 只写变化的 hive → 记账；重复应用同内容不重复写注册表。
use path_editor_core::db::history::NewSnapshot;
use path_editor_core::db::profiles::{get_profile_paths_view, list_profiles, save_profile};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::service::{self, PathHiveIo};
use path_editor_core::{bootstrap, CoreError, EnvHive, PathEntry, PathSnapshot};
use std::cell::RefCell;
use tempfile::TempDir;

fn entry(path: &str, enabled: bool) -> PathEntry {
    PathEntry {
        path: path.to_string(),
        enabled,
    }
}

/// 记录写入的 fake 注册表；`read_enabled` 与写入后的状态保持一致。
struct FakeIo {
    sys: RefCell<Vec<String>>,
    usr: RefCell<Vec<String>>,
    writes: RefCell<Vec<(EnvHive, Vec<String>)>>,
}

impl FakeIo {
    fn new(sys: &[&str], usr: &[&str]) -> Self {
        Self {
            sys: RefCell::new(sys.iter().map(|s| s.to_string()).collect()),
            usr: RefCell::new(usr.iter().map(|s| s.to_string()).collect()),
            writes: RefCell::new(Vec::new()),
        }
    }

    fn write_count(&self) -> usize {
        self.writes.borrow().len()
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
        self.writes.borrow_mut().push((hive, paths.to_vec()));
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
fn apply_profile_writes_changed_hive_then_skips_identical_second_apply() {
    let dir = TempDir::new().unwrap();
    let key = "33".repeat(32);
    let mut db = Db::open_encrypted(&dir.path().join("apply.db"), &key).unwrap();
    db.migrate_to_current().unwrap();

    // 当前状态基线必须与 fake 注册表一致，否则第一次应用会因「当前快照为空」而误判。
    bootstrap::ensure_baseline_from(
        &mut db,
        PathSnapshot {
            system: vec![entry("C:\\A", true)],
            user: vec![entry("D:\\U", true)],
        },
    )
    .unwrap();

    save_profile(
        &mut db,
        "Work",
        None,
        &[],
        NewSnapshot {
            kind: path_editor_core::db::history::SnapshotKind::Profile,
            source: EventSource::Cli,
            system: vec![entry("C:\\A", true), entry("C:\\B", false)],
            user: vec![entry("D:\\U", true)],
            env: Vec::new(),
            parent_uid: None,
        },
    )
    .unwrap();

    let io = FakeIo::new(&["C:\\A"], &["D:\\U"]);
    let pending = dir.path().join("pending.json");

    let first = service::apply_profile_io(&io, &mut db, "Work", EventSource::Cli, &pending)
        .expect("应用配置失败");
    assert_eq!(
        io.write_count(),
        1,
        "只有 system 发生变化（新增禁用项），user 不得重复写"
    );
    assert_eq!(io.writes.borrow()[0].0, EnvHive::System);
    assert!(matches!(
        first.system,
        path_editor_core::service::HiveOutcome::Applied
    ));

    let view = list_profiles(&db)
        .unwrap()
        .into_iter()
        .find(|p| p.name == "Work")
        .expect("配置必须存在");
    assert_eq!(view.apply_count, 1, "第一次应用必须记账");
    assert!(view.last_applied_at.is_some());

    // 第二次应用：内容与当前状态一致 → 不写注册表，但仍记账。
    let second = service::apply_profile_io(&io, &mut db, "Work", EventSource::Cli, &pending)
        .expect("第二次应用失败");
    assert_eq!(io.write_count(), 1, "内容一致时不得重复写注册表");
    assert!(matches!(
        second.system,
        path_editor_core::service::HiveOutcome::Skipped
    ));
    assert!(matches!(
        second.user,
        path_editor_core::service::HiveOutcome::Skipped
    ));

    let view = list_profiles(&db)
        .unwrap()
        .into_iter()
        .find(|p| p.name == "Work")
        .expect("配置必须存在");
    assert_eq!(view.apply_count, 2, "每次应用都必须记账（即使无写入）");

    // 视图与配置内容一致：名称、更新时间与两个 hive 的条目都来自同一次读取。
    let paths = get_profile_paths_view(&db, "Work")
        .unwrap()
        .expect("配置视图必须存在");
    assert_eq!(paths.name, "Work");
    assert_eq!(paths.updated_at, view.updated_at);
    assert_eq!(paths.system.len(), 2);
    assert_eq!(paths.user, vec![entry("D:\\U", true)]);
}
