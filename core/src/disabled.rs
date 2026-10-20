use crate::error::{CoreError, ErrorCode};
use crate::fs::atomic_write;
use crate::persist::{
    migrate, parse_error_quarantined, rotate_backup, Versioned, PERSIST_SCHEMA_VERSION,
};
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

#[cfg(not(test))]
fn pending_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("pending_path_snapshot.json")
}

#[cfg(test)]
fn pending_path() -> PathBuf {
    std::env::temp_dir().join("patheditor_test_pending_snapshot.json")
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

fn read_state() -> Result<DisabledState, CoreError> {
    let path = disabled_file_path();
    if !path.exists() {
        return Ok(DisabledState::default());
    }

    // 空白文件保持既有语义：按默认值处理（不算损坏，不隔离）。
    let content = fs::read_to_string(&path).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "load_disabled_state",
            format!("无法读取 disabled.json: {e}"),
        )
    })?;
    if content.trim().is_empty() {
        return Ok(DisabledState::default());
    }

    let versioned: Versioned<DisabledState> = serde_json::from_str(&content)
        .map_err(|e| parse_error_quarantined(&path, "disabled.json", e))?;
    migrate(versioned, "disabled.json")
}

/// 本次进程是否应停写 v5 文件（spec §7.8 H7）。
///
/// **为什么用进程级标志而不是给每个函数加 `Db` 参数**：`disabled` 是模块级无状态
/// 函数集合，几十个调用点；传 `Db` 会污染整个 API，而它需要知道的只是
/// 「迁移是否已完成」这一个**只读的启动期事实**。
///
/// 这与 spec §3.2 禁止的「全局 Db 单例」不同：这里不持有任何服务或连接，
/// 只是一个布尔开关，且只能由启动序列设置一次。
static V5_WRITES_DISABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// 由启动序列调用：DB 可用且 v5 迁移已完成时，停写 `disabled.json`（spec §7.8）。
///
/// **降级模式（DB 打不开）必须传 `false`** —— 那时 `disabled.json` 是唯一的状态
/// 兜底（spec §7.8 的兜底要求），停写会让禁用状态彻底丢失。
///
/// 用 `AtomicBool` 而非 `OnceLock`：**可测性**。`OnceLock` 只能设一次，
/// 测试无法在同一进程里验证开与关两种状态。生产路径仍只在启动时设置一次。
pub fn set_v5_writes_disabled(disabled: bool) {
    V5_WRITES_DISABLED.store(disabled, std::sync::atomic::Ordering::Relaxed);
}

/// 当前是否停写 v5 文件。默认 `false`（未设置时保持旧行为，测试零改动）。
fn v5_writes_disabled() -> bool {
    V5_WRITES_DISABLED.load(std::sync::atomic::Ordering::Relaxed)
}

/// 从 DB 取「当前 PATH 状态」的注入点（方案 A）。
///
/// **为什么用注入而不是直接调 `db` 模块**：`db` 已经依赖 `disabled`
/// （`migrate_v5` 读它），反向依赖会成环。注入把「状态从哪来」这件事
/// 交给启动序列决定，`disabled` 只负责调用。
///
/// 返回 `(system, user)` 的完整有序快照；`None` 表示 DB 侧没有可用状态
/// （未注入 / 库不可用 / 尚无 current 快照），此时**回落到读文件**——
/// 这正是 spec §7.8 的降级兜底要求。
type CurrentSnapshotLoader =
    Box<dyn Fn() -> Option<(Vec<PathEntry>, Vec<PathEntry>)> + Send + Sync>;

static CURRENT_SNAPSHOT_LOADER: std::sync::OnceLock<CurrentSnapshotLoader> =
    std::sync::OnceLock::new();

/// 由启动序列注册「从 DB 取当前快照」的实现。
///
/// 与 [`set_v5_writes_disabled`] 配套：停写生效后，读也必须改从 DB 来，
/// 否则会读到一份**不再更新**的文件（D-1）。
pub fn set_current_snapshot_loader(loader: CurrentSnapshotLoader) {
    let _ = CURRENT_SNAPSHOT_LOADER.set(loader);
}

