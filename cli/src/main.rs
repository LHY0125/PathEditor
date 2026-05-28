use clap::{Parser, Subcommand};
use path_editor_core as core;
use serde_json::json;

#[derive(Parser)]
#[command(name = "patheditor", version = "5.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 列出 PATH 路径
    List {
        #[arg(short, long)] system: bool,
        #[arg(short, long)] user: bool,
        #[arg(long)] json: bool,
    },
    /// 添加一条路径
    Add {
        path: String,
        #[arg(short, long)] system: bool,
        #[arg(short, long)] user: bool,
    },
    /// 删除指定位置的路径
    Remove {
        index: usize,
        #[arg(short, long)] system: bool,
    },
    /// 编辑指定位置的路径
    Edit {
        index: usize,
        new_path: String,
        #[arg(short, long)] system: bool,
    },
    /// 上移路径（--steps 指定移动格数，默认 1）
    MoveUp {
        index: usize,
        #[arg(long, default_value = "1")] steps: usize,
        #[arg(short, long)] system: bool,
    },
    /// 下移路径（--steps 指定移动格数，默认 1）
    MoveDown {
        index: usize,
        #[arg(long, default_value = "1")] steps: usize,
        #[arg(short, long)] system: bool,
    },
    /// 清理无效和重复路径
    Clean {
        #[arg(short, long)] system: bool,
        #[arg(short, long)] user: bool,
        #[arg(long)] dry_run: bool,
        #[arg(long)] json: bool,
    },
    /// 启用指定位置的路径
    Enable {
        index: usize,
        #[arg(short, long)] system: bool,
        #[arg(short, long)] user: bool,
    },
    /// 禁用指定位置的路径
    Disable {
        index: usize,
        #[arg(short, long)] system: bool,
        #[arg(short, long)] user: bool,
    },
    /// 从文件导入 PATH（JSON/CSV/TXT）
    Import {
        file: String,
        #[arg(long, default_value = "both")] target: String,
    },
    /// 导出 PATH 为文件
    Export {
        #[arg(long, default_value = "json")] format: String,
        #[arg(short, long)] output: Option<String>,
    },
    /// 创建注册表备份
    Backup,
    /// 检测可执行文件冲突
    Conflicts { #[arg(long)] json: bool },
    /// 列出 PATH 目录中的可执行文件
    Scan {
        #[arg(long)] query: Option<String>,
        #[arg(long)] json: bool,
    },
    /// 检查管理员权限
    CheckAdmin { #[arg(long)] json: bool },
    /// 管理配置文件
    #[command(subcommand)]
    Profile(ProfileCmd),
}

#[derive(Subcommand)]
enum ProfileCmd {
    /// 列出所有配置
    List { #[arg(long)] json: bool },
    /// 保存当前 PATH 为配置
    Save { name: String },
    /// 加载配置（预览）
    Load { name: String },
    /// 应用配置（写入注册表）
    Apply { name: String },
    /// 删除配置
    Delete { name: String },
}

fn exit_err(msg: &str) -> ! {
    eprintln!("错误: {msg}");
    std::process::exit(1);
}

fn ensure_single_target(system: bool, user: bool) -> &'static str {
    if system && user { exit_err("不能同时指定 --system 和 --user"); }
    if system { "system" } else { "user" }
}

type SaveFn = fn(Vec<String>) -> Result<(), String>;

fn verify_and_save(target: &str, original: &[String], new_list: Vec<String>) {
    let reload = if target == "system" {
        core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e))
    } else {
        core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e))
    };
    if reload != original {
        exit_err("注册表已被其他进程修改，请重新执行操作");
    }
    let save: SaveFn = if target == "system" { core::registry::save_system_paths } else { core::registry::save_user_paths };
    save(new_list).unwrap_or_else(|e| exit_err(&e));
}

