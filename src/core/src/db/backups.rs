//! `.pebak` 备份格式与备份索引。
//!
//! `.pebak` = **明文文件头 + DPAPI 加密正文**：
//!
//! ```text
//! 偏移  长度  字段
//! 0     5     magic "PEBK1"
//! 5     1     version
//! 6     8     created_at（Unix 毫秒，小端）
//! 14    1     kind（0=env, 1=path）
//! 15    4     variable_count（小端）
//! 19    4     plaintext_len（小端）
//! 23    32    payload_hash = BLAKE3(hash_salt || canonical_json(payload))
//! ─────────────────────────────────────────────────────────
//! 55    n     DPAPI 密文（正文）
//! ```
//!
//! **头部不含秘密**：索引与列表只读头，不必解密。
//! `hash_salt` 来自 DPAPI 密钥文件、**不写进文件**，因此外部观察者无法用
//! 候选明文直接验证 `payload_hash`（弱化字典攻击面，但**它不是加密**）。

use super::crypto::KeyMaterial;
use super::{db_err, Db};
use crate::backup::{EnvBackupPayload, EnvBackupVar};
use crate::domain::error::{CoreError, ErrorCode};
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// 文件魔数。
pub const MAGIC: &[u8; 5] = b"PEBK1";

/// 文件头长度：`5 + 1 + 8 + 1 + 4 + 4 + 32 = 55`。
pub const HEADER_LEN: usize = 5 + 1 + 8 + 1 + 4 + 4 + 32;

/// 当前 `.pebak` 格式版本。
const FORMAT_VERSION: u8 = 1;

/// 备份种类：环境变量。
const KIND_ENV: u8 = 0;
/// 备份种类：PATH。
#[allow(dead_code)]
const KIND_PATH: u8 = 1;

/// `.pebak` 文件头（明文部分）。
#[derive(Debug, Clone)]
pub struct PebakHeader {
    /// 格式版本
    pub version: u8,
    /// 创建时间（Unix 毫秒）
    pub created_at: i64,
    /// 备份种类
    pub kind: u8,
    /// 变量数（**无需解密即可读出**）
    pub variable_count: u32,
    /// 正文明文长度
    pub plaintext_len: u32,
    /// 去重哈希
    pub payload_hash: [u8; 32],
}

/// 备份索引行的对外视图。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupView {
    /// 索引 uid
    pub uid: String,
    /// `env` / `path`
    pub kind: String,
    /// 创建时间（Unix 毫秒）
    pub created_at: i64,
    /// 去重哈希（十六进制）
    pub content_hash: String,
    /// 文件字节数
    pub size_bytes: i64,
    /// 文件路径
    pub file_path: String,
    /// 是否为 DPAPI 加密的 `.pebak`
    pub encrypted: bool,
    /// 变量数（`.pebak` 来自文件头；`.json` 解析内容得到）
    pub variable_count: u32,
}

