//! 同步包的加密格式。
//!
//! ## 布局
//!
//! ```text
//! magic "PESYNC1" (8) | version u32 (4) | header_len u32 (4) | header JSON
//! | salt (16) | nonce (12) | ciphertext | tag (16)
//! ```
//!
//! ## AAD 覆盖 header
//!
//! **这是本模块最容易被写错的一处。** 若只把 salt + nonce 作为 AAD，
//! 攻击者可以改 `change_count` 或 `device_id` 而不触发 AEAD 验证失败——
//! 包能解开、能往返，只是里面说的是假话。AAD 必须覆盖：
//!
//! ```text
//! magic || version || header_len || header JSON || salt || nonce
//! ```
//!
//! 把 salt 与 nonce 也放进去，是因为它们虽在明文里，被替换同样会改变解密语义。
//!
//! ## 口令处理
//!
//! 口令**不落盘**，错误文案**不得包含口令**——所有错误消息都是固定中文串。

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

use crate::error::{CoreError, ErrorCode};

/// 包魔数。
///
/// 写的是 `magic "PESYNC1" (8)`，但 `PESYNC1` 只有 **7 个字符**——
/// 这是规格里的一处笔误。取 8 字节定长（尾随 NUL）以便按 `u64` 对齐比较，
/// 并让「8 字节 magic」的文字描述成立。
const MAGIC: &[u8; 8] = b"PESYNC1\0";
/// 包格式版本（与 `CURRENT_SCHEMA_VERSION` 无关：前者是**文件格式**，后者是**库结构**）。
const FORMAT_VERSION: u32 = 1;
/// Argon2id 盐长度。
const SALT_LEN: usize = 16;
/// ChaCha20-Poly1305 nonce 长度。
const NONCE_LEN: usize = 12;
/// AEAD tag 长度。
const TAG_LEN: usize = 16;

/// 派生密钥长度。
const KEY_LEN: usize = 32;

/// 同步包的明文头（进 AAD 与落盘）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleHeader {
    /// 库结构版本（`CURRENT_SCHEMA_VERSION`）
    pub schema_version: u32,
    /// 导出方设备 id
    pub device_id: String,
    /// 导出时间（Unix 毫秒）
    pub exported_at: i64,
    /// 导出时的逻辑时钟（导入方据此提升自己的 lamport）
    pub base_lamport: i64,
    /// 变更条数（header 里的自述，导入时与实际 payload 交叉校验）
    pub change_count: u32,
}

/// Argon2id 派生 32 字节密钥。
///
/// 参数由 **写死**（m=65536 KiB, t=3, p=1）：它们决定 bundle 的
/// 跨设备兼容性，调高会让旧客户端解不开、调低会削弱抗暴力破解。
pub fn derive_key(passphrase: &str, salt: &[u8; SALT_LEN]) -> Result<[u8; KEY_LEN], CoreError> {
    let params = Params::new(65536, 3, 1, Some(KEY_LEN))
        .map_err(|_| CoreError::new(ErrorCode::Crypto, "derive_key", "口令派生参数不合法"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut out = [0u8; KEY_LEN];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut out)
        .map_err(|_| {
            // 文案固定：绝不把 passphrase 或派生中间值带进错误里
            CoreError::new(ErrorCode::Crypto, "derive_key", "口令派生失败")
        })?;
    Ok(out)
}

/// 把 header 与 payload 封装成加密包。
pub fn seal(passphrase: &str, header: &BundleHeader, payload: &[u8]) -> Result<Vec<u8>, CoreError> {
    let header_json = serde_json::to_vec(header).map_err(|e| {
        CoreError::new(ErrorCode::Crypto, "seal", format!("header 序列化失败: {e}"))
    })?;
    let header_len = u32::try_from(header_json.len())
        .map_err(|_| CoreError::new(ErrorCode::SyncFormat, "seal", "header 过长"))?;

    let salt = random_bytes::<SALT_LEN>()?;
    let nonce_bytes = random_bytes::<NONCE_LEN>()?;
    let key = derive_key(passphrase, &salt)?;

    // 前导段一次性拼好：它既是输出的一部分，也是 AAD 的一部分
    let mut prefix = Vec::with_capacity(8 + 4 + 4 + header_json.len() + SALT_LEN + NONCE_LEN);
    prefix.extend_from_slice(MAGIC);
    prefix.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    prefix.extend_from_slice(&header_len.to_le_bytes());
    prefix.extend_from_slice(&header_json);
    prefix.extend_from_slice(&salt);
    prefix.extend_from_slice(&nonce_bytes);

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: payload,
                aad: &prefix,
            },
        )
        .map_err(|_| CoreError::new(ErrorCode::Crypto, "seal", "加密失败"))?;

    // `encrypt` 返回的密文尾部已带 16 字节 tag
    prefix.extend_from_slice(&ciphertext);
    Ok(prefix)
}

