//! PATH 快照合并与 pending 落盘。
//!
//! DB 可用时由 `service` 传入 `current_snapshot`，本模块只负责与注册表合并；
//! DB 不可用时没有持久化快照，只返回注册表当前状态。
//! pending 是崩溃安全机制：注册表已写、DB 当前快照未落盘时记录，下次补写。

use crate::domain::error::{CoreError, ErrorCode};
use crate::platform::fs::atomic_write;
use crate::platform::persist::{
    migrate, parse_error_quarantined, rotate_backup, Versioned, PERSIST_SCHEMA_VERSION,
};
use crate::platform::registry;
use crate::{PathEntry, PathSnapshot};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// 生产用 pending 快照路径（`~/.patheditor/pending_path_snapshot.json`）。
///
/// 测试用 [`save_pending_path_snapshot_at`] 等显式路径变体注入 `TempDir`，
/// 不再通过 `cfg(test)` 改写同一个全局路径。
pub(crate) fn default_pending_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("pending_path_snapshot.json")
}

/// 用调用方提供的注册表值与持久化快照合并。
///
/// 抽出来是为了让保存链路可以注入 fake 注册表（集成测试），
/// 而不必读真实 HKCU/HKLM。
pub fn merge_path_snapshot(
    registry_system: Vec<String>,
    registry_user: Vec<String>,
    persisted: Option<PathSnapshot>,
) -> Result<PathSnapshot, CoreError> {
    let (system_snapshot, user_snapshot) = match persisted {
        Some(snapshot) => (snapshot.system, snapshot.user),
        None => (Vec::new(), Vec::new()),
    };

    Ok(PathSnapshot {
        system: merge_hive(registry_system, system_snapshot),
        user: merge_hive(registry_user, user_snapshot),
    })
}

fn key(path: &str) -> String {
    path.trim().to_lowercase()
}

fn merge_hive(registry_paths: Vec<String>, snapshot: Vec<PathEntry>) -> Vec<PathEntry> {
    let registry_by_key: HashMap<String, String> = registry_paths
        .iter()
        .map(|path| (key(path), path.clone()))
        .collect();
    let mut seen: HashSet<String> = HashSet::new();
    let mut merged = Vec::new();

    // 快照提供原始顺序。启用但已不在注册表中的条目视为被外部删除，不能复活；
    // 禁用条目即使已从注册表移除，也要保留下来供用户重新启用。
    for entry in snapshot {
        let entry_key = key(&entry.path);
        if entry_key.is_empty() || !seen.insert(entry_key.clone()) {
            continue;
        }

        if let Some(current_path) = registry_by_key.get(&entry_key) {
            merged.push(PathEntry {
                path: current_path.clone(),
                enabled: entry.enabled,
            });
        } else if !entry.enabled {
            merged.push(entry);
        }
    }

    // 注册表是启用路径的真相来源；快照中没有的新路径追加到末尾。
    for path in registry_paths {
        let path_key = key(&path);
        if path_key.is_empty() || !seen.insert(path_key) {
            continue;
        }
        merged.push(PathEntry {
            path,
            enabled: true,
        });
    }

    merged
}

/// 读取注册表当前值，并与持久化快照合并。
pub fn load_path_snapshot_with(persisted: Option<PathSnapshot>) -> Result<PathSnapshot, CoreError> {
    let registry_system = registry::load_system_paths()
        .map_err(|e| CoreError::new(e.code, "load_path_snapshot", e.message))?;
    let registry_user = registry::load_user_paths()
        .map_err(|e| CoreError::new(e.code, "load_path_snapshot", e.message))?;
    merge_path_snapshot(registry_system, registry_user, persisted)
}

/// 读取注册表当前状态（没有 DB 快照时的降级路径）。
pub fn load_path_snapshot() -> Result<PathSnapshot, CoreError> {
    load_path_snapshot_with(None)
}

/// 记录「注册表已写成功、数据库快照未落盘」的待补写状态，供下次运行补写。
///
/// 只保留最后一次待补写内容（覆盖式）：每次调用都会整体覆盖旧文件，
/// 避免堆积多个互相矛盾的待补写版本。
pub fn save_pending_path_snapshot(
    system: Option<Vec<PathEntry>>,
    user: Option<Vec<PathEntry>>,
) -> Result<(), CoreError> {
    save_pending_path_snapshot_at(&default_pending_path(), system, user)
}

