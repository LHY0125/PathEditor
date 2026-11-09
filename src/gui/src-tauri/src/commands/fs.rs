use path_editor_core::platform::fs;
use path_editor_core::{CoreError, PathEntry};

#[tauri::command(async)]
pub fn import_file(path: String) -> Result<(Vec<PathEntry>, Vec<PathEntry>), CoreError> {
    let content = fs::read_text_file_scoped(&path)?;
    fs::import_paths(&path, &content)
}

#[tauri::command(async)]
pub fn export_path_entries(
    sys: Vec<PathEntry>,
    usr: Vec<PathEntry>,
    format: String,
) -> Result<String, CoreError> {
    fs::export_path_entries(&sys, &usr, &format)
}

/// 在资源管理器中打开数据目录（健康卡片兜底入口）。
#[tauri::command(async)]
pub fn open_data_dir() -> Result<(), path_editor_core::CoreError> {
    let dir = path_editor_core::db::crypto::data_home();
    std::fs::create_dir_all(&dir).map_err(|e| {
        path_editor_core::CoreError::new(
            path_editor_core::ErrorCode::Io,
            "open_data_dir",
            format!("无法创建数据目录: {e}"),
        )
    })?;
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| {
                path_editor_core::CoreError::new(
                    path_editor_core::ErrorCode::Io,
                    "open_data_dir",
                    format!("无法打开数据目录: {e}"),
                )
            })?;
    }
    Ok(())
}