fn load_and_save(system: bool, f: impl FnOnce(Vec<String>) -> Vec<String>) {
    let target = ensure_single_target(system, false);
    let list = if target == "system" {
        core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e))
    } else {
        core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e))
    };
    let new_list = f(list.clone());
    verify_and_save(target, &list, new_list);
}

// ── 命令实现 ──

fn cmd_list(system: bool, user: bool, json_out: bool) {
    let mut sys: Vec<String> = vec![];
    let mut usr: Vec<String> = vec![];
    if system || (!system && !user) {
        sys = core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e));
    }
    if user || (!system && !user) {
        usr = core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e));
    }
    if json_out {
        let output = json!({ "system": { "paths": sys, "count": sys.len() }, "user": { "paths": usr, "count": usr.len() } });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        if !sys.is_empty() {
            println!("═══ 系统 PATH ({}) ═══", sys.len());
            for (i, p) in sys.iter().enumerate() { println!("  [{}] {}", i, p); }
        }
        if !usr.is_empty() {
            println!("═══ 用户 PATH ({}) ═══", usr.len());
            for (i, p) in usr.iter().enumerate() { println!("  [{}] {}", i, p); }
        }
    }
}

fn cmd_add(path: String, system: bool, user: bool) {
    let target = ensure_single_target(system, user);
    load_and_save(system || false, |mut list| {
        list.push(path.clone());
        list
    });
    let label = if target == "system" { "系统" } else { "用户" };
    println!("已添加到{} PATH: {path}", label);
    core::system::broadcast_env_change();
}

fn cmd_remove(index: usize, system: bool) {
    let target = ensure_single_target(system, false);
    let mut list = if target == "system" {
        core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e))
    } else {
        core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e))
    };
    let original = list.clone();
    if index >= list.len() { exit_err(&format!("索引 {index} 超出范围 (共 {} 条)", list.len())); }
    let removed = list.remove(index);
    verify_and_save(target, &original, list);
    println!("已删除: {removed}");
    core::system::broadcast_env_change();
}

fn cmd_edit(index: usize, new_path: String, system: bool) {
    let target = ensure_single_target(system, false);
    let mut list = if target == "system" {
        core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e))
    } else {
        core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e))
    };
    if index >= list.len() { exit_err(&format!("索引 {index} 超出范围 (共 {} 条)", list.len())); }
    let original = list.clone();
    let old = std::mem::replace(&mut list[index], new_path.clone());
    verify_and_save(target, &original, list);
    println!("已编辑: {old} → {new_path}");
    core::system::broadcast_env_change();
}

fn cmd_move(index: usize, steps: usize, system: bool, up: bool) {
    load_and_save(system || false, |mut list| {
        if index >= list.len() { exit_err(&format!("索引 {index} 超出范围 (共 {} 条)", list.len())); }
        let end = if up {
            if steps > index { 0 } else { index - steps }
        } else {
            let max = list.len() - 1;
            if index + steps > max { max } else { index + steps }
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
    let target = ensure_single_target(system, user);
    let list = if target == "system" {
        core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e))
    } else {
        core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e))
    };
    let (kept, removed) = core::registry::clean_paths(list.clone());

    if json_out {
        println!("{}", json!({ "kept": kept, "removed": removed, "kept_count": kept.len(), "removed_count": removed.len() }).to_string());
    } else if dry_run {
        println!("═══ 将被移除（{} 条）═══", removed.len());
        for r in &removed { println!("  ✗ {}", r); }
        println!("═══ 将保留（{} 条）═══", kept.len());
        for k in &kept { println!("  ✓ {}", k); }
    } else {
        let kept_count = kept.len();
        verify_and_save(target, &list, kept);
        println!("清理完成：移除 {} 条，保留 {} 条", removed.len(), kept_count);
        core::system::broadcast_env_change();
        if !removed.is_empty() {
            for r in &removed { println!("  已移除: {}", r); }
        }
    }
}

