//! 密钥文件的 DACL 加固。
//!
//! **为什么 DPAPI 之外还需要 DACL**：
//!
//! | 威胁 | DPAPI | DACL |
//! | --- | --- | --- |
//! | 同机**另一个用户读取**密钥内容 | ✅ 挡（解不开 blob） | — |
//! | 同机**另一个用户删除/替换**密钥文件 | ❌ 不挡 | ✅ 挡 |
//!
//! 缺失 DACL 的后果不是信息泄露，而是**拒绝服务**：密钥文件被替换或删除后，
//! 进程下次启动发现「文件在但解不开」→ 按返回 `Crypto` 错误、进入降级模式，
//! **历史永久不可读且用户无法自愈**。
//!
//! 实现要点：
//! - DACL 用 SDDL 描述：`D:(A;;GA;;;SY)(A;;GA;;;<current-user-SID>)` ——
//!   只授权 SYSTEM 与当前用户完全访问；SDDL 由系统解析，不手工构造 ACL 结构。
//! - 写密钥后**立即**设置；失败则删除半成品并返回 `Crypto` 错误
//!   （宁可不写，也不留下权限过宽的密钥文件）。
//! - 启动时**只校验不修改**：过宽仅 `log::warn!`，避免误伤用户自建目录。

use crate::domain::error::{CoreError, ErrorCode};
use std::path::Path;

/// 取当前进程用户的 SID 字符串。
fn current_user_sid_string() -> Result<Vec<u16>, CoreError> {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    // SAFETY: GetCurrentProcess 返回伪句柄，无需释放；OpenProcessToken 成功后
    // token 句柄由下面的 CloseHandle 释放。
    let mut token: HANDLE = std::ptr::null_mut();
    let ok = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
    if ok == 0 {
        return Err(win_err("OpenProcessToken"));
    }

    // 两段式调用：先问长度，再取数据（TOKEN_USER 含变长 SID）
    let mut len: u32 = 0;
    // SAFETY: 传入 null 缓冲区只为查询所需长度；失败是预期的 ERROR_INSUFFICIENT_BUFFER。
    unsafe {
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut len);
    }
    if len == 0 {
        // SAFETY: token 由本函数创建且未被转移。
        unsafe { CloseHandle(token) };
        return Err(win_err("GetTokenInformation(size)"));
    }

    let mut buf = vec![0u8; len as usize];
    // SAFETY: buf 的长度由上一次调用给出；token 有效。
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buf.as_mut_ptr() as *mut core::ffi::c_void,
            len,
            &mut len,
        )
    };
    // SAFETY: token 由本函数创建且未被转移。
    unsafe { CloseHandle(token) };
    if ok == 0 {
        return Err(win_err("GetTokenInformation"));
    }

    // SAFETY: 成功时 buf 的首部是一个对齐的 TOKEN_USER。
    let token_user = unsafe { &*(buf.as_ptr() as *const TOKEN_USER) };
    let mut sid_str: *mut u16 = std::ptr::null_mut();
    // SAFETY: User.Sid 是系统填充的有效 PSID；sid_str 由 LocalAlloc 分配。
    let ok = unsafe { ConvertSidToStringSidW(token_user.User.Sid, &mut sid_str) };
    if ok == 0 {
        return Err(win_err("ConvertSidToStringSidW"));
    }

    let mut out = Vec::new();
    // SAFETY: sid_str 是 NUL 结尾的宽字符串，由系统分配。
    unsafe {
        let mut p = sid_str;
        while *p != 0 {
            out.push(*p);
            p = p.add(1);
        }
        windows_sys::Win32::Foundation::LocalFree(
            sid_str as windows_sys::Win32::Foundation::HLOCAL,
        );
    }
    Ok(out)
}

