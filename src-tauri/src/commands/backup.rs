use chrono::Local;
use std::path::PathBuf;
use winreg::enums::*;
use crate::commands::registry::{self, SYS_REG_PATH, USER_REG_PATH};

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
/// 在保存前调用，备份的是注册表中的当前值（保存前的状态）
#[tauri::command]
pub fn backup_registry(custom_dir: Option<String>) -> Result<String, String> {
    let backup_dir = match custom_dir {
        Some(ref dir) if !dir.is_empty() => std::path::PathBuf::from(dir),
        _ => backup_base_dir(),
    };

    std::fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("无法创建备份目录: {}", e))?;

    // 读取当前注册表中的值（保存前的旧值）
    let sys_paths = registry::load_paths(
        HKEY_LOCAL_MACHINE,
        SYS_REG_PATH,
        "系统",
    )?;
    let user_paths = registry::load_paths(
        HKEY_CURRENT_USER,
        USER_REG_PATH,
        "用户",
    )?;

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

    std::fs::write(&filepath, &content)
        .map_err(|e| format!("无法写入备份文件: {}", e))?;

    let result = filepath.to_string_lossy().to_string();
    log::info!("备份已保存到: {}", result);
    Ok(result)
}
