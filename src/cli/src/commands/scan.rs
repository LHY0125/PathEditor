use crate::output::print_json;
use crate::runtime::exit_core_error;
use path_editor_core as core;
use serde_json::json;

/// 读取两边的 PATH 用于扫描。
///
/// **不吞错误**：旧实现 `if let Ok(..)` 让读失败退化成空 PATH，
/// `conflicts` 会打印「未发现冲突」并 exit 0 —— 脚本把「读失败」当「无冲突」。
fn load_scan_paths() -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    let mut errors: Vec<(&str, core::CoreError)> = Vec::new();
    match core::platform::registry::load_system_paths() {
        Ok(v) => paths.extend(v),
        Err(e) => errors.push(("系统", e)),
    }
    match core::platform::registry::load_user_paths() {
        Ok(v) => paths.extend(v),
        Err(e) => errors.push(("用户", e)),
    }
    if let Some((_, first)) = errors.first() {
        if paths.is_empty() {
            eprintln!("错误: 系统与用户 PATH 都读取失败，扫描中止");
            exit_core_error(first);
        }
        for (label, e) in &errors {
            eprintln!(
                "警告: 读取{label} PATH 失败，扫描结果可能不完整: {}",
                e.message
            );
        }
    }
    paths
}

pub(crate) fn cmd_conflicts(json_out: bool) {
    let paths = load_scan_paths();
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
    let paths = load_scan_paths();
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
