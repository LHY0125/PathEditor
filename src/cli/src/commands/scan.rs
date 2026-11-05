use crate::output::print_json;
use crate::runtime::exit_core_error;
use path_editor_core as core;
use serde_json::json;

/// 读取两边的 PATH 用于扫描。
///
/// **不吞错误**：旧实现 `if let Ok(..)` 让读失败退化成空 PATH，
/// `conflicts` 会打印「未发现冲突」并 exit 0 —— 脚本把「读失败」当「无冲突」。
fn load_scan_paths() -> Vec<String> {
    let (paths, warnings) = merge_scan_paths(
        core::platform::registry::load_system_paths(),
        core::platform::registry::load_user_paths(),
    )
    .unwrap_or_else(|e| exit_core_error(&e));
    for (label, message) in warnings {
        eprintln!("警告: 读取{label} PATH 失败，扫描结果可能不完整: {message}");
    }
    paths
}

/// 合并两侧读取的结果与告警：`(paths, warnings)`。
type ScanPaths = (Vec<String>, Vec<(&'static str, String)>);

/// 合并两侧读取结果：**两侧都失败**才返回 `Err`，单侧失败只产生一条警告。
///
/// 判据必须是「错误数」而不是「读到的条目数」：某侧 PATH 本就是空的很常见，
/// 用 `paths.is_empty()` 会把「只坏了一边」误报成两边都坏，脚本据此中止扫描。
fn merge_scan_paths(
    system: Result<Vec<String>, core::CoreError>,
    user: Result<Vec<String>, core::CoreError>,
) -> Result<ScanPaths, core::CoreError> {
    let mut paths: Vec<String> = Vec::new();
    let mut warnings: Vec<(&'static str, String)> = Vec::new();
    let mut first_error: Option<core::CoreError> = None;
    for (label, result) in [("系统", system), ("用户", user)] {
        match result {
            Ok(v) => paths.extend(v),
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(e.clone());
                }
                warnings.push((label, e.message));
            }
        }
    }
    if warnings.len() == 2 {
        return Err(first_error.expect("两侧都失败时必有错误"));
    }
    Ok((paths, warnings))
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

#[cfg(test)]
mod tests {
    use super::merge_scan_paths;

    fn err(msg: &str) -> path_editor_core::CoreError {
        path_editor_core::CoreError::new(
            path_editor_core::ErrorCode::Io,
            "load_paths",
            msg.to_string(),
        )
    }

    /// 单侧读失败 + 另一侧 PATH 恰好为空，**不得**被当成「两侧都读失败」。
    ///
    /// 回归守卫：旧实现用 `paths.is_empty()` 判定全部失败，这种机器上
    /// `patheditor scan` 会直接中止，而它其实只丢了一侧数据。
    #[test]
    fn single_hive_failure_with_empty_other_side_is_not_fatal() {
        let (paths, warnings) = merge_scan_paths(Ok(Vec::new()), Err(err("拒绝访问")))
            .expect("单侧失败必须仍可继续扫描");
        assert!(paths.is_empty());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].0, "用户");
        assert!(warnings[0].1.contains("拒绝访问"));
    }

    /// 两侧都失败 → Err（不得把「读不到」伪装成「无冲突」）。
    #[test]
    fn both_hives_failing_is_fatal() {
        let result = merge_scan_paths(Err(err("系统坏了")), Err(err("用户坏了")));
        assert!(result.is_err(), "两侧都失败必须返回 Err");
    }
}
