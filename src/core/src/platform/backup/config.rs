//! 备份目录与保留份数配置。

use std::path::{Path, PathBuf};

use super::{ENV_BACKUP_KEEP, PATH_BACKUP_KEEP};

pub(crate) fn config_file_path() -> PathBuf {
    if let Some(file) = std::env::var_os("PATHEDITOR_CONFIG_FILE") {
        if !file.is_empty() {
            return PathBuf::from(file);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("config.ini")
}

/// 判断「读配置失败」是否应当 `log::warn!` 提示，而不是静默回落。
///
/// 口径（两种情形必须分开）：
/// - `NotFound` —— 文件不存在是首次使用的正常情形，**静默**回落（且不得创建文件）；
/// - 其它（`PermissionDenied`、路径指向目录、I/O 错误等）—— 用户可修复的异常，
///   **必须 warn**，否则配置写坏了在 CLI 下完全不可见（CLI 未初始化
///   logger，core 的 warn 会被丢弃，但仍须按 GUI/服务侧能收到的口径发出）。
pub(crate) fn should_warn_on_read_error(e: &std::io::Error) -> bool {
    e.kind() != std::io::ErrorKind::NotFound
}

/// 读取备份保留份数；任何异常一律回落 `fallback`。
///
/// 回落情形：文件不存在 / 键不存在 / 值非整数 /
/// 值为负数 / 值空 / 文件不可读。**不报错、不中止备份**。
/// 读取**无副作用**：文件不存在时不创建。
///
/// 落日志口径：值非法与文件不可读**各记一次 `log::warn!`**；
/// 文件不存在是首次使用的正常情形，**静默回落**（见 [`should_warn_on_read_error`]）。
///
/// 手写极简 INI 解析（`key = value`，`;` 或 `#` 起始为注释），不引入新依赖——
/// 单键配置不值得拉一个 crate，与项目「手写 FNV-1a 而不引 sha2」的先例一致。
///
/// `key` 是配置键名（`"env_backup_keep"` / `"path_backup_keep"`），
/// `fallback` 是回落值（分别为 [`ENV_BACKUP_KEEP`] / [`PATH_BACKUP_KEEP`]）。
pub(crate) fn read_backup_keep(path: &Path, key: &str, fallback: usize) -> usize {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            // 不存在 → 静默；不可读 → 必须可见（两种情形不得合并处理）。
            if should_warn_on_read_error(&e) {
                log::warn!(
                    "config.ini 不可读（{}: {}），回落默认 {}",
                    path.display(),
                    e,
                    fallback
                );
            }
            return fallback;
        }
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let Some((k, value)) = line.split_once('=') else {
            continue;
        };
        if k.trim() != key {
            continue;
        }
        match value.trim().parse::<usize>() {
            Ok(n) => return n,
            Err(_) => {
                log::warn!(
                    "config.ini 的 {} 值非法（{}），回落默认 {}",
                    key,
                    value.trim(),
                    fallback
                );
                return fallback;
            }
        }
    }
    fallback
}

/// 读取 env 备份保留份数（[`read_backup_keep`] 的 env 专用入口）。
pub(crate) fn read_env_backup_keep(path: &Path) -> usize {
    read_backup_keep(path, "env_backup_keep", ENV_BACKUP_KEEP)
}

/// 读取 path 备份保留份数（[`read_backup_keep`] 的 path 专用入口）。
pub(crate) fn path_backup_keep() -> usize {
    read_backup_keep(&config_file_path(), "path_backup_keep", PATH_BACKUP_KEEP)
}

/// 从真实配置文件读取保留份数。
pub(crate) fn env_backup_keep() -> usize {
    read_env_backup_keep(&config_file_path())
}

/// 备份根目录。`PATHEDITOR_BACKUP_DIR` 存在时用它（测试隔离用），
/// 否则回落 `~/.patheditor/backups/`。
pub(crate) fn backup_base_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("PATHEDITOR_BACKUP_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("backups")
}

/// env 备份目录的公开访问器。
pub fn env_backup_dir() -> PathBuf {
    backup_base_dir()
}

/// 获取备份目录路径
pub fn get_appdata_dir() -> String {
    backup_base_dir().to_string_lossy().to_string()
}
