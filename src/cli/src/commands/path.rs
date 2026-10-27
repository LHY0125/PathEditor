//! PATH 相关子命令的实现。
//!
//! 只做参数转换与输出；注册表写入、快照与并发校验全部交给 `core`。

use crate::output::print_json;
use crate::runtime::{
    ensure_single_target, exit_core_error, exit_err, load_and_save, load_operate_save,
    load_path_snapshot_any, save_path_write,
};
use path_editor_core as core;
use serde_json::json;

pub(crate) fn cmd_list(system: bool, user: bool, json_out: bool) {
    let snapshot = load_path_snapshot_any().unwrap_or_else(|e| exit_core_error(&e));
    let mut sys = Vec::new();
    let mut usr = Vec::new();
    if system || !user {
        sys = snapshot.system;
    }
    if user || !system {
        usr = snapshot.user;
    }
    if json_out {
        let output = json!({
            "system": {
                "entries": sys.iter().enumerate().map(|(index, entry)| json!({
                    "index": index,
                    "path": entry.path,
                    "enabled": entry.enabled,
                })).collect::<Vec<_>>(),
                "count": sys.len(),
                "enabledCount": sys.iter().filter(|entry| entry.enabled).count(),
            },
            "user": {
                "entries": usr.iter().enumerate().map(|(index, entry)| json!({
                    "index": index,
                    "path": entry.path,
                    "enabled": entry.enabled,
                })).collect::<Vec<_>>(),
                "count": usr.len(),
                "enabledCount": usr.iter().filter(|entry| entry.enabled).count(),
            },
        });
        print_json(&output);
    } else {
        if !sys.is_empty() {
            let disabled = sys.iter().filter(|entry| !entry.enabled).count();
            println!(
                "═══ 系统 PATH ({} 条，其中禁用 {} 条) ═══",
                sys.len(),
                disabled
            );
            for (i, entry) in sys.iter().enumerate() {
                println!(
                    "  [{}] [{}] {}",
                    i,
                    if entry.enabled { "✓" } else { "✗" },
                    entry.path
                );
            }
        }
        if !usr.is_empty() {
            let disabled = usr.iter().filter(|entry| !entry.enabled).count();
            println!(
                "═══ 用户 PATH ({} 条，其中禁用 {} 条) ═══",
                usr.len(),
                disabled
            );
            for (i, entry) in usr.iter().enumerate() {
                println!(
                    "  [{}] [{}] {}",
                    i,
                    if entry.enabled { "✓" } else { "✗" },
                    entry.path
                );
            }
        }
    }
}

pub(crate) fn cmd_add(path: String, system: bool, user: bool) {
    let target = ensure_single_target(system, user);
    load_and_save(system, |mut list| {
        list.push(core::PathEntry {
            path: path.clone(),
            enabled: true,
        });
        list
    });
    let label = if target == "system" {
        "系统"
    } else {
        "用户"
    };
    println!("已添加到{} PATH: {path}", label);
    // 广播已由 core 的保存编排完成。
}

pub(crate) fn cmd_remove(index: usize, system: bool) {
    load_operate_save(system, index, |mut list, idx| {
        let removed = list.remove(idx);
        (list, format!("已删除: {}", removed.path))
    });
}

pub(crate) fn cmd_edit(index: usize, new_path: String, system: bool) {
    load_operate_save(system, index, |mut list, idx| {
        let old = list[idx].path.clone();
        list[idx].path = new_path.clone();
        (list, format!("已编辑: {old} → {new_path}"))
    });
}

pub(crate) fn cmd_move(index: usize, steps: usize, system: bool, up: bool) {
    load_and_save(system, |mut list| {
        if index >= list.len() {
            exit_err(&format!("索引 {index} 超出范围 (共 {} 条)", list.len()));
        }
        let end = if up {
            index.saturating_sub(steps)
        } else {
            let max = list.len() - 1;
            if index + steps > max {
                max
            } else {
                index + steps
            }
        };
        let removed = list.remove(index);
        list.insert(end, removed);
        list
    });
    let dir = if up { "上移" } else { "下移" };
    println!("{dir} {steps} 格完成");
    // 广播已由 core 完成。
}

