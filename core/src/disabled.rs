use crate::fs::atomic_write;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[cfg(not(test))]
fn disabled_file_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("disabled.json")
}

#[cfg(test)]
fn disabled_file_path() -> PathBuf {
    std::env::temp_dir().join("patheditor_test_disabled.json")
}

#[derive(Serialize, Deserialize, Default)]
struct DisabledState {
    #[serde(default)]
    system: Vec<String>,
    #[serde(default)]
    user: Vec<String>,
}

/// 保存禁用路径列表（即时持久化，不依赖注册表保存按钮）
pub fn save_disabled_state(system: Vec<String>, user: Vec<String>) -> Result<(), String> {
    let state = DisabledState { system, user };
    let path = disabled_file_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("无法创建配置目录: {}", e))?;
    }

    let json =
        serde_json::to_string_pretty(&state).map_err(|e| format!("JSON 序列化失败: {}", e))?;

    atomic_write(&path, &json).map_err(|e| format!("无法写入 disabled.json: {}", e))?;

    log::info!("已保存禁用状态到: {}", path.display());
    Ok(())
}

/// 加载禁用路径列表，返回 (system_disabled, user_disabled)
pub fn load_disabled_state() -> Result<(Vec<String>, Vec<String>), String> {
    let path = disabled_file_path();

    if !path.exists() {
        return Ok((vec![], vec![]));
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("无法读取 disabled.json: {}", e))?;

    if content.trim().is_empty() {
        return Ok((vec![], vec![]));
    }

    let state: DisabledState =
        serde_json::from_str(&content).map_err(|e| format!("JSON 解析失败: {}", e))?;

    Ok((state.system, state.user))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_state() {
        // roundtrip
        let sys = vec!["C:\\sys1".into(), "C:\\sys2".into()];
        let usr = vec!["D:\\usr1".into()];
        save_disabled_state(sys.clone(), usr.clone()).unwrap();
        let (loaded_sys, loaded_usr) = load_disabled_state().unwrap();
        assert_eq!(loaded_sys, sys);
        assert_eq!(loaded_usr, usr);

        // overwrite
        let new_sys = vec!["C:\\new".into()];
        save_disabled_state(new_sys.clone(), vec![]).unwrap();
        let (loaded, _) = load_disabled_state().unwrap();
        assert_eq!(loaded, new_sys);

        // empty
        save_disabled_state(vec![], vec![]).unwrap();
        let result = load_disabled_state().unwrap();
        assert!(result.0.is_empty() && result.1.is_empty());
    }
}
