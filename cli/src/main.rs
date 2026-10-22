use clap::{Parser, Subcommand};
use path_editor_core as core;
use serde_json::json;

mod env_ops;
mod import_export;
mod maintenance_ops;
mod profile_ops;
mod runtime;
mod scan_ops;
mod sync_ops;

use import_export::{cmd_export, cmd_import};
use profile_ops::{
    profile_apply, profile_delete, profile_export_v5, profile_import, profile_list, profile_load,
    profile_rename, profile_save,
};
use runtime::{
    ensure_single_target, exit_err, exit_persist_error, load_and_save, load_operate_save,
    load_path_snapshot_any, save_path_write,
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
    /// 同步（文件式变更包，跨设备搬运配置）
    #[command(subcommand)]
    Sync(SyncCmd),
    /// 维护任务（保留策略清理、v5 原件归档、遗留明文备份清理）
    #[command(subcommand)]
    Maintenance(MaintenanceCmd),
}

/// 维护子命令（spec §6.5.3）。
#[derive(Subcommand)]
enum MaintenanceCmd {
    /// 清理过期的变更与冲突（已导出 90 天 / 已解决 30 天）
    Prune {
        /// 只统计，不删除
        #[arg(long)]
        dry_run: bool,
    },
    /// 把 v5 原件（disabled.json / profiles/*.json）归档为 .v5.bak（只改名，不删除）
    ArchiveV5 {
        /// 只显示将归档的文件，不改名
        #[arg(long)]
        dry_run: bool,
    },
    /// 把遗留 env_backup_*.json 转为 .pebak 后删除（必须先 --yes）
    CleanupLegacyBackups {
        /// 只显示计划，不改动任何文件
        #[arg(long)]
        dry_run: bool,
        /// 确认执行（不加就只打印提示并以 1 退出）
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum SyncCmd {
    /// 把待导出的变更写成加密同步包
    Export {
        /// 输出文件路径（.pesync）
        file: String,
    },
    /// 解密并应用一个同步包（只改 DB，不改注册表）
    Import {
        /// 同步包路径
        file: String,
    },
    /// 显示待导出变更数与未解决冲突数
    Status,
    /// 列出冲突
    Conflicts {
        /// 含已解决的冲突
        #[arg(long)]
        all: bool,
    },
    /// 解决一条冲突
    Resolve {
        /// 冲突 id（来自 `sync conflicts`）
        id: i64,
        /// 取哪一边：local 或 remote
        #[arg(long)]
        take: String,
    },
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
    /// 导出为 v5 `profiles/*.json` 格式（降级退路，输出到 stdout）
    ExportV5 { name: String },
    /// 从 v5 `profiles/*.json` 导入配置（只写 DB，不改注册表）
    Import { file: String },
}

#[derive(Subcommand)]
enum EnvCmd {
    /// 列出环境变量元数据（敏感变量不含明文，非敏感变量给出可能截断的 preview）
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
        /// 只显示将要发生的变更，不写注册表
        #[arg(long)]
        dry_run: bool,
        /// 以 JSON 输出变更摘要（需与 --dry-run 同用）
        #[arg(long, requires = "dry_run")]
        json: bool,
        #[arg(short, long)]
        system: bool,
    },
    /// 新建变量
    ///
    /// **纯新建语义**：同名变量会被拒绝（`NameExists`），不会覆盖 —— 因此无需
    /// revision 守卫（新建没有「旧值」可被并发覆盖）。注意检查与写入是两步，
    /// 存在毫秒级竞态窗口。
    ///
    /// 值必须经三通道之一提供；都不给会**立即报错**（不进入交互提示）。
    Add {
        name: String,
        /// 值。省略且不给 --stdin/--value-file 时报错（不进入交互提示）；
        /// 敏感值建议改用 --stdin 或 --value-file
        value: Option<String>,
        /// 从标准输入读取值（读到 EOF）
        #[arg(long, conflicts_with_all = ["value", "value_file"])]
        stdin: bool,
        /// 从文件读取值
        #[arg(long, conflicts_with_all = ["value", "stdin"])]
        value_file: Option<String>,
        /// 注册表类型：string (REG_SZ) 或 expand (REG_EXPAND_SZ)
        #[arg(long, default_value = "string", value_parser = ["string", "expand"])]
        kind: String,
        /// 只显示将要发生的变更，不写注册表
        #[arg(long)]
        dry_run: bool,
        /// 以 JSON 输出变更摘要（需与 --dry-run 同用）
        #[arg(long, requires = "dry_run")]
        json: bool,
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
        /// 只显示将要发生的变更，不写注册表
        #[arg(long)]
        dry_run: bool,
        /// 以 JSON 输出变更摘要（需与 --dry-run 同用）
        #[arg(long, requires = "dry_run")]
        json: bool,
        #[arg(short, long)]
        system: bool,
    },
    /// 立即创建一份环境变量备份
    Backup {
        #[arg(long)]
        json: bool,
    },
    /// 列出已有的环境变量备份（按时间倒序）
    Backups {
        #[arg(long)]
        json: bool,
    },
    /// 从备份文件恢复环境变量
    Restore {
        /// 备份文件路径
        file: String,
        /// 只显示将要发生的变更，不写注册表
        #[arg(long)]
        dry_run: bool,
        /// 跳过 revision 校验直接覆盖（不豁免保护名单与类型判定）
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
}

// ── 命令实现 ──

fn cmd_list(system: bool, user: bool, json_out: bool) {
    let snapshot = load_path_snapshot_any().unwrap_or_else(|e| exit_persist_error(&e));
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
    // 广播已由 core（save_path_with_sidecar_io）完成。
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
    // 广播已由 core 完成。
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
    // 广播已由 core 完成（各 hive 写成功时各自广播）；dry-run/json 不写，故不广播。
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
        // N9：`kept` 是 `clean_paths` 输出的**启用**路径（`Vec<String>`），
        // 构造 `PathEntry` 时只能落成 `enabled: true`。这是等价的——输入本就是
        // 「当前启用路径」，清理不涉及禁用项（禁用项在 `disabled.json` 快照里）。
        let entries: Vec<core::PathEntry> = kept
            .iter()
            .map(|p| core::PathEntry {
                path: p.clone(),
                enabled: true,
            })
            .collect();
        // D14（行为变化）：收口到 `save_path_write` 后**顺带提交 sidecar 快照**——
        // 此前 `clean` 只写注册表、不落快照，导致 `disabled.json` 陈旧（已修）。
        // Wave 3.1：不传 expected（CLI 无并发窗口，见 `save_path_write`）。
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

fn cmd_toggle(index: usize, system: bool, user: bool, enable: bool) {
    let target = ensure_single_target(system, user);
    let mut snapshot = load_path_snapshot_any().unwrap_or_else(|e| exit_persist_error(&e));
    let entries = if target == "system" {
        &mut snapshot.system
    } else {
        &mut snapshot.user
    };
    if index >= entries.len() {
        exit_err(&format!("索引 {index} 超出范围 (共 {} 条)", entries.len()));
    }

    // Wave 3.1：不再取 expected（CLI 无并发窗口，见 `save_path_write`）。
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
                dry_run,
                json,
                system,
            } => env_ops::cmd_env_set(
                name, value, stdin, value_file, revision, force, dry_run, json, system,
            ),
            EnvCmd::Add {
                name,
                value,
                stdin,
                value_file,
                kind,
                dry_run,
                json,
                system,
            } => env_ops::cmd_env_add(name, value, stdin, value_file, kind, dry_run, json, system),
            EnvCmd::Remove {
                name,
                revision,
                force,
                dry_run,
                json,
                system,
            } => env_ops::cmd_env_remove(name, revision, force, dry_run, json, system),
            EnvCmd::Backup { json } => env_ops::cmd_env_backup(json),
            EnvCmd::Backups { json } => env_ops::cmd_env_backups(json),
            EnvCmd::Restore {
                file,
                dry_run,
                force,
                json,
            } => env_ops::cmd_env_restore(file, dry_run, force, json),
        },
        Command::Profile(cmd) => match cmd {
            ProfileCmd::List { json } => profile_list(json),
            ProfileCmd::Save { name } => profile_save(name),
            ProfileCmd::Load { name } => profile_load(name),
            ProfileCmd::Apply { name } => profile_apply(name),
            ProfileCmd::Delete { name } => profile_delete(name),
            ProfileCmd::Rename { old, new } => profile_rename(old, new),
            ProfileCmd::ExportV5 { name } => profile_export_v5(name),
            ProfileCmd::Import { file } => profile_import(file),
        },
        Command::Sync(cmd) => match cmd {
            SyncCmd::Export { file } => sync_ops::sync_export(&file),
            SyncCmd::Import { file } => sync_ops::sync_import(&file),
            SyncCmd::Status => sync_ops::sync_status(),
            SyncCmd::Conflicts { all } => sync_ops::sync_conflicts(all),
            SyncCmd::Resolve { id, take } => sync_ops::sync_resolve(id, &take),
        },
        Command::Maintenance(cmd) => match cmd {
            MaintenanceCmd::Prune { dry_run } => maintenance_ops::maintenance_prune(dry_run),
            MaintenanceCmd::ArchiveV5 { dry_run } => {
                maintenance_ops::maintenance_archive_v5(dry_run)
            }
            MaintenanceCmd::CleanupLegacyBackups { dry_run, yes } => {
                maintenance_ops::maintenance_cleanup_legacy(dry_run, yes)
            }
        },
    }
}

