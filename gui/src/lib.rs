mod commands;

use tauri::Manager;

/// 启动时的数据库初始化序列（spec §7.1 / §7.2）。
///
/// 拆成独立函数而不是塞进 `setup` 闭包：`setup` 的返回值是 `Result<(), Box<dyn Error>>`，
/// 把错误类型收窄成 `CoreError` 才能在这里做分步日志，也便于将来加启动自检。
fn setup_database(db: &mut path_editor_core::db::Db) -> Result<(), path_editor_core::CoreError> {
    // ① best-effort 补写 pending。失败只告警：它代表的是上一次操作遗留的
    //    「注册表已写、快照未落盘」，补不上也不该阻止本次启动。
    if let Err(e) =
        path_editor_core::service::retry_pending_path_state(&path_editor_core::history::NoopHistory)
    {
        log::warn!("启动时补写 pending 失败（继续启动）: {}", e.message);
    }

    // ② schema 建库/迁移
    db.migrate_to_current()?;

    // ③ v5 遗留文件一次性导入（幂等：migration_imports 保证重跑不重复）
    let home = path_editor_core::db::crypto::config_dir();
    match path_editor_core::db::migrate_v5::migrate_legacy(db, &home) {
        Ok(report) => {
            if report.imported > 0 || report.failed > 0 {
                log::info!(
                    "v5 文件迁移完成：导入 {}，跳过 {}，失败 {}",
                    report.imported,
                    report.skipped,
                    report.failed
                );
            }
        }
        // 迁移失败不阻断启动：DB 仍可用于新记录，旧文件保持原样等下次重试
        Err(e) => log::warn!("v5 文件迁移失败（继续启动）: {}", e.message),
    }

    // ③b 确保 DB 里有「当前状态」基线（D-6）。
    //
    // **为什么必须有**：D-1 方案 A 让读路径依赖 `app_state.current_snapshot_id`。
    // 但 `import_disabled` 只在 `disabled.json` 存在时才产生 `kind=current` 快照——
    // 新装用户没有这个文件，迁移后 `current_snapshot_id` 就是 NULL。
    // 此时读路径回落到 `read_state()`，而文件已停写且不存在 → 空状态 →
    // **注册表里被禁用的条目会显示成启用**（用户可能在第一次写操作之前就禁用过条目）。
    //
    // 修法：迁移结束时若仍无 `kind=current` 快照，用**注册表当前状态**建一条基线。
    // 放在这里而不是 `migrate_v5` 里，是因为迁移的职责是「导入 v5 文件」，
    // 读注册表属于启动序列。
    ensure_current_snapshot_baseline(db);

    // ④ 备份索引（P1-2）：扫描 backups/ 写入 `backups` 表。
    // 没有这一步，`backup_list` 会永远返回空——命令在、数据恒空是最容易误导的形态。
    // 索引是派生数据（spec §4.2 第 5 条），重建成本低，每次启动重扫一遍即可。
    let backup_dir = path_editor_core::backup::env_backup_dir();
    match path_editor_core::db::backups::index_backups(db, &backup_dir) {
        Ok(n) if n > 0 => log::info!("备份索引：{} 条", n),
        Ok(_) => {}
        Err(e) => log::warn!("备份索引失败（继续启动）: {}", e.message),
    }

    // ⑤ v5 文件停写（spec §7.8 H7 / 缺口清单 P1-4）。
    //
    // **只有走到这里才停写**：DB 打开成功、schema 迁移完成、v5 文件已导入。
    // 任何一步失败都会让 `setup_database` 返回 `Err`，调用方进入降级模式并
    // **不设置**这个标志 —— 那时 `disabled.json` 是唯一的状态兜底（spec §7.8），
    // 停写会让禁用状态彻底丢失。
    //
    // 前置条件：`current_snapshot_id` 必须真的在维护（`commands::service` 的
    // `update_current_snapshot`），否则停写后 PATH 的当前状态在 DB 里没有落点。
    path_editor_core::disabled::set_v5_writes_disabled(true);
    log::info!("v5 文件停写已启用（disabled.json 不再更新）");

    // ⑤ 配套注册「从 DB 读当前状态」的加载器（D-1 方案 A）。
    //
    // 停写之后读路径也必须改从 DB 来，否则会读到一份永不更新的文件。
    // 这里注册的是「拿当前快照的 PATH 条目」：读 `app_state.current_snapshot_id`
    // 指向的快照，转换为 `(system, user)`。
    //
    // **注意**：这个闭包在 `disabled.rs` 内部被调用，届时 `DbState` 已经由
    // `app.manage` 托管 —— 但闭包拿不到 `AppHandle`。因此这里注册的是一个
    // **进程级只读缓存**：启动时把当前快照读进内存，之后由写路径刷新。
    //
    // 这条路径的正确性依赖 `app_state.current_snapshot_id` 被维护
    // （`commands::service::update_current_snapshot`，P1-4 前置）。
    let snapshot_cache: SnapshotCache = std::sync::Arc::new(std::sync::RwLock::new(
        path_editor_core::db::history::current_snapshot(db)
            .ok()
            .flatten()
            .map(|v| (v.system, v.user)),
    ));
    let cache_for_loader = snapshot_cache.clone();
    path_editor_core::disabled::set_current_snapshot_loader(Box::new(move || {
        cache_for_loader.read().ok().and_then(|g| g.clone())
    }));
    READ_SNAPSHOT_CACHE.set(snapshot_cache).ok();
    log::info!("当前状态读取已切到 DB（disabled.json 只读归档）");

    // ⑥ 派生表重建（Task 12）。
    //
    // 三张派生表（spec §4.2 第 5 条「可随时重建，不是真相」）：
    // - `search_fts`：只在**为空**时全量重建。每次启动都重建会让搜索索引
    //   随快照数线性变慢，而它一旦非空就说明已经建过（写入路径会增量维护）。
    // - `daily_stats`：每次启动全量重建。成本是一次全表扫描，当前数据量下是毫秒级；
    //   `rebuild_daily` 内部已按 T-1 记录耗时，等真有瓶颈时日志里有基线。
    // - `var_references`：每次启动全量重建。它只保存当前快照的边，
    //   而当前快照可能在上次退出后被改过（见未覆盖项 #2）。
    //
    // 全部失败只告警：DB 可用性比派生数据新鲜度重要，派生表下次启动会再算。
    //
    // ⚠️ **CLI 不做这些重建**（未覆盖项 #2 / M1 的 R-02）：
    // CLI 的写路径（`cli/src/runtime.rs`）用 `NoopHistory`，不碰 DB 的当前快照，
    // 因此「CLI 改 PATH 后 GUI 的引用图/统计仍是旧的」是已知残余风险。
    // 修它需要 CLI 也接 DB 的 current 快照，属范围扩张，M2 不做。
    rebuild_derived_tables(db);

    Ok(())
}

