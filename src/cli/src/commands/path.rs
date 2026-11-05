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
    // 空串/纯空白会被 `join_path` 过滤掉：命令打印「已添加」但注册表零变化，
    // 同时 DB 快照里留下一条幽灵条目。
    if path.trim().is_empty() {
        exit_err("路径不能为空");
    }
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
            // `--steps` 是裸 usize：`index + steps` 溢出在 debug 构建会 panic
            //（exit 101 绕过 0–5 契约），release 会回绕把条目移到顶部。
            index.saturating_add(steps).min(max)
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
    // `--system` / `--user` 是**筛选器**（与 `list` 同语义）：同给则两者都清。
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
    // 用**完整快照**（含禁用项）而不是只读注册表：`save_path_write` 会把传入
    // 条目整体作为该 hive 的 current 快照提交，只读启用项会把 DB 里的禁用条目
    // 一并抹掉（`patheditor disable` 的结果就此丢失）。
    let snapshot = load_path_snapshot_any().unwrap_or_else(|e| exit_core_error(&e));
    let entries = if target == "system" {
        snapshot.system
    } else {
        snapshot.user
    };
    let (kept, removed) = core::platform::registry::clean_path_entries(entries);
    let kept_paths: Vec<String> = kept.iter().map(|e| e.path.clone()).collect();
    let removed_paths: Vec<String> = removed.iter().map(|e| e.path.clone()).collect();

    if json_out {
        println!(
            "{}",
            json!({
                "target": target,
                "dryRun": dry_run,
                "kept": kept_paths,
                "removed": removed_paths,
                "keptCount": kept_paths.len(),
                "removedCount": removed_paths.len(),
            })
        );
    } else if dry_run {
        println!("═══ {label} PATH — 将被移除（{} 条）═══", removed.len());
        for r in &removed {
            println!("  ✗ {}", r.path);
        }
        println!("═══ {label} PATH — 将保留（{} 条）═══", kept.len());
        for k in &kept {
            println!("  {} {}", if k.enabled { "✓" } else { "✗" }, k.path);
        }
    } else {
        let (kept_count, removed_count) = (kept.len(), removed.len());
        // 收口到 `save_path_write` 后同时提交当前快照；
        // 不传 expected（CLI 无并发窗口，见 `save_path_write`）。
        if target == "system" {
            save_path_write(Some(kept), None);
        } else {
            save_path_write(None, Some(kept));
        }
        println!("{label} PATH 清理完成：移除 {removed_count} 条，保留 {kept_count} 条");
        for r in &removed_paths {
            println!("  已移除: {r}");
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
    let path =
        core::platform::backup::backup_registry(None).unwrap_or_else(|e| exit_core_error(&e));
    println!("备份已保存: {path}");
}
