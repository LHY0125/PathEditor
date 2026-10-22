#![cfg(windows)]
// 一文件一测试：同一进程内跑第二个 SQLCipher 测试会 STATUS_STACK_OVERFLOW
// （GNU/MSVC 均已复现）。见测试约束说明。
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::Db;
use tempfile::TempDir;

#[test]
fn key_is_created_once_and_persists_across_reloads() {
    // 用独立目录，避免污染真实 ~/.patheditor
    let dir = TempDir::new().unwrap();

    let a = load_or_create_key(dir.path()).expect("首次创建失败");
    let b = load_or_create_key(dir.path()).expect("二次读取失败");
    assert_eq!(a.db_key, b.db_key, "密钥必须稳定");
    assert_eq!(a.hash_salt, b.hash_salt, "salt 必须稳定");
    assert_ne!(a.db_key, [0u8; 32], "密钥不能全零");
    assert_ne!(a.hash_salt, [0u8; 16], "salt 不能全零");

    let db_path = dir.path().join("k.db");
    let mut db = Db::open_encrypted(&db_path, &key_hex(&a)).expect("打开失败");
    db.migrate_to_current().expect("迁移失败");

    // 重新读取密钥后仍能打开
    let c = load_or_create_key(dir.path()).unwrap();
    Db::open_encrypted(&db_path, &key_hex(&c)).expect("重开后失败");

    // 密钥文件确实落在 <home>/keys/ 下（不是别处）
    assert!(
        dir.path().join("keys").join("db.key.dpapi").exists(),
        "密钥文件路径应为 <home>/keys/db.key.dpapi"
    );

    // 错误密钥必须打不开（DPAPI 密钥与库密钥是两回事）
    let other = TempDir::new().unwrap();
    let d = load_or_create_key(other.path()).unwrap();
    assert_ne!(a.db_key, d.db_key, "不同目录必须生成不同密钥");
    assert!(
        Db::open_encrypted(&db_path, &key_hex(&d)).is_err(),
        "用另一份密钥必须打不开"
    );

    // 密钥文件 DACL 必须只授权当前用户 + SYSTEM。
    // 计划把这个断言放在 db_key_lifecycle 里；但它不触碰 SQLCipher，
    // 因此与「一文件一测试」的 SQLCipher 约束无关，可以同文件。
    let key_path = dir.path().join("keys").join("db.key.dpapi");
    assert!(
        path_editor_core::db::dacl::key_dacl_is_restricted(&key_path).unwrap(),
        "密钥文件 DACL 必须仅授权当前用户与 SYSTEM"
    );
}
