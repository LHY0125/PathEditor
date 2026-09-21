use path_editor_core as core;

pub(crate) fn exit_err(msg: &str) -> ! {
    eprintln!("错误: {msg}");
    std::process::exit(1);
}

/// core 侧持久化错误（`CoreError`）统一出口：透传 message，退出码由错误码映射。
///
/// PATH 命令的注册表错误恒为退出码 1（CLAUDE.md 契约）；sidecar/pending 等
/// 持久化错误沿用 `CoreError::exit_code()`（冲突 3、其余 1），message 语义不变。
pub(crate) fn exit_persist_error(err: &core::CoreError) -> ! {
    eprintln!("错误: {}", err.message);
    std::process::exit(err.exit_code());
}

/// 按结构化错误决定退出码与输出（F-06）。
///
/// 退出码由 `CoreError::exit_code()` 统一映射（冲突 3，其余 1），
/// 不再匹配 `[E_CONFLICT]` 文本前缀 —— 判定只看 `code`。
pub(crate) fn exit_core_error(err: &core::CoreError) -> ! {
    eprintln!("错误: {}", err.message);
    std::process::exit(err.exit_code());
}

/// 备份失败时在 stderr 提示，**不改变退出码**（设计文档 K2）。
///
/// CLI 未初始化 logger，core 的 `log::warn!` 在此被丢弃 —— 必须经返回值显式打印。
pub(crate) fn warn_if_backup_failed(outcome: &core::backup::BackupOutcome) {
    if let core::backup::BackupOutcome::Failed(reason) = outcome {
        eprintln!("警告: 环境变量写前备份失败（写入已完成）: {reason}");
    }
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

/// 乐观并发校验（读-比-写）+ 注册表写入（保守方案：保留在 CLI 层）。
///
/// F-08 的完整终态是把读-比-写下沉到 `core::service::apply_path_snapshot`
/// 内部；本任务为避免大重构，先让服务层提供编排原语（注册表写入、广播、
/// sidecar/pending 事务），校验留在 CLI。语义合并延后到 Wave 2 收口。
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
    let snapshot = core::disabled::load_path_snapshot().unwrap_or_else(|e| exit_persist_error(&e));

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
    let snapshot = core::disabled::load_path_snapshot().unwrap_or_else(|e| exit_persist_error(&e));
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
/// **保留同名同签名，函数体转调 `core::service::commit_sidecar_snapshot`**
/// （W2-N3 推荐做法：6 个外部调用点 + 4 个内部调用点零改动）。
/// pending 落盘、防御性清除等事务语义全部由服务层承担（F-08 迁移）。
///
/// 失败时不丢状态：服务层把待补写内容落到 pending 文件并按
/// `SidecarOutcome::Pending` / `Failed` 诚实报告；本函数据此映射退出码 1
/// 与 stderr 文案（策略层）。
pub(crate) fn persist_snapshot(
    system: Option<Vec<core::PathEntry>>,
    user: Option<Vec<core::PathEntry>>,
) {
    match core::service::commit_sidecar_snapshot(system, user) {
        Ok(outcome) => match outcome.sidecar {
            core::service::SidecarOutcome::Saved => {}
            core::service::SidecarOutcome::Pending(e) => {
                exit_err(&sidecar_failure_message(&e.message, true))
            }
            core::service::SidecarOutcome::Failed(e) => exit_err(&format!(
                "{}\n（待补写状态记录失败，请手工核对）",
                sidecar_failure_message(&e.message, false)
            )),
        },
        Err(e) => exit_err(&e.message),
    }
}

/// 若存在上次未落盘的快照，先补写；成功即清除待补写状态。
///
/// **补写事务已下沉到 `core::service::retry_pending_path_state`（sidecar-only，
/// 不触碰注册表）**；本函数只保留 CLI 策略层语义（W2-N1）：把服务的 `Err`
/// 映射为 `eprintln!` 警告、不阻断当前命令——best-effort 行为与 Wave 1 一致。
///
/// **所有 PATH 写命令入口都必须先调用本函数**，否则陈旧 pending 会在后续任一
/// flush 时把刚写入的注册表与 sidecar 双双覆盖回旧状态。现有调用点：
/// `load_and_save` / `load_operate_save`（runtime.rs）、`cmd_import`
/// （import_export.rs）、`profile_apply`（profile_ops.rs）、`cmd_toggle`
/// （main.rs）。新增 PATH 写命令时必须同样在开头先调用本函数。
pub(crate) fn flush_pending_snapshot() {
    if let Err(e) = core::service::retry_pending_path_state() {
        eprintln!("警告: 待补写快照仍未能落盘: {}", e.message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_prefix_matches_core_constant() {
        // 契约（F-06 重定位）：`[E_CONFLICT]` 前缀仍是前端过渡期判定依据与
        // core 消息的稳定标识，测试改为直接断言 core 常量本身；CLI 退出码
        // 已由 `CoreError.exit_code()` 驱动，不再依赖文本前缀。
        use path_editor_core as core;
        let msg = core::registry::conflict_message();
        assert!(
            msg.starts_with("[E_CONFLICT]"),
            "core 冲突消息必须以 [E_CONFLICT] 开头，实际: {msg}"
        );
    }

    #[test]
    fn core_conflict_maps_to_exit_code_3() {
        // 契约：冲突退出码 3 由 CoreError.exit_code() 决定
        let e = core::CoreError::new(
            core::ErrorCode::Conflict,
            "update_env_var",
            core::registry::conflict_message(),
        );
        assert_eq!(e.exit_code(), 3);
    }

    #[test]
    fn core_other_errors_map_to_exit_code_1() {
        let e = core::CoreError::new(core::ErrorCode::Protected, "op", "保护");
        assert_eq!(e.exit_code(), 1);
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
