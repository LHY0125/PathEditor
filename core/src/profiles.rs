use crate::error::{CoreError, ErrorCode};
use crate::fs::atomic_write;
use crate::path_entry::PathEntry;
use crate::persist::{
    migrate, parse_error_quarantined, read_versioned_file, rotate_backup, Versioned,
    PERSIST_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[cfg(not(test))]
fn profiles_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("profiles")
}

#[cfg(test)]
fn profiles_dir() -> PathBuf {
    std::env::temp_dir().join("patheditor_test_profiles")
}

fn validate_profile_name(name: &str) -> Result<(), CoreError> {
    if name.is_empty() {
        return Err(CoreError::new(
            ErrorCode::InvalidName,
            "validate_profile_name",
            "配置名称不能为空",
        ));
    }
    if name.len() > 255 {
        return Err(CoreError::new(
            ErrorCode::InvalidName,
            "validate_profile_name",
            "配置名称过长（最大 255 字符）",
        ));
    }
    if name.contains('\0') || name.chars().any(|c| c.is_control()) {
        return Err(CoreError::new(
            ErrorCode::InvalidName,
            "validate_profile_name",
            "配置名称包含非法字符",
        ));
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(CoreError::new(
            ErrorCode::InvalidName,
            "validate_profile_name",
            "配置名称包含非法字符",
        ));
    }
    for ch in name.chars() {
        if "<>:\"|?*".contains(ch) {
            return Err(CoreError::new(
                ErrorCode::InvalidName,
                "validate_profile_name",
                "配置名称包含非法字符",
            ));
        }
    }
    Ok(())
}

fn profile_path(name: &str) -> PathBuf {
    profiles_dir().join(format!("{}.json", name))
}

/// 兼容旧调用方的名称，底层复用统一的 PathEntry。
pub type ProfilePathEntry = PathEntry;

/// 配置元数据（列表返回用）。
#[derive(Serialize, Deserialize, Debug)]
pub struct ProfileMeta {
    pub name: String,
    pub created: String,
    pub modified: String,
}

/// 配置文件内容；持久化时包装在 `Versioned` 信封中携带 `schemaVersion`。
#[derive(Serialize, Deserialize, Debug)]
pub struct ProfileData {
    pub name: String,
    pub sys: Vec<ProfilePathEntry>,
    pub user: Vec<ProfilePathEntry>,
    pub created: String,
    pub modified: String,
}

/// 列出所有配置文件的元数据。
pub fn list_profiles() -> Result<Vec<ProfileMeta>, CoreError> {
    let dir = profiles_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut profiles: Vec<ProfileMeta> = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "list_profiles",
            format!("无法读取配置目录: {e}"),
        )
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        // 单个配置文件损坏不影响列表：跳过并告警（保持既有容错语义）。
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Ok(versioned) = serde_json::from_str::<Versioned<ProfileData>>(&content) {
            let version = versioned.schema_version;
            if let Ok(data) = migrate(versioned, "配置文件") {
                profiles.push(ProfileMeta {
                    name: data.name,
                    created: data.created,
                    modified: data.modified,
                });
                continue;
            }
            log::warn!("配置文件 {} 版本过高（{version}），已跳过", path.display());
        }
    }

    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(profiles)
}

/// 保存当前 PATH 为配置文件。
pub fn save_profile(
    name: &str,
    sys: Vec<ProfilePathEntry>,
    user: Vec<ProfilePathEntry>,
) -> Result<(), CoreError> {
    validate_profile_name(name)?;
    let dir = profiles_dir();
    fs::create_dir_all(&dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "save_profile",
            format!("无法创建配置目录: {e}"),
        )
    })?;

    let path = profile_path(name);
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    // 覆盖已有配置时保留原始创建时间
    let created = if path.exists() {
        read_profile_file(&path)
            .ok()
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

    let json = serde_json::to_string_pretty(&Versioned {
        schema_version: PERSIST_SCHEMA_VERSION,
        inner: &data,
    })
    .map_err(|e| {
        CoreError::new(
            ErrorCode::Internal,
            "save_profile",
            format!("JSON 序列化失败: {e}"),
        )
    })?;
    rotate_backup(&path)?;
    atomic_write(&path, &json).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "save_profile",
            format!("无法写入配置文件: {e}"),
        )
    })?;

    log::info!("已保存配置: {}", path.display());
    Ok(())
}

