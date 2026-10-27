use crate::runtime::{exit_core_error, load_path_snapshot_any, save_path_write};
use path_editor_core as core;

pub(crate) fn cmd_import(file: String, target: String) {
    let content = core::platform::fs::read_text_file(&file).unwrap_or_else(|e| exit_core_error(&e));
    let (sys_entries, usr_entries) =
        core::platform::fs::import_paths(&file, &content).unwrap_or_else(|e| exit_core_error(&e));
    match target.as_str() {
        "system" => {
            save_path_write(Some(sys_entries), None);
            println!("已导入到系统 PATH");
        }
        "user" => {
            save_path_write(None, Some(usr_entries));
            println!("已导入到用户 PATH");
        }
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
