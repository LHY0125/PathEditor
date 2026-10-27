#![cfg(windows)]
// 一文件一测试（SQLCipher）。`.pebak` 走 DPAPI，因此整体限定 Windows。
use path_editor_core::platform::backup::{EnvBackupHives, EnvBackupPayload, EnvBackupVar};
use path_editor_core::db::backups::{
    index_backups, list_backups, read_env_backup_any, read_pebak_header, write_env_backup_pebak,
    HEADER_LEN,
};
use path_editor_core::db::crypto::{key_hex, load_or_create_key};
use path_editor_core::db::Db;
use path_editor_core::EnvValueKind;
use tempfile::TempDir;

#[test]
fn pebak_backup_is_indexed_without_decrypting() {
    let dir = TempDir::new().unwrap();

    let key = load_or_create_key(dir.path()).unwrap();
    let mut db = Db::open_encrypted(&dir.path().join("b.db"), &key_hex(&key)).unwrap();
    db.migrate_to_current().unwrap();

    let backup_dir = dir.path().join("backups");
    // `EnvBackupPayload` 没有 `empty_for_test` 构造器，显式构造
    let payload = EnvBackupPayload {
        captured_at: 1_700_000_000_000,
        hives: EnvBackupHives {
            system: vec![],
            user: vec![
                EnvBackupVar {
                    name: "A".into(),
                    kind: EnvValueKind::String,
                    value: "1".into(),
                    revision: "r1".into(),
                },
                EnvBackupVar {
                    name: "B".into(),
                    kind: EnvValueKind::ExpandString,
                    value: "%A%;2".into(),
                    revision: "r2".into(),
                },
            ],
        },
    };
    let path = write_env_backup_pebak(&backup_dir, &payload, &key).unwrap();
    assert!(
        path.extension().unwrap().eq_ignore_ascii_case("pebak"),
        "扩展名必须是 .pebak"
    );

    // 文件头必须是 magic，且不含明文标记
    let raw = std::fs::read(&path).unwrap();
    assert_eq!(&raw[..5], b"PEBK1", "magic 应为 PEBK1");
    assert!(raw.len() > HEADER_LEN, "文件必须包含加密正文");
    let as_text = String::from_utf8_lossy(&raw);
    assert!(!as_text.contains("%A%;2"), "正文明文不得出现在 .pebak 里");
    assert!(!as_text.contains("\"hives\""), "JSON 结构不得以明文出现");

    // 头部可在不解密的情况下读出变量数
    let header = read_pebak_header(&path).unwrap();
    assert_eq!(header.variable_count, 2, "变量数应来自文件头");
    assert_eq!(header.created_at, 1_700_000_000_000);

    // 索引：只读头，不解密
    let n = index_backups(&mut db, &backup_dir).unwrap();
    assert_eq!(n, 1);
    let list = list_backups(&db).unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].encrypted);
    assert_eq!(list[0].variable_count, 2, "变量数应来自文件头，无需解密");
    assert_eq!(list[0].kind, "env");

    // 重复索引必须幂等（同一文件不产生第二行）
    assert_eq!(index_backups(&mut db, &backup_dir).unwrap(), 1);
    assert_eq!(
        list_backups(&db).unwrap().len(),
        1,
        "重复索引不得产生重复行"
    );

    // 解密读回：内容与写入一致
    let restored = read_env_backup_any(&path, &key).unwrap();
    assert_eq!(restored, payload);

    // 用另一份密钥（不同 salt）读取 → 哈希不一致，必须报错
    let other_dir = TempDir::new().unwrap();
    let other_key = load_or_create_key(other_dir.path()).unwrap();
    let err = read_env_backup_any(&path, &other_key);
    assert!(
        err.is_err(),
        "salt 不同时 payload_hash 校验应失败（防止跨安装误读索引）"
    );
}