fn cmd_toggle(index: usize, system: bool, user: bool, enable: bool) {
    let target = ensure_single_target(system, user);
    let list = if target == "system" {
        core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e))
    } else {
        core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e))
    };
    if index >= list.len() { exit_err(&format!("索引 {index} 超出范围 (共 {} 条)", list.len())); }
    let path = &list[index];

    let (mut sys_dis, mut usr_dis) = core::disabled::load_disabled_state().unwrap_or_else(|_| (vec![], vec![]));
    let target_list: &mut Vec<String> = if target == "system" { &mut sys_dis } else { &mut usr_dis };

    if enable {
        target_list.retain(|p| p != path);
    } else if !target_list.contains(path) {
        target_list.push(path.clone());
    }
    core::disabled::save_disabled_state(sys_dis, usr_dis).unwrap_or_else(|e| exit_err(&e));
    let action = if enable { "启用" } else { "禁用" };
    println!("已{action}: {path}");
}

fn cmd_import(file: String, target: String) {
    let content = core::fs::read_text_file(&file).unwrap_or_else(|e| exit_err(&e));
    let (sys, usr) = core::fs::import_paths(&file, &content).unwrap_or_else(|e| exit_err(&e));
    match target.as_str() {
        "system" => {
            let orig = core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e));
            verify_and_save("system", &orig, sys);
            println!("已导入到系统 PATH");
        }
        "user" => {
            let orig = core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e));
            verify_and_save("user", &orig, usr);
            println!("已导入到用户 PATH");
        }
        _ => {
            let orig_sys = core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e));
            let orig_usr = core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e));
            verify_and_save("system", &orig_sys, sys);
            verify_and_save("user", &orig_usr, usr);
            println!("已导入到系统 + 用户 PATH");
        }
    }
    core::system::broadcast_env_change();
}

fn cmd_export(format: String, output: Option<String>) {
    let sys = core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e));
    let usr = core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e));
    let content = core::fs::export_paths(&sys, &usr, &format);
    if let Some(path) = output {
        std::fs::write(&path, &content).unwrap_or_else(|e| exit_err(&format!("无法写入文件: {e}")));
        println!("已导出到: {path}");
    } else {
        println!("{content}");
    }
}

fn cmd_backup() {
    let path = core::backup::backup_registry(None).unwrap_or_else(|e| exit_err(&e));
    println!("备份已保存: {path}");
}

fn cmd_conflicts(json_out: bool) {
    let mut paths: Vec<String> = vec![];
    if let Ok(sys) = core::registry::load_system_paths() { paths.extend(sys); }
    if let Ok(usr) = core::registry::load_user_paths() { paths.extend(usr); }
    let conflicts = core::scanner::scan_conflicts(paths).unwrap_or_else(|e| exit_err(&e));
    if json_out {
        println!("{}", serde_json::to_string_pretty(&conflicts).unwrap());
    } else if conflicts.is_empty() {
        println!("未发现可执行文件冲突。");
    } else {
        println!("═══ 可执行文件冲突（{} 个）═══\n", conflicts.len());
        for c in &conflicts {
            println!("  {}", c.name);
            for loc in &c.locations {
                println!("    {}  {}", if loc.priority == 0 { "✓ 优先" } else { "✗ 遮蔽" }, loc.dir);
            }
            println!();
        }
    }
}

fn cmd_scan(query: Option<String>, json_out: bool) {
    let mut paths: Vec<String> = vec![];
    if let Ok(sys) = core::registry::load_system_paths() { paths.extend(sys); }
    if let Ok(usr) = core::registry::load_user_paths() { paths.extend(usr); }
    let groups = core::scanner::scan_tools(paths, query.unwrap_or_default()).unwrap_or_else(|e| exit_err(&e));
    if json_out {
        println!("{}", serde_json::to_string_pretty(&groups).unwrap());
    } else {
        for g in &groups {
            if !g.exists { println!("  {} (不存在)", g.dir); continue; }
            println!("═══ {} ═══", g.dir);
            for exe in &g.exes { println!("  {}", exe); }
        }
    }
}

