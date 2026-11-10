use path_editor_core::db::history::{
    current_snapshot, get_snapshot, insert_snapshot, list_events, list_pinned, set_snapshot_note,
    set_snapshot_pinned, EnvValueRow, EventFilter, NewSnapshot, SnapshotKind,
};
use path_editor_core::db::Db;
use path_editor_core::domain::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

#[test]
fn snapshot_roundtrip_and_masking() {
    let dir = TempDir::new().unwrap();

    let mut db = Db::open(&dir.path().join("h.db")).unwrap();
    db.ensure_schema().unwrap();

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

#[test]
fn snapshot_note_and_pin_roundtrip() {
    let dir = TempDir::new().unwrap();
    let mut db = Db::open(&dir.path().join("note.db")).unwrap();
    db.ensure_schema().unwrap();

    let uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::History,
            source: EventSource::Cli,
            system: Vec::new(),
            user: vec![PathEntry {
                path: "C:\\Tools".into(),
                enabled: true,
            }],
            env: Vec::new(),
            parent_uid: None,
        },
    )
    .unwrap();

    // 默认：无备注、未固定
    let view = get_snapshot(&db, &uid, false).unwrap();
    assert!(view.note.is_none());
    assert!(!view.pinned);
    assert!(list_pinned(&db).unwrap().is_empty());

    // 备注两端空白被裁剪
    set_snapshot_note(&mut db, &uid, Some("  装驱动前  ")).unwrap();
    let view = get_snapshot(&db, &uid, false).unwrap();
    assert_eq!(view.note.as_deref(), Some("装驱动前"));

    // 固定后出现在固定列表
    set_snapshot_pinned(&mut db, &uid, true).unwrap();
    let pinned = list_pinned(&db).unwrap();
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned[0].uid, uid);
    assert!(get_snapshot(&db, &uid, false).unwrap().pinned);

    // 超长备注被拒绝（500 字符上限，按字符而非字节）
    let too_long = "备".repeat(501);
    let err = set_snapshot_note(&mut db, &uid, Some(&too_long)).unwrap_err();
    assert_eq!(err.code, path_editor_core::ErrorCode::InvalidValue);

    // 全空白 = 清空
    set_snapshot_note(&mut db, &uid, Some("   ")).unwrap();
    assert!(get_snapshot(&db, &uid, false).unwrap().note.is_none());

    // 取消固定
    set_snapshot_pinned(&mut db, &uid, false).unwrap();
    assert!(list_pinned(&db).unwrap().is_empty());

    // 不存在的 uid → NotFound（两种写操作一致）
    let err = set_snapshot_note(&mut db, "missing", Some("x")).unwrap_err();
    assert_eq!(err.code, path_editor_core::ErrorCode::NotFound);
    let err = set_snapshot_pinned(&mut db, "missing", true).unwrap_err();
    assert_eq!(err.code, path_editor_core::ErrorCode::NotFound);
}