/// 设置密钥文件的 DACL：**仅当前用户 + SYSTEM**。
///
/// 失败时调用方**必须删除半成品文件**（本函数不删除，由 `create_key` 负责，
/// 因为它知道文件路径的完整生命周期）。
pub fn set_key_dacl(path: &Path) -> Result<(), CoreError> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows_sys::Win32::Security::{
        SetFileSecurityW, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    };

    let sid = current_user_sid_string()?;
    // SDDL：D: = DACL 段；每个 (A;;GA;;;principal) 是一条允许-完全访问 ACE。
    // SY = SYSTEM。用显式 SID 而非 OW（Owner Rights），因为文件属主在创建后
    // 可能被改；显式绑定当前用户才符合「仅此用户 + SYSTEM」。
    let sddl = format!("D:(A;;GA;;;SY)(A;;GA;;;{})", String::from_utf16_lossy(&sid));
    let sddl_w: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
    let path_w: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: sddl_w 是 NUL 结尾的有效宽字符串。
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl_w.as_ptr(),
            1, // SDDL_REVISION_1
            &mut sd,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(win_err(
            "ConvertStringSecurityDescriptorToSecurityDescriptorW",
        ));
    }

    // 用 SetFileSecurityW 而非 SetNamedSecurityInfoW：前者直接接受整个安全描述符，
    // 后者要求单独传 DACL 指针（传 null 等于不设 DACL，是静默无效的写法）。
    // SAFETY: path_w 是有效宽字符串；sd 由上面的调用分配且含 DACL。
    let ok = unsafe { SetFileSecurityW(path_w.as_ptr(), DACL_SECURITY_INFORMATION, sd) };
    let os_err = std::io::Error::last_os_error();

    // SAFETY: sd 由 ConvertStringSecurityDescriptorToSecurityDescriptorW 用 LocalAlloc 分配。
    unsafe { LocalFree(sd as windows_sys::Win32::Foundation::HLOCAL) };

    if ok == 0 {
        return Err(CoreError::new(
            ErrorCode::Crypto,
            "set_key_dacl",
            format!("设置密钥文件 DACL 失败: {os_err}"),
        ));
    }
    Ok(())
}

/// 读取并校验密钥文件的 DACL：只允许**当前用户**与 **SYSTEM**。
///
/// 返回 `Ok(true)` 表示权限受限；`Ok(false)` 表示存在非预期主体。
/// **只读不写**。
pub fn key_dacl_is_restricted(path: &Path) -> Result<bool, CoreError> {
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};

    let path_w: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();

    // SAFETY: path_w 是有效的 NUL 结尾宽字符串；其余出参允许为 null（只要了 DACL）。
    let rc = unsafe {
        GetNamedSecurityInfoW(
            path_w.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut sd,
        )
    };
    if rc != 0 {
        return Err(CoreError::new(
            ErrorCode::Crypto,
            "key_dacl_check",
            format!("读取密钥文件 ACL 失败（Win32 错误 {rc}）"),
        ));
    }

    let mut sddl_ptr: *mut u16 = std::ptr::null_mut();
    // SAFETY: sd 由 GetNamedSecurityInfoW 分配；只要 DACL 段的字符串形式。
    let ok = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            sd,
            1,
            DACL_SECURITY_INFORMATION,
            &mut sddl_ptr,
            std::ptr::null_mut(),
        )
    };
    // SAFETY: sd 由 GetNamedSecurityInfoW 用 LocalAlloc 分配。
    unsafe {
        windows_sys::Win32::Foundation::LocalFree(sd as windows_sys::Win32::Foundation::HLOCAL)
    };
    if ok == 0 {
        return Err(win_err(
            "ConvertSecurityDescriptorToStringSecurityDescriptorW",
        ));
    }

    let mut dacl_sddl = Vec::new();
    // SAFETY: sddl_ptr 是 NUL 结尾的宽字符串，由系统分配。
    unsafe {
        let mut p = sddl_ptr;
        while *p != 0 {
            dacl_sddl.push(*p);
            p = p.add(1);
        }
        windows_sys::Win32::Foundation::LocalFree(
            sddl_ptr as windows_sys::Win32::Foundation::HLOCAL,
        );
    }
    let dacl = String::from_utf16_lossy(&dacl_sddl).to_uppercase();

    // 任一「广泛主体」出现即判为过宽。
    const BROAD_PRINCIPALS: &[&str] = &[";;;WD)", ";;;BU)", ";;;AU)", ";;;BG)", ";;;PS)"];
    Ok(!BROAD_PRINCIPALS.iter().any(|p| dacl.contains(p)))
}

/// 把 `GetLastError` 包装为结构化错误。
fn win_err(op: &'static str) -> CoreError {
    CoreError::new(
        ErrorCode::Crypto,
        op,
        format!("{} 失败: {}", op, std::io::Error::last_os_error()),
    )
}
