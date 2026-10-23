// 一文件一测试（SQLCipher）。
//
// 启动引导的幂等性：基线只建一次；FTS 只首次构建；派生表水位未变时跳过。
use path_editor_core::bootstrap::{self, BootstrapOptions};
use path_editor_core::db::search;
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::{PathEntry, PathSnapshot};
use tempfile::TempDir;

#[test]
fn bootstrap_creates_baseline_once_and_skips_unchanged_derived_tables() {
    let dir = TempDir::new().unwrap();
    let key = "22".repeat(32);
    let mut db = Db::open_encrypted(&dir.path().join("bootstrap.db"), &key).unwrap();
    db.migrate_to_current().unwrap();

    // 先手动建立基线，避免 bootstrap 读取真实注册表。
    let snapshot = PathSnapshot {
        system: vec![PathEntry {
            path: "C:\\A".into(),
            enabled: true,
        }],
        user: vec![PathEntry {
            path: "D:\\U".into(),
            enabled: false,
        }],
    };
    assert!(bootstrap::ensure_baseline_from(&mut db, snapshot).unwrap());
    assert!(
        !bootstrap::ensure_baseline_from(
            &mut db,
            PathSnapshot {
                system: vec![],
                user: vec![],
            },
        )
        .unwrap(),
        "已有基线时必须幂等返回 false"
    );

    let options = BootstrapOptions {
        backup_dir: Some(dir.path().join("backups")),
        pending_path: Some(dir.path().join("pending.json")),
        derived_watermark: Some(dir.path().join("derived.watermark")),
    };

    let first = bootstrap::initialize(&mut db, EventSource::Gui, &options).unwrap();
    assert!(!first.baseline_created, "基线已存在，不得重复创建");
    assert!(
        first.warnings.is_empty(),
        "不应有警告: {:?}",
        first.warnings
    );
    assert!(first.fts_indexed > 0, "首次启动必须构建 FTS 索引");
    assert!(first.derived_rebuilt.is_some(), "首次启动必须重建派生表");

    let second = bootstrap::initialize(&mut db, EventSource::Gui, &options).unwrap();
    assert!(!second.baseline_created);
    assert_eq!(second.fts_indexed, 0, "FTS 已非空，不得重复构建");
    assert!(
        second.derived_rebuilt.is_none(),
        "水位未变，必须跳过派生表重建"
    );
    assert_eq!(
        search::index_count(&db).unwrap(),
        first.fts_indexed as i64,
        "第二次启动不得改变索引行数"
    );
}
