// 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{
    current_snapshot, get_snapshot, insert_snapshot, list_events, EnvValueRow, EventFilter,
    NewSnapshot, SnapshotKind,
};
use path_editor_core::db::Db;
use path_editor_core::domain::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn snapshot_roundtrip_and_masking() {
    let dir = TempDir::new().unwrap();

    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("h.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::Current,
            source: EventSource::Gui,
            system: vec![PathEntry {
                path: "C:\\Windows".into(),
                enabled: true,
            }],
            user: vec![PathEntry {
                path: "C:\\Tools".into(),
                enabled: false,
            }],
            env: vec![
                EnvValueRow {
                    hive: "user".into(),
                    name: "MY_TOKEN".into(),
                    kind: "String".into(),
                    value: Some("secret".into()),
                    revision: "abc".into(),
                    // 故意传 false：写入层必须用 is_sensitive 覆盖成 true
                    sensitive: false,
                    restorable: true,
                },
                EnvValueRow {
                    hive: "user".into(),
                    name: "PLAIN_ONE".into(),
                    kind: "String".into(),
                    value: Some("visible".into()),
                    revision: "def".into(),
                    sensitive: false,
                    restorable: true,
                },
            ],
            parent_uid: None,
        },
    )
    .unwrap();

    // 掩码路径：sensitive=1 且 reveal=false → value 必须为 None
    let masked = get_snapshot(&db, &uid, false).unwrap();
    assert_eq!(masked.system.len(), 1);
    assert!(!masked.user[0].enabled);
    assert_eq!(masked.kind, "current");

    let token = masked.env.iter().find(|e| e.name == "MY_TOKEN").unwrap();
    assert_eq!(token.value, None, "敏感值默认必须被掩码");
    assert!(token.sensitive, "sensitive 必须由 is_sensitive 覆盖为 true");

    let plain = masked.env.iter().find(|e| e.name == "PLAIN_ONE").unwrap();
    assert_eq!(
        plain.value.as_deref(),
        Some("visible"),
        "非敏感值不应被掩码"
    );

    // reveal 路径才返回明文
    let revealed = get_snapshot(&db, &uid, true).unwrap();
    let token = revealed.env.iter().find(|e| e.name == "MY_TOKEN").unwrap();
    assert_eq!(token.value.as_deref(), Some("secret"));

    // insert_snapshot 不写事件；事件由 HistorySink 负责
    let page = list_events(&db, &EventFilter::default()).unwrap();
    assert!(page.items.is_empty(), "insert_snapshot 不应自行写事件");
    assert_eq!(page.total, 0);

    // current_snapshot 在 app_state 未指向任何快照时为 None
    assert!(current_snapshot(&db).unwrap().is_none());

    // 不存在的 uid → NotFound
    let err = get_snapshot(&db, "no-such-uid", false).unwrap_err();
    assert_eq!(err.code, path_editor_core::ErrorCode::NotFound);
}
