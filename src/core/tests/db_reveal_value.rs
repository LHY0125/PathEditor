// 一文件一测试（SQLCipher）。
//
// ：`history_reveal_env_value` 的 core 侧实现。
//
// 语义：
// - 掩码是**默认**：`get_snapshot(reveal=false)` 对敏感值返回 None；
// - 明文必须经**独立命令**显式请求，是一次可审计的动作；
// - `restorable = 0` 的变量（`Unsupported` 类型）没有值可 reveal，返回 NotFound。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{
    get_snapshot, insert_snapshot, reveal_env_value, EnvValueRow, NewSnapshot, SnapshotKind,
};
use path_editor_core::db::Db;
use path_editor_core::domain::history::EventSource;
use path_editor_core::PathEntry;
use tempfile::TempDir;

fn env(
    hive: &str,
    name: &str,
    value: Option<&str>,
    sensitive: bool,
    restorable: bool,
) -> EnvValueRow {
    EnvValueRow {
        hive: hive.into(),
        name: name.into(),
        kind: if restorable {
            "ExpandString".into()
        } else {
            "Unsupported".into()
        },
        value: value.map(|s| s.into()),
        revision: "r1".into(),
        sensitive,
        restorable,
    }
}

#[test]
fn reveal_returns_plaintext_only_when_explicitly_requested() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("rv.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let uid = insert_snapshot(
        &mut db,
        NewSnapshot {
            kind: SnapshotKind::History,
            source: EventSource::Gui,
            system: vec![],
            user: vec![PathEntry {
                path: "C:\\U".into(),
                enabled: true,
            }],
            env: vec![
                // 非敏感：默认就该看到
                env("user", "PLAIN_VAR", Some("plain-value"), false, true),
                // 敏感：默认掩码，必须显式 reveal
                env("user", "MY_TOKEN", Some("super-secret"), true, true),
                // Unsupported：没有值可 reveal
                env("user", "DWORD_VAR", None, false, false),
            ],
            parent_uid: None,
        },
    )
    .unwrap();

    // ── 默认路径必须掩码 ──
    let masked = get_snapshot(&db, &uid, false).unwrap();
    let token = masked
        .env
        .iter()
        .find(|e| e.name == "MY_TOKEN")
        .expect("MY_TOKEN 应存在");
    assert!(token.sensitive, "必须标记为敏感");
    assert_eq!(token.value, None, "默认路径不得返回敏感明文");

    let plain = masked
        .env
        .iter()
        .find(|e| e.name == "PLAIN_VAR")
        .expect("PLAIN_VAR 应存在");
    assert_eq!(
        plain.value.as_deref(),
        Some("plain-value"),
        "非敏感值默认可见"
    );

    // ── 显式 reveal：敏感与非敏感都返回明文 ──
    assert_eq!(
        reveal_env_value(&db, &uid, "user", "MY_TOKEN").unwrap(),
        "super-secret",
        "显式 reveal 必须返回明文"
    );
    assert_eq!(
        reveal_env_value(&db, &uid, "user", "PLAIN_VAR").unwrap(),
        "plain-value"
    );

    // ── restorable = 0：没有值，必须 NotFound 而不是空串 ──
    let err =
        reveal_env_value(&db, &uid, "user", "DWORD_VAR").expect_err("Unsupported 类型不应有值");
    assert_eq!(
        err.code,
        path_editor_core::ErrorCode::NotFound,
        "restorable=0 必须返回 NotFound，实际 {:?}",
        err.code
    );

    // ── 不存在的变量 ──
    let err = reveal_env_value(&db, &uid, "user", "NO_SUCH_VAR").expect_err("不存在的变量必须报错");
    assert_eq!(err.code, path_editor_core::ErrorCode::NotFound);

    // ── 不存在的快照 ──
    let err = reveal_env_value(&db, "no-such-uid", "user", "PLAIN_VAR")
        .expect_err("不存在的快照必须报错");
    assert_eq!(err.code, path_editor_core::ErrorCode::NotFound);

    // ── hive 不匹配：MY_TOKEN 在 user，用 system 查应该找不到 ──
    let err = reveal_env_value(&db, &uid, "system", "MY_TOKEN")
        .expect_err("hive 不匹配时不应返回 user 的值");
    assert_eq!(err.code, path_editor_core::ErrorCode::NotFound);

    // ── reveal 只读：不改变快照内容 ──
    let after = get_snapshot(&db, &uid, false).unwrap();
    let token_after = after
        .env
        .iter()
        .find(|e| e.name == "MY_TOKEN")
        .expect("MY_TOKEN 仍在");
    assert_eq!(token_after.value, None, "reveal 后默认读取仍须掩码");
}
