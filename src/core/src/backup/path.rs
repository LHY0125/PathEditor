//! PATH 备份与轮换。

use super::config::{backup_base_dir, path_backup_keep};
use crate::error::{CoreError, ErrorCode};
use crate::registry::{self, SYS_REG_PATH, USER_REG_PATH};
use chrono::Local;
use std::path::{Path, PathBuf};
use winreg::enums::*;

/// path 备份保留策略轮换：只删除本功能生成的 `path_backup_*.txt`，保留最近 `keep` 份。
///
/// 授权范围（用户明确授权的例外），与 [`rotate_env_backups`] 同构：
/// 1. 只匹配文件名 `path_backup_*.txt`；
/// 2. 只在给定目录内操作（自定义备份目录不参与轮换）；
/// 3. 只删最旧的、超出 `keep` 之外的文件；
/// 4. 绝不删除 `env_backup_*.json`、`.bak`、`.corrupt-*` 及目录中任何其它文件。
///
/// `keep = 0` 是合法输入（用户明确要求零保留），会删空所有候选文件。
///
/// # Returns
/// - `Ok(Vec<PathBuf>)` — 被删除的文件路径
/// - `Err(CoreError)` — 目录枚举失败（code=`Io`）；单个文件删除失败只记 warn
pub(crate) fn rotate_path_backups(dir: &Path, keep: usize) -> Result<Vec<PathBuf>, CoreError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "rotate_path_backups",
            format!("枚举备份目录 {} 失败: {}", dir.display(), e),
        )
    })?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // 只收 path_backup_*.txt；`.txt.bak` / `.txt.corrupt-*` 因后缀不是 `.txt` 天然排除。
        if !name.starts_with("path_backup_") || !name.ends_with(".txt") {
            continue;
        }
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        candidates.push((name, path));
    }
    if candidates.len() <= keep {
        return Ok(Vec::new());
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    let mut removed = Vec::new();
    for (_, path) in candidates.into_iter().skip(keep) {
        match std::fs::remove_file(&path) {
            Ok(()) => {
                log::info!("已轮换删除旧 path 备份: {}", path.display());
                removed.push(path);
            }
            Err(e) => log::warn!("删除旧 path 备份 {} 失败: {}", path.display(), e),
        }
    }
    Ok(removed)
}

/// 备份当前注册表中的系统 PATH 和用户 PATH
/// 在保存前调用，备份的是注册表中的当前值（保存前的状态）
pub fn backup_registry(custom_dir: Option<String>) -> Result<String, CoreError> {
    let backup_dir = match custom_dir {
        Some(ref dir) if !dir.is_empty() => {
            let p = std::path::PathBuf::from(dir);
            let normalized = dir.replace('/', "\\").to_lowercase();
            if normalized.starts_with("c:\\windows\\")
                || normalized.starts_with("c:\\program files\\")
            {
                return Err(CoreError::new(
                    ErrorCode::InvalidValue,
                    "backup_registry",
                    "不允许备份到系统目录",
                ));
            }
            p
        }
        _ => backup_base_dir(),
    };

    std::fs::create_dir_all(&backup_dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "backup_registry",
            format!("无法创建备份目录: {}", e),
        )
    })?;

    // 读取当前注册表中的值（保存前的旧值）
    let sys_paths = registry::load_paths(HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统")
        .map_err(|m| CoreError::new(ErrorCode::Io, "backup_registry", m))?;
    let user_paths = registry::load_paths(HKEY_CURRENT_USER, USER_REG_PATH, "用户")
        .map_err(|m| CoreError::new(ErrorCode::Io, "backup_registry", m))?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    let filename = format!("path_backup_{}.txt", timestamp);
    let filepath = backup_dir.join(&filename);

    let mut content = String::new();
    content.push_str(&format!(
        "PathEditor Backup - {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    content.push_str("\n[System PATH]\n");
    for path in &sys_paths {
        content.push_str(&format!("{}\n", path));
    }
    content.push_str("\n[User PATH]\n");
    for path in &user_paths {
        content.push_str(&format!("{}\n", path));
    }

    std::fs::write(&filepath, &content).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "backup_registry",
            format!("无法写入备份文件: {}", e),
        )
    })?;

    // 与 env 备份对称，写后轮换。只作用于 path_backup_*.txt，且 custom_dir 不轮换
    // （用户指定目录可能混有他自己的文件，轮换风险高于收益）。
    //
    // 注意：轮换失败只 log::warn!，而 CLI 未初始化 logger（见 runtime.rs:28），
    // 因此该警告在 CLI 下**不可见**。这是**有意的**，与 env 侧 rotate_env_backups 对称
    // （它同样只有 log::warn!）。**不要**据此误以为用户会看到提示 —— 若要统一解决，
    // 应作为独立 observability 议题（CLI 初始化 logger 或轮换失败计入退出口），不在本波。
    if custom_dir.is_none() {
        if let Err(e) = rotate_path_backups(&backup_dir, path_backup_keep()) {
            log::warn!("path 备份轮换失败（备份已落盘）: {}", e.message);
        }
    }

    let result = filepath.to_string_lossy().to_string();
    log::info!("备份已保存到: {}", result);
    Ok(result)
}