/// 尝试从 DB 取当前状态；未注入或取不到时返回 `None`。
fn load_state_from_db() -> Option<DisabledState> {
    let loader = CURRENT_SNAPSHOT_LOADER.get()?;
    let (system, user) = loader()?;
    // DB 侧存的是**完整有序快照**（含 enabled 标记），直接映射成 DisabledState：
    // `system`/`user` 两个「禁用名列表」由快照派生，与文件版的语义一致。
    Some(DisabledState {
        system: disabled_lists(&system),
        user: disabled_lists(&user),
        system_snapshot: system,
        user_snapshot: user,
    })
}

fn write_state(state: &DisabledState) -> Result<(), CoreError> {
    // spec §7.8 H7：迁移成功后正常模式不再写 disabled.json。
    // 状态已由 `app_state.current_snapshot_id` 承载（见 `commands::service`）。
    if v5_writes_disabled() {
        log::debug!("v5 文件停写已启用，跳过 disabled.json 写入");
        return Ok(());
    }
    let path = disabled_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CoreError::new(
                ErrorCode::Io,
                "save_disabled_state",
                format!("无法创建配置目录: {e}"),
            )
        })?;
    }

    let json = serde_json::to_string_pretty(&Versioned {
        schema_version: PERSIST_SCHEMA_VERSION,
        inner: state,
    })
    .map_err(|e| {
        CoreError::new(
            ErrorCode::Internal,
            "save_disabled_state",
            format!("JSON 序列化失败: {e}"),
        )
    })?;
    // 写入前把上一份主文件轮换为 .bak；轮换失败则中止写入（不丢回滚副本）。
    rotate_backup(&path)?;
    atomic_write(&path, &json).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "save_disabled_state",
            format!("无法写入 disabled.json: {e}"),
        )
    })?;
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
pub fn save_disabled_state(system: Vec<String>, user: Vec<String>) -> Result<(), CoreError> {
    let mut state = read_state()?;
    state.system = system;
    state.user = user;
    write_state(&state)
}

/// 加载旧的禁用字符串接口。
pub fn load_disabled_state() -> Result<(Vec<String>, Vec<String>), CoreError> {
    let state = read_state()?;
    Ok((state.system, state.user))
}

