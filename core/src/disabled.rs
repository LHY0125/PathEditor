use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn disabled_file_path() -> PathBuf {
    dirs::data_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("PathEditor")
        .join("disabled.json")
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
        fs::create_dir_all(parent)
            .map_err(|e| format!("无法创建配置目录: {}", e))?;
    }

    let json = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("JSON 序列化失败: {}", e))?;

    fs::write(&path, &json)
        .map_err(|e| format!("无法写入 disabled.json: {}", e))?;

    log::info!("已保存禁用状态到: {}", path.display());
    Ok(())
}

/// 加载禁用路径列表，返回 (system_disabled, user_disabled)
pub fn load_disabled_state() -> Result<(Vec<String>, Vec<String>), String> {
    let path = disabled_file_path();

    if !path.exists() {
        return Ok((vec![], vec![]));
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("无法读取 disabled.json: {}", e))?;

    if content.trim().is_empty() {
        return Ok((vec![], vec![]));
    }

    let state: DisabledState = serde_json::from_str(&content)
        .map_err(|e| format!("JSON 解析失败: {}", e))?;

    Ok((state.system, state.user))
}