/// [`save_pending_path_snapshot`] 的显式路径版本（测试注入 `TempDir`）。
pub fn save_pending_path_snapshot_at(
    path: &Path,
    system: Option<Vec<PathEntry>>,
    user: Option<Vec<PathEntry>>,
) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CoreError::new(
                ErrorCode::Io,
                "save_pending_path_snapshot",
                format!("无法创建配置目录: {e}"),
            )
        })?;
    }

    let snapshot = PathSnapshot {
        system: system.unwrap_or_default(),
        user: user.unwrap_or_default(),
    };
    let json = serde_json::to_string_pretty(&Versioned {
        schema_version: PERSIST_SCHEMA_VERSION,
        inner: &snapshot,
    })
    .map_err(|e| {
        CoreError::new(
            ErrorCode::Internal,
            "save_pending_path_snapshot",
            format!("JSON 序列化失败: {e}"),
        )
    })?;
    rotate_backup(path)?;
    atomic_write(path, &json).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "save_pending_path_snapshot",
            format!("无法写入待补写快照: {e}"),
        )
    })?;
    log::info!("已记录待补写快照到: {}", path.display());
    Ok(())
}

/// 读取待补写状态；文件不存在时返回 `Ok(None)`。
pub fn load_pending_path_snapshot() -> Result<Option<PathSnapshot>, CoreError> {
    load_pending_path_snapshot_at(&default_pending_path())
}

/// [`load_pending_path_snapshot`] 的显式路径版本（测试注入 `TempDir`）。
pub fn load_pending_path_snapshot_at(path: &Path) -> Result<Option<PathSnapshot>, CoreError> {
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "load_pending_path_snapshot",
            format!("无法读取待补写快照: {e}"),
        )
    })?;
    if content.trim().is_empty() {
        return Ok(None);
    }

    match serde_json::from_str::<Versioned<PathSnapshot>>(&content) {
        Ok(versioned) => migrate(versioned, "待补写快照").map(Some),
        Err(e) => Err(parse_error_quarantined(path, "待补写快照", e)),
    }
}

/// 清除待补写状态（补写成功后调用）；文件不存在时同样返回 `Ok(())`。
pub fn clear_pending_path_snapshot() -> Result<(), CoreError> {
    clear_pending_path_snapshot_at(&default_pending_path())
}

/// [`clear_pending_path_snapshot`] 的显式路径版本（测试注入 `TempDir`）。
pub fn clear_pending_path_snapshot_at(path: &Path) -> Result<(), CoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CoreError::new(
            ErrorCode::Io,
            "clear_pending_path_snapshot",
            format!("无法删除待补写快照: {e}"),
        )),
    }
}

/// 是否存在待补写状态。
pub fn has_pending_path_snapshot() -> bool {
    has_pending_path_snapshot_at(&default_pending_path())
}

/// [`has_pending_path_snapshot`] 的显式路径版本（测试注入 `TempDir`）。
pub fn has_pending_path_snapshot_at(path: &Path) -> bool {
    path.exists()
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
    fn merge_preserves_disabled_orphan_and_snapshot_order() {
        let merged = merge_hive(
            vec!["C:\\A".into(), "C:\\C".into(), "C:\\D".into()],
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
            vec![entry("C:\\A", true), entry("C:\\Gone", true)],
        );

        assert_eq!(merged, vec![entry("C:\\A", true)]);
    }

    #[test]
    fn pending_snapshot_roundtrip_lifecycle() {
        // 每测试独立 TempDir：不再共享固定临时文件，也不需要全局测试锁。
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("pending_path_snapshot.json");
        let bak = temp.path().join("pending_path_snapshot.json.bak");
        assert!(!has_pending_path_snapshot_at(&path));
        assert!(load_pending_path_snapshot_at(&path).unwrap().is_none());

        // save → load 得到同一内容（system/user 各造不同条目，含禁用项以验证 enabled 保留）
        let sys = vec![
            entry("C:\\pending_sys1", true),
            entry("C:\\pending_sys2", false),
        ];
        let usr = vec![entry("D:\\pending_usr1", false)];
        save_pending_path_snapshot_at(&path, Some(sys.clone()), Some(usr.clone())).unwrap();
        assert!(has_pending_path_snapshot_at(&path));
        let loaded = load_pending_path_snapshot_at(&path)
            .unwrap()
            .expect("应有待补写快照");
        assert_eq!(loaded.system, sys);
        assert_eq!(loaded.user, usr);
        // 写入端补上 schemaVersion=1；首次保存无 .bak
        let content = fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(v["schemaVersion"], 1);
        assert!(!bak.exists(), "首次保存不应产生 .bak");

        // 第二次保存：上一份轮换为 .bak
        let sys2 = vec![entry("C:\\pending_new", true)];
        save_pending_path_snapshot_at(&path, Some(sys2.clone()), None).unwrap();
        assert!(bak.exists(), "第二次保存应把上一份轮换为 .bak");
        let loaded2 = load_pending_path_snapshot_at(&path)
            .unwrap()
            .expect("应有 pending");
        assert_eq!(loaded2.system, sys2);

        // clear → has=false → load 再回 Ok(None)
        clear_pending_path_snapshot_at(&path).unwrap();
        assert!(!has_pending_path_snapshot_at(&path));
        assert!(load_pending_path_snapshot_at(&path).unwrap().is_none());
    }
}
