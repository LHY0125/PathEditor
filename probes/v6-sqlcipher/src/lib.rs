//! v6 SQLCipher 探针：只验证构建与加密行为，不进入生产代码。

use rusqlite::Connection;
use std::path::Path;

/// 只打开连接并设置 key，不触碰数据库页；用于错误密钥测试。
pub fn open_with_key(path: &Path, key_hex: &str) -> rusqlite::Result<Connection> {
    assert_eq!(key_hex.len(), 64, "密钥必须是 32 字节（64 个 hex 字符）");
    assert!(
        key_hex.chars().all(|c| c.is_ascii_hexdigit()),
        "密钥必须是十六进制字符串"
    );
    let conn = Connection::open(path)?;
    // SQLCipher 的 key PRAGMA 必须用双引号包住 x'...'，不能用参数绑定。
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key_hex))?;
    Ok(conn)
}

/// 打开、校验密钥，然后设置 v6 的 PRAGMA。
pub fn open_encrypted_with(
    path: &Path,
    key_hex: &str,
    memory_security: bool,
) -> rusqlite::Result<Connection> {
    let conn = open_with_key(path, key_hex)?;
    // 先做一次只读查询，让 SQLCipher 用 HMAC 校验密钥；错误密钥在这里返回 Err，
    // 不会先被 journal_mode=WAL 的写操作触发 panic。
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })?;
    let memory = if memory_security { "ON" } else { "OFF" };
    conn.execute_batch(&format!(
        "PRAGMA cipher_memory_security = {memory};
         PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;"
    ))?;
    Ok(conn)
}

/// 默认开启 cipher_memory_security 的便捷入口。
pub fn open_encrypted(path: &Path, key_hex: &str) -> rusqlite::Result<Connection> {
    open_encrypted_with(path, key_hex, true)
}

#[cfg(windows)]
pub mod dpapi {
    //! Windows DPAPI 探针：验证 v6 的本地密钥/备份加密路径可用。

    use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        }
    }

    /// 用当前 Windows 用户作用域的 DPAPI 加密。
    pub fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
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
            return Err(format!(
                "CryptProtectData 失败: {}",
                std::io::Error::last_os_error()
            ));
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
        unsafe {
            LocalFree(output.pbData as HLOCAL);
        }
        Ok(bytes)
    }

    /// 解密 DPAPI 密文。
    pub fn unprotect(cipher: &[u8]) -> Result<Vec<u8>, String> {
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
            return Err(format!(
                "CryptUnprotectData 失败: {}",
                std::io::Error::last_os_error()
            ));
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
        unsafe {
            LocalFree(output.pbData as HLOCAL);
        }
        Ok(bytes)
    }
}
