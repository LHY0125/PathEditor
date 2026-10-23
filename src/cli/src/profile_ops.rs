//! `profile` 子命令：配置方案的增删改查与应用。
//!
//! 全部走 **DB**（`core::db::profiles`）。GUI 与 CLI 共用同一份数据。

use crate::output::print_json;
use crate::runtime::exit_core_error;
use path_editor_core as core;
use path_editor_core::db::profiles as db_profiles;

/// 打开默认库；失败按 CLI 退出码约定退出。
fn open_db() -> core::db::Db {
    core::db::open_and_migrate().unwrap_or_else(|e| exit_core_error(&e))
}

pub(crate) fn profile_list(json_out: bool) {
    let db = open_db();
    let list = db_profiles::list_profiles(&db).unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        print_json(&list);
    } else if list.is_empty() {
        println!("暂无配置文件。");
    } else {
        for profile in &list {
            // DB 版的时间是 Unix 毫秒；展示成本地时间
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
    // 先合并注册表与 DB 当前快照，确保已禁用的孤儿条目也进入配置并保留顺序。
    let snapshot =
        core::service::load_path_snapshot_db(&db).unwrap_or_else(|e| exit_core_error(&e));

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

pub(crate) fn profile_apply(name: String) {
    let mut db = open_db();
    // 双 hive 一次编排：core 内按「读配置 → 只写变化的 hive → 记账」执行；
    // 记账失败只告警（注册表已写成功的事实优先于统计）。
    core::service::apply_profile(&mut db, &name, core::history::EventSource::Cli)
        .unwrap_or_else(|e| exit_core_error(&e));

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
