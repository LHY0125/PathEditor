use crate::fs::atomic_write;
use crate::registry;
use crate::{PathEntry, PathSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
struct DisabledState {
    #[serde(default)]
    system: Vec<String>,
    #[serde(default)]
    user: Vec<String>,
    /// 完整有序快照，用于让被禁用的路径在注册表移除后仍能跨重启恢复。
    #[serde(default, rename = "systemSnapshot")]
    system_snapshot: Vec<PathEntry>,
    #[serde(default, rename = "userSnapshot")]
    user_snapshot: Vec<PathEntry>,
}

fn read_state() -> Result<DisabledState, String> {
    let path = disabled_file_path();
    if !path.exists() {
        return Ok(DisabledState::default());
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("无法读取 disabled.json: {}", e))?;
    if content.trim().is_empty() {
        return Ok(DisabledState::default());
    }

    serde_json::from_str(&content).map_err(|e| format!("JSON 解析失败: {}", e))
}

fn write_state(state: &DisabledState) -> Result<(), String> {
    let path = disabled_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("无法创建配置目录: {}", e))?;
    }

    let json =
        serde_json::to_string_pretty(state).map_err(|e| format!("JSON 序列化失败: {}", e))?;
    atomic_write(&path, &json).map_err(|e| format!("无法写入 disabled.json: {}", e))?;
    log::info!("已保存禁用状态到: {}", path.display());
    Ok(())
}

fn key(path: &str) -> String {
    path.trim().to_lowercase()
}

fn disabled_lists(entries: &[PathEntry]) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| !entry.enabled)
        .map(|entry| entry.path.clone())
        .collect()
}

fn merge_hive(
    registry_paths: Vec<String>,
    legacy_disabled: Vec<String>,
    snapshot: Vec<PathEntry>,
) -> Vec<PathEntry> {
    let registry_by_key: HashMap<String, String> = registry_paths
        .iter()
        .map(|path| (key(path), path.clone()))
        .collect();
    let disabled_keys: HashSet<String> = legacy_disabled.iter().map(|path| key(path)).collect();
    let mut seen: HashSet<String> = HashSet::new();
    let mut merged = Vec::new();

    // 快照提供原始顺序。启用但已不在注册表中的条目视为被外部删除，不能复活。
    for entry in snapshot {
        let entry_key = key(&entry.path);
        if entry_key.is_empty() || !seen.insert(entry_key.clone()) {
            continue;
        }

        if let Some(current_path) = registry_by_key.get(&entry_key) {
            merged.push(PathEntry {
                path: current_path.clone(),
                enabled: entry.enabled && !disabled_keys.contains(&entry_key),
            });
        } else if !entry.enabled {
            merged.push(entry);
        }
    }

    // 注册表是启用路径的真相来源；快照中没有的新路径追加到末尾。
    for path in registry_paths {
        let path_key = key(&path);
        if path_key.is_empty() || !seen.insert(path_key.clone()) {
            continue;
        }
        merged.push(PathEntry {
            path,
            enabled: !disabled_keys.contains(&path_key),
        });
    }

    // 兼容旧版 disabled.json：只有禁用字符串、没有完整快照时，把孤儿禁用项补到末尾。
    for path in legacy_disabled {
        let path_key = key(&path);
        if path_key.is_empty() || !seen.insert(path_key) {
            continue;
        }
        merged.push(PathEntry {
            path,
            enabled: false,
        });
    }

    merged
}

/// 保存旧的禁用字符串接口；新代码应优先使用 `save_path_snapshot` 以保持顺序。
pub fn save_disabled_state(system: Vec<String>, user: Vec<String>) -> Result<(), String> {
    let mut state = read_state()?;
    state.system = system;
    state.user = user;
    write_state(&state)
}

/// 加载旧的禁用字符串接口。
pub fn load_disabled_state() -> Result<(Vec<String>, Vec<String>), String> {
    let state = read_state()?;
    Ok((state.system, state.user))
}

/// 保存一个或多个 hive 的完整有序快照。传 `None` 表示保留该 hive 的现有状态。
pub fn save_path_snapshot(
    system: Option<Vec<PathEntry>>,
    user: Option<Vec<PathEntry>>,
) -> Result<(), String> {
    let mut state = read_state()?;
    if let Some(entries) = system {
        state.system = disabled_lists(&entries);
        state.system_snapshot = entries;
    }
    if let Some(entries) = user {
        state.user = disabled_lists(&entries);
        state.user_snapshot = entries;
    }
    write_state(&state)
}

/// 合并注册表当前值与持久化快照，返回包含禁用项和原始顺序的完整快照。
pub fn load_path_snapshot() -> Result<PathSnapshot, String> {
    let registry_system = registry::load_system_paths()?;
    let registry_user = registry::load_user_paths()?;
    let state = read_state()?;

    Ok(PathSnapshot {
        system: merge_hive(registry_system, state.system, state.system_snapshot),
        user: merge_hive(registry_user, state.user, state.user_snapshot),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, enabled: bool) -> PathEntry {
        PathEntry {
            path: path.to_string(),
            enabled,
        }
    }

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

    #[test]
    fn merge_preserves_disabled_orphan_and_snapshot_order() {
        let merged = merge_hive(
            vec!["C:\\A".into(), "C:\\C".into(), "C:\\D".into()],
            vec!["C:\\B".into()],
            vec![
                entry("C:\\A", true),
                entry("C:\\B", false),
                entry("C:\\C", true),
            ],
        );

        assert_eq!(
            merged,
            vec![
                entry("C:\\A", true),
                entry("C:\\B", false),
                entry("C:\\C", true),
                entry("C:\\D", true),
            ]
        );
    }

    #[test]
    fn merge_drops_enabled_entries_missing_from_registry() {
        let merged = merge_hive(
            vec!["C:\\A".into()],
            vec![],
            vec![entry("C:\\A", true), entry("C:\\Gone", true)],
        );

        assert_eq!(merged, vec![entry("C:\\A", true)]);
    }

    #[test]
    fn legacy_disabled_orphan_is_restored_without_snapshot() {
        let merged = merge_hive(vec!["C:\\A".into()], vec!["C:\\Old".into()], vec![]);

        assert_eq!(merged, vec![entry("C:\\A", true), entry("C:\\Old", false)]);
    }
}
