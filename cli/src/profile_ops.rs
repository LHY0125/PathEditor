//! `profile` 子命令：配置方案的增删改查与应用。
//!
//! **P0-1 方案 A**：全部走 **DB**（`core::db::profiles`），不再读写
//! `~/.patheditor/profiles/*.json`。GUI 与 CLI 共用同一份数据，
//! 消除「两个真相源」。
//!
//! 迁移期说明：v5 的 `profiles/*.json` 由启动时的 `migrate_legacy` 一次性导入，
//! 之后**不再写入**（spec §7.8 的停写原则）。旧文件保留在原地不删（spec §7.4）。

use crate::runtime::{exit_core_error, exit_persist_error, save_path_write};
use path_editor_core as core;
use path_editor_core::db::profiles as db_profiles;

/// 打开默认库；失败按 CLI 退出码约定退出（spec §8：Crypto → 5，其余 → 1）。
fn open_db() -> core::db::Db {
    core::db::open_and_migrate().unwrap_or_else(|e| exit_core_error(&e))
}

pub(crate) fn profile_list(json_out: bool) {
    let db = open_db();
    let list = db_profiles::list_profiles(&db).unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        println!("{}", serde_json::to_string_pretty(&list).unwrap());
    } else if list.is_empty() {
        println!("暂无配置文件。");
    } else {
        for profile in &list {
            // DB 版的时间是 Unix 毫秒；展示成本地时间，与 v5 版的字符串口径一致
            let when = chrono::DateTime::from_timestamp_millis(profile.updated_at)
                .map(|t| {
                    t.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_else(|| profile.updated_at.to_string());
            println!("  {}  ({})", profile.name, when);
        }
    }
}

pub(crate) fn profile_save(name: String) {
    let mut db = open_db();
    // 先合并注册表与 disabled.json，确保已禁用的孤儿条目也进入配置并保留顺序。
    let snapshot = core::disabled::load_path_snapshot().unwrap_or_else(|e| exit_persist_error(&e));

    let snap = core::db::history::NewSnapshot {
        kind: core::db::history::SnapshotKind::Profile,
        source: core::history::EventSource::Cli,
        system: snapshot.system,
        user: snapshot.user,
        env: Vec::new(),
        parent_uid: None,
    };
    db_profiles::save_profile(&mut db, &name, None, &[], snap)
        .unwrap_or_else(|e| exit_core_error(&e));
    println!("已保存配置: {name}");
}

pub(crate) fn profile_load(name: String) {
    let db = open_db();
    let Some((system, user)) =
        db_profiles::profile_paths(&db, &name).unwrap_or_else(|e| exit_core_error(&e))
    else {
        exit_core_error(&core::CoreError::new(
            core::ErrorCode::NotFound,
            "profile_load",
            format!("配置不存在或没有内容: {name}"),
        ));
    };
    println!("═══ 系统 PATH ({} 条) ═══", system.len());
    for entry in &system {
        println!(
            "  [{}] {}",
            if entry.enabled { "✓" } else { "✗" },
            entry.path
        );
    }
    println!("═══ 用户 PATH ({} 条) ═══", user.len());
    for entry in &user {
        println!(
            "  [{}] {}",
            if entry.enabled { "✓" } else { "✗" },
            entry.path
        );
    }
}

/// 把一个 DB 配置导出成 v5 `profiles/*.json` 格式（M1 欠账 P2-2）。
///
/// **按名字**指定配置（与其它 `profile` 子命令一致），内部转成 uid 后交给
/// `core::db::v5_export::export_profile_v5`。输出到 stdout，可直接重定向成文件。
///
/// 这是**降级到 v5 的退路**：DB 不可用或需要回到旧版本时，用它可以导出
/// 与 v5 自身写出的格式完全同构的文件（含 `schemaVersion` 信封）。
pub(crate) fn profile_export_v5(name: String) {
    let db = open_db();
    let Some(view) =
        db_profiles::get_profile_by_name(&db, &name).unwrap_or_else(|e| exit_core_error(&e))
    else {
        exit_core_error(&core::CoreError::new(
            core::ErrorCode::NotFound,
            "profile_export_v5",
            format!("配置不存在: {name}"),
        ));
    };
    let json = core::db::v5_export::export_profile_v5(&db, &view.uid)
        .unwrap_or_else(|e| exit_core_error(&e));
    println!("{json}");
}

pub(crate) fn profile_apply(name: String) {
    let db = open_db();
    let Some((system, user)) =
        db_profiles::profile_paths(&db, &name).unwrap_or_else(|e| exit_core_error(&e))
    else {
        exit_core_error(&core::CoreError::new(
            core::ErrorCode::NotFound,
            "profile_apply",
            format!("配置不存在或没有内容: {name}"),
        ));
    };
    // 双 hive 一次提交（core 内顺序 system → user，含备份/sidecar/广播）。
    // Wave 3.1：不再取 expected（CLI 无并发窗口，见 `save_path_write`）。
    save_path_write(Some(system), Some(user));

    // P0-1：应用成功后记账（失败只告警——注册表已写成功的事实优先于统计）
    let mut db = db;
    if let Err(e) = db_profiles::mark_profile_applied_by_name(&mut db, &name) {
        eprintln!("警告: 配置应用计数更新失败: {}", e.message);
    }

    println!("配置文件 \"{name}\" 已写入注册表。");
}

pub(crate) fn profile_delete(name: String) {
    let mut db = open_db();
    let removed =
        db_profiles::delete_profile_by_name(&mut db, &name).unwrap_or_else(|e| exit_core_error(&e));
    if removed {
        println!("已删除配置: {name}");
    } else {
        exit_core_error(&core::CoreError::new(
            core::ErrorCode::NotFound,
            "profile_delete",
            format!("配置不存在: {name}"),
        ));
    }
}

pub(crate) fn profile_rename(old_name: String, new_name: String) {
    let mut db = open_db();
    let renamed = db_profiles::rename_profile(&mut db, &old_name, &new_name)
        .unwrap_or_else(|e| exit_core_error(&e));
    if renamed {
        println!("已重命名: {old_name} → {new_name}");
    } else {
        exit_core_error(&core::CoreError::new(
            core::ErrorCode::NotFound,
            "profile_rename",
            format!("配置不存在或名称未变: {old_name}"),
        ));
    }
}