/// 重建三张派生表（Task 12）。
///
/// 抽成独立函数便于测试（`db_derived_rebuild.rs` 直接调它模拟启动序列）——
/// 若写在 `setup_database` 里，测试就得复现整个启动流程（含注册表读取）。
fn rebuild_derived_tables(db: &mut path_editor_core::db::Db) {
    // `search_fts`：仅当为空时才重建
    match db.query_row_with("SELECT count(*) FROM search_fts", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(0) => match path_editor_core::db::search::rebuild_index(db) {
            Ok(n) => log::info!("FTS 索引：首次构建 {} 条", n),
            Err(e) => log::warn!("FTS 索引重建失败（继续启动）: {}", e.message),
        },
        Ok(_) => {}
        Err(e) => log::warn!("FTS 索引计数失败（继续启动）: {}", e.message),
    }

    // `daily_stats`：每次启动全量重建（内部有耗时日志）
    match path_editor_core::db::stats::rebuild_daily(db) {
        Ok(n) => log::info!("daily_stats 重建：{} 天", n),
        Err(e) => log::warn!("daily_stats 重建失败（继续启动）: {}", e.message),
    }

    // `var_references`：每次启动全量重建
    match path_editor_core::db::refs::rebuild_refs(db) {
        Ok(n) => log::info!("引用图重建：{} 条边", n),
        Err(e) => log::warn!("引用图重建失败（继续启动）: {}", e.message),
    }
}

