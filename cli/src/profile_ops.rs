use crate::runtime::{exit_err, exit_persist_error, persist_snapshot, verify_and_save};
use path_editor_core as core;

pub(crate) fn profile_list(json_out: bool) {
    let list = core::profiles::list_profiles().unwrap_or_else(|e| exit_persist_error(&e));
    if json_out {
        println!("{}", serde_json::to_string_pretty(&list).unwrap());
    } else if list.is_empty() {
        println!("暂无配置文件。");
    } else {
        for profile in &list {
            println!("  {}  ({})", profile.name, profile.modified);
        }
    }
}

pub(crate) fn profile_save(name: String) {
    // 先合并注册表与 disabled.json，确保已禁用的孤儿条目也进入配置并保留顺序。
    let snapshot = core::disabled::load_path_snapshot().unwrap_or_else(|e| exit_persist_error(&e));
    core::profiles::save_profile(&name, snapshot.system, snapshot.user)
        .unwrap_or_else(|e| exit_persist_error(&e));
    println!("已保存配置: {name}");
}

pub(crate) fn profile_load(name: String) {
    let data = core::profiles::load_profile(&name).unwrap_or_else(|e| exit_persist_error(&e));
    println!("═══ 系统 PATH ({} 条) ═══", data.sys.len());
    for entry in &data.sys {
        println!(
            "  [{}] {}",
            if entry.enabled { "✓" } else { "✗" },
            entry.path
        );
    }
    println!("═══ 用户 PATH ({} 条) ═══", data.user.len());
    for entry in &data.user {
        println!(
            "  [{}] {}",
            if entry.enabled { "✓" } else { "✗" },
            entry.path
        );
    }
}

pub(crate) fn profile_apply(name: String) {
    super::runtime::flush_pending_snapshot();
    let data = core::profiles::load_profile(&name).unwrap_or_else(|e| exit_persist_error(&e));
    let new_sys: Vec<String> = data
        .sys
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| entry.path.clone())
        .collect();
    let new_usr: Vec<String> = data
        .user
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| entry.path.clone())
        .collect();

    let orig_sys = core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e));
    let orig_usr = core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e));
    verify_and_save("system", &orig_sys, new_sys);
    verify_and_save("user", &orig_usr, new_usr);
    persist_snapshot(Some(data.sys), Some(data.user));

    core::system::broadcast_env_change();
    println!("配置文件 \"{name}\" 已写入注册表。");
}

pub(crate) fn profile_delete(name: String) {
    core::profiles::delete_profile(&name).unwrap_or_else(|e| exit_persist_error(&e));
    println!("已删除配置: {name}");
}

pub(crate) fn profile_rename(old_name: String, new_name: String) {
    core::profiles::rename_profile(&old_name, &new_name).unwrap_or_else(|e| exit_persist_error(&e));
    println!("已重命名: {old_name} → {new_name}");
}