/// 读取单个配置文件（含版本信封解析与损坏隔离）。
fn read_profile_file(path: &std::path::Path) -> Result<ProfileData, CoreError> {
    let versioned = read_versioned_file(path, "配置文件")?;
    migrate(versioned, "配置文件")
}

/// 加载配置文件。
pub fn load_profile(name: &str) -> Result<ProfileData, CoreError> {
    validate_profile_name(name)?;
    let path = profile_path(name);
    if !path.exists() {
        return Err(CoreError::new(
            ErrorCode::NotFound,
            "load_profile",
            format!("配置文件不存在: {name}"),
        ));
    }
    let content = fs::read_to_string(&path).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "load_profile",
            format!("无法读取配置文件: {e}"),
        )
    })?;
    serde_json::from_str::<Versioned<ProfileData>>(&content)
        .map_err(|e| parse_error_quarantined(&path, "配置文件", e))
        .and_then(|versioned| migrate(versioned, "配置文件"))
}

/// 删除配置文件。
pub fn delete_profile(name: &str) -> Result<(), CoreError> {
    validate_profile_name(name)?;
    let path = profile_path(name);
    fs::remove_file(&path).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "delete_profile",
            format!("无法删除配置文件: {e}"),
        )
    })?;
    log::info!("已删除配置: {}", path.display());
    Ok(())
}

