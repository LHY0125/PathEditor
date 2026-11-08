//! 文件系统边界：文本读取、原子写入、目录黑名单与导出写文件。

use crate::domain::error::{CoreError, ErrorCode};

/// 原子写入：先写同目录的随机临时文件，再 rename 覆盖。
///
/// 临时名带 UUID 且用 `create_new`：可预测的 `<name>.tmp` 让同目录攻击者
/// 预置符号链接/文件，把写入引到别的目标；`create_new` 同时避免误覆写。
pub fn atomic_write(path: &std::path::Path, content: &str) -> std::io::Result<()> {
    use std::io::Write as _;

    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let tmp = parent.join(format!(".{name}.{}.tmp", uuid::Uuid::now_v7()));

    let result = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        // 失败时清理临时文件；目标文件保持原样（rename 未发生）。
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// 读取文本文件内容（供前端原生对话框选择文件后使用）
/// 仅允许 .json / .csv / .txt 扩展名，防止任意文件读取
pub fn read_text_file(path: &str) -> Result<String, CoreError> {
    read_text_file_impl(path, std::path::Path::new(path))
}

/// [`read_text_file`] 的实际实现：扩展名按 `display` 判定，内容从 `read_path` 读取。
///
/// 分开的原因是 scoped 版本要先 `canonicalize` 做白名单校验，再**用同一个
/// 规范化路径**读取——按原路径二次打开会留下「校验后换链接」的窗口。
fn read_text_file_impl(display: &str, read_path: &std::path::Path) -> Result<String, CoreError> {
    // 文本导入（PATH / 环境变量列表）的正常体积是几十 KB；8 MiB 之外只可能是
    // 选错文件（视频、镜像）。先 stat 再读，避免把整文件读进内存才发现不对。
    const MAX_TEXT_BYTES: u64 = 8 * 1024 * 1024;
    match std::fs::metadata(read_path) {
        Ok(meta) if meta.len() > MAX_TEXT_BYTES => {
            return Err(CoreError::new(
                ErrorCode::InvalidValue,
                "read_text_file",
                format!(
                    "文件过大（{} 字节，上限 {} 字节），请确认选的是 PATH/变量列表文本",
                    meta.len(),
                    MAX_TEXT_BYTES
                ),
            ));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {} // 读取阶段会给出更准确的错误
    }

    let ext = std::path::Path::new(display)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "json" | "csv" | "txt") {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "read_text_file",
            format!("不支持的文件类型: .{}（仅允许 .json/.csv/.txt）", ext),
        ));
    }
    std::fs::read_to_string(read_path).map_err(|e| {
        let code = if e.kind() == std::io::ErrorKind::NotFound {
            ErrorCode::NotFound
        } else {
            ErrorCode::Io
        };
        CoreError::new(code, "read_text_file", format!("无法读取文件: {}", e))
    })
}

/// 文件操作允许的根目录：用户目录、临时目录、当前工作目录。
fn allowed_roots() -> Vec<std::path::PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home);
    }
    roots.push(std::env::temp_dir());
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    roots
}

/// 规范化路径是否落在 [`allowed_roots`] 之内。
fn is_within_allowed_roots(canonical: &std::path::Path) -> bool {
    allowed_roots().into_iter().any(|root| {
        std::fs::canonicalize(root)
            .map(|canonical_root| canonical.starts_with(canonical_root))
            .unwrap_or(false)
    })
}

