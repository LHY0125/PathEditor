//! Windows DPAPI 封装（用户作用域）。
//!
//! 移植自 M0 探针 `probes/v6-sqlcipher/src/lib.rs` 的 `dpapi` 模块 —— 该实现已在
//! GNU 与 MSVC 双平台实测通过（M0 结果 §二）。改动仅两处：
//! 1. 错误类型从 `String` 换成 `CoreError`（`ErrorCode::Crypto`），与其他模块统一；
//! 2. 补充中文文档注释。
//!
//! **作用域语义**：`CryptProtectData` 默认绑定当前 Windows 用户，换用户/换机器
//! 不可解密 —— 这是 spec D3 的刻意选择（免输入口令），文档必须写明后果。

use crate::error::{CoreError, ErrorCode};
use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

/// 构造 DPAPI 的输入 blob（不拥有数据，生命周期由调用方保证）。
fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    }
}

/// 用当前 Windows 用户作用域的 DPAPI 加密。
///
/// **从不弹 UI**（`CRYPTPROTECT_UI_FORBIDDEN`）：GUI/CLI 后台调用时不会卡住。
pub fn protect(plain: &[u8]) -> Result<Vec<u8>, CoreError> {
    let input = blob(plain);
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: input 指向有效切片；output 由 DPAPI 填充，随后用 LocalFree 释放。
    let ok = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(CoreError::new(
            ErrorCode::Crypto,
            "dpapi_protect",
            format!("DPAPI 加密失败: {}", std::io::Error::last_os_error()),
        ));
    }
    // SAFETY: 成功时 DPAPI 返回已初始化的缓冲区，长度由 cbData 给出。
    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    // SAFETY: pbData 由 CryptProtectData 用 LocalAlloc 分配，必须用 LocalFree 释放。
    unsafe {
        LocalFree(output.pbData as HLOCAL);
    }
    Ok(bytes)
}

/// 解密 DPAPI 密文。
///
/// 换用户/换机器时必然失败 —— 调用方应映射为 `ErrorCode::Crypto` 并提示
/// 「备份绑定当前 Windows 账户，无法在其它账户下解密」。
pub fn unprotect(cipher: &[u8]) -> Result<Vec<u8>, CoreError> {
    let input = blob(cipher);
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: 同上；ppszdatadescr 允许为 null。
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(CoreError::new(
            ErrorCode::Crypto,
            "dpapi_unprotect",
            format!(
                "DPAPI 解密失败（密钥属于其它 Windows 账户或机器）: {}",
                std::io::Error::last_os_error()
            ),
        ));
    }
    // SAFETY: 成功时 DPAPI 返回已初始化的缓冲区，长度由 cbData 给出。
    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    // SAFETY: pbData 由 CryptUnprotectData 用 LocalAlloc 分配，必须用 LocalFree 释放。
    unsafe {
        LocalFree(output.pbData as HLOCAL);
    }
    Ok(bytes)
}
