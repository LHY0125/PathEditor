//! v5 文件版配置命令（**已从 `invoke_handler` 下线**）。
//!
//! P0-1 方案 A 之后，配置方案统一走 DB（见 `commands::history` 的
//! `profile_list` / `profile_save` / `profile_paths` / `profile_rename` /
//! `profile_delete_by_name`）。本模块的命令**不再注册**，保留原因：
//!
//! 1. `core::profiles` 仍是 v5 兼容通路——`migrate_legacy` 读它，
//!    spec §7.5 的「导出 v5 JSON」将来要用；
//! 2. 命令函数本身是那套 API 的薄封装，删掉会让「v5 通路」在 GUI 侧失去入口，
//!    将来做导出时还要重写。
//!
//! **不要重新注册它们**——那会让 GUI 重新写 `profiles/*.json`，
//! 与 DB 形成第二个真相源（这正是 P0-1 要消除的）。
#![allow(dead_code)]

use path_editor_core::profiles;

#[tauri::command]
pub fn list_profiles() -> Result<Vec<profiles::ProfileMeta>, String> {
    // F-11：core 侧已迁移为 CoreError；IPC 契约仍是自由文本，透传 message。
    profiles::list_profiles().map_err(|e| e.message)
}
#[tauri::command]
pub fn save_profile(
    name: String,
    sys: Vec<profiles::ProfilePathEntry>,
    user: Vec<profiles::ProfilePathEntry>,
) -> Result<(), String> {
    profiles::save_profile(&name, sys, user).map_err(|e| e.message)
}
#[tauri::command]
pub fn load_profile(name: String) -> Result<profiles::ProfileData, String> {
    profiles::load_profile(&name).map_err(|e| e.message)
}
#[tauri::command]
pub fn delete_profile(name: String) -> Result<(), String> {
    profiles::delete_profile(&name).map_err(|e| e.message)
}
#[tauri::command]
pub fn rename_profile(old_name: String, new_name: String) -> Result<(), String> {
    profiles::rename_profile(&old_name, &new_name).map_err(|e| e.message)
}