/// 保存一个或多个 hive 的完整有序快照。传 `None` 表示保留该 hive 的现有状态。
pub fn save_path_snapshot(
    system: Option<Vec<PathEntry>>,
    user: Option<Vec<PathEntry>>,
) -> Result<(), CoreError> {
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
pub fn load_path_snapshot() -> Result<PathSnapshot, CoreError> {
    let registry_system = registry::load_system_paths()?;
    let registry_user = registry::load_user_paths()?;
    // **D-1（方案 A）**：停写生效后，状态从 DB 来而不是文件。
    //
    // 停写让 `disabled.json` 冻结在迁移那一刻；继续读它意味着「读一份永不更新的
    // 快照」。DB 的 `kind=current` 快照才是活的真相（由 `commands::service` 的
    // `update_current_snapshot` 维护）。
    //
    // DB 取不到时回落读文件 —— 降级模式（spec §7.8 的兜底要求）下文件仍是
    // 唯一来源，因为那时 `set_v5_writes_disabled(false)` 会让写路径继续更新它。
    let state = if v5_writes_disabled() {
        match load_state_from_db() {
            Some(s) => s,
            None => {
                log::debug!("DB 侧无当前快照，回落读 disabled.json");
                read_state()?
            }
        }
    } else {
        read_state()?
    };

    Ok(PathSnapshot {
        system: merge_hive(registry_system, state.system, state.system_snapshot),
        user: merge_hive(registry_user, state.user, state.user_snapshot),
    })
}

/// 记录「注册表已写成功、sidecar 快照未落盘」的待补写状态，供下次运行补写。
///
/// 只保留最后一次待补写内容（覆盖式）：每次调用都会整体覆盖旧文件，
/// 避免堆积多个互相矛盾的待补写版本。
pub fn save_pending_path_snapshot(
    system: Option<Vec<PathEntry>>,
    user: Option<Vec<PathEntry>>,
) -> Result<(), CoreError> {
    let path = pending_path();
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
    rotate_backup(&path)?;
    atomic_write(&path, &json).map_err(|e| {
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
    let path = pending_path();
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path).map_err(|e| {
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
        Err(e) => Err(parse_error_quarantined(&path, "待补写快照", e)),
    }
}

/// 清除待补写状态（补写成功后调用）；文件不存在时同样返回 `Ok(())`。
pub fn clear_pending_path_snapshot() -> Result<(), CoreError> {
    let path = pending_path();
    match fs::remove_file(&path) {
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
    pending_path().exists()
}

// merge_hive 的 golden 用例（F-10 第 5 类禁用项兼容性）。
// merge_hive 是本文件私有 fn，只有子模块可访问。
#[cfg(test)]
#[path = "registry/golden/merge_golden_tests.rs"]
mod merge_golden_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, enabled: bool) -> PathEntry {
        PathEntry {
            path: path.to_string(),
            enabled,
        }
    }

    /// **P1-4 行为测试**：停写开关真的阻止文件写入。
    ///
    /// 用 `AtomicBool` 而非 `OnceLock` 正是为了这条测试——它需要在同一进程里
    /// 验证开与关两种状态。测试结束必须复位，否则会污染同进程的其它测试。
    #[test]
    fn v5_writes_disabled_skips_file_write() {
        let _guard = crate::persist::test_persist_lock();
        let path = disabled_file_path();
        let _ = fs::remove_file(&path);

        // ① 默认（不停写）：写入真的落盘
        set_v5_writes_disabled(false);
        save_path_snapshot(Some(vec![entry("C:\\kept", true)]), None).unwrap();
        assert!(path.exists(), "默认状态下必须写文件");

        // ② 停写：再写一次，文件内容**不变**（不是被删，是没被更新）
        let before = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_file(&path);
        set_v5_writes_disabled(true);
        save_path_snapshot(Some(vec![entry("C:\\skipped", true)]), None).unwrap();
        assert!(!path.exists(), "停写状态下不得创建/更新 disabled.json");
        // 停写时 `write_state` 提前返回 `Ok(())`，不报错——调用方无感

        // ③ 复位，避免污染同进程其它测试
        set_v5_writes_disabled(false);
        let _ = fs::remove_file(&path);
        let _ = before;
    }

    // disabled.json 持久化原语（roundtrip / schemaVersion / .bak / quarantine）
    // 按 pending 快照原语的同款约定合并为一个生命周期测试：所有测试共享固定
    // 临时路径（std::env::temp_dir()），F-11 测试还会直接写原始文件内容，
    // 拆成多个测试会有并行竞态。
    #[test]
    fn disabled_state_lifecycle_with_schema_backup_and_quarantine() {
        let _guard = crate::persist::test_persist_lock();
        let path = disabled_file_path();
        let bak = path.with_file_name("patheditor_test_disabled.json.bak");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&bak);

        // roundtrip
        let sys = vec!["C:\\sys1".into(), "C:\\sys2".into()];
        let usr = vec!["D:\\usr1".into()];
        save_disabled_state(sys.clone(), usr.clone()).unwrap();
        let (loaded_sys, loaded_usr) = load_disabled_state().unwrap();
        assert_eq!(loaded_sys, sys);
        assert_eq!(loaded_usr, usr);
        // 写入端补上 schemaVersion=1
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["schemaVersion"], 1);

        // overwrite：第二次保存应把上一份轮换为 .bak
        save_disabled_state(vec!["C:\\new".into()], vec![]).unwrap();
        let (loaded, _) = load_disabled_state().unwrap();
        assert_eq!(loaded, vec!["C:\\new".to_string()]);
        assert!(bak.exists(), "第二次保存应把上一份轮换为 .bak");
        let bak_v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&bak).unwrap()).unwrap();
        assert_eq!(bak_v["system"], serde_json::json!(["C:\\sys1", "C:\\sys2"]));

        // empty
        save_disabled_state(vec![], vec![]).unwrap();
        let result = load_disabled_state().unwrap();
        assert!(result.0.is_empty() && result.1.is_empty());
        let _ = fs::remove_file(&bak);

        // 无 schemaVersion 的旧格式文件 → 按 v1 读取（向后兼容）
        let legacy = r#"{
  "system": ["C:\\legacy_sys"],
  "user": ["D:\\legacy_usr"],
  "systemSnapshot": [{"path": "C:\\legacy_sys", "enabled": false}],
  "userSnapshot": []
}"#;
        fs::write(&path, legacy).unwrap();
        let (legacy_sys, legacy_usr) = load_disabled_state().unwrap();
        assert_eq!(legacy_sys, vec!["C:\\legacy_sys".to_string()]);
        assert_eq!(legacy_usr, vec!["D:\\legacy_usr".to_string()]);

        // 截断 JSON → 隔离 + Parse
        fs::write(&path, r#"{"system": ["C:\\trunc"#).unwrap();
        let err = load_disabled_state().unwrap_err();
        assert_eq!(err.code, ErrorCode::Parse, "实际错误: {err}");
        assert!(!path.exists(), "坏文件应被隔离移走");
        let parent = path.parent().unwrap();
        let quarantined: Vec<_> = fs::read_dir(parent)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("patheditor_test_disabled.json.corrupt-"))
            .collect();
        assert_eq!(quarantined.len(), 1, "应恰好产生一个隔离文件");
        let corrupt = fs::read_to_string(parent.join(&quarantined[0])).unwrap();
        assert!(corrupt.contains("trunc"), "隔离文件应保留原损坏内容");
        let _ = fs::remove_file(parent.join(&quarantined[0]));

        // schemaVersion: 999 → Parse + 「文件由更新版本写入」提示，不隔离
        fs::write(&path, r#"{"schemaVersion": 999, "system": [], "user": []}"#).unwrap();
        let err = load_disabled_state().unwrap_err();
        assert_eq!(err.code, ErrorCode::Parse);
        assert!(
            err.message.contains("更新版本"),
            "错误应提示文件由更新版本写入: {err}"
        );
        assert!(path.exists(), "版本过高的文件不应被隔离");

        // 收尾：清掉主文件，避免影响其他共享该路径的测试
        fs::remove_file(&path).ok();
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

    // pending 快照原语（roundtrip / schemaVersion / .bak）按控制者裁决 R3
    // 合并为一个生命周期测试：测试共享固定临时路径（std::env::temp_dir()），
    // 拆成多个测试会有并行竞态。
    #[test]
    fn pending_snapshot_roundtrip_lifecycle() {
        let _guard = crate::persist::test_persist_lock();
        // 上次运行可能残留文件（此前崩溃遗留），先清掉再断言 missing 前置。
        let _ = clear_pending_path_snapshot();
        let bak = pending_path().with_file_name("patheditor_test_pending_snapshot.json.bak");
        let _ = fs::remove_file(&bak);
        assert!(!has_pending_path_snapshot());
        assert!(load_pending_path_snapshot().unwrap().is_none());

        // save → load 得到同一内容（system/user 各造不同条目，含禁用项以验证 enabled 保留）
        let sys = vec![
            entry("C:\\pending_sys1", true),
            entry("C:\\pending_sys2", false),
        ];
        let usr = vec![entry("D:\\pending_usr1", false)];
        save_pending_path_snapshot(Some(sys.clone()), Some(usr.clone())).unwrap();
        assert!(has_pending_path_snapshot());
        let loaded = load_pending_path_snapshot()
            .unwrap()
            .expect("应有待补写快照");
        assert_eq!(loaded.system, sys);
        assert_eq!(loaded.user, usr);
        // 写入端补上 schemaVersion=1；首次保存无 .bak
        let content = fs::read_to_string(pending_path()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(v["schemaVersion"], 1);
        assert!(!bak.exists(), "首次保存不应产生 .bak");

        // 第二次保存：上一份轮换为 .bak
        let sys2 = vec![entry("C:\\pending_new", true)];
        save_pending_path_snapshot(Some(sys2.clone()), None).unwrap();
        assert!(bak.exists(), "第二次保存应把上一份轮换为 .bak");
        let loaded2 = load_pending_path_snapshot().unwrap().expect("应有 pending");
        assert_eq!(loaded2.system, sys2);

        // clear → has=false → load 再回 Ok(None)
        clear_pending_path_snapshot().unwrap();
        assert!(!has_pending_path_snapshot());
        assert!(load_pending_path_snapshot().unwrap().is_none());
        let _ = fs::remove_file(&bak);
    }
}