/// 写一份 `.pebak` 备份，返回落盘路径。
///
/// 写入顺序：**先写临时文件再 rename** —— 中途崩溃不会留下半份备份
/// 。
pub fn write_env_backup_pebak(
    dir: &Path,
    payload: &EnvBackupPayload,
    material: &KeyMaterial,
) -> Result<PathBuf, CoreError> {
    let json = serde_json::to_vec(payload).map_err(|e| {
        CoreError::new(
            ErrorCode::Parse,
            "write_pebak",
            format!("备份序列化失败: {e}"),
        )
    })?;
    let variable_count = (payload.hives.system.len() + payload.hives.user.len()) as u32;
    let hash = payload_hash(&json, &material.hash_salt);
    let cipher = super::dpapi::protect(&json)?;

    let mut out = Vec::with_capacity(HEADER_LEN + cipher.len());
    out.extend_from_slice(MAGIC);
    out.push(FORMAT_VERSION);
    out.extend_from_slice(&payload.captured_at.to_le_bytes());
    out.push(KIND_ENV);
    out.extend_from_slice(&variable_count.to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&hash);
    out.extend_from_slice(&cipher);

    std::fs::create_dir_all(dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "write_pebak",
            format!("创建备份目录失败: {e}"),
        )
    })?;
    // 文件名用**写入时刻**，不是 `payload.captured_at`。
    // 后者是「这份数据是什么时候采集的」——导入历史载荷时它可能很旧，
    // 用旧时间戳命名会让新文件立刻被轮换当作「最旧的」删掉（实测：
    // 25 份旧备份 + 1 份 captured_at=2026-01 的新备份，keep=4 时新文件当场消失）。
    let path = dir.join(format!(
        "env_backup_{}.pebak",
        chrono::Local::now().format("%Y%m%d_%H%M%S_%3f")
    ));
    let tmp = dir.join(format!(".{}.tmp", Uuid::now_v7()));
    std::fs::write(&tmp, &out).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "write_pebak",
            format!("写备份临时文件失败: {e}"),
        )
    })?;
    // rename 在同目录内是原子的（Windows 上目标已存在会失败，这正是我们要的：
    // 同一毫秒内重复备份不应静默覆盖）。
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        CoreError::new(ErrorCode::Io, "write_pebak", format!("备份落盘失败: {e}"))
    })?;
    Ok(path)
}

/// 读 `.pebak` 文件头（**不解密**）。
pub fn read_pebak_header(path: &Path) -> Result<PebakHeader, CoreError> {
    let bytes = std::fs::read(path).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "read_pebak",
            format!("读取备份文件失败: {e}"),
        )
    })?;
    parse_header(&bytes)
}

/// 解析头字节。
fn parse_header(bytes: &[u8]) -> Result<PebakHeader, CoreError> {
    if bytes.len() < HEADER_LEN {
        return Err(CoreError::new(
            ErrorCode::Parse,
            "read_pebak",
            format!(
                "备份文件过短：{} 字节，不足 {HEADER_LEN} 字节的头",
                bytes.len()
            ),
        ));
    }
    if &bytes[..5] != MAGIC {
        return Err(CoreError::new(
            ErrorCode::Parse,
            "read_pebak",
            "文件不是 PathEditor 加密备份（magic 不匹配）",
        ));
    }
    let version = bytes[5];
    if version != FORMAT_VERSION {
        return Err(CoreError::new(
            ErrorCode::Parse,
            "read_pebak",
            format!("备份格式版本 {version} 不受支持（当前 {FORMAT_VERSION}）"),
        ));
    }
    let mut created = [0u8; 8];
    created.copy_from_slice(&bytes[6..14]);
    let mut count = [0u8; 4];
    count.copy_from_slice(&bytes[15..19]);
    let mut plain_len = [0u8; 4];
    plain_len.copy_from_slice(&bytes[19..23]);
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes[23..55]);
    Ok(PebakHeader {
        version,
        created_at: i64::from_le_bytes(created),
        kind: bytes[14],
        variable_count: u32::from_le_bytes(count),
        plaintext_len: u32::from_le_bytes(plain_len),
        payload_hash: hash,
    })
}

