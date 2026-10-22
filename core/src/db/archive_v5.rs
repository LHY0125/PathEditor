//! v5 原件归档。
//!
//! 把 `disabled.json` 与 `profiles/*.json` 重命名为 `<原名>.v5.bak`。
//! **只改名，不删除**；目标已存在时跳过并记录，绝不覆盖。
//!
//! 归档是「v5 停写」的收尾动作：文件还在，但名字明确表示「这是 v5 时代的快照」。

use crate::error::{CoreError, ErrorCode};
use std::path::{Path, PathBuf};

/// 归档结果。
#[derive(Debug, Default)]
pub struct ArchiveReport {
    /// 成功改名的目标路径（dry-run 时是「将要改名」的目标）
    pub archived: Vec<PathBuf>,
    /// 跳过项与原因
    pub skipped: Vec<(PathBuf, String)>,
}

/// 归档 v5 原件。
///
/// `home` 由调用方显式传入：`config_dir()` 走 `SHGetKnownFolderPath`，
/// `HOME` / `USERPROFILE` 都不可重定向，测试必须用 `TempDir`。
///
/// `dry_run = true` 时**不动任何文件**，只报告将要改名的目标。
pub fn archive_v5_sources(home: &Path, dry_run: bool) -> Result<ArchiveReport, CoreError> {
    let mut report = ArchiveReport::default();
    let mut candidates = Vec::new();

    let disabled = home.join("disabled.json");
    if disabled.is_file() {
        candidates.push(disabled);
    }
    let profiles = home.join("profiles");
    if profiles.is_dir() {
        let entries = std::fs::read_dir(&profiles).map_err(|e| {
            CoreError::new(
                ErrorCode::Io,
                "archive_v5",
                format!("读取 {} 失败: {e}", profiles.display()),
            )
        })?;
        for entry in entries {
            let path = entry
                .map_err(|e| {
                    CoreError::new(ErrorCode::Io, "archive_v5", format!("读取目录项失败: {e}"))
                })?
                .path();
            // `is_file()`：目录也可能叫 `xxx.json`，改名它会失败并让整次归档中止。
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
                candidates.push(path);
            }
        }
    }

    // 排序只为让输出可复现：`read_dir` 的顺序由文件系统决定，两次运行可能不同。
    candidates.sort();

    for src in candidates {
        let mut name = src
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default();
        name.push(".v5.bak");
        let dst = src.with_file_name(name);

        // **绝不覆盖**：目标已存在说明之前归档过，或用户自己放了同名文件。
        if dst.exists() {
            report
                .skipped
                .push((src, format!("目标已存在: {}", dst.display())));
            continue;
        }
        if !dry_run {
            std::fs::rename(&src, &dst).map_err(|e| {
                CoreError::new(
                    ErrorCode::Io,
                    "archive_v5",
                    format!("归档 {} 失败: {e}", src.display()),
                )
            })?;
        }
        report.archived.push(dst);
    }
    Ok(report)
}
