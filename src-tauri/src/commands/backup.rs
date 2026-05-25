use chrono::Local;
use std::fs;
use std::path::PathBuf;

fn backup_base_dir() -> PathBuf {
    dirs::data_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("PathEditor")
        .join("backups")
}

/// 获取 APPDATA 路径下的备份目录
#[tauri::command]
pub fn get_appdata_dir() -> String {
    backup_base_dir().to_string_lossy().to_string()
}

/// 备份当前注册表中的系统 PATH 和用户 PATH
/// 返回备份文件的路径
#[tauri::command]
pub fn backup_registry(custom_dir: Option<String>, sys_paths: Vec<String>, user_paths: Vec<String>) -> Result<String, String> {
    // 确定备份目录
    let backup_dir = match custom_dir {
        Some(ref dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => backup_base_dir(),
    };

    // 创建目录
    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("无法创建备份目录: {}", e))?;

    // 生成带时间戳的文件名
    let timestamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    let filename = format!("path_backup_{}.txt", timestamp);
    let filepath = backup_dir.join(&filename);

    // 写入备份内容
    let mut content = String::new();
    content.push_str(&format!("PathEditor Backup - {}\n", Local::now().format("%Y-%m-%d %H:%M:%S")));
    content.push_str("\n[System PATH]\n");
    for path in &sys_paths {
        content.push_str(&format!("{}\n", path));
    }
    content.push_str("\n[User PATH]\n");
    for path in &user_paths {
        content.push_str(&format!("{}\n", path));
    }

    fs::write(&filepath, &content)
        .map_err(|e| format!("无法写入备份文件: {}", e))?;

    let result = filepath.to_string_lossy().to_string();
    log::info!("备份已保存到: {}", result);
    Ok(result)
}
