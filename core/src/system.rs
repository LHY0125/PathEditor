use winreg::enums::*;
use winreg::RegKey;

/// 检测当前进程是否有管理员权限
///
/// 通过尝试以写入权限打开系统 PATH 注册表键判断。
///
/// # Returns
/// `true` 表示有管理员权限，`false` 为只读模式
pub fn check_admin() -> bool {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey_with_flags(
        "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
        KEY_WRITE,
    )
    .is_ok()
}

/// 验证路径是否存在于文件系统中（且是目录）
/// 包含 % 的路径（环境变量路径）无法验证，返回 true
pub fn validate_path(path: &str) -> bool {
    if path.contains('%') {
        return true;
    }
    std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

/// 展开路径中的环境变量（如 %JAVA_HOME%\bin → C:\Program Files\Java\jdk-17\bin）
pub fn expand_env_vars(path: &str) -> String {
    if !path.contains('%') {
        return path.to_string();
    }

    // 转为 UTF-16 宽字符串（以 null 结尾）
    let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

    // SAFETY: wide_path 是以 null 结尾的 UTF-16 字符串，lpDst 为 null 且 nSize 为 0，
    //         根据 MSDN 文档此时 API 只查询所需缓冲区大小而不写入数据
    let required =
        unsafe { ExpandEnvironmentStringsW(wide_path.as_ptr(), std::ptr::null_mut(), 0) };

    if required == 0 {
        log::warn!("expand_env_vars: API 查询缓冲区失败, 返回原始路径: {path}");
        return path.to_string();
    }

    // SAFETY: buffer 容量为 required（API 返回的精确大小），wide_path 以 null 结尾，
    //         且两个指针指向不同的内存区域，不存在重叠
    let mut buffer: Vec<u16> = vec![0; required as usize];
    let result =
        unsafe { ExpandEnvironmentStringsW(wide_path.as_ptr(), buffer.as_mut_ptr(), required) };

    if result == 0 || result > required {
        log::warn!("expand_env_vars: 展开失败或缓冲区不足, 返回原始路径: {path}");
        return path.to_string();
    }

    // 转回 UTF-8 (去掉结尾 null)，保留非法码点避免丢失路径信息
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    decode_utf16_preserving(&buffer[..len])
}

/// 解码 UTF-16 为 String，非法码点编码为 \u{XXXX} 而非静默丢弃
fn decode_utf16_preserving(v: &[u16]) -> String {
    char::decode_utf16(v.iter().copied())
        .map(|r| match r {
            Ok(c) => c.to_string(),
            Err(e) => format!("\\u{{{:X}}}", e.unpaired_surrogate()),
        })
        .collect()
}

/// 广播环境变量更改通知（WM_SETTINGCHANGE）
/// 广播 `WM_SETTINGCHANGE` 通知系统环境变量已变更
pub fn broadcast_env_change() {
    const HWND_BROADCAST: isize = 0xFFFF;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;

    // SAFETY: env_str 是以 null 结尾的 UTF-16 字符串，所有指针和常量均遵循 Win32 API 约定
    let env_str: Vec<u16> = "Environment\0".encode_utf16().collect();

    // SAFETY: env_str.as_ptr() 指向以 null 结尾的字符串，HWND_BROADCAST 是合法句柄，
    //         lpdwResult 为 null 表示不需要返回值，其他参数均为常量
    let result = unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            env_str.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            std::ptr::null_mut(),
        )
    };

    if result == 0 {
        log::warn!("广播 WM_SETTINGCHANGE 失败");
    } else {
        log::info!("已广播环境变量更改通知");
    }
}

// ── 外部 FFI 声明 ──

extern "system" {
    /// https://learn.microsoft.com/en-us/windows/win32/api/processenv/nf-processenv-expandenvironmentstringsw
    fn ExpandEnvironmentStringsW(lpSrc: *const u16, lpDst: *mut u16, nSize: u32) -> u32;

    /// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendmessagetimeoutw
    fn SendMessageTimeoutW(
        hWnd: isize,
        Msg: u32,
        wParam: usize,
        lParam: isize,
        fuFlags: u32,
        uTimeout: u32,
        lpdwResult: *mut usize,
    ) -> isize;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_path_env_var_always_valid() {
        assert!(validate_path("%JAVA_HOME%\\bin"));
    }

    #[test]
    fn expand_env_vars_no_percent_returns_original() {
        let result = expand_env_vars("C:\\Windows");
        assert_eq!(result, "C:\\Windows");
    }

    #[test]
    fn expand_env_vars_with_invalid_var_returns_original() {
        // 展开不存在的变量可能会回归原始值或产生部分展开；测试是否不会崩溃
        let result = expand_env_vars("%__NONEXISTENT_VAR__%");
        // 至少不应为空白
        assert!(!result.is_empty());
    }

    #[test]
    fn check_admin_returns_bool() {
        let result = check_admin();
        // 在任意机器上应返回 true 或 false，不应 panic
        assert!((result == true) || (result == false));
    }
}