pub(crate) fn cmd_clean(system: bool, user: bool, dry_run: bool, json_out: bool) {
    if system && user {
        exit_err("不能同时指定 --system 和 --user");
    }

    let clean_sys = system || !user;
    let clean_usr = user || !system;

    if clean_sys {
        clean_one("system", dry_run, json_out);
    }
    if clean_usr {
        clean_one("user", dry_run, json_out);
    }
    // 广播已由 core 完成（各 hive 写成功时各自广播）；dry-run/json 不写，故不广播。
}

fn clean_one(target: &str, dry_run: bool, json_out: bool) {
    let label = if target == "system" {
        "系统"
    } else {
        "用户"
    };
    let list = if target == "system" {
        core::platform::registry::load_system_paths().unwrap_or_else(|e| exit_core_error(&e))
    } else {
        core::platform::registry::load_user_paths().unwrap_or_else(|e| exit_core_error(&e))
    };
    let (kept, removed) = core::platform::registry::clean_paths(list.clone());

    if json_out {
        println!(
            "{}",
            json!({ "target": target, "kept": kept, "removed": removed, "kept_count": kept.len(), "removed_count": removed.len() })
        );
    } else if dry_run {
        println!("═══ {label} PATH — 将被移除（{} 条）═══", removed.len());
        for r in &removed {
            println!("  ✗ {}", r);
        }
        println!("═══ {label} PATH — 将保留（{} 条）═══", kept.len());
        for k in &kept {
            println!("  ✓ {}", k);
        }
    } else {
        let kept_count = kept.len();
        // `kept` 是 `clean_paths` 输出的**启用**路径（`Vec<String>`），
        // 构造 `PathEntry` 时只能落成 `enabled: true`。这是等价的——输入本就是
        // 「当前启用路径」，清理不涉及禁用项（禁用项在 DB 当前快照里）。
        let entries: Vec<core::PathEntry> = kept
            .iter()
            .map(|p| core::PathEntry {
                path: p.clone(),
                enabled: true,
            })
            .collect();
        // 收口到 `save_path_write` 后同时提交当前快照；
        // 不传 expected（CLI 无并发窗口，见 `save_path_write`）。
        save_path_write(Some(entries), None);
        println!(
            "{label} PATH 清理完成：移除 {} 条，保留 {} 条",
            removed.len(),
            kept_count
        );
        if !removed.is_empty() {
            for r in &removed {
                println!("  已移除: {}", r);
            }
        }
    }
}

pub(crate) fn cmd_toggle(index: usize, system: bool, user: bool, enable: bool) {
    let target = ensure_single_target(system, user);
    let mut snapshot = load_path_snapshot_any().unwrap_or_else(|e| exit_core_error(&e));
    let entries = if target == "system" {
        &mut snapshot.system
    } else {
        &mut snapshot.user
    };
    if index >= entries.len() {
        exit_err(&format!("索引 {index} 超出范围 (共 {} 条)", entries.len()));
    }

    // 不再取 expected（CLI 无并发窗口，见 `save_path_write`）。
    entries[index].enabled = enable;
    let path = entries[index].path.clone();

    // 收口到 core 单一编排（含备份/sidecar/广播）。
    if target == "system" {
        save_path_write(Some(snapshot.system), None);
    } else {
        save_path_write(None, Some(snapshot.user));
    }
    let action = if enable { "启用" } else { "禁用" };
    println!("已{action}: {path}");
}

pub(crate) fn cmd_backup() {
    let path = core::platform::backup::backup_registry(None).unwrap_or_else(|e| exit_core_error(&e));
    println!("备份已保存: {path}");
}
