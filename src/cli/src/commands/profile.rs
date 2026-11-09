//! `profile` 子命令：配置方案的增删改查与应用。
//!
//! 全部走 **DB**（`core::db::profiles`）。GUI 与 CLI 共用同一份数据。

use crate::output::print_json;
use crate::runtime::{exit_core_error, open_db_or_exit};
use path_editor_core as core;
use path_editor_core::db::profiles as db_profiles;

pub(crate) fn profile_list(json_out: bool) {
    let db = open_db_or_exit();
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
    let mut db = open_db_or_exit();
    // 先合并注册表与 DB 当前快照，确保已禁用的孤儿条目也进入配置并保留顺序。
    let snapshot =
        core::app::service::load_path_snapshot_db(&db).unwrap_or_else(|e| exit_core_error(&e));

    let env = core::app::service::collect_env_snapshot_rows().unwrap_or_else(|e| {
        eprintln!(
            "警告: 采集环境变量快照失败（配置快照只含 PATH）: {}",
            e.message
        );
        Vec::new()
    });
    let snap = core::db::history::NewSnapshot {
        kind: core::db::history::SnapshotKind::Profile,
        source: core::domain::history::EventSource::Cli,
        system: snapshot.system,
        user: snapshot.user,
        env,
        parent_uid: None,
    };
    db_profiles::save_profile(&mut db, &name, None, &[], snap)
        .unwrap_or_else(|e| exit_core_error(&e));
    println!("已保存配置: {name}");
}

pub(crate) fn profile_load(name: String) {
    let db = open_db_or_exit();
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
    let mut db = open_db_or_exit();
    // 双 hive 一次编排：core 内按「读配置 → 只写变化的 hive → 记账」执行；
    // 记账失败只告警（注册表已写成功的事实优先于统计）。
    core::app::service::apply_profile(&mut db, &name, core::domain::history::EventSource::Cli)
        .unwrap_or_else(|e| exit_core_error(&e));

    println!("配置文件 \"{name}\" 已写入注册表。");
}

pub(crate) fn profile_delete(name: String) {
    let mut db = open_db_or_exit();
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
    let mut db = open_db_or_exit();
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

/// `profile diff <LEFT> <RIGHT> [--json]`
pub(crate) fn profile_diff(left: String, right: String, json_out: bool) {
    let db = open_db_or_exit();
    let diff =
        db_profiles::diff_profiles(&db, &left, &right).unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        print_json(&diff);
        return;
    }
    println!("配置差异：{left} ↔ {right}");
    println!(
        "系统 PATH：新增 {} / 删除 {} / 移动 {} / 启用变化 {}",
        diff.paths.system.added.len(),
        diff.paths.system.removed.len(),
        diff.paths.system.reordered.len(),
        diff.paths.system.toggled.len()
    );
    println!(
        "用户 PATH：新增 {} / 删除 {} / 移动 {} / 启用变化 {}",
        diff.paths.user.added.len(),
        diff.paths.user.removed.len(),
        diff.paths.user.reordered.len(),
        diff.paths.user.toggled.len()
    );
    println!(
        "环境变量：新增 {} / 删除 {} / 修改 {}",
        diff.paths.env_added.len(),
        diff.paths.env_removed.len(),
        diff.paths.env_modified.len()
    );
    if !diff.tags_added.is_empty() {
        println!("新增标签：{}", diff.tags_added.join(", "));
    }
    if !diff.tags_removed.is_empty() {
        println!("删除标签：{}", diff.tags_removed.join(", "));
    }
}
