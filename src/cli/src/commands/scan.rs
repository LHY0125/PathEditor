use crate::output::print_json;
use crate::runtime::exit_core_error;
use path_editor_core as core;
use serde_json::json;

pub(crate) fn cmd_conflicts(json_out: bool) {
    let mut paths: Vec<String> = vec![];
    if let Ok(sys) = core::platform::registry::load_system_paths() {
        paths.extend(sys);
    }
    if let Ok(usr) = core::platform::registry::load_user_paths() {
        paths.extend(usr);
    }
    let conflicts =
        core::platform::scanner::scan_conflicts(paths).unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        print_json(&conflicts);
    } else if conflicts.is_empty() {
        println!("未发现可执行文件冲突。");
    } else {
        println!("═══ 可执行文件冲突（{} 个）═══\n", conflicts.len());
        for conflict in &conflicts {
            println!("  {}", conflict.name);
            for location in &conflict.locations {
                println!(
                    "    {}  {}",
                    if location.priority == 0 {
                        "✓ 优先"
                    } else {
                        "✗ 遮蔽"
                    },
                    location.dir
                );
            }
            println!();
        }
    }
}

pub(crate) fn cmd_scan(query: Option<String>, json_out: bool) {
    let mut paths: Vec<String> = vec![];
    if let Ok(sys) = core::platform::registry::load_system_paths() {
        paths.extend(sys);
    }
    if let Ok(usr) = core::platform::registry::load_user_paths() {
        paths.extend(usr);
    }
    let groups = core::platform::scanner::scan_tools(paths, query.unwrap_or_default())
        .unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        print_json(&groups);
    } else {
        for group in &groups {
            if !group.exists {
                println!("  {} (不存在)", group.dir);
                continue;
            }
            println!("═══ {} ═══", group.dir);
            for exe in &group.exes {
                println!("  {}", exe);
            }
        }
    }
}

pub(crate) fn cmd_check_admin(json_out: bool) {
    let is_admin = core::platform::system::check_admin();
    if json_out {
        println!("{}", json!({ "admin": is_admin }));
    } else {
        println!("管理员权限: {}", if is_admin { "是" } else { "否" });
    }
}
