// M0 探针：DPAPI 加解密往返（一文件一测试，见 p1_open.rs 说明）。
#![cfg(windows)]

#[test]
fn dpapi_roundtrip() {
    let plain = b"path-editor-secret-probe";
    let cipher = v6_sqlcipher_probe::dpapi::protect(plain).expect("CryptProtectData 失败");
    assert_ne!(&cipher[..], &plain[..], "密文不能等于明文");
    let back = v6_sqlcipher_probe::dpapi::unprotect(&cipher).expect("CryptUnprotectData 失败");
    assert_eq!(back, plain);
}