/// 读取任意格式的 env 备份：`.pebak` 解密，`.json` 明文兼容。
///
/// `.json` 路径给明文警告。
pub fn read_env_backup_any(
    path: &Path,
    material: &KeyMaterial,
) -> Result<EnvBackupPayload, CoreError> {
    let is_pebak = path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("pebak"))
        .unwrap_or(false);
    if !is_pebak {
        log::warn!(
            "读取遗留明文备份 {}：DPAPI 加密的 .pebak 才是 v6 的默认格式",
            path.display()
        );
        return crate::backup::read_env_backup(path);
    }

    let bytes = std::fs::read(path).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "read_pebak",
            format!("读取备份文件失败: {e}"),
        )
    })?;
    let header = parse_header(&bytes)?;
    let plain = super::dpapi::unprotect(&bytes[HEADER_LEN..])?;

    // 头部声称的长度必须与实到明文一致，否则文件被截断/篡改
    if plain.len() != header.plaintext_len as usize {
        return Err(CoreError::new(
            ErrorCode::Crypto,
            "read_pebak",
            format!(
                "备份正文长度不符：头部声明 {}，实到 {}",
                header.plaintext_len,
                plain.len()
            ),
        ));
    }
    // payload_hash 只是完整性提示（非 MAC）；不匹配说明内容与索引不一致
    let actual = payload_hash(&plain, &material.hash_salt);
    if actual != header.payload_hash {
        return Err(CoreError::new(
            ErrorCode::Crypto,
            "read_pebak",
            "备份内容哈希与头部不一致，文件可能已损坏",
        ));
    }

    serde_json::from_slice(&plain).map_err(|e| {
        CoreError::new(
            ErrorCode::Parse,
            "read_pebak",
            format!("备份内容解析失败: {e}"),
        )
    })
}

/// 扫描目录并把备份写入索引表，返回索引条数。
///
/// `.pebak` 只读头（不解密）；遗留 `.json` 解析内容以得到真实变量数。
/// 同一文件重复索引时按 `file_path + content_hash` 更新，不产生重复行。
pub fn index_backups(db: &mut Db, dir: &Path) -> Result<usize, CoreError> {
    if !dir.exists() {
        return Ok(0);
    }
    let entries = std::fs::read_dir(dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "index_backups",
            format!("枚举备份目录失败: {e}"),
        )
    })?;

    let mut count = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("env_backup_") && !name.starts_with("path_backup_") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }

        let is_pebak = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("pebak"))
            .unwrap_or(false);
        let (kind, created_at, content_hash, variable_count) = if is_pebak {
            let h = read_pebak_header(&path)?;
            (
                if h.kind == KIND_PATH { "path" } else { "env" }.to_string(),
                h.created_at,
                hex32(&h.payload_hash),
                h.variable_count,
            )
        } else if path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("json"))
            .unwrap_or(false)
        {
            // 遗留 .json：解析内容拿真实变量数（与 .pebak 不同，后者读头即可）
            match crate::backup::read_env_backup(&path) {
                Ok(p) => (
                    "env".to_string(),
                    p.captured_at,
                    file_hash(&path)?,
                    (p.hives.system.len() + p.hives.user.len()) as u32,
                ),
                // 单个损坏文件不应让整份索引失败
                Err(e) => {
                    log::warn!("跳过无法解析的遗留备份 {}: {}", path.display(), e.message);
                    continue;
                }
            }
        } else if name.ends_with(".txt") {
            // path 备份是明文文本，无哈希可用：用文件内容哈希
            (
                "path".to_string(),
                meta.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0),
                file_hash(&path)?,
                0,
            )
        } else {
            continue;
        };

        upsert_index(
            db,
            &path.to_string_lossy(),
            kind,
            created_at,
            content_hash,
            meta.len() as i64,
            variable_count,
        )?;
        count += 1;
    }
    Ok(count)
}

/// 列出索引中的备份，按时间倒序。
///
/// **真正的「不解密」实现**：`variable_count` 来自 `BackupView` 之外的头部字段，
/// 但 `backups` 表不存变量数（避免与文件脱节），因此这里对 `.pebak` 读头
/// （55 字节，纯 IO，不触发 DPAPI）。读取失败时降级为 0，**不影响列表本身**。
pub fn list_backups(db: &Db) -> Result<Vec<BackupView>, CoreError> {
    let mut stmt = db.prepare(
        "SELECT uid, kind, created_at, content_hash, size_bytes, file_path
         FROM backups WHERE deleted_at IS NULL
         ORDER BY created_at DESC, id DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let file_path: String = r.get(5)?;
            let encrypted = file_path.to_ascii_lowercase().ends_with(".pebak");
            let variable_count = if encrypted {
                // 只读文件头；失败不阻断列表
                read_pebak_header(Path::new(&file_path))
                    .map(|h| h.variable_count)
                    .unwrap_or(0)
            } else {
                0
            };
            Ok(BackupView {
                uid: r.get(0)?,
                kind: r.get(1)?,
                created_at: r.get(2)?,
                content_hash: r.get(3)?,
                size_bytes: r.get(4)?,
                file_path,
                encrypted,
                variable_count,
            })
        })
        .map_err(|e| db_err("list_backups", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("list_backups", e))?;
    Ok(rows)
}