/// 重命名配置文件。
pub fn rename_profile(old_name: &str, new_name: &str) -> Result<(), CoreError> {
    validate_profile_name(old_name)?;
    validate_profile_name(new_name)?;
    let old_path = profile_path(old_name);
    let new_path = profile_path(new_name);
    if !old_path.exists() {
        return Err(CoreError::new(
            ErrorCode::NotFound,
            "rename_profile",
            format!("配置文件不存在: {old_name}"),
        ));
    }
    if old_path != new_path && new_path.exists() {
        return Err(CoreError::new(
            ErrorCode::NameExists,
            "rename_profile",
            format!("目标配置名已存在: {new_name}"),
        ));
    }

    let mut data: ProfileData = serde_json::from_str::<Versioned<ProfileData>>(
        &fs::read_to_string(&old_path).map_err(|e| {
            CoreError::new(
                ErrorCode::Io,
                "rename_profile",
                format!("无法读取配置文件: {e}"),
            )
        })?,
    )
    .map_err(|e| parse_error_quarantined(&old_path, "配置文件", e))
    .and_then(|versioned| migrate(versioned, "配置文件"))?;

    data.name = new_name.to_string();
    data.modified = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let json = serde_json::to_string_pretty(&Versioned {
        schema_version: PERSIST_SCHEMA_VERSION,
        inner: &data,
    })
    .map_err(|e| {
        CoreError::new(
            ErrorCode::Internal,
            "rename_profile",
            format!("JSON 序列化失败: {e}"),
        )
    })?;
    rotate_backup(&new_path)?;
    atomic_write(&new_path, &json).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "rename_profile",
            format!("无法写入配置文件: {e}"),
        )
    })?;

    if old_path != new_path {
        fs::remove_file(&old_path).map_err(|e| {
            CoreError::new(
                ErrorCode::Io,
                "rename_profile",
                format!("无法删除旧配置文件: {e}"),
            )
        })?;
    }

    log::info!(
        "已重命名配置: {} -> {}",
        old_path.display(),
        new_path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry(path: &str) -> ProfilePathEntry {
        ProfilePathEntry {
            path: path.into(),
            enabled: true,
        }
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
    fn validate_name_rejects_null_bytes() {
        assert!(validate_profile_name("foo\0bar").is_err());
    }

    #[test]
    fn validate_name_rejects_control_chars() {
        assert!(validate_profile_name("foo\tbar").is_err());
        assert!(validate_profile_name("foo\nbar").is_err());
    }

    #[test]
    fn validate_name_rejects_too_long() {
        let long_name = "a".repeat(256);
        assert!(validate_profile_name(&long_name).is_err());
    }

    #[test]
    fn validate_name_accepts_255_chars() {
        let name = "a".repeat(255);
        assert!(validate_profile_name(&name).is_ok());
    }

    #[test]
    fn profile_crud() {
        // save -> load -> delete
        let name = "__test_profile_crud";
        let _ = delete_profile(name);
        save_profile(
            name,
            vec![test_entry("C:\\sys")],
            vec![test_entry("D:\\usr")],
        )
        .unwrap();
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

    // ── F-11：schemaVersion / .bak / quarantine 测试（profiles）──

    #[test]
    fn legacy_profile_without_schema_version_reads_as_v1() {
        let name = "__test_f11_legacy";
        let path = profile_path(name);
        let _ = fs::remove_file(&path);
        fs::create_dir_all(profiles_dir()).unwrap();
        // 旧格式：无 schemaVersion
        let legacy = r#"{
  "name": "__test_f11_legacy",
  "sys": [{"path": "C:\\old", "enabled": true}],
  "user": [],
  "created": "2025-01-01T00:00:00",
  "modified": "2025-01-01T00:00:00"
}"#;
        fs::write(&path, legacy).unwrap();

        let data = load_profile(name).unwrap();
        assert_eq!(data.name, "__test_f11_legacy");
        assert_eq!(data.sys[0].path, "C:\\old");

        // 重新保存后写入端补上 schemaVersion
        save_profile(name, vec![test_entry("C:\\new")], vec![]).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(v["schemaVersion"], 1);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn truncated_profile_is_quarantined_and_returns_parse() {
        let name = "__test_f11_trunc";
        let path = profile_path(name);
        let _ = fs::remove_file(&path);
        fs::create_dir_all(profiles_dir()).unwrap();
        fs::write(&path, r#"{"name": "__test_f11_trunc", "sys": [{"path":"C:"#).unwrap();

        let err = load_profile(name).unwrap_err();
        assert_eq!(err.code, ErrorCode::Parse, "实际错误: {err}");
        assert!(!path.exists(), "坏文件应被隔离移走");
        let parent = path.parent().unwrap();
        let quarantined: Vec<_> = fs::read_dir(parent)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("__test_f11_trunc.json.corrupt-"))
            .collect();
        assert_eq!(quarantined.len(), 1, "应恰好产生一个隔离文件");
        fs::remove_file(parent.join(&quarantined[0])).ok();
    }

    #[test]
    fn future_schema_version_profile_returns_parse_with_hint() {
        let name = "__test_f11_future";
        let path = profile_path(name);
        let _ = fs::remove_file(&path);
        fs::create_dir_all(profiles_dir()).unwrap();
        fs::write(
            &path,
            r#"{"schemaVersion": 999, "name": "__test_f11_future", "sys": [], "user": [], "created": "", "modified": ""}"#,
        )
        .unwrap();

        let err = load_profile(name).unwrap_err();
        assert_eq!(err.code, ErrorCode::Parse);
        assert!(
            err.message.contains("更新版本"),
            "错误应提示文件由更新版本写入: {err}"
        );
        assert!(path.exists(), "版本过高的文件不应被隔离");
        fs::remove_file(&path).ok();
    }

    #[test]
    fn profile_save_rotates_previous_file_to_bak() {
        let name = "__test_f11_bak";
        let path = profile_path(name);
        let bak = path.with_file_name("__test_f11_bak.json.bak");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&bak);
        fs::create_dir_all(profiles_dir()).unwrap();

        save_profile(name, vec![test_entry("C:\\first")], vec![]).unwrap();
        assert!(!bak.exists(), "首次保存不应产生 .bak");

        save_profile(name, vec![test_entry("C:\\second")], vec![]).unwrap();
        assert!(bak.exists(), "第二次保存应把上一份轮换为 .bak");
        let bak_v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&bak).unwrap()).unwrap();
        assert_eq!(bak_v["sys"][0]["path"], "C:\\first");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&bak);
    }
}