#[cfg(test)]
mod help_tests {
    //! CLI help 文案的回归测试。
    //!
    //! 放在 `main.rs`（crate root）而非 `env_ops.rs`：bin-only crate 的 root 模块
    //! 对子模块是私有的，`struct Cli` 未 `pub`，子模块里写 `crate::Cli` 不可见。
    //! root 内可直接取用（plan Task 4 Step 2 预留的退路）。
    use super::*;
    use clap::CommandFactory;

    /// #35 / #38：`env add --help` 必须写明「纯新建语义」与「省略值会报错」。
    ///
    /// 可证伪性：删掉 `Add` 变体的文档注释或字段注释，对应片段消失，断言失败。
    /// 这是把「豁免理由」从源码注释提升为**用户可见契约**的回归测试。
    /// 刻意断言**具体短语**而非「解析成功」——后者无法证明文案存在。
    #[test]
    fn env_add_help_documents_pure_create_and_no_prompt() {
        let mut cmd = Cli::command();
        let env = cmd.find_subcommand_mut("env").expect("env 子命令必须存在");
        let add = env.find_subcommand_mut("add").expect("add 子命令必须存在");
        let help = add.render_long_help().to_string();

        assert!(
            help.contains("纯新建"),
            "add --help 必须写明纯新建语义（同名会被拒绝、不会覆盖）: {help}"
        );
        assert!(
            help.contains("不进入交互提示"),
            "add --help 必须写明省略值会报错、不交互: {help}"
        );
    }

