//! 同步包加密：往返、错误口令、篡改密文、篡改 header 四种情况。
//!
//! 一文件一测试：见 M0 §5.3。
use path_editor_core::db::sync::bundle::{open, seal, BundleHeader};
use path_editor_core::ErrorCode;

#[test]
fn bundle_roundtrips_and_rejects_wrong_passphrase_and_tampering() {
    let header = BundleHeader {
        schema_version: 1,
        device_id: "dev-1".into(),
        exported_at: 1_700_000_000,
        base_lamport: 5,
        change_count: 1,
    };
    let payload = br#"{"changes":[]}"#;

    let sealed = seal("correct horse", &header, payload).unwrap();
    assert!(sealed.starts_with(b"PESYNC1"), "必须带 magic");

    // ① 口令错误 → Crypto 错误（不是 panic，不是返回空）
    let err = open("wrong", &sealed).unwrap_err();
    assert_eq!(err.code, ErrorCode::Crypto, "错误口令必须是 Crypto");

    // ② 篡改密文任一字节 → 拒绝（改倒数第 20 字节，落在 ciphertext 内）
    let mut tampered = sealed.clone();
    let n = tampered.len();
    tampered[n - 20] ^= 0xFF;
    assert_eq!(
        open("correct horse", &tampered).unwrap_err().code,
        ErrorCode::Crypto,
        "篡改密文必须被 AEAD 拒绝"
    );

    // ③ 篡改 header（影响 AAD）→ 拒绝
    //
    // header JSON 紧跟在 magic(8) + version(4) + header_len(4) 之后。
    // 改 JSON 内部一个字节，AAD 随之变化，AEAD 校验必然失败。
    let header_start = 16usize;
    let mut header_tampered = sealed.clone();
    // 把 `"schema_version"` 里的 `s` 换成 `t`
    assert_eq!(header_tampered[header_start], b'{');
    header_tampered[header_start + 2] = b't';
    assert!(
        open("correct horse", &header_tampered).is_err(),
        "header 被改必须拒绝"
    );

    // ④ 正确口令 → 原样返回
    let (h2, p2) = open("correct horse", &sealed).unwrap();
    assert_eq!(h2, header, "header 必须完整还原");
    assert_eq!(p2, payload, "payload 必须逐字节一致");

    // ⑤ magic 不对 → 拒绝（且不能 panic）
    let mut bad_magic = sealed.clone();
    bad_magic[0] = b'X';
    assert!(open("correct horse", &bad_magic).is_err());

    // ⑥ 截断的包 → 拒绝（不能 panic，不能越界）
    assert!(open("correct horse", &sealed[..20]).is_err());
}
