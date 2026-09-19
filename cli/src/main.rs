use clap::{Parser, Subcommand};
use path_editor_core as core;
use serde_json::json;

mod env_ops;
mod import_export;
mod profile_ops;
mod runtime;
mod scan_ops;

use import_export::{cmd_export, cmd_import};
use profile_ops::{
    profile_apply, profile_delete, profile_list, profile_load, profile_rename, profile_save,
};
use runtime::{
    ensure_single_target, exit_err, flush_pending_snapshot, load_and_save, load_operate_save,
    persist_snapshot, verify_and_save,
};
use scan_ops::{cmd_check_admin, cmd_conflicts, cmd_scan};

#[derive(Parser)]
#[command(name = "patheditor", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 列出 PATH 路径
    List {
        #[arg(short, long)]
        system: bool,
        #[arg(short, long)]
        user: bool,
        #[arg(long)]
        json: bool,
    },
    /// 添加一条路径
    Add {
        path: String,
        #[arg(short, long)]
        system: bool,
        #[arg(short, long)]
        user: bool,
    },
    /// 删除指定位置的路径
    Remove {
        index: usize,
        #[arg(short, long)]
        system: bool,
    },
    /// 编辑指定位置的路径
    Edit {
        index: usize,
        new_path: String,
        #[arg(short, long)]
        system: bool,
    },
    /// 上移路径（--steps 指定移动格数，默认 1）
    MoveUp {
        index: usize,
        #[arg(long, default_value = "1")]
        steps: usize,
        #[arg(short, long)]
        system: bool,
    },
    /// 下移路径（--steps 指定移动格数，默认 1）
    MoveDown {
        index: usize,
        #[arg(long, default_value = "1")]
        steps: usize,
        #[arg(short, long)]
        system: bool,
    },
    /// 清理无效和重复路径
    Clean {
        #[arg(short, long)]
        system: bool,
        #[arg(short, long)]
        user: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// 启用指定位置的路径
    Enable {
        index: usize,
        #[arg(short, long)]
        system: bool,
        #[arg(short, long)]
        user: bool,
    },
    /// 禁用指定位置的路径
    Disable {
        index: usize,
        #[arg(short, long)]
        system: bool,
        #[arg(short, long)]
        user: bool,
    },
    /// 从文件导入 PATH（JSON/CSV/TXT）
    Import {
        file: String,
        #[arg(long, default_value = "both")]
        target: String,
    },
    /// 导出 PATH 为文件
    Export {
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// 创建注册表备份
    Backup,
    /// 检测可执行文件冲突
    Conflicts {
        #[arg(long)]
        json: bool,
    },
    /// 列出 PATH 目录中的可执行文件
    Scan {
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// 检查管理员权限
    CheckAdmin {
        #[arg(long)]
        json: bool,
    },
    /// 管理通用环境变量（Path 除外，请使用 PATH 专用命令）
    #[command(subcommand)]
    Env(EnvCmd),
    /// 管理配置文件
    #[command(subcommand)]
    Profile(ProfileCmd),
}

#[derive(Subcommand)]
enum ProfileCmd {
    /// 列出所有配置
    List {
        #[arg(long)]
        json: bool,
    },
    /// 保存当前 PATH 为配置
    Save { name: String },
    /// 加载配置（预览）
    Load { name: String },
    /// 应用配置（写入注册表）
    Apply { name: String },
    /// 删除配置
    Delete { name: String },
    /// 重命名配置
    Rename {
        #[arg(long)]
        old: String,
        #[arg(long)]
        new: String,
    },
}

#[derive(Subcommand)]
enum EnvCmd {
    /// 列出环境变量元数据（不含明文）
    List {
        #[arg(short, long)]
        system: bool,
        #[arg(short, long, conflicts_with = "system")]
        user: bool,
        #[arg(long)]
        json: bool,
    },
    /// 读取单个变量的明文
    Get {
        name: String,
        #[arg(short, long)]
        system: bool,
    },
    /// 修改已有变量的值（类型不变）
    Set {
        name: String,
        /// 值（敏感值建议改用 --stdin 或 --value-file，避免进 shell 历史）
        #[arg(long)]
        value: Option<String>,
        /// 从标准输入读取值（读到 EOF）
        #[arg(long, conflicts_with_all = ["value", "value_file"])]
        stdin: bool,
        /// 从文件读取值
        #[arg(long, conflicts_with_all = ["value", "stdin"])]
        value_file: Option<String>,
        /// 并发校验摘要（来自 `env list --json` 的 revision 字段）
        #[arg(long, conflicts_with = "force")]
        revision: Option<String>,
        /// 跳过并发校验直接覆盖
        #[arg(long)]
        force: bool,
        #[arg(short, long)]
        system: bool,
    },
    /// 新建变量
    Add {
        name: String,
        /// 值（敏感值建议改用 --stdin 或 --value-file）
        value: Option<String>,
        #[arg(long, conflicts_with_all = ["value", "value_file"])]
        stdin: bool,
        #[arg(long, conflicts_with_all = ["value", "stdin"])]
        value_file: Option<String>,
        /// 注册表类型：string (REG_SZ) 或 expand (REG_EXPAND_SZ)
        #[arg(long, default_value = "string", value_parser = ["string", "expand"])]
        kind: String,
        #[arg(short, long)]
        system: bool,
    },
    /// 删除变量
    Remove {
        name: String,
        #[arg(long, conflicts_with = "force")]
        revision: Option<String>,
        /// 跳过并发校验直接删除
        #[arg(long)]
        force: bool,
        #[arg(short, long)]
        system: bool,
    },
}

// ── 命令实现 ──

fn cmd_list(system: bool, user: bool, json_out: bool) {
    let snapshot = core::disabled::load_path_snapshot().unwrap_or_else(|e| exit_err(&e));
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
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
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

fn cmd_add(path: String, system: bool, user: bool) {
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
    core::system::broadcast_env_change();
}

fn cmd_remove(index: usize, system: bool) {
    load_operate_save(system, index, |mut list, idx| {
        let removed = list.remove(idx);
        (list, format!("已删除: {}", removed.path))
    });
}

fn cmd_edit(index: usize, new_path: String, system: bool) {
    load_operate_save(system, index, |mut list, idx| {
        let old = list[idx].path.clone();
        list[idx].path = new_path.clone();
        (list, format!("已编辑: {old} → {new_path}"))
    });
}

fn cmd_move(index: usize, steps: usize, system: bool, up: bool) {
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
    core::system::broadcast_env_change();
}

fn cmd_clean(system: bool, user: bool, dry_run: bool, json_out: bool) {
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

    if !dry_run && !json_out {
        core::system::broadcast_env_change();
    }
}

fn clean_one(target: &str, dry_run: bool, json_out: bool) {
    let label = if target == "system" {
        "系统"
    } else {
        "用户"
    };
    let list = if target == "system" {
        core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e))
    } else {
        core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e))
    };
    let (kept, removed) = core::registry::clean_paths(list.clone());

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
        verify_and_save(target, &list, kept);
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

