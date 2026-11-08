//
// 配置应用的**失败传播**：某个 hive 写入失败时，`apply_profile_io` 必须返回 Err，
// 而不是把它包成一次「成功」的应用。
//
// 这条用例锁的是 P1-1 的整改点：`HiveOutcome::Failed` 只表示「哪个 hive 失败」，
// 调用方（CLI / GUI）靠返回值的 Ok/Err 决定打印「已写入」还是报错。若失败被吞成
// Ok，CLI 会 exit 0 并声称写成功，用户以为配置已生效。
use path_editor_core::app::service::{self, PathHiveIo};
use path_editor_core::db::history::NewSnapshot;
use path_editor_core::db::profiles::save_profile;
use path_editor_core::domain::history::EventSource;
use path_editor_core::{app::bootstrap, CoreError, EnvHive, ErrorCode, PathEntry, PathSnapshot};
use std::cell::RefCell;

fn entry(path: &str, enabled: bool) -> PathEntry {
    PathEntry {
        path: path.to_string(),
        enabled,
    }
}

/// 系统 hive 写入必失败的 fake 注册表；用户 hive 正常。
///
/// 只让**一个** hive 失败是刻意的：两个都失败无法区分「逐 hive 汇总」与
/// 「第一个失败就整体短路」这两种实现。
struct SystemWriteFailsIo {
    sys: RefCell<Vec<String>>,
    usr: RefCell<Vec<String>>,
}

impl PathHiveIo for SystemWriteFailsIo {
    fn read_enabled(&self, hive: EnvHive) -> Result<Vec<String>, CoreError> {
        Ok(match hive {
            EnvHive::System => self.sys.borrow().clone(),
            EnvHive::User => self.usr.borrow().clone(),
        })
    }

    fn write_enabled(&self, hive: EnvHive, paths: &[String]) -> Result<(), CoreError> {
        match hive {
            EnvHive::System => Err(CoreError::new(
                ErrorCode::PermissionDenied,
                "write_enabled",
                "拒绝访问：需要管理员权限",
            )),
            EnvHive::User => {
                *self.usr.borrow_mut() = paths.to_vec();
                Ok(())
            }
        }
    }

    fn backup(&self) -> Result<(), CoreError> {
        Ok(())
    }
}

#[test]
fn apply_profile_returns_err_when_a_hive_write_fails() {
    let (dir, mut db) = crate::common::new_db("apply_fail.db");

    // 基线必须与 fake 注册表的当前状态一致，否则「无变化」短路会让本用例空跑。
    bootstrap::ensure_baseline_from(
        &mut db,
        PathSnapshot {
            system: vec![entry("C:\\A", true)],
            user: vec![entry("D:\\U", true)],
        },
    )
    .unwrap();

    // 配置两侧都与当前状态不同：system 必然触发写入（并失败），user 也会写入。
    save_profile(
        &mut db,
        "Broken",
        None,
        &[],
        NewSnapshot {
            kind: path_editor_core::db::history::SnapshotKind::Profile,
            source: EventSource::Cli,
            system: vec![entry("C:\\A", true), entry("C:\\NEW", true)],
            user: vec![entry("D:\\U", true), entry("D:\\NEW", true)],
            env: Vec::new(),
            parent_uid: None,
        },
    )
    .unwrap();

    let io = SystemWriteFailsIo {
        sys: RefCell::new(vec!["C:\\A".to_string()]),
        usr: RefCell::new(vec!["D:\\U".to_string()]),
    };
    let pending = dir.path().join("pending.json");

    let err = service::apply_profile_io(&io, &mut db, "Broken", EventSource::Cli, &pending)
        .expect_err("某个 hive 写入失败时必须返回 Err，而不是包装成成功");

    assert_eq!(
        err.code,
        ErrorCode::PermissionDenied,
        "错误码应来自失败的 hive，而不是被改写成 Internal: {}",
        err.message
    );
    assert!(
        err.message.contains("系统") || err.message.contains("system"),
        "错误文案必须指明哪个 hive 失败: {}",
        err.message
    );

    // 失败不得被记为一次成功应用：记账发生在两侧都成功之后。
    let view = path_editor_core::db::profiles::list_profiles(&db)
        .unwrap()
        .into_iter()
        .find(|p| p.name == "Broken")
        .expect("配置必须存在");
    assert_eq!(view.apply_count, 0, "失败的应用不得记账");
}
