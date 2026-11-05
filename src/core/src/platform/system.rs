use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};
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
    // 与 PATH 读写共用同一个常量：两处各写一份键路径，改名时会漂移。
    hklm.open_subkey_with_flags(crate::platform::registry::SYS_REG_PATH, KEY_WRITE)
        .is_ok()
}

/// PATH 语境下允许展开的系统变量。
///
/// 这些名字既不是密钥，也常出现在 PATH 里；其余变量若命中
/// [`crate::domain::env_var::is_sensitive`] 则**不展开**——展开结果会经
/// `expand_env_vars` IPC 回到前端，等于绕过 `reveal_env_var` 的敏感掩码。
const PATH_EXPAND_ALLOWED: &[&str] = &[
    "SystemRoot",
    "windir",
    "SystemDrive",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "ProgramW6432",
    "CommonProgramFiles",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "HOMEDRIVE",
    "HOMEPATH",
    "TEMP",
    "TMP",
    "USERNAME",
    "PUBLIC",
];

/// 路径里是否引用了「敏感且不在白名单」的 `%NAME%`。
fn contains_sensitive_reference(path: &str) -> bool {
    let mut rest = path;
    while let Some(start) = rest.find('%') {
        let tail = &rest[start + 1..];
        let Some(end) = tail.find('%') else {
            break;
        };
        let name = &tail[..end];
        if !name.is_empty()
            && !PATH_EXPAND_ALLOWED
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(name))
            && crate::domain::env_var::is_sensitive(name)
        {
            return true;
        }
        rest = &tail[end + 1..];
    }
    false
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
/// 包含 % 的路径（环境变量路径）无法展开，返回原始路径
pub fn expand_env_vars(path: &str) -> String {
    if !path.contains('%') {
        return path.to_string();
    }
    // 敏感变量不展开：返回值会经 IPC 到前端，必须与 `env list` 的掩码口径一致。
    if contains_sensitive_reference(path) {
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
    //        且两个指针指向不同的内存区域，不存在重叠
    let mut buffer = vec![0_u16; required as usize];
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

/// `WM_SETTINGCHANGE` 的 lParam 载体：NUL 结尾的 `"Environment"`。
///
/// 接收窗口在处理消息期间读取该缓冲区，因此它必须比「发送」活得久。
/// 内容恒定，进程级静态分配一次即可 —— 后台线程不持有临时 `Vec`，
/// 线程被提前结束时也不会留下悬垂指针。
static ENV_SETTING_WIDE: std::sync::LazyLock<Vec<u16>> =
    std::sync::LazyLock::new(|| "Environment\0".encode_utf16().collect());

/// 在途广播计数。`broadcast_env_change` 返回后仍可能有线程在发送，
/// 短命进程（CLI）退出前必须 [`flush_env_broadcasts`]，否则通知会被截断。
static PENDING_BROADCASTS: std::sync::LazyLock<(std::sync::Mutex<usize>, std::sync::Condvar)> =
    std::sync::LazyLock::new(|| (std::sync::Mutex::new(0), std::sync::Condvar::new()));

/// 释放一个在途名额并唤醒等待者。
fn release_broadcast_slot() {
    let (lock, cvar) = &*PENDING_BROADCASTS;
    let mut pending = lock.lock().unwrap_or_else(|e| e.into_inner());
    *pending = pending.saturating_sub(1);
    cvar.notify_all();
}

/// 等待所有在途的环境变更广播发送完毕。
///
/// 单次发送最长 5 秒（窗口挂起时），因此本函数有界。
/// **GUI 常驻进程无需调用**；CLI 在 `main` 返回前调用，保持
/// 「命令成功 ⇒ 通知已发出」的原有语义（异步化不得让短命进程丢掉通知）。
pub fn flush_env_broadcasts() {
    let (lock, cvar) = &*PENDING_BROADCASTS;
    let mut pending = lock.lock().unwrap_or_else(|e| e.into_inner());
    while *pending > 0 {
        pending = cvar.wait(pending).unwrap_or_else(|e| e.into_inner());
    }
}

/// 广播环境变量更改通知（`WM_SETTINGCHANGE`）。
///
/// **必须非阻塞**：`SendMessageTimeoutW(HWND_BROADCAST, …)` 对每个顶层窗口依次同步发送，
/// 单次耗时是各窗口处理时间的累加；实测本机 3.3–4.0 秒（`SMTO_ABORTIFHUNG` 只在窗口
/// 真正挂起时才提前放弃，正常但繁忙的窗口照样排队）。这条广播在保存编排里、且调用方
/// 持有 DB 互斥量，同步执行会让「保存」固定多出数秒——因此放到独立线程，通知本身是
/// best-effort，发送结果只写日志。
///
/// 超时仍保留 5 秒：它现在只约束后台线程的生命周期，不再影响调用方。
pub fn broadcast_env_change() {
    let lparam = ENV_SETTING_WIDE.as_ptr() as isize;
    {
        let (lock, _) = &*PENDING_BROADCASTS;
        *lock.lock().unwrap_or_else(|e| e.into_inner()) += 1;
    }
    let spawned = std::thread::Builder::new()
        .name("patheditor-env-broadcast".into())
        .spawn(move || {
            // SAFETY: lparam 指向进程内静态的 NUL 结尾 UTF-16 字符串，生命周期覆盖整个进程；
            //        HWND_BROADCAST 是合法句柄；lpdwResult 为 null 表示不需要返回值。
            let result = unsafe {
                SendMessageTimeoutW(
                    HWND_BROADCAST as _,
                    WM_SETTINGCHANGE,
                    0,
                    lparam,
                    SMTO_ABORTIFHUNG,
                    5000,
                    std::ptr::null_mut(),
                )
            };

            if result == 0 {
                log::warn!("广播 WM_SETTINGCHANGE 失败或超时");
            } else {
                log::info!("已广播环境变量更改通知");
            }
            release_broadcast_slot();
        });
    if spawned.is_err() {
        // 线程都起不来说明进程已处于资源耗尽状态；通知是 best-effort，记日志后放弃，
        // 但不能让计数泄漏（否则 CLI 的 flush 会永久等待）。
        log::warn!("无法创建环境变更广播线程，本次通知已跳过");
        release_broadcast_slot();
    }
}

/// 尝试成为单实例（命名互斥量，当前用户会话作用域）。
///
/// 返回 `true` 表示本进程是唯一实例；`false` 表示已有实例在运行。
/// 成功时**故意不释放句柄**：互斥量由进程生命周期持有，进程退出时由系统关闭。
/// 双开会重复跑启动序列、并发写同一个 DB 与注册表。
pub fn acquire_single_instance(name: &str) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};

    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: wide 是 NUL 结尾的宽字符串；lpMutexAttributes 为 null 表示
    // 默认安全描述符；bInitialOwner = 0 表示创建后不立即持有。
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
    if handle.is_null() {
        // 创建失败时宁可允许启动，也不要因为一次 API 失败把用户锁在门外。
        log::warn!("创建单实例互斥量失败，按多实例继续");
        return true;
    }
    // SAFETY: handle 刚由 CreateMutexW 创建；超时 0 表示只探测不等待。
    let wait = unsafe { WaitForSingleObject(handle, 0) };
    if wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED {
        true
    } else {
        // SAFETY: handle 由本函数创建且不再使用。
        unsafe { CloseHandle(handle) };
        false
    }
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
        // 在任意机器上都不应 panic；具体权限值取决于当前进程。
        let _result = check_admin();
    }

    /// 广播必须非阻塞：保存编排在持有 DB 互斥量的情况下调用它。
    ///
    /// 可证伪性：把 `broadcast_env_change` 改回同步 `SendMessageTimeoutW`，
    /// 本机实测该调用单次 3.3–4.0 秒，本断言立即失败（阈值 500 ms 留足线程
    /// 创建与调度抖动）。这条契约是「保存秒开」的唯一保证，不能只靠注释。
    #[test]
    fn broadcast_env_change_does_not_block_caller() {
        let started = std::time::Instant::now();
        broadcast_env_change();
        let elapsed = started.elapsed();
        assert!(
            elapsed < std::time::Duration::from_millis(500),
            "广播必须在独立线程执行；本次调用耗时 {elapsed:?}"
        );
    }
}
