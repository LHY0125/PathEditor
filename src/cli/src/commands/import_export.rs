use crate::runtime::{exit_core_error, load_path_snapshot_any, save_path_write};
use path_editor_core as core;

pub(crate) fn cmd_import(file: String, target: String) {
    let content = core::platform::fs::read_text_file(&file).unwrap_or_else(|e| exit_core_error(&e));
    let (sys_entries, usr_entries) =
        core::platform::fs::import_paths(&file, &content).unwrap_or_else(|e| exit_core_error(&e));
    match target.as_str() {
        "system" => {
            // 单 hive 导入时该侧为空 = 把整个 PATH 写成空串。TXT 源文件永远
            // 只产生用户侧条目，这个守卫不是理论问题。
            if sys_entries.is_empty() {
                crate::runtime::exit_err(
                    "源文件的系统 PATH 段为空，已中止：继续会把系统 PATH 清空",
                );
            }
            save_path_write(Some(sys_entries), None);
            println!("已导入到系统 PATH");
        }
        "user" => {
            if usr_entries.is_empty() {
                crate::runtime::exit_err(
                    "源文件的用户 PATH 段为空，已中止：继续会把用户 PATH 清空",
                );
            }
            save_path_write(None, Some(usr_entries));
            println!("已导入到用户 PATH");
        }
        // clap 的 `value_parser` 已限定为 system/user/both，这里只剩 both。
        _ => {
            // 双 hive：一次调用同时提交（core 内顺序 system → user）。
            // 不再取 expected（CLI 无并发窗口，见 `save_path_write`）。
            let sys = (!sys_entries.is_empty()).then_some(sys_entries);
            let usr = (!usr_entries.is_empty()).then_some(usr_entries);
            save_path_write(sys, usr);
            println!("已导入到系统 + 用户 PATH");
        }
    }
    // 广播已由 core 内部完成。
}

pub(crate) fn cmd_export(format: String, output: Option<String>) {
    let snapshot = load_path_snapshot_any().unwrap_or_else(|e| exit_core_error(&e));
    let content =
        core::platform::fs::export_path_entries(&snapshot.system, &snapshot.user, &format)
            .unwrap_or_else(|e| exit_core_error(&e));
    if let Some(path) = output {
        // 系统目录拒绝规则在 core（与备份共用），CLI 不复制判定。
        core::platform::fs::write_export_file(&path, &content)
            .unwrap_or_else(|e| exit_core_error(&e));
        println!("已导出到: {path}");
    } else {
        println!("{content}");
    }
}