/// 确保 DB 里有一条 `kind=current` 快照；没有就用注册表当前状态建一条（D-6）。
///
/// 幂等：已有 `current_snapshot_id` 时什么都不做。
/// 失败只告警——它是降级可容忍的（读路径会回落到文件），不该阻断启动。
fn ensure_current_snapshot_baseline(db: &mut path_editor_core::db::Db) {
    // 已有基线：检查 `app_state.current_snapshot_id` 是否指向一条 kind=current
    let has_current: Result<i64, _> = db.query_row_with(
        "SELECT count(*) FROM app_state a
           JOIN snapshots s ON s.id = a.current_snapshot_id
          WHERE a.id = 1 AND s.kind = 'current'",
        [],
        |r| r.get::<_, i64>(0),
    );
    if matches!(has_current, Ok(n) if n > 0) {
        return;
    }

    // 没有基线：用注册表当前状态建一条
    let (system, user) = match path_editor_core::disabled::load_path_snapshot() {
        Ok(s) => (s.system, s.user),
        Err(e) => {
            log::warn!("建立当前状态基线失败（读注册表出错）: {}", e.message);
            return;
        }
    };
    let snap = path_editor_core::db::history::NewSnapshot {
        kind: path_editor_core::db::history::SnapshotKind::Current,
        source: path_editor_core::history::EventSource::Migration,
        system,
        user,
        env: Vec::new(),
        parent_uid: None,
    };
    let uid = match path_editor_core::db::history::insert_snapshot(db, snap) {
        Ok(uid) => uid,
        Err(e) => {
            log::warn!("建立当前状态基线失败（写入失败）: {}", e.message);
            return;
        }
    };
    let version = match path_editor_core::db::history::app_state_version(db) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("建立当前状态基线失败（读 version）: {}", e.message);
            return;
        }
    };
    match path_editor_core::db::history::set_current_snapshot(db, &uid, version) {
        Ok(true) => log::info!("已用注册表当前状态建立 current 快照基线"),
        Ok(false) => log::warn!("建立基线的 CAS 冲突（有并发写），读路径将回落到文件"),
        Err(e) => log::warn!("建立当前状态基线失败: {}", e.message),
    }
}

/// 进程级的「当前 PATH 快照」缓存。
///
/// `disabled.rs` 的加载器是 `fn()` 类型（无参数、无捕获），无法直接持有 `Db`。
/// 用一个进程级缓存桥接：启动时填充，写路径成功后刷新。
///
/// 这不是「全局 Db 单例」——它只缓存一份 `(Vec<PathEntry>, Vec<PathEntry>)`，
/// 没有任何数据库句柄或服务。
/// 一份 `(system, user)` 的 PATH 快照。
type PathPair = (
    Vec<path_editor_core::PathEntry>,
    Vec<path_editor_core::PathEntry>,
);

/// 进程级读缓存：`None` 表示 DB 侧还没有当前快照。
type SnapshotCache = std::sync::Arc<std::sync::RwLock<Option<PathPair>>>;

static READ_SNAPSHOT_CACHE: std::sync::OnceLock<SnapshotCache> = std::sync::OnceLock::new();

