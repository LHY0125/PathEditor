//! `.v5.bak` 归档：只改名、幂等、不覆盖已有备份。
//!
//! 一文件一测试（M0 §5.3）。
use path_editor_core::db::archive_v5::archive_v5_sources;
use tempfile::TempDir;

#[test]
fn archives_v5_sources_once_and_never_overwrites() {
    let dir = TempDir::new().unwrap();
    let home = dir.path();
    std::fs::create_dir_all(home.join("profiles")).unwrap();
    std::fs::write(home.join("disabled.json"), "{}").unwrap();
    std::fs::write(home.join("profiles").join("Work.json"), "{}").unwrap();
    // 已存在的备份：必须跳过
    std::fs::write(home.join("disabled.json.v5.bak"), "old").unwrap();

    let dry = archive_v5_sources(home, true).unwrap();
    assert_eq!(dry.archived.len(), 1, "dry-run 只报告 profiles/Work.json");
    assert!(
        home.join("profiles").join("Work.json").exists(),
        "dry-run 不得改名"
    );

    let real = archive_v5_sources(home, false).unwrap();
    assert_eq!(real.archived.len(), 1);
    assert_eq!(real.skipped.len(), 1, "disabled.json 因目标已存在而跳过");

    assert!(home.join("profiles").join("Work.json.v5.bak").exists());
    assert!(!home.join("profiles").join("Work.json").exists());
    assert_eq!(
        std::fs::read_to_string(home.join("disabled.json.v5.bak")).unwrap(),
        "old",
        "已有备份不得被覆盖"
    );

    let again = archive_v5_sources(home, false).unwrap();
    assert!(again.archived.is_empty(), "第二次运行无候选");
}
