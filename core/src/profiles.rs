use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn profiles_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("profiles")
}

fn validate_profile_name(name: &str) -> Result<(), String> {
    if name.is_empty() { return Err("配置名称不能为空".into()); }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("配置名称包含非法字符".into());
    }
    for ch in name.chars() {
        if "<>:\"|?*".contains(ch) {
            return Err("配置名称包含非法字符".into());
        }
    }
    Ok(())
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
pub fn list_profiles() -> Result<Vec<ProfileMeta>, String> {
    let dir = profiles_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut profiles: Vec<ProfileMeta> = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("无法读取配置目录: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
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
pub fn save_profile(
    name: &str,
    sys: Vec<ProfilePathEntry>,
    user: Vec<ProfilePathEntry>,
) -> Result<(), String> {
    validate_profile_name(name)?;
    let dir = profiles_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("无法创建配置目录: {}", e))?;

    let path = profile_path(name);
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    // 覆盖已有配置时保留原始创建时间
    let created = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str::<ProfileData>(&c).ok())
            .map(|d| d.created)
            .unwrap_or_else(|| now.clone())
    } else {
        now.clone()
    };

    let data = ProfileData {
        name: name.to_string(),
        sys,
        user,
        created,
        modified: now,
    };

    let json =
        serde_json::to_string_pretty(&data).map_err(|e| format!("JSON 序列化失败: {}", e))?;
    fs::write(&path, &json).map_err(|e| format!("无法写入配置文件: {}", e))?;

    log::info!("已保存配置: {}", path.display());
    Ok(())
}

/// 加载配置文件
pub fn load_profile(name: &str) -> Result<ProfileData, String> {
    validate_profile_name(name)?;
    let path = profile_path(name);
    if !path.exists() {
        return Err(format!("配置文件不存在: {}", name));
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("无法读取配置文件: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("JSON 解析失败: {}", e))
}

/// 删除配置文件
pub fn delete_profile(name: &str) -> Result<(), String> {
    validate_profile_name(name)?;
    let path = profile_path(name);
    fs::remove_file(&path).map_err(|e| format!("无法删除配置文件: {}", e))?;
    log::info!("已删除配置: {}", path.display());
    Ok(())
}

/// 重命名配置文件
pub fn rename_profile(old_name: &str, new_name: &str) -> Result<(), String> {
    validate_profile_name(old_name)?;
    validate_profile_name(new_name)?;
    let old_path = profile_path(old_name);
    if !old_path.exists() {
        return Err(format!("配置文件不存在: {}", old_name));
    }

    let mut data: ProfileData =
        serde_json::from_str(&fs::read_to_string(&old_path).map_err(|e| format!("无法读取配置文件: {}", e))?).map_err(|e| format!("JSON 解析失败: {}", e))?;

    data.name = new_name.to_string();
    data.modified = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let new_path = profile_path(new_name);
    let json =
        serde_json::to_string_pretty(&data).map_err(|e| format!("JSON 序列化失败: {}", e))?;
    fs::write(&new_path, &json).map_err(|e| format!("无法写入配置文件: {}", e))?;

    if old_path != new_path {
        fs::remove_file(&old_path).map_err(|e| format!("无法删除旧配置文件: {}", e))?;
    }

    log::info!("已重命名配置: {} -> {}", old_name, new_name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry(path: &str) -> ProfilePathEntry {
        ProfilePathEntry { path: path.into(), enabled: true }
    }

    #[test]
    fn validate_name_rejects_empty() {
        assert!(validate_profile_name("").is_err());
    }

    #[test]
    fn validate_name_rejects_path_traversal() {
        assert!(validate_profile_name("../../evil").is_err());
        assert!(validate_profile_name("foo\\bar").is_err());
    }

    #[test]
    fn validate_name_rejects_reserved_chars() {
        assert!(validate_profile_name("foo:bar").is_err());
        assert!(validate_profile_name("foo<bar").is_err());
    }

    #[test]
    fn profile_crud() {
        // save -> load -> delete
        let name = "__test_profile_crud";
        let _ = delete_profile(name);
        save_profile(name, vec![test_entry("C:\\sys")], vec![test_entry("D:\\usr")]).unwrap();
        let loaded = load_profile(name).unwrap();
        assert_eq!(loaded.sys[0].path, "C:\\sys");
        delete_profile(name).unwrap();
        assert!(load_profile(name).is_err());

        // rename
        let old_name = "__test_rename_old";
        let new_name = "__test_rename_new";
        let _ = delete_profile(old_name);
        let _ = delete_profile(new_name);
        save_profile(old_name, vec![test_entry("C:\\x")], vec![]).unwrap();
        rename_profile(old_name, new_name).unwrap();
        assert!(load_profile(old_name).is_err());
        let renamed = load_profile(new_name).unwrap();
        assert_eq!(renamed.name, new_name);
        delete_profile(new_name).unwrap();

        // list
        let _ = delete_profile("__test_list_a");
        let _ = delete_profile("__test_list_b");
        save_profile("__test_list_a", vec![], vec![]).unwrap();
        save_profile("__test_list_b", vec![], vec![]).unwrap();
        let list = list_profiles().unwrap();
        let names: Vec<&str> = list.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"__test_list_a"));
        delete_profile("__test_list_a").unwrap();
        delete_profile("__test_list_b").unwrap();
    }
}
