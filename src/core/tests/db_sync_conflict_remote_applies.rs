//! 「采纳远端」必须**真的把远端实体写进本地**，而不只是追加一条 change。
//!
//! 回归点（审查报告 P1-2）：旧实现 `resolve_conflict(Remote)` 只 `record_change_tx`，
//! 实体保持原样 —— 下次导出会用「本地仍存活的实体」覆盖刚记录的 new_value，
//! 第三台设备也永远看不到用户选的远端内容。
//!
//! 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::history::{NewSnapshot, SnapshotKind};
use path_editor_core::db::profiles::{get_profile_by_name, rename_profile, save_profile};
use path_editor_core::db::sync::conflict::{list_conflicts, resolve_conflict, Resolution};
use path_editor_core::db::sync::export::export_bundle;
use path_editor_core::db::sync::import::import_bundle;
use path_editor_core::db::Db;
use path_editor_core::domain::history::EventSource;
use tempfile::TempDir;

fn open_db(dir: &TempDir, name: &str) -> Db {
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join(name), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    db
}

fn empty_snapshot() -> NewSnapshot {
    NewSnapshot {
        kind: SnapshotKind::Profile,
        source: EventSource::Gui,
        system: vec![],
        user: vec![],
        env: vec![],
        parent_uid: None,
    }
}

#[test]
fn resolving_conflict_to_remote_applies_remote_profile() {
    let dir = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();

    let mut a = open_db(&src, "a.db");
    let mut b = open_db(&dst, "b.db");

    // A 建配置 → 导出 → B 导入（两边拥有同一个 profile uid）
    save_profile(&mut a, "base", None, &[], empty_snapshot()).unwrap();
    let bundle = dir.path().join("base.pesync");
    export_bundle(&mut a, "pw", &bundle).unwrap();
    import_bundle(&mut b, "pw", &bundle).unwrap();
    assert!(
        get_profile_by_name(&b, "base").unwrap().is_some(),
        "前置：B 必须已导入同一个配置"
    );

    // 两边在同一个 lamport 上各自改名 → 下次导入必然产生冲突
    rename_profile(&mut a, "base", "A-renamed").unwrap();
    rename_profile(&mut b, "base", "B-renamed").unwrap();

    let b_before = get_profile_by_name(&b, "B-renamed").unwrap().unwrap();
    let bundle = dir.path().join("rename.pesync");
    export_bundle(&mut a, "pw", &bundle).unwrap();
    let report = import_bundle(&mut b, "pw", &bundle).unwrap();
    assert_eq!(report.conflicts, 1, "同名同 lamport 的改名必须记冲突");

    let conflict = list_conflicts(&b, false).unwrap().remove(0);
    assert_eq!(conflict.field.as_deref(), Some("name"));

    // ── 关键断言：采纳远端后，本地实体必须变成 A 的名字 ──
    resolve_conflict(&mut b, conflict.id, Resolution::Remote).unwrap();

    assert!(
        get_profile_by_name(&b, "A-renamed").unwrap().is_some(),
        "采纳远端后本地配置必须改名为 A-renamed"
    );
    assert!(
        get_profile_by_name(&b, "B-renamed").unwrap().is_none(),
        "旧的本地名必须不再存在"
    );

    // uid 保持不变：同步引用仍然指向同一个实体
    let after = get_profile_by_name(&b, "A-renamed").unwrap().unwrap();
    assert_eq!(after.uid, b_before.uid);
}
