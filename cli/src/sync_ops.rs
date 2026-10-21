//! `sync` 子命令：同步包的导出、导入、状态与冲突处理。
//!
//! **口令安全**（spec §5.2 / 计划 Task 8）：
//! - 用 `rpassword` 隐藏输入，不进 shell history、不出现在 `ps`；
//! - 口令只存在于内存，不落盘、不写日志；
//! - 错误文案**不得包含口令**——`core` 的错误消息本就不含，这里也不拼接。
//!
//! **同步不写注册表**：导入只改 DB。要生效必须另跑 `profile apply` 或
//! PATH 命令。

use crate::runtime::exit_core_error;
use path_editor_core as core;
use path_editor_core::db::sync::conflict::{list_conflicts, resolve_conflict, Resolution};
use path_editor_core::db::sync::{export_bundle, import_bundle};
use std::path::Path;

/// 打开默认库；失败按 CLI 退出码约定退出（spec §8：Crypto → 5，其余 → 1）。
fn open_db() -> core::db::Db {
    core::db::open_and_migrate().unwrap_or_else(|e| exit_core_error(&e))
}

/// 交互式读口令（两次确认）。空口令直接拒绝。
fn prompt_passphrase(confirm: bool) -> String {
    let first = match rpassword::prompt_password("同步口令: ") {
        Ok(p) => p,
        Err(e) => {
            // 只报 IO 原因，绝不回显口令
            eprintln!("错误: 读取口令失败: {e}");
            std::process::exit(1);
        }
    };
    if first.is_empty() {
        eprintln!("错误: 同步口令不能为空");
        std::process::exit(1);
    }
    if confirm {
        let second = match rpassword::prompt_password("再输一次: ") {
            Ok(p) => p,
            Err(e) => {
                eprintln!("错误: 读取口令失败: {e}");
                std::process::exit(1);
            }
        };
        if first != second {
            eprintln!("错误: 两次输入的口令不一致");
            std::process::exit(1);
        }
    }
    first
}

/// `sync export <FILE>`：把待导出变更写成加密包。
pub(crate) fn sync_export(file: &str) {
    let passphrase = prompt_passphrase(true);
    let mut db = open_db();
    let report = export_bundle(&mut db, &passphrase, Path::new(file))
        .unwrap_or_else(|e| exit_core_error(&e));
    println!(
        "已导出 {} 条变更到 {}",
        report.change_count,
        report.path.display()
    );
}

/// `sync import <FILE>`：解密并应用一个同步包。
pub(crate) fn sync_import(file: &str) {
    let passphrase = prompt_passphrase(false);
    let mut db = open_db();
    let report = import_bundle(&mut db, &passphrase, Path::new(file))
        .unwrap_or_else(|e| exit_core_error(&e));
    println!(
        "导入完成：应用 {}，跳过 {}，冲突 {}",
        report.applied, report.skipped, report.conflicts
    );
    if report.conflicts > 0 {
        // 有冲突时提示下一步，但**不改退出码**：导入本身成功了，
        // 冲突是待人工处理的正常结果，不是错误。
        eprintln!(
            "提示: 有 {} 条冲突待解决，用 `patheditor sync conflicts` 查看",
            report.conflicts
        );
    }
}

/// `sync status`：待导出数与未解决冲突数。
pub(crate) fn sync_status() {
    let db = open_db();
    let pending =
        core::db::sync::change_log::pending_export(&db).unwrap_or_else(|e| exit_core_error(&e));
    let conflicts =
        core::db::sync::conflict::unresolved_count(&db).unwrap_or_else(|e| exit_core_error(&e));
    println!("待导出变更: {}", pending.len());
    println!("未解决冲突: {conflicts}");
}

/// `sync conflicts [--all]`：列出冲突。
pub(crate) fn sync_conflicts(include_resolved: bool) {
    let db = open_db();
    let list = list_conflicts(&db, include_resolved).unwrap_or_else(|e| exit_core_error(&e));
    if list.is_empty() {
        println!("没有冲突。");
        return;
    }
    for c in &list {
        let field = c.field.as_deref().unwrap_or("(整实体)");
        let status = match (&c.resolved_at, &c.resolution) {
            (Some(_), Some(r)) => format!("已解决({r})"),
            _ => "未解决".to_string(),
        };
        println!(
            "[{}] {} {}.{}  local={:?}  remote={:?}  来自设备 {}  {}",
            c.id,
            c.entity_type,
            c.entity_uid,
            field,
            c.local_value,
            c.remote_value,
            c.remote_device,
            status
        );
    }
}

/// `sync resolve <ID> --take local|remote`：解决一条冲突。
pub(crate) fn sync_resolve(id: i64, take: &str) {
    let Some(resolution) = Resolution::parse(take) else {
        eprintln!("错误: --take 只能是 local 或 remote");
        std::process::exit(2);
    };
    let mut db = open_db();
    resolve_conflict(&mut db, id, resolution).unwrap_or_else(|e| exit_core_error(&e));
    println!("冲突 {id} 已按 {take} 解决");
}
