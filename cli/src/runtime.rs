use path_editor_core as core;

pub(crate) fn exit_err(msg: &str) -> ! {
    eprintln!("错误: {msg}");
    std::process::exit(1);
}

pub(crate) fn ensure_single_target(system: bool, user: bool) -> &'static str {
    if system && user {
        exit_err("不能同时指定 --system 和 --user");
    }
    if system {
        "system"
    } else {
        "user"
    }
}

type SaveFn = fn(Vec<String>) -> Result<(), String>;

pub(crate) fn verify_and_save(target: &str, original: &[String], new_list: Vec<String>) {
    let reload = if target == "system" {
        core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e))
    } else {
        core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e))
    };
    if reload != original {
        exit_err("注册表已被其他进程修改，请重新执行操作");
    }
    let save: SaveFn = if target == "system" {
        core::registry::save_system_paths
    } else {
        core::registry::save_user_paths
    };
    save(new_list).unwrap_or_else(|e| exit_err(&e));
}

pub(crate) fn load_and_save(
    system: bool,
    f: impl FnOnce(Vec<core::PathEntry>) -> Vec<core::PathEntry>,
) {
    let target = ensure_single_target(system, false);
    let snapshot = core::disabled::load_path_snapshot().unwrap_or_else(|e| exit_err(&e));

    if target == "system" {
        let original = enabled_paths(&snapshot.system);
        let entries = f(snapshot.system);
        verify_and_save(target, &original, enabled_paths(&entries));
        core::disabled::save_path_snapshot(Some(entries), None).unwrap_or_else(|e| exit_err(&e));
    } else {
        let original = enabled_paths(&snapshot.user);
        let entries = f(snapshot.user);
        verify_and_save(target, &original, enabled_paths(&entries));
        core::disabled::save_path_snapshot(None, Some(entries)).unwrap_or_else(|e| exit_err(&e));
    }
}

/// 加载、检查索引、操作、验证、保存的通用模式。索引基于完整快照（含禁用项）。
pub(crate) fn load_operate_save(
    system: bool,
    index: usize,
    operate: impl FnOnce(Vec<core::PathEntry>, usize) -> (Vec<core::PathEntry>, String),
) {
    let target = ensure_single_target(system, false);
    let snapshot = core::disabled::load_path_snapshot().unwrap_or_else(|e| exit_err(&e));
    let entries = if target == "system" {
        snapshot.system
    } else {
        snapshot.user
    };
    if index >= entries.len() {
        exit_err(&format!("索引 {index} 超出范围 (共 {} 条)", entries.len()));
    }

    let original = enabled_paths(&entries);
    let (new_entries, message) = operate(entries, index);
    verify_and_save(target, &original, enabled_paths(&new_entries));
    if target == "system" {
        core::disabled::save_path_snapshot(Some(new_entries), None)
            .unwrap_or_else(|e| exit_err(&e));
    } else {
        core::disabled::save_path_snapshot(None, Some(new_entries))
            .unwrap_or_else(|e| exit_err(&e));
    }

    println!("{message}");
    core::system::broadcast_env_change();
}

pub(crate) fn enabled_paths(entries: &[core::PathEntry]) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| entry.path.clone())
        .collect()
}

#[allow(dead_code)] // 兼容后续 CLI 调试/导出使用；生产路径统一保存完整快照。
pub(crate) fn disabled_paths(entries: &[core::PathEntry]) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| !entry.enabled)
        .map(|entry| entry.path.clone())
        .collect()
}

/// 注册表写入成功后再提交完整有序快照，保持两个存储的一致性边界。
pub(crate) fn persist_snapshot(
    system: Option<Vec<core::PathEntry>>,
    user: Option<Vec<core::PathEntry>>,
) {
    core::disabled::save_path_snapshot(system, user).unwrap_or_else(|e| exit_err(&e));
}
