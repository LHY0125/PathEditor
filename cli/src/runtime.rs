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
    flush_pending_snapshot();
    let snapshot = core::disabled::load_path_snapshot().unwrap_or_else(|e| exit_err(&e));

    if target == "system" {
        let original = enabled_paths(&snapshot.system);
        let entries = f(snapshot.system);
        verify_and_save(target, &original, enabled_paths(&entries));
        persist_snapshot(Some(entries), None);
    } else {
        let original = enabled_paths(&snapshot.user);
        let entries = f(snapshot.user);
        verify_and_save(target, &original, enabled_paths(&entries));
        persist_snapshot(None, Some(entries));
    }
}

/// 加载、检查索引、操作、验证、保存的通用模式。索引基于完整快照（含禁用项）。
pub(crate) fn load_operate_save(
    system: bool,
    index: usize,
    operate: impl FnOnce(Vec<core::PathEntry>, usize) -> (Vec<core::PathEntry>, String),
) {
    let target = ensure_single_target(system, false);
    flush_pending_snapshot();
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
        persist_snapshot(Some(new_entries), None);
    } else {
        persist_snapshot(None, Some(new_entries));
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

/// 构造 sidecar 快照保存失败时的错误文案（纯函数，便于测试）。
///
/// 两个分支都必须包含「注册表已写入」，让用户/脚本明确知道注册表侧已经生效、
/// 只有侧车快照未落盘，避免误判为整个操作失败。
pub(crate) fn sidecar_failure_message(err: &str, pending_ok: bool) -> String {
    if pending_ok {
        format!(
            "注册表已写入，但快照保存失败: {err}\n已记录待补写状态，下次运行 PATH 命令会自动补写"
        )
    } else {
        format!(
            "注册表已写入，但快照保存失败: {err}\n且待补写状态记录失败，注册表与快照可能不一致，请手工核对"
        )
    }
}

/// 注册表写入成功后提交完整有序快照，保持两个存储的一致性边界。
///
/// 失败时不丢状态：把待补写内容落到 pending 文件，退出码 1 并在 stderr
/// 明确说明「注册表已写入、快照未落、已记录待补写」，供下次运行自动补写。
pub(crate) fn persist_snapshot(
    system: Option<Vec<core::PathEntry>>,
    user: Option<Vec<core::PathEntry>>,
) {
    if let Err(e) = core::disabled::save_path_snapshot(system.clone(), user.clone()) {
        // pending 快照是覆盖式整份落盘，`save_pending_path_snapshot` 的 None 语义
        // 是「空数组」而非「保留该 hive」（与 `save_path_snapshot` 不同）。
        // 若把 None 原样落 pending，补写时会把未操作的 hive 清成空。
        // 因此先用当前快照把 None 侧填充为现有内容，保证补写是无损的整份覆盖。
        let pending = match core::disabled::load_path_snapshot() {
            Ok(snap) => {
                let sys = system.unwrap_or_else(|| snap.system.clone());
                let usr = user.unwrap_or_else(|| snap.user.clone());
                core::disabled::save_pending_path_snapshot(Some(sys), Some(usr))
            }
            // 当前快照读取失败时无法安全构造无损 pending，跳过落盘；
            // 错误文案仍说明注册表已写入，提示手工核对。
            Err(pe) => Err(pe),
        };
        match pending {
            Ok(()) => exit_err(&sidecar_failure_message(&e, true)),
            Err(pe) => exit_err(&format!(
                "{}\n（待补写状态记录失败: {pe}）",
                sidecar_failure_message(&e, false)
            )),
        }
    }
}

/// 若存在上次未落盘的快照，先补写；成功即清除待补写状态。
///
/// 补写是 best-effort：失败不阻断当前命令，只打印警告。
/// pending 是覆盖式整份快照（两个 hive 均为 `Some`），直接整份传回
/// `save_path_snapshot` 即为正确形态。
fn flush_pending_snapshot() {
    let pending = match core::disabled::load_pending_path_snapshot() {
        Ok(Some(p)) => p,
        Ok(None) => return,
        Err(e) => {
            eprintln!("警告: 无法读取待补写快照状态: {e}");
            return;
        }
    };
    match core::disabled::save_path_snapshot(Some(pending.system), Some(pending.user)) {
        Ok(()) => {
            let _ = core::disabled::clear_pending_path_snapshot();
        }
        Err(e) => eprintln!("警告: 待补写快照仍未能落盘: {e}"),
    }
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

    #[test]
    fn sidecar_failure_message_pending_ok_mentions_registry_written_and_pending() {
        let msg = sidecar_failure_message("磁盘已满", true);
        assert!(
            msg.contains("注册表已写入"),
            "必须包含「注册表已写入」: {msg}"
        );
        assert!(msg.contains("待补写"), "必须包含「待补写」: {msg}");
        assert!(msg.contains("磁盘已满"), "必须透传原始错误: {msg}");
    }

    #[test]
    fn sidecar_failure_message_pending_failed_mentions_registry_written_and_manual_check() {
        let msg = sidecar_failure_message("磁盘已满", false);
        assert!(
            msg.contains("注册表已写入"),
            "必须包含「注册表已写入」: {msg}"
        );
        assert!(msg.contains("手工核对"), "必须提示手工核对: {msg}");
        assert!(msg.contains("磁盘已满"), "必须透传原始错误: {msg}");
    }
}
