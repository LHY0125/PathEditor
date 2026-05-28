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
    /// 检测可执行文件冲突
    Conflicts { #[arg(long)] json: bool },
    /// 列出 PATH 中可执行文件
    Scan {
        #[arg(long)] query: Option<String>,
        #[arg(long)] json: bool,
    },
    /// 管理配置文件
    #[command(subcommand)]
    Profile(ProfileCmd),
    /// 检查管理员权限
    CheckAdmin { #[arg(long)] json: bool },
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

fn pick_target(system: bool, user: bool) -> &'static str {
    if system { "system" } else { "user" }
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
    let target = pick_target(system, user);
    let (mut list, save_fn): (Vec<String>, _) = if target == "system" {
        (core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e)),
         core::registry::save_system_paths as fn(Vec<String>) -> Result<(), String>)
    } else {
        (core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e)),
         core::registry::save_user_paths as fn(Vec<String>) -> Result<(), String>)
    };
    list.push(path.clone());
    save_fn(list).unwrap_or_else(|e| exit_err(&e));
    let label = if target == "system" { "系统" } else { "用户" };
    println!("已添加到{} PATH: {path}", label);
}

fn cmd_remove(index: usize, system: bool) {
    let target = pick_target(system, false);
    let (mut list, save_fn): (Vec<String>, _) = if target == "system" {
        (core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e)),
         core::registry::save_system_paths as fn(Vec<String>) -> Result<(), String>)
    } else {
        (core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e)),
         core::registry::save_user_paths as fn(Vec<String>) -> Result<(), String>)
    };
    if index >= list.len() { exit_err(&format!("索引 {index} 超出范围 (共 {} 条)", list.len())); }
    let removed = list.remove(index);
    save_fn(list).unwrap_or_else(|e| exit_err(&e));
    println!("已删除: {removed}");
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
    let sys_entries = sys.into_iter().map(|p| core::profiles::ProfilePathEntry { path: p, enabled: true }).collect();
    let usr_entries = usr.into_iter().map(|p| core::profiles::ProfilePathEntry { path: p, enabled: true }).collect();
    core::profiles::save_profile(name.clone(), sys_entries, usr_entries).unwrap_or_else(|e| exit_err(&e));
    println!("已保存配置: {name}");
}

fn profile_load(name: String) {
    let data = core::profiles::load_profile(name.clone()).unwrap_or_else(|e| exit_err(&e));
    println!("═══ 系统 PATH ({} 条) ═══", data.sys.len());
    for e in &data.sys { println!("  [{}] {}", if e.enabled { "✓" } else { "✗" }, e.path); }
    println!("═══ 用户 PATH ({} 条) ═══", data.user.len());
    for e in &data.user { println!("  [{}] {}", if e.enabled { "✓" } else { "✗" }, e.path); }
}

fn profile_apply(name: String) {
    let data = core::profiles::load_profile(name.clone()).unwrap_or_else(|e| exit_err(&e));
    let sys: Vec<String> = data.sys.into_iter().filter(|e| e.enabled).map(|e| e.path).collect();
    let usr: Vec<String> = data.user.into_iter().filter(|e| e.enabled).map(|e| e.path).collect();
    core::registry::save_system_paths(sys).unwrap_or_else(|e| exit_err(&e));
    core::registry::save_user_paths(usr).unwrap_or_else(|e| exit_err(&e));
    core::system::broadcast_env_change();
    println!("配置文件 \"{name}\" 已写入注册表。");
}

fn profile_delete(name: String) {
    core::profiles::delete_profile(name.clone()).unwrap_or_else(|e| exit_err(&e));
    println!("已删除配置: {name}");
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::List { system, user, json } => cmd_list(system, user, json),
        Command::Add { path, system, user } => cmd_add(path, system, user),
        Command::Remove { index, system } => cmd_remove(index, system),
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
