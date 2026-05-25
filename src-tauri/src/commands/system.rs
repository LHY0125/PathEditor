use winreg::enums::*;
use winreg::RegKey;

/// 检测当前进程是否有管理员权限（尝试写入系统注册表键）
#[tauri::command]
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
#[tauri::command]
pub fn validate_path(path: &str) -> bool {
    if path.contains('%') {
        return true;
    }
    std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

/// 展开路径中的环境变量（如 %JAVA_HOME%\bin → C:\Program Files\Java\jdk-17\bin）
#[tauri::command]
pub fn expand_env_vars(path: &str) -> String {
    if !path.contains('%') {
        return path.to_string();
    }

    // 转为 UTF-16 宽字符串
    let wide_path: Vec<u16> = path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    // 先查询需要的缓冲区大小 (lpDst=NULL)
    let required = unsafe {
        ExpandEnvironmentStringsW(wide_path.as_ptr(), std::ptr::null_mut(), 0)
    };

    if required == 0 {
        return path.to_string();
    }

    // 实际展开
    let mut buffer: Vec<u16> = vec![0; required as usize];
    let result = unsafe {
        ExpandEnvironmentStringsW(wide_path.as_ptr(), buffer.as_mut_ptr(), required)
    };

    if result == 0 {
        return path.to_string();
    }

    // 转回 UTF-8 (去掉结尾 null)
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}

/// 广播环境变量更改通知（WM_SETTINGCHANGE）
#[tauri::command]
pub fn broadcast_env_change() {
    const HWND_BROADCAST: isize = 0xFFFF;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;

    let env_str: Vec<u16> = "Environment\0".encode_utf16().collect();

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
    fn ExpandEnvironmentStringsW(
        lpSrc: *const u16,
        lpDst: *mut u16,
        nSize: u32,
    ) -> u32;

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
