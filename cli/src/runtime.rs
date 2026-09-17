use path_editor_core as core;

pub(crate) fn exit_err(msg: &str) -> ! {
    eprintln!("错误: {msg}");
    std::process::exit(1);
}

/// 冲突错误的结构化前缀。与 core 的 `ERR_CONFLICT` 常量对齐 ——
/// 中文正文仅供人工阅读，判定只看前缀。
pub(crate) const CONFLICT_PREFIX: &str = "[E_CONFLICT]";

/// 消息是否表示 revision 冲突（可按前缀重试恢复）。
pub(crate) fn is_conflict(msg: &str) -> bool {
    msg.starts_with(CONFLICT_PREFIX)
}

/// 冲突退出：stderr 输出 core 原文，退出码 3。
///
/// 与 `exit_err`（退出码 1）分开，使脚本能区分「重新 list 取 revision 后可恢复」
/// 与致命错误，无需 grep 中文文案。
pub(crate) fn exit_conflict(msg: &str) -> ! {
    eprintln!("错误: {msg}");
    std::process::exit(3);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_prefix_is_detected() {
        // core 的 ERR_CONFLICT 原文（前缀 + 中文正文）必须判为冲突
        assert!(is_conflict("[E_CONFLICT] 变量已被其他进程修改，请重新加载"));
        assert!(is_conflict("[E_CONFLICT]"));
    }

    #[test]
    fn non_conflict_messages_are_not_detected() {
        assert!(!is_conflict("错误: 索引 3 超出范围"));
        assert!(!is_conflict("变量已被其他进程修改，请重新加载"));
        // 前缀必须在开头，中段出现不算
        assert!(!is_conflict("前置文本 [E_CONFLICT] 变量已被其他进程修改"));
        assert!(!is_conflict(""));
    }

    #[test]
    fn conflict_prefix_matches_core_constant() {
        // 契约：core 常量必须以该前缀开头，否则 CLI 退出码 3 永不触发
        use path_editor_core as core;
        let msg = core::registry::conflict_message();
        assert!(
            is_conflict(&msg),
            "core 冲突消息必须以 [E_CONFLICT] 开头，实际: {msg}"
        );
    }
}
