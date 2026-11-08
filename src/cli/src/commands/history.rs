//! `history-*` 子命令：快照备注与固定标记。

use crate::output::print_json;
use crate::runtime::{exit_core_error, open_db_or_exit};
use path_editor_core as core;
use serde_json::json;

/// `history-note <UID> [TEXT]`：设置或清空备注。
pub(crate) fn history_note(uid: String, note: Option<String>, json_out: bool) {
    let mut db = open_db_or_exit();
    let value = note
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());
    core::db::history::set_snapshot_note(&mut db, &uid, value)
        .unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        print_json(&json!({ "uid": uid, "note": value }));
    } else {
        println!("已更新备注: {uid}");
    }
}

/// `history-pin <UID> [--off]`：固定或取消固定。
pub(crate) fn history_pin(uid: String, off: bool, json_out: bool) {
    let mut db = open_db_or_exit();
    let pinned = !off;
    core::db::history::set_snapshot_pinned(&mut db, &uid, pinned)
        .unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        print_json(&json!({ "uid": uid, "pinned": pinned }));
    } else {
        println!(
            "{}: {uid}",
            if pinned {
                "已固定"
            } else {
                "已取消固定"
            }
        );
    }
}

/// `history-pinned [--json]`：列出固定快照。
pub(crate) fn history_pinned(json_out: bool) {
    let Some(db) = crate::runtime::open_db_silent() else {
        crate::runtime::exit_err("固定快照需要历史库；请先运行一次 GUI 建立数据库");
    };
    let list = core::db::history::list_pinned(&db).unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        print_json(&list);
        return;
    }
    if list.is_empty() {
        println!("没有固定快照。");
        return;
    }
    for item in &list {
        println!(
            "{}  {}",
            item.uid,
            item.note.as_deref().unwrap_or("(无备注)")
        );
    }
}