    /// #33：`env list --help` 不得声称「不含明文」，须限定为「敏感变量不含明文」。
    ///
    /// 可证伪性：把描述行改回旧文案，`敏感变量不含明文` 消失，断言失败。
    #[test]
    fn env_list_help_scopes_the_no_plaintext_claim() {
        let mut cmd = Cli::command();
        let env = cmd.find_subcommand_mut("env").expect("env 子命令必须存在");
        let list = env
            .find_subcommand_mut("list")
            .expect("list 子命令必须存在");
        let help = list.render_long_help().to_string();

        assert!(
            help.contains("敏感变量不含明文"),
            "list --help 必须把「不含明文」限定到敏感变量: {help}"
        );
    }

    /// `profile export-v5 --help` 必须写明「输出到 stdout」与「降级退路」。
    ///
    /// 可证伪性：删掉 `ExportV5` 变体的文档注释，对应片段消失，断言失败。
    /// 断言具体短语而非「解析成功」——后者无法证明文案存在。
    #[test]
    fn profile_export_v5_help_documents_stdout_and_fallback() {
        let mut cmd = Cli::command();
        let profile = cmd
            .find_subcommand_mut("profile")
            .expect("profile 子命令必须存在");
        let export = profile
            .find_subcommand_mut("export-v5")
            .expect("export-v5 子命令必须存在");
        let help = export.render_long_help().to_string();

        assert!(
            help.contains("stdout"),
            "export-v5 --help 必须写明输出到 stdout: {help}"
        );
        assert!(
            help.contains("降级"),
            "export-v5 --help 必须写明用途是降级退路: {help}"
        );
    }
}