/// 写路径成功后刷新缓存，让 `load_path_snapshot()` 读到最新状态。
pub fn refresh_read_snapshot(
    system: Vec<path_editor_core::PathEntry>,
    user: Vec<path_editor_core::PathEntry>,
) {
    if let Some(cache) = READ_SNAPSHOT_CACHE.get() {
        if let Ok(mut g) = cache.write() {
            *g = Some((system, user));
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let level = if cfg!(debug_assertions) {
                log::LevelFilter::Info
            } else {
                log::LevelFilter::Warn
            };
            app.handle()
                .plugin(tauri_plugin_log::Builder::default().level(level).build())?;

            // v6 M1：打开历史库。失败时进入降级模式（spec D16 fail-open）：
            // 注册表编辑照常，history_*/profile_*/backup_* 命令返回 database 错误。
            //
            // 启动序列（spec §7.1）：
            //   ① best-effort 补写 pending（注册表已写、快照未落盘的 gap）
            //   ② 建库/迁移 schema
            //   ③ 首次启动时把 v5 遗留文件导入 DB（幂等，可重跑）
            let db = match path_editor_core::db::open_default() {
                Ok(mut db) => match setup_database(&mut db) {
                    Ok(()) => Some(db),
                    Err(e) => {
                        log::error!("历史库初始化失败，进入降级模式: {}", e.message);
                        None
                    }
                },
                Err(e) => {
                    log::warn!("历史库打开失败，进入降级模式: {}", e.message);
                    None
                }
            };
            app.manage(commands::history::DbState(std::sync::Mutex::new(db)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::registry::load_system_paths,
            commands::registry::load_user_paths,
            commands::system::check_admin,
            commands::system::get_path_capabilities,
            commands::system::validate_path,
            commands::system::expand_env_vars,
            commands::system::broadcast_env_change,
            commands::backup::backup_registry,
            commands::backup::backup_env_vars,
            commands::backup::list_env_backups,
            commands::backup::preview_env_backup,
            commands::backup::restore_env_backup,
            commands::fs::read_text_file,
            commands::fs::import_file,
            commands::fs::export_path_entries,
            commands::disabled::save_disabled_state,
            commands::disabled::load_disabled_state,
            commands::disabled::save_path_snapshot,
            commands::disabled::load_path_snapshot,
            commands::scanner::scan_paths,
            commands::registry::clean_path_entries,
            // P0-1 方案 A：v5 文件版 profile 命令**已下线**。
            // 配置方案统一走 DB（`profile_list` / `profile_save` / `profile_paths` /
            // `profile_rename` / `profile_delete_by_name`）。
            // 保留 `commands::profiles` 模块本身：`core::profiles` 仍是 v5 兼容通路
            // （`migrate_legacy` 读它；spec §7.5 的「导出 v5 JSON」将来要用）。
            commands::env_var::list_all_env_vars,
            commands::env_var::reveal_env_var,
            commands::env_var::update_env_var,
            commands::env_var::create_env_var,
            commands::env_var::delete_env_var,
            commands::service::save_path_with_sidecar,
            commands::service::apply_path_snapshot,
            commands::service::retry_pending_path_state,
            commands::service::apply_profile,
            commands::history::history_list_events,
            commands::history::history_get_snapshot,
            commands::history::history_get_current,
            commands::history::history_reveal_env_value,
            commands::history::profile_list,
            commands::history::profile_delete,
            commands::history::profile_mark_applied,
            commands::history::backup_list,
            commands::history::profile_save,
            commands::history::profile_export_v5,
            // ── M2（Task 9）：检索 / 统计 / 对比 / 引用图 / 冲突历史 ──
            commands::m2::search,
            commands::m2::stats_dashboard,
            commands::m2::stats_rebuild,
            commands::m2::compare_snapshots,
            commands::m2::refs_dependents,
            commands::m2::refs_dependencies,
            commands::m2::refs_cycles,
            commands::m2::refs_rebuild,
            commands::m2::search_rebuild,
            commands::m2::scan_history_list,
            commands::m2::scan_history_diff,
            commands::m2::scan_history_record,
            commands::history::profile_paths,
            commands::history::profile_rename,
            commands::history::profile_delete_by_name,
            commands::history::profile_mark_applied_by_name,
            commands::history::history_restore_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