/// 刷新备份索引并返回列表（生产入口）。
///
/// 每次调用前先增量索引：只在启动时索引会漏掉「启动后新做的备份」。
/// `index_backups` 按 `file_path` 去重，重复调用幂等。
pub fn refresh_and_list(db: &mut Db) -> Result<Vec<BackupView>, CoreError> {
    refresh_and_list_at(db, &crate::backup::env_backup_dir())
}

/// [`refresh_and_list`] 的显式目录版本（测试注入 `TempDir`）。
///
/// 索引失败不阻断列表：已有索引行仍然有价值；失败原因记日志。
pub fn refresh_and_list_at(db: &mut Db, dir: &Path) -> Result<Vec<BackupView>, CoreError> {
    if let Err(e) = index_backups(db, dir) {
        log::warn!("备份索引刷新失败（展示已有索引）: {}", e.message);
    }
    list_backups(db)
}

/// 插入或更新一条索引（同一 `file_path` 只保留一行）。
///
/// **不用 `ON CONFLICT(file_path)`**：`backups` 表没有 `file_path`
/// 的唯一约束（只有 `uid`），UPSERT 的目标列必须匹配某个唯一索引，否则 SQLite
/// 直接报错。改为「先查后写」，语义等价且不改现有 DDL。
fn upsert_index(
    db: &mut Db,
    file_path: &str,
    kind: String,
    created_at: i64,
    content_hash: String,
    size_bytes: i64,
    variable_count: u32,
) -> Result<(), CoreError> {
    let _ = variable_count; // 变量数实时从文件读，不落库（避免与文件脱节）
    db.with_transaction(|tx| {
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM backups WHERE file_path = ?1 AND deleted_at IS NULL",
                params![file_path],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| db_err("upsert_backup", e))?;

        match existing {
            Some(id) => tx.execute(
                "UPDATE backups
                 SET kind = ?1, created_at = ?2, content_hash = ?3, size_bytes = ?4
                 WHERE id = ?5",
                params![kind, created_at, content_hash, size_bytes, id],
            ),
            None => tx.execute(
                "INSERT INTO backups (uid, kind, created_at, content_hash, size_bytes, file_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    Uuid::now_v7().to_string(),
                    kind,
                    created_at,
                    content_hash,
                    size_bytes,
                    file_path
                ],
            ),
        }
        .map_err(|e| db_err("upsert_backup", e))?;
        Ok(())
    })
}

/// `BLAKE3(hash_salt || payload)` 的十六进制表示。
fn payload_hash(payload: &[u8], salt: &[u8; 16]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(salt);
    h.update(payload);
    *h.finalize().as_bytes()
}

/// 文件内容哈希（遗留格式没有 `payload_hash` 头）。
fn file_hash(path: &Path) -> Result<String, CoreError> {
    let bytes = std::fs::read(path).map_err(|e| {
        CoreError::new(ErrorCode::Io, "index_backups", format!("读取文件失败: {e}"))
    })?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

/// 32 字节哈希的十六进制表示。
fn hex32(bytes: &[u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(64);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// 把明文载荷的变量按 hive 展开（供迁移复用）。
#[allow(dead_code)]
fn flatten_vars(payload: &EnvBackupPayload) -> Vec<(&'static str, &EnvBackupVar)> {
    let mut out = Vec::new();
    for v in &payload.hives.system {
        out.push(("system", v));
    }
    for v in &payload.hives.user {
        out.push(("user", v));
    }
    out
}
