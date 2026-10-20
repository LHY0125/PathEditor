// 一文件一测试：见 M0 §5.3。
//
// J-7 整改的守卫测试：`app_state` 单行必须在**建库时**就存在。
// 若它为空，`set_current_snapshot` 的 CAS 永远匹配 0 行、恒返回 false，
// spec §十二「并发写：CAS 冲突返回 conflict」将永远不可达。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{
    app_state_version, insert_snapshot, set_current_snapshot, NewSnapshot, SnapshotKind,
};
use path_editor_core::db::Db;
use path_editor_core::history::EventSource;
use tempfile::TempDir;

#[test]
fn app_state_is_seeded_at_creation_and_cas_works() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("cas.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    // ① 建库即有 app_state 单行，且 version = 1
    let version = app_state_version(&db).expect("建库后 app_state 必须已有单行");
    assert_eq!(version, 1, "初始 version 应为 1");

    // ② device_id 必须是非空的真实值，而不是 record 时的 'local' 兜底
    let device_id: String = db
        .query_row_with("SELECT device_id FROM app_state WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_ne!(device_id, "local", "device_id 必须是真实 UUID，不是兜底值");
    assert_eq!(device_id.len(), 36, "UUIDv7 字符串应为 36 字符");

    // ③ CAS 必须真的能成功一次（旧实现里这里恒 false）
    let uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![],
            user: vec![],
            env: vec![],
            parent_uid: None,
        },
    )
    .unwrap();

    assert!(
        set_current_snapshot(&mut db, &uid, version).unwrap(),
        "用正确的 expected_version 做 CAS 必须成功"
    );
    assert_eq!(
        app_state_version(&db).unwrap(),
        version + 1,
        "CAS 成功必须递增 version"
    );

    // ④ 用陈旧的 version 重试必须失败（返回 false，而不是静默覆盖）
    assert!(
        !set_current_snapshot(&mut db, &uid, version).unwrap(),
        "陈旧 version 的 CAS 必须失败"
    );

    // ⑤ 第二次 migrate_to_current 不得重置 app_state（幂等，不覆盖 device_id）
    let before: String = db
        .query_row_with("SELECT device_id FROM app_state WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    db.migrate_to_current().unwrap();
    let after: String = db
        .query_row_with("SELECT device_id FROM app_state WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(before, after, "重复迁移不得覆盖 device_id");
}