/// 校验同步包路径：扩展名必须是 `.pesync`；`must_be_inside` 时还要求
/// 目标落在用户目录 / 临时目录 / 工作目录内（导出写在别处会被拒）。
///
/// 导入只校验扩展名：用户经常把同步包放在别的盘/移动介质上。
pub fn validate_sync_path(
    path: &str,
    must_be_inside: bool,
) -> Result<std::path::PathBuf, CoreError> {
    let p = std::path::PathBuf::from(path);
    let is_pesync = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pesync"))
        .unwrap_or(false);
    if !is_pesync {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "validate_sync_path",
            format!("同步包必须是 .pesync 文件: {path}"),
        ));
    }
    if must_be_inside {
        let resolved = match std::fs::canonicalize(&p) {
            Ok(c) => c,
            Err(_) => {
                // 导出目标通常尚不存在：校验父目录，再把文件名拼回去。
                let parent = p
                    .parent()
                    .filter(|x| !x.as_os_str().is_empty())
                    .ok_or_else(|| {
                        CoreError::new(
                            ErrorCode::InvalidValue,
                            "validate_sync_path",
                            format!("同步包路径非法: {path}"),
                        )
                    })?;
                let parent = std::fs::canonicalize(parent).map_err(|e| {
                    CoreError::new(
                        ErrorCode::InvalidValue,
                        "validate_sync_path",
                        format!("同步包目录不可用: {e}"),
                    )
                })?;
                parent.join(p.file_name().unwrap_or_default())
            }
        };
        if !is_within_allowed_roots(&resolved) {
            return Err(CoreError::new(
                ErrorCode::PermissionDenied,
                "validate_sync_path",
                "只允许把同步包写入用户目录、临时目录或当前工作目录",
            ));
        }
    }
    Ok(p)
}

/// 读取导入文件时限制在用户目录、临时目录或当前工作目录内。
pub fn read_text_file_scoped(path: &str) -> Result<String, CoreError> {
    let requested = std::path::Path::new(path);
    let canonical = std::fs::canonicalize(requested).map_err(|e| {
        let code = if e.kind() == std::io::ErrorKind::NotFound {
            ErrorCode::NotFound
        } else {
            ErrorCode::Io
        };
        CoreError::new(
            code,
            "read_text_file_scoped",
            format!("无法读取文件: {}", e),
        )
    })?;

    if !is_within_allowed_roots(&canonical) {
        return Err(CoreError::new(
            ErrorCode::PermissionDenied,
            "read_text_file_scoped",
            "仅允许读取用户目录、临时目录或当前工作目录中的导入文件",
        ));
    }

    // 用规范化路径读取：校验与读取之间不可能再被换链接绕过。
    read_text_file_impl(path, &canonical)
}

/// 拒绝把用户数据写入 Windows 系统目录（`C:\Windows\`、`C:\Program Files\`）。
///
/// 备份与导出共用同一判定；`action` 只影响错误文案（「备份」/「导出」）。
/// 外层（CLI/GUI）不得复制这段黑名单。
pub(crate) fn ensure_not_system_dir(
    path: &str,
    operation: &'static str,
    action: &str,
) -> Result<(), CoreError> {
    // 尽量解析成真实路径（处理 8.3 短名、相对路径、`\\?\` 前缀）；目标文件
    // 可能还不存在，此时退化为「解析父目录 + 文件名」。
    let requested = std::path::Path::new(path);
    let resolved = std::fs::canonicalize(requested).or_else(|_| match requested.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            std::fs::canonicalize(parent).map(|c| c.join(requested.file_name().unwrap_or_default()))
        }
        _ => Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
    });
    let normalized = resolved
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
        .replace('/', "\\");
    let trimmed_raw = normalized.trim_end_matches('\\');
    let trimmed = trimmed_raw
        .strip_prefix("\\\\?\\")
        .unwrap_or(trimmed_raw)
        .to_ascii_lowercase();

    // 盘符取 `%SystemDrive%`：系统装在 D: 的机器上，硬编码 `c:\` 会整条失效。
    let sys_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    let sys_drive = sys_drive.trim_end_matches(['\\', '/']).to_ascii_lowercase();
    for name in ["windows", "program files", "program files (x86)"] {
        let blocked = format!("{sys_drive}\\{name}");
        if trimmed == blocked || trimmed.starts_with(&format!("{blocked}\\")) {
            return Err(CoreError::new(
                ErrorCode::InvalidValue,
                operation,
                format!("不允许{action}到系统目录: {path}"),
            ));
        }
    }
    Ok(())
}

/// 把导出内容写到用户指定文件。
///
/// 系统目录拒绝规则与备份共用 [`ensure_not_system_dir`]；写失败为 `Io`。
pub fn write_export_file(path: &str, content: &str) -> Result<(), CoreError> {
    ensure_not_system_dir(path, "write_export_file", "导出")?;
    std::fs::write(path, content).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "write_export_file",
            format!("无法写入文件: {e}"),
        )
    })
}