/// 解密并校验一个包，返回 header 与 payload。
///
/// **所有失败都返回 `Crypto`**：错误口令、密文被改、header 被改，对调用方
/// 是同一件事——「这个包我打不开」。区分它们会泄露信息（例如「口令对了但
/// 密文坏了」暗示口令正确）。
pub fn open(passphrase: &str, bytes: &[u8]) -> Result<(BundleHeader, Vec<u8>), CoreError> {
    let (header, prefix, salt, nonce_bytes, ciphertext) = split(bytes)?;
    let key = derive_key(passphrase, &salt)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: &ciphertext,
                aad: &prefix,
            },
        )
        .map_err(|_| CoreError::new(ErrorCode::Crypto, "open", "口令错误或同步包已被篡改"))?;
    Ok((header, plaintext))
}

/// 切分包结构，返回 `(header, aad 前缀, salt, nonce, ciphertext)`。
#[allow(clippy::type_complexity)]
fn split(
    bytes: &[u8],
) -> Result<
    (
        BundleHeader,
        Vec<u8>,
        [u8; SALT_LEN],
        [u8; NONCE_LEN],
        Vec<u8>,
    ),
    CoreError,
> {
    let bad = |why: &str| CoreError::new(ErrorCode::SyncFormat, "open", why.to_string());

    // 固定头：magic(8) + version(4) + header_len(4)
    const FIXED: usize = 8 + 4 + 4;
    if bytes.len() < FIXED + SALT_LEN + NONCE_LEN + TAG_LEN {
        return Err(bad("同步包长度不足"));
    }
    if &bytes[..8] != MAGIC {
        return Err(bad("不是同步包（magic 不符）"));
    }
    let version = u32::from_le_bytes(bytes[8..12].try_into().expect("已校验长度"));
    if version != FORMAT_VERSION {
        return Err(bad(&format!("同步包格式版本 {version} 不受支持")));
    }
    let header_len = u32::from_le_bytes(bytes[12..16].try_into().expect("已校验长度")) as usize;
    let header_end = FIXED
        .checked_add(header_len)
        .ok_or_else(|| bad("同步包 header 长度溢出"))?;
    let tail_needed = header_end + SALT_LEN + NONCE_LEN + TAG_LEN;
    if bytes.len() < tail_needed {
        return Err(bad("同步包长度与 header 声明不符"));
    }

    let header: BundleHeader = serde_json::from_slice(&bytes[FIXED..header_end])
        .map_err(|_| bad("同步包 header 无法解析"))?;

    let salt_start = header_end;
    let salt: [u8; SALT_LEN] = bytes[salt_start..salt_start + SALT_LEN]
        .try_into()
        .expect("已校验长度");
    let nonce_start = salt_start + SALT_LEN;
    let nonce_bytes: [u8; NONCE_LEN] = bytes[nonce_start..nonce_start + NONCE_LEN]
        .try_into()
        .expect("已校验长度");
    let cipher_start = nonce_start + NONCE_LEN;

    // AAD = 从 magic 到 nonce 的全部字节（含 salt 与 nonce 本身）
    let prefix = bytes[..cipher_start].to_vec();
    let ciphertext = bytes[cipher_start..].to_vec();

    Ok((header, prefix, salt, nonce_bytes, ciphertext))
}

/// 用 OS 熵源取 N 字节。
///
/// 不用 `rand` crate：`getrandom` 已是依赖，
/// 少一个传递依赖就少一处供应链面。
fn random_bytes<const N: usize>() -> Result<[u8; N], CoreError> {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf)
        .map_err(|_| CoreError::new(ErrorCode::Crypto, "random", "无法获取系统随机数"))?;
    Ok(buf)
}
