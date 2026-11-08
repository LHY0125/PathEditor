#![cfg(windows)]
// 数据库不加密：本文件守的是「本机盐只创建一次、稳定、跨目录不同」与 DACL。
use path_editor_core::db::crypto::load_or_create_key;
use tempfile::TempDir;

#[test]
fn salt_is_created_once_and_persists_across_reloads() {
    // 用独立目录，避免污染真实 ~/.patheditor
    let dir = TempDir::new().unwrap();

    let a = load_or_create_key(dir.path()).expect("首次创建失败");
    let b = load_or_create_key(dir.path()).expect("二次读取失败");
    assert_eq!(a.hash_salt, b.hash_salt, "salt 必须稳定");
    assert_ne!(a.hash_salt, [0u8; 16], "salt 不能全零");

    // 密钥文件确实落在 <home>/keys/ 下（不是别处）
    let key_path = dir.path().join("keys").join("db.key.dpapi");
    assert!(
        key_path.exists(),
        "密钥文件路径应为 <home>/keys/db.key.dpapi"
    );

    // 不同目录必须生成不同的盐：否则两份安装的 .pebak 索引会互相误认
    let other = TempDir::new().unwrap();
    let d = load_or_create_key(other.path()).unwrap();
    assert_ne!(a.hash_salt, d.hash_salt, "不同目录必须生成不同盐");

    // 密钥文件 DACL 必须只授权当前用户 + SYSTEM。
    // 失败时把实际 SDDL 打出来：CI runner 的令牌/临时目录 ACL 与开发机不同，
    // 只有「哪个主体多出来了」才能判断是实现问题还是环境差异。
    let sddl = path_editor_core::db::dacl::key_dacl_sddl(&key_path).unwrap();
    assert!(
        path_editor_core::db::dacl::key_dacl_is_restricted(&key_path).unwrap(),
        "密钥文件 DACL 必须仅授权当前用户与 SYSTEM，实际 SDDL: {sddl}"
    );
}
