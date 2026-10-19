// M0 探针：DACL 相关符号在 windows-sys 中可编译（一文件一测试，见 p1_open.rs 说明）。
#![cfg(windows)]

#[test]
fn dacl_api_symbols_compile() {
    use windows_sys::Win32::Security::Authorization::{
        GetNamedSecurityInfoW, SetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    let _ = SE_FILE_OBJECT;
    let _ = GetNamedSecurityInfoW;
    let _ = SetNamedSecurityInfoW;
}