fn cmd_toggle(index: usize, system: bool, user: bool, enable: bool) {
    let target = ensure_single_target(system, user);
    flush_pending_snapshot();
    let mut snapshot = core::disabled::load_path_snapshot().unwrap_or_else(|e| exit_err(&e));
    let entries = if target == "system" {
        &mut snapshot.system
    } else {
        &mut snapshot.user
    };
    if index >= entries.len() {
        exit_err(&format!("索引 {index} 超出范围 (共 {} 条)", entries.len()));
    }

    let original: Vec<String> = entries
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| entry.path.clone())
        .collect();
    entries[index].enabled = enable;

    let new_list: Vec<String> = entries
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| entry.path.clone())
        .collect();
    verify_and_save(target, &original, new_list);

    let path = entries[index].path.clone();
    if target == "system" {
        persist_snapshot(Some(snapshot.system), None);
    } else {
        persist_snapshot(None, Some(snapshot.user));
    }
    core::system::broadcast_env_change();
    let action = if enable { "启用" } else { "禁用" };
    println!("已{action}: {path}");
}

fn cmd_backup() {
    let path = core::backup::backup_registry(None).unwrap_or_else(|e| exit_err(&e));
    println!("备份已保存: {path}");
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::List { system, user, json } => cmd_list(system, user, json),
        Command::Add { path, system, user } => cmd_add(path, system, user),
        Command::Remove { index, system } => cmd_remove(index, system),
        Command::Edit {
            index,
            new_path,
            system,
        } => cmd_edit(index, new_path, system),
        Command::MoveUp {
            index,
            steps,
            system,
        } => cmd_move(index, steps, system, true),
        Command::MoveDown {
            index,
            steps,
            system,
        } => cmd_move(index, steps, system, false),
        Command::Clean {
            system,
            user,
            dry_run,
            json,
        } => cmd_clean(system, user, dry_run, json),
        Command::Enable {
            index,
            system,
            user,
        } => cmd_toggle(index, system, user, true),
        Command::Disable {
            index,
            system,
            user,
        } => cmd_toggle(index, system, user, false),
        Command::Import { file, target } => cmd_import(file, target),
        Command::Export { format, output } => cmd_export(format, output),
        Command::Backup => cmd_backup(),
        Command::Conflicts { json } => cmd_conflicts(json),
        Command::Scan { query, json } => cmd_scan(query, json),
        Command::CheckAdmin { json } => cmd_check_admin(json),
        Command::Env(cmd) => match cmd {
            EnvCmd::List { system, user, json } => env_ops::cmd_env_list(system, user, json),
            EnvCmd::Get { name, system } => env_ops::cmd_env_get(name, system),
            EnvCmd::Set {
                name,
                value,
                stdin,
                value_file,
                revision,
                force,
                system,
            } => env_ops::cmd_env_set(name, value, stdin, value_file, revision, force, system),
            EnvCmd::Add {
                name,
                value,
                stdin,
                value_file,
                kind,
                system,
            } => env_ops::cmd_env_add(name, value, stdin, value_file, kind, system),
            EnvCmd::Remove {
                name,
                revision,
                force,
                system,
            } => env_ops::cmd_env_remove(name, revision, force, system),
        },
        Command::Profile(cmd) => match cmd {
            ProfileCmd::List { json } => profile_list(json),
            ProfileCmd::Save { name } => profile_save(name),
            ProfileCmd::Load { name } => profile_load(name),
            ProfileCmd::Apply { name } => profile_apply(name),
            ProfileCmd::Delete { name } => profile_delete(name),
            ProfileCmd::Rename { old, new } => profile_rename(old, new),
        },
    }
}
