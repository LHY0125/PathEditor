use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn profiles_dir() -> PathBuf {
    dirs::data_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("profiles")
}

fn profile_path(name: &str) -> PathBuf {
    profiles_dir().join(format!("{}.json", name))
}

/// 内部用的 PathEntry（与前端 PathEntry 字段一致）
#[derive(Serialize, Deserialize, Clone)]
pub struct ProfilePathEntry {
    pub path: String,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ProfileMeta {
    pub name: String,
    pub created: String,
    pub modified: String,
}

#[derive(Serialize, Deserialize)]
pub struct ProfileData {
    pub name: String,
    pub sys: Vec<ProfilePathEntry>,
    pub user: Vec<ProfilePathEntry>,
    pub created: String,
    pub modified: String,
}

/// 列出所有配置文件的元数据
#[tauri::command]
pub fn list_profiles() -> Result<Vec<ProfileMeta>, String> {
    let dir = profiles_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut profiles: Vec<ProfileMeta> = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("无法读取配置目录: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(true, |e| e != "json") {
            continue;
        }
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("无法读取 {}: {}", path.display(), e))?;
        if let Ok(data) = serde_json::from_str::<ProfileData>(&content) {
            profiles.push(ProfileMeta {
                name: data.name,
                created: data.created,
                modified: data.modified,
            });
        }
    }

    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(profiles)
}

/// 保存当前 PATH 为配置文件
#[tauri::command]
pub fn save_profile(
    name: String,
    sys: Vec<ProfilePathEntry>,
    user: Vec<ProfilePathEntry>,
) -> Result<(), String> {
    let dir = profiles_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("无法创建配置目录: {}", e))?;

    let path = profile_path(&name);
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let data = ProfileData {
        name,
        sys,
        user,
        created: now.clone(),
        modified: now,
    };

    let json =
        serde_json::to_string_pretty(&data).map_err(|e| format!("JSON 序列化失败: {}", e))?;
    fs::write(&path, &json).map_err(|e| format!("无法写入配置文件: {}", e))?;

    log::info!("已保存配置: {}", path.display());
    Ok(())
}

/// 加载配置文件
#[tauri::command]
pub fn load_profile(name: String) -> Result<ProfileData, String> {
    let path = profile_path(&name);
    if !path.exists() {
        return Err(format!("配置文件不存在: {}", name));
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("无法读取配置文件: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("JSON 解析失败: {}", e))
}

/// 删除配置文件
#[tauri::command]
pub fn delete_profile(name: String) -> Result<(), String> {
    let path = profile_path(&name);
    fs::remove_file(&path).map_err(|e| format!("无法删除配置文件: {}", e))?;
    log::info!("已删除配置: {}", path.display());
    Ok(())
}

/// 重命名配置文件
#[tauri::command]
pub fn rename_profile(old_name: String, new_name: String) -> Result<(), String> {
    let old_path = profile_path(&old_name);
    if !old_path.exists() {
        return Err(format!("配置文件不存在: {}", old_name));
    }

    let mut data: ProfileData =
        serde_json::from_str(&fs::read_to_string(&old_path).map_err(|e| format!("无法读取配置文件: {}", e))?).map_err(|e| format!("JSON 解析失败: {}", e))?;

    data.name = new_name.clone();
    data.modified = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let new_path = profile_path(&new_name);
    let json =
        serde_json::to_string_pretty(&data).map_err(|e| format!("JSON 序列化失败: {}", e))?;
    fs::write(&new_path, &json).map_err(|e| format!("无法写入配置文件: {}", e))?;

    if old_path != new_path {
        fs::remove_file(&old_path).map_err(|e| format!("无法删除旧配置文件: {}", e))?;
    }

    log::info!("已重命名配置: {} -> {}", old_name, new_name);
    Ok(())
}
