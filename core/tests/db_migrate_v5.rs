// 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::migrate_v5::migrate_legacy;
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn legacy_files_import_once_and_are_idempotent() {
    let dir = TempDir::new().unwrap();

    // 旧文件是 `Versioned<T>` 信封（persist.rs:37，serde flatten），
    // 正常路径必须带 schemaVersion；裸 JSON 只是「无版本头的旧格式」，
    // 两种都要覆盖。
    std::fs::create_dir_all(dir.path().join("profiles")).unwrap();
    std::fs::write(
        dir.path().join("disabled.json"),
        r#"{"schemaVersion":1,"system":[],"user":[],"systemSnapshot":[{"path":"C:\\A","enabled":true}],"userSnapshot":[{"path":"D:\\B","enabled":false}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("profiles").join("Work.json"),
        r#"{"schemaVersion":1,"name":"Work","sys":[{"path":"C:\\W","enabled":true}],"user":[],"created":"2026-01-01","modified":"2026-01-01"}"#,
    )
    .unwrap();
    // 无版本头的旧格式（v1 之前）也必须能导入
    std::fs::write(
        dir.path().join("profiles").join("Legacy.json"),
        r#"{"name":"Legacy","sys":[],"user":[{"path":"E:\\L","enabled":true}],"created":"2025-01-01","modified":"2025-01-01"}"#,
    )
    .unwrap();

    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("m.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let r1 = migrate_legacy(&mut db, dir.path()).unwrap();
    assert_eq!(r1.imported, 3, "disabled + 2 profiles（含无版本头）");
    assert_eq!(r1.failed, 0, "夹具本身合法，不应有失败项");

    // 导入的内容确实落库
    let snapshot_count: i64 = db
        .query_row_with("SELECT count(*) FROM snapshots", [], |r| r.get(0))
        .unwrap();
    assert_eq!(snapshot_count, 3, "每个旧文件一条快照");

    let profile_count: i64 = db
        .query_row_with(
            "SELECT count(*) FROM profiles WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(profile_count, 2);

    // 无版本头的旧格式也要真的导入（不是被解析成默认值）
    let user_entries: i64 = db
        .query_row_with(
            "SELECT count(*) FROM path_entries WHERE hive = 'user' AND path = ?1",
            ["E:\\L"],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(user_entries, 1, "无版本头配置的 user 条目必须导入");

    // 迁移标记
    let migrated: i64 = db
        .query_row_with("SELECT migrated_v5 FROM app_state WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(migrated, 1);

    // 幂等：重跑必须全部跳过
    let r2 = migrate_legacy(&mut db, dir.path()).unwrap();
    assert_eq!(r2.imported, 0, "重复迁移必须幂等");
    assert_eq!(r2.skipped, 3);
    assert_eq!(r2.failed, 0);

    // 且没有产生重复快照
    let snapshot_count: i64 = db
        .query_row_with("SELECT count(*) FROM snapshots", [], |r| r.get(0))
        .unwrap();
    assert_eq!(snapshot_count, 3, "重跑不得新增快照");

    // 原件必须原样保留（迁移只读）
    assert!(dir.path().join("disabled.json").exists());
    assert!(dir.path().join("profiles").join("Work.json").exists());
    assert!(dir.path().join("profiles").join("Legacy.json").exists());

    // ──：新装用户没有 disabled.json 时，迁移不产生 kind=current 快照 ──
    //
    // 这是**已知边界**，不是缺陷：建基线是启动序列（`setup_database` 的 ③b）
    // 的职责，不是迁移的。此处把边界钉下来，避免将来有人误以为迁移会兜底。
    {
        let dir2 = TempDir::new().unwrap();
        // 只放一个 profiles 文件，**不放 disabled.json**
        std::fs::create_dir_all(dir2.path().join("profiles")).unwrap();
        std::fs::write(
            dir2.path().join("profiles").join("Only.json"),
            r#"{"schemaVersion":1,"name":"Only","sys":[],"user":[],"created":"2026-01-01","modified":"2026-01-01"}"#,
        )
        .unwrap();

        let key2 = load_or_create_key(dir2.path()).unwrap();
        let mut db2 = Db::open_encrypted(&dir2.path().join("n.db"), &key_hex(&key2)).unwrap();
        db2.migrate_to_current().unwrap();
        migrate_legacy(&mut db2, dir2.path()).unwrap();

        let has_current: i64 = db2
            .query_row_with(
                "SELECT count(*) FROM snapshots WHERE kind = 'current'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            has_current, 0,
            "无 disabled.json 时迁移不产生 current 快照——建基线由启动序列负责"
        );
    }
}
