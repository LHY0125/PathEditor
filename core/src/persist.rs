//! 版本化持久化原语（F-11，Wave 2 Task 6）。
//!
//! `disabled.json` / `pending_path_snapshot.json` / `profiles/*.json` 共用：
//! - 顶层 `schemaVersion` 信封（[`Versioned`]），无版本头的旧文件按 v1 读取；
//! - 写入前把上一份主文件轮换为 `<file>.bak`（[`rotate_backup`]）；
//! - 解析失败时把坏文件隔离为 `<file>.corrupt-<ts>`（[`quarantine`]）。

use crate::error::{CoreError, ErrorCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// 测试专用：序列化所有触碰固定临时持久化路径（disabled.json / pending /
/// profiles）的测试，消除 disabled.rs 与 service.rs 测试间的并行竞态。
///
/// 持锁失败的 poisoned 状态直接吞掉（取内层数据继续），避免一个测试 panic
/// 后连锁污染其他测试。
#[cfg(test)]
pub(crate) fn test_persist_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    let mutex = LOCK.get_or_init(|| std::sync::Mutex::new(()));
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// 当前持久化文件 schema 版本。
pub(crate) const PERSIST_SCHEMA_VERSION: u32 = 1;

/// 带版本头的持久化信封。
///
/// `schemaVersion` 缺失（旧格式文件）时按 v1 处理；写入端始终写当前版本。
/// `inner` 使用 `flatten` 序列化，磁盘上仍是「原字段 + schemaVersion」的平铺 JSON，
/// 对 Tauri IPC 返回形状无影响（调用方取 `inner` 使用）。
#[derive(Serialize, Deserialize)]
pub(crate) struct Versioned<T> {
    /// schema 版本号；缺失时按 v1 读取（向后兼容）
    #[serde(rename = "schemaVersion", default = "default_schema_version")]
    pub(crate) schema_version: u32,
    /// 实际持久化数据（序列化时平铺到顶层）
    #[serde(flatten)]
    pub(crate) inner: T,
}

/// 旧格式文件没有 `schemaVersion` 字段时的缺省值。
fn default_schema_version() -> u32 {
    PERSIST_SCHEMA_VERSION
}

/// 在文件名末尾追加后缀：`disabled.json` → `disabled.json.bak`。
pub(crate) fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// 写入前把当前主文件轮换为 `<file>.bak`（存在时才轮换）。
///
/// 轮换失败返回 Io 错误并中止写入 —— 宁可不写，也不能丢掉唯一的回滚副本。
pub(crate) fn rotate_backup(path: &Path) -> Result<(), CoreError> {
    if path.exists() {
        let bak = with_suffix(path, ".bak");
        fs::copy(path, &bak).map_err(|e| {
            CoreError::new(
                ErrorCode::Io,
                "rotate_backup",
                format!("备份 {} 到 {} 失败: {e}", path.display(), bak.display()),
            )
        })?;
    }
    Ok(())
}

/// 校验读取到的 schemaVersion 并取出数据（当前只支持 v1）。
///
/// 无版本头的旧文件已在反序列化时按 v1 补齐；更高版本说明文件由更新版本
/// 写入，返回 `ErrorCode::Parse`，避免用旧代码误解新格式造成数据损坏。
pub(crate) fn migrate<T>(versioned: Versioned<T>, label: &str) -> Result<T, CoreError> {
    if versioned.schema_version > PERSIST_SCHEMA_VERSION {
        return Err(CoreError::new(
            ErrorCode::Parse,
            "load_versioned",
            format!(
                "{label} 的 schemaVersion 为 {}，高于当前支持的 {PERSIST_SCHEMA_VERSION}，文件由更新版本写入，请升级 PathEditor",
                versioned.schema_version
            ),
        ));
    }
    Ok(versioned.inner)
}

/// 读取失败时把损坏文件隔离到 `<file>.corrupt-<ts>`，返回隔离目标路径。
///
/// rename 失败（文件被占用等）时**不删除原文件**（rename 原子性保证原文件
/// 留在原地），返回 Io 错误；调用方应通过 [`parse_error_quarantined`] 把
/// 隔离结果并入读取错误文案，保证失败不被静默吞掉。
pub(crate) fn quarantine(path: &Path) -> Result<PathBuf, CoreError> {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S%3f");
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let mut target = path.with_file_name(format!("{name}.corrupt-{ts}"));
    // Windows 上 rename 到已存在目标会失败：同一毫秒内重复隔离时追加序号。
    let mut n = 0u32;
    while target.exists() {
        n += 1;
        target = path.with_file_name(format!("{name}.corrupt-{ts}-{n}"));
        if n > 999 {
            break;
        }
    }
    fs::rename(path, &target).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "quarantine",
            format!(
                "隔离损坏文件 {} 失败（原文件保留在原地）: {e}",
                path.display()
            ),
        )
    })?;
    Ok(target)
}

/// 解析失败时的统一处置：隔离坏文件并返回 `ErrorCode::Parse` 错误。
///
/// 隔离失败（rename 失败）时原文件保留在原地，隔离失败原因并入错误文案，
/// 保证失败不被静默吞掉；读取错误照常返回 Parse。
pub(crate) fn parse_error_quarantined(
    path: &Path,
    label: &str,
    err: impl std::fmt::Display,
) -> CoreError {
    let note = match quarantine(path) {
        Ok(target) => format!("损坏文件已隔离到 {}", target.display()),
        Err(qe) => format!("隔离损坏文件失败（{qe}），原文件保留在原地"),
    };
    log::error!("{label} 解析失败: {err}；{note}");
    CoreError::new(
        ErrorCode::Parse,
        "load_versioned",
        format!("{label} 解析失败: {err}；{note}"),
    )
}

/// 读取版本化 JSON 文件的通用骨架（文件存在且非空时）。
///
/// 供各持久化文件实现「读文件 → 解析信封 → 失败隔离/版本校验」；
/// 缺失与空白内容由调用方按各自默认值语义处理。
pub(crate) fn read_versioned_file<T: DeserializeOwned>(
    path: &Path,
    label: &str,
) -> Result<Versioned<T>, CoreError> {
    let content = fs::read_to_string(path).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "load_versioned",
            format!("无法读取 {label}: {e}"),
        )
    })?;
    match serde_json::from_str::<Versioned<T>>(&content) {
        Ok(versioned) => Ok(versioned),
        Err(e) => Err(parse_error_quarantined(path, label, e)),
    }
}
