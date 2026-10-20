// 一文件一测试：见 M0 §5.3。
//
// **B-1 的验收**：测试全程只碰 `TempDir`，不调 `load_profile(name)`。
// 若调它，路径由 `profiles_dir()` = `dirs::home_dir()` 决定，
// 会在用户真实的 `~/.patheditor/profiles/` 下留下文件。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
use path_editor_core::db::profiles::save_profile;
use path_editor_core::db::v5_export::export_profile_v5;
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use path_editor_core::profiles::load_profile_from;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn exported_v5_profile_roundtrips_through_load_profile_from() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("v5.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    // 造一个带快照的配置（save_profile 接 NewSnapshot，内部会建快照）
    let profile_uid = save_profile(
        &mut db,
        "WorkEnv",
        Some("工作环境"),
        &[],
        NewSnapshot {
            kind: SnapshotKind::Profile,
            source: EventSource::Gui,
            system: vec![
                PathEntry {
                    path: "C:\\Program Files\\Java\\jdk21\\bin".into(),
                    enabled: true,
                },
                PathEntry {
                    path: "C:\\Disabled".into(),
                    enabled: false,
                },
            ],
            user: vec![PathEntry {
                path: "C:\\Users\\me\\bin".into(),
                enabled: true,
            }],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();

    // ── 导出 ──
    let json = export_profile_v5(&db, &profile_uid).expect("导出失败");

    // 输出的必须是 v5 的 Versioned<ProfileData> 信封
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("输出必须是合法 JSON");
    assert!(
        parsed.get("schemaVersion").is_some(),
        "v5 配置必须带 schemaVersion（Versioned 信封），实际: {json}"
    );
    assert_eq!(parsed["name"], "WorkEnv");

    // ── 往返：写进 TempDir，再用新入口读回 ──
    let out_path = dir.path().join("exported.json");
    std::fs::write(&out_path, &json).unwrap();

    let roundtrip = load_profile_from(&out_path).expect("load_profile_from 必须能读回导出的文件");
    assert_eq!(roundtrip.name, "WorkEnv");
    assert_eq!(roundtrip.sys.len(), 2, "system 两条");
    assert_eq!(roundtrip.user.len(), 1, "user 一条");

    // 顺序与 enabled 都必须保留（跨层契约）
    assert_eq!(roundtrip.sys[0].path, "C:\\Program Files\\Java\\jdk21\\bin");
    assert!(roundtrip.sys[0].enabled);
    assert_eq!(roundtrip.sys[1].path, "C:\\Disabled");
    assert!(!roundtrip.sys[1].enabled, "enabled=false 必须保留");
    assert_eq!(roundtrip.user[0].path, "C:\\Users\\me\\bin");

    // ── 不存在的配置 ──
    let err = export_profile_v5(&db, "no-such-uid").expect_err("不存在的配置必须报错");
    assert_eq!(err.code, path_editor_core::ErrorCode::NotFound);

    // ── 全程只碰 TempDir：确认没有在用户目录留下文件 ──
    // （这条断言是形式化的：若上面的实现偷偷调用了 load_profile(name)，
    //  它在用户目录创建的路径不会出现在本 TempDir 里，但也不会被这里发现。
    //  真正的保证是代码路径——测试只调 load_profile_from(path)。）
    assert!(out_path.exists(), "导出文件应存在于 TempDir");
}
