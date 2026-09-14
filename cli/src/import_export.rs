use crate::runtime::{enabled_paths, exit_err, persist_snapshot, verify_and_save};
use path_editor_core as core;

pub(crate) fn cmd_import(file: String, target: String) {
    let content = core::fs::read_text_file(&file).unwrap_or_else(|e| exit_err(&e));
    let (sys_entries, usr_entries) =
        core::fs::import_paths(&file, &content).unwrap_or_else(|e| exit_err(&e));
    match target.as_str() {
        "system" => {
            let orig = core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e));
            verify_and_save("system", &orig, enabled_paths(&sys_entries));
            persist_snapshot(Some(sys_entries), None);
            println!("已导入到系统 PATH");
        }
        "user" => {
            let orig = core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e));
            verify_and_save("user", &orig, enabled_paths(&usr_entries));
            persist_snapshot(None, Some(usr_entries));
            println!("已导入到用户 PATH");
        }
        _ => {
            if !sys_entries.is_empty() {
                let orig_sys = core::registry::load_system_paths().unwrap_or_else(|e| exit_err(&e));
                verify_and_save("system", &orig_sys, enabled_paths(&sys_entries));
            }
            if !usr_entries.is_empty() {
                let orig_usr = core::registry::load_user_paths().unwrap_or_else(|e| exit_err(&e));
                verify_and_save("user", &orig_usr, enabled_paths(&usr_entries));
            }
            persist_snapshot(
                (!sys_entries.is_empty()).then_some(sys_entries),
                (!usr_entries.is_empty()).then_some(usr_entries),
            );
            println!("已导入到系统 + 用户 PATH");
        }
    }
    core::system::broadcast_env_change();
}

pub(crate) fn cmd_export(format: String, output: Option<String>) {
    let snapshot = core::disabled::load_path_snapshot().unwrap_or_else(|e| exit_err(&e));
    let content = core::fs::export_path_entries(&snapshot.system, &snapshot.user, &format)
        .unwrap_or_else(|e| exit_err(&e));
    if let Some(path) = output {
        let normalized = path.replace('/', "\\").to_lowercase();
        if normalized.starts_with("c:\\windows\\") || normalized.starts_with("c:\\program files\\")
        {
            exit_err(&format!("不允许导出到系统目录: {path}"));
        }
        std::fs::write(&path, &content).unwrap_or_else(|e| exit_err(&format!("无法写入文件: {e}")));
        println!("已导出到: {path}");
    } else {
        println!("{content}");
    }
}
