//! 导出：收集 → 加密 → 写文件 →（成功后）标记 exported_at。
//!
//! 三条断言对应三个可能的错误实现：
//! 1. 导出后待导出队列清空（否则同一批会被反复导出）
//! 2. 导出文件能被 `open()` 读回且 header 自洽（否则包是废的）
//! 3. **写文件失败时待导出队列不变**（顺序反了会永久丢失变更）
//!
//! 一文件一测试（SQLCipher）。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::sync::bundle::{open, BundleHeader};
use path_editor_core::db::sync::change_log::{pending_export, record_change_tx, NewChange};
use path_editor_core::db::sync::export::export_bundle;
use path_editor_core::db::Db;
use tempfile::TempDir;

fn seed(db: &mut Db, n: usize) {
    for i in 0..n {
        db.with_transaction(|tx| {
            record_change_tx(
                tx,
                NewChange {
                    entity_type: "profile".into(),
                    entity_uid: format!("p{i}"),
                    field: None,
                    old_value: String::new(),
                    new_value: format!("{{\"name\":\"cfg{i}\"}}"),
                },
            )
        })
        .unwrap();
    }
}

#[test]
fn export_writes_bundle_then_marks_exported_and_never_loses_changes_on_write_failure() {
    let dir = TempDir::new().unwrap();
    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("e.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();
    seed(&mut db, 3);

    // ── ① 失败路径优先：写到一个不可能成功的路径 ──
    //
    // 目录不存在 → 写文件必然失败。此时**待导出队列必须原封不动**。
    // 若实现先 mark_exported 再写文件，这 3 条会永久丢失（既没导出成功、
    // 又不在待导出队列里），且没有任何测试会报错。
    let bad_path = dir.path().join("no-such-dir").join("out.pesync");
    let err = export_bundle(&mut db, "pw", &bad_path).unwrap_err();
    assert_eq!(
        pending_export(&db).unwrap().len(),
        3,
        "写文件失败后待导出队列必须不变（顺序不能反）: {err:?}"
    );
    assert!(!bad_path.exists(), "失败时不该留下半个文件");

    // ── ② 成功路径 ──
    let good_path = dir.path().join("out.pesync");
    let report = export_bundle(&mut db, "pw", &good_path).unwrap();
    assert_eq!(report.change_count, 3, "导出条数必须等于待导出条数");
    assert_eq!(report.path, good_path);
    assert!(good_path.exists(), "文件必须真的写出来");
    assert!(
        pending_export(&db).unwrap().is_empty(),
        "导出成功后待导出队列必须清空"
    );

    // ── ③ 包能读回，且 header 与 payload 自洽 ──
    let bytes = std::fs::read(&good_path).unwrap();
    let (header, payload) = open("pw", &bytes).unwrap();
    assert_eq!(header.change_count, 3, "header 自述条数必须与实际一致");
    assert!(
        header.base_lamport >= 3,
        "base_lamport 必须反映导出时的时钟"
    );
    assert!(!header.device_id.is_empty(), "header 必须带 device_id");

    let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    let changes = json["changes"]
        .as_array()
        .expect("payload 必须有 changes 数组");
    assert_eq!(changes.len(), 3, "payload 里的变更条数必须与 header 一致");
    // bundle 里禁止出现本地整数主键
    for c in changes {
        assert!(c.get("id").is_none(), "bundle 不得包含本地整数主键 id");
        assert!(c.get("uid").is_some(), "每条变更必须带 uid");
    }

    // ── ④ 错误口令解不开（证明真的加密了，不是明文） ──
    assert!(open("wrong", &bytes).is_err(), "包必须是加密的");
    // 明文里不得出现实体 uid
    let raw = String::from_utf8_lossy(&bytes);
    assert!(!raw.contains("cfg0"), "明文泄漏了 payload 内容");

    // ⑤ 补一条新变更后，只导出新的那条
    seed(&mut db, 1);
    let report2 = export_bundle(&mut db, "pw", &dir.path().join("out2.pesync")).unwrap();
    assert_eq!(report2.change_count, 1, "只导出新增的变更");
}

// 让 `BundleHeader` 的 import 不被 clippy 判为未使用（类型仅用于文档性断言）
#[allow(dead_code)]
fn _assert_header_type(_: &BundleHeader) {}
