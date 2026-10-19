#![cfg(windows)]

#[test]
fn dpapi_roundtrip() {
    let plain = b"path-editor-secret-probe";
    let cipher = v6_sqlcipher_probe::dpapi::protect(plain).expect("CryptProtectData 失败");
    assert_ne!(&cipher[..], &plain[..], "密文不能等于明文");
    let back = v6_sqlcipher_probe::dpapi::unprotect(&cipher).expect("CryptUnprotectData 失败");
    assert_eq!(back, plain);
}

#[test]
fn dacl_api_symbols_compile() {
    use windows_sys::Win32::Security::Authorization::{
        GetNamedSecurityInfoW, SetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    let _ = SE_FILE_OBJECT;
    let _ = GetNamedSecurityInfoW;
    let _ = SetNamedSecurityInfoW;
}