fn cmd_check_admin(json_out: bool) {
    let is_admin = core::system::check_admin();
    if json_out {
        println!("{}", json!({"admin": is_admin}));
    } else {
        println!("管理员权限: {}", if is_admin { "是" } else { "否" });
    }
}

fn profile_list(json_out: bool) {
    let list = core::profiles::list_profiles().unwrap_or_else(|e| exit_err(&e));
    if json_out {
        println!("{}", serde_json::to_string_pretty(&list).unwrap());
    } else if list.is_empty() {
        println!("暂无配置文件。");
    } else {
        for p in &list { println!("  {}  ({})", p.name, p.modified); }
    }
}

fn profile_save(name: String) {
    let sys = core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e));
    let usr = core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e));
    let sys_entries = sys.into_iter().map(|p| core::ProfilePathEntry { path: p, enabled: true }).collect();
    let usr_entries = usr.into_iter().map(|p| core::ProfilePathEntry { path: p, enabled: true }).collect();
    core::profiles::save_profile(&name, sys_entries, usr_entries).unwrap_or_else(|e| exit_err(&e));
    println!("已保存配置: {name}");
}

fn profile_load(name: String) {
    let data = core::profiles::load_profile(&name).unwrap_or_else(|e| exit_err(&e));
    println!("═══ 系统 PATH ({} 条) ═══", data.sys.len());
    for e in &data.sys { println!("  [{}] {}", if e.enabled { "✓" } else { "✗" }, e.path); }
    println!("═══ 用户 PATH ({} 条) ═══", data.user.len());
    for e in &data.user { println!("  [{}] {}", if e.enabled { "✓" } else { "✗" }, e.path); }
}

fn profile_apply(name: String) {
    let data = core::profiles::load_profile(&name).unwrap_or_else(|e| exit_err(&e));
    let sys: Vec<String> = data.sys.into_iter().filter(|e| e.enabled).map(|e| e.path).collect();
    let usr: Vec<String> = data.user.into_iter().filter(|e| e.enabled).map(|e| e.path).collect();
    core::registry::save_system_paths(sys).unwrap_or_else(|e| exit_err(&e));
    core::registry::save_user_paths(usr).unwrap_or_else(|e| exit_err(&e));
    core::system::broadcast_env_change();
    println!("配置文件 \"{name}\" 已写入注册表。");
}

fn profile_delete(name: String) {
    core::profiles::delete_profile(&name).unwrap_or_else(|e| exit_err(&e));
    println!("已删除配置: {name}");
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::List { system, user, json } => cmd_list(system, user, json),
        Command::Add { path, system, user } => cmd_add(path, system, user),
        Command::Remove { index, system } => cmd_remove(index, system),
        Command::Edit { index, new_path, system } => cmd_edit(index, new_path, system),
        Command::MoveUp { index, steps, system } => cmd_move(index, steps, system, true),
        Command::MoveDown { index, steps, system } => cmd_move(index, steps, system, false),
        Command::Clean { system, user, dry_run, json } => cmd_clean(system, user, dry_run, json),
        Command::Enable { index, system, user } => cmd_toggle(index, system, user, true),
        Command::Disable { index, system, user } => cmd_toggle(index, system, user, false),
        Command::Import { file, target } => cmd_import(file, target),
        Command::Export { format, output } => cmd_export(format, output),
        Command::Backup => cmd_backup(),
        Command::Conflicts { json } => cmd_conflicts(json),
        Command::Scan { query, json } => cmd_scan(query, json),
        Command::CheckAdmin { json } => cmd_check_admin(json),
        Command::Profile(cmd) => match cmd {
            ProfileCmd::List { json } => profile_list(json),
            ProfileCmd::Save { name } => profile_save(name),
            ProfileCmd::Load { name } => profile_load(name),
            ProfileCmd::Apply { name } => profile_apply(name),
            ProfileCmd::Delete { name } => profile_delete(name),
        },
    }
}
