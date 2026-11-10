mod commands;

use tauri::Manager;

/// 初始窗口尺寸上限：4K@100% 下若仍按 98% 打开会得到接近 4K 的窗口，
/// 超出桌面工具的合理默认值；2K 及高缩放场景不受影响。
const MAX_INITIAL_WIDTH: f64 = 2400.0;
const MAX_INITIAL_HEIGHT: f64 = 1500.0;
const MIN_WINDOW_WIDTH: f64 = 1100.0;
const MIN_WINDOW_HEIGHT: f64 = 700.0;
const INITIAL_WINDOW_RATIO: f64 = 0.98;

/// 按显示器工作区计算启动窗口尺寸（逻辑像素）。
///
/// 工作区已排除任务栏；缩放由调用方用逻辑尺寸交给 Tauri 处理。
fn initial_window_size(work_width: f64, work_height: f64) -> (f64, f64) {
    let width = (work_width * INITIAL_WINDOW_RATIO).clamp(MIN_WINDOW_WIDTH, MAX_INITIAL_WIDTH);
    let height = (work_height * INITIAL_WINDOW_RATIO).clamp(MIN_WINDOW_HEIGHT, MAX_INITIAL_HEIGHT);
    (width, height)
}

/// Tauri 的 `set_size` 设置的是客户区，边框和标题栏会额外占用工作区；
/// 目标外框尺寸必须扣掉实测外框差，否则窗口底边会越过任务栏/屏幕。
fn inner_size_for_outer(
    outer_width: f64,
    outer_height: f64,
    frame_width: f64,
    frame_height: f64,
) -> (f64, f64) {
    (
        (outer_width - frame_width).max(MIN_WINDOW_WIDTH),
        (outer_height - frame_height).max(MIN_WINDOW_HEIGHT),
    )
}

/// 启动时的数据库初始化序列。
///
/// 迁移、pending 补写、当前状态基线、备份索引、保留策略、派生表重建
/// 全部在 `core::app::bootstrap`；这里只负责把结果写进 GUI 的日志。
/// 引导失败（schema 不兼容、旧加密库、磁盘错误等）由调用方决定是否进入降级模式。
fn setup_database(db: &mut path_editor_core::db::Db) -> Result<(), path_editor_core::CoreError> {
    let report = path_editor_core::app::bootstrap::initialize(
        db,
        path_editor_core::domain::history::EventSource::Gui,
        &path_editor_core::app::bootstrap::BootstrapOptions::default(),
    )?;

    for warning in &report.warnings {
        log::warn!("{}", warning);
    }
    if report.baseline_created {
        log::info!("已用注册表当前状态建立 current 快照基线");
    }
    if report.backups_indexed > 0 {
        log::info!("备份索引：{} 条", report.backups_indexed);
    }
    if report.changes_pruned > 0 || report.conflicts_pruned > 0 {
        log::info!(
            "保留策略清理：changes {} 条，冲突 {} 条",
            report.changes_pruned,
            report.conflicts_pruned
        );
    }
    if report.fts_indexed > 0 {
        log::info!("FTS 索引：首次构建 {} 条", report.fts_indexed);
    }
    if let Some((days, edges)) = report.derived_rebuilt {
        log::info!("派生表重建：daily_stats {} 天，引用图 {} 条边", days, edges);
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 单实例：双开会重复跑启动序列（基线快照、备份索引、派生表重建），
    // 并让两个进程并发写同一个数据库与注册表。
    if !path_editor_core::platform::system::acquire_single_instance("Local\\PathEditor.Gui") {
        // 此时 logger 尚未初始化；直接退出即可（第二个实例不做任何事）。
        return;
    }

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

            // 打开历史库。失败时进入降级模式：
            // 注册表编辑照常，history_*/profile_*/backup_* 命令返回 database 错误。
            //
            // 启动序列：
            //  ① 建库/迁移 schema
            //  ② 补写上次未落盘的 pending 快照
            //  ③ 建立当前状态基线
            //  ④ 备份索引 / 保留策略 / 派生表重建
            let (db, init_error) = match path_editor_core::db::open_default() {
                Ok(mut db) => match setup_database(&mut db) {
                    Ok(()) => (Some(db), None),
                    Err(e) => {
                        log::error!("历史库初始化失败，进入降级模式: {}", e.message);
                        (None, Some(e))
                    }
                },
                Err(e) => {
                    log::warn!("历史库打开失败，进入降级模式: {}", e.message);
                    (None, Some(e))
                }
            };
            app.manage(commands::db_state::DbState::new(db, init_error));

            // 2K/高 DPI：按显示器工作区 98% 初始化，最大化利用横向与纵向空间。
            // 尺寸使用逻辑像素，Windows 125%/150% 缩放会自动换算；保留可缩放。
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = window.current_monitor() {
                    let scale = monitor.scale_factor();
                    let logical = monitor.work_area().size.to_logical::<f64>(scale);
                    let (outer_width, outer_height) =
                        initial_window_size(logical.width, logical.height);
                    // 边框/标题栏是外框与客户区的差值；按逻辑像素扣掉后再 set_size。
                    let frame_width = window
                        .outer_size()
                        .map(|outer| {
                            outer
                                .width
                                .saturating_sub(window.inner_size().map_or(0, |i| i.width))
                        })
                        .map_or(0.0, |width| f64::from(width) / scale);
                    let frame_height = window
                        .outer_size()
                        .map(|outer| {
                            outer
                                .height
                                .saturating_sub(window.inner_size().map_or(0, |i| i.height))
                        })
                        .map_or(0.0, |height| f64::from(height) / scale);
                    let (width, height) =
                        inner_size_for_outer(outer_width, outer_height, frame_width, frame_height);
                    let _ = window.set_size(tauri::LogicalSize::new(width, height));
                    let _ = window.center();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::check_admin,
            commands::system::get_path_capabilities,
            commands::system::validate_path,
            commands::system::expand_env_vars,
            commands::backup::backup_env_vars,
            commands::backup::list_env_backups,
            commands::backup::preview_env_backup,
            commands::backup::restore_env_backup,
            commands::backup::backup_reconcile,
            commands::backup::backup_purge_missing,
            commands::fs::import_file,
            commands::fs::open_data_dir,
            commands::fs::export_path_entries,
            commands::disabled::load_path_snapshot,
            commands::scanner::scan_paths,
            commands::scanner::scan_history_list,
            commands::scanner::scan_history_diff,
            commands::registry::clean_path_entries,
            commands::env_var::list_all_env_vars,
            commands::env_var::reveal_env_var,
            commands::env_var::update_env_var,
            commands::env_var::create_env_var,
            commands::env_var::delete_env_var,
            commands::service::save_path_with_sidecar,
            commands::history::history_list_events,
            commands::history::history_get_snapshot,
            commands::history::history_get_current,
            commands::history::history_export_events,
            commands::history::history_reveal_env_value,
            commands::history::history_set_note,
            commands::history::history_set_pinned,
            commands::history::history_list_pinned,
            commands::history::db_health,
            commands::history::history_restore_env_snapshot,
            commands::history::history_restore_full_snapshot,
            commands::profiles::profile_list,
            commands::profiles::profile_diff,
            commands::profiles::profile_apply,
            commands::profiles::profile_save,
            // ── 检索 / 统计 / 对比 / 引用图 ──
            commands::analytics::search,
            commands::analytics::stats_dashboard,
            commands::analytics::retention_settings,
            commands::analytics::retention_save,
            commands::analytics::compare_snapshots,
            commands::analytics::compare_many,
            commands::rules::rules_list,
            commands::rules::rules_create,
            commands::rules::rules_delete,
            commands::rules::rules_set_enabled,
            commands::rules::rules_apply,
            commands::analytics::refs_dependents,
            commands::analytics::refs_dependencies,
            commands::analytics::refs_cycles,
            commands::profiles::profile_paths,
            commands::profiles::profile_rename,
            commands::profiles::profile_delete_by_name,
            commands::history::history_restore_snapshot,
            // ── 同步中心 ──
            commands::sync::sync_status,
            commands::sync::sync_export,
            commands::sync::sync_import,
            commands::sync::sync_list_conflicts,
            commands::sync::sync_resolve_conflict,
            commands::sync::sync_resolve_conflicts,
            commands::sync::sync_conflict_detail,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{initial_window_size, inner_size_for_outer};

    #[test]
    fn two_k_high_scaling_uses_98_percent_work_area() {
        // 2560×1440 @125% 的工作区约为 2048×1120
        let (width, height) = initial_window_size(2048.0, 1120.0);
        assert!((width - 2007.04).abs() < 0.01);
        assert!((height - 1097.6).abs() < 0.01);
    }

    #[test]
    fn four_k_full_scale_is_capped() {
        let (width, height) = initial_window_size(3840.0, 2116.0);
        assert_eq!((width, height), (2400.0, 1500.0));
    }

    #[test]
    fn small_work_area_keeps_minimum_size() {
        let (width, height) = initial_window_size(800.0, 600.0);
        assert_eq!((width, height), (1100.0, 700.0));
    }

    #[test]
    fn frame_is_subtracted_from_outer_target() {
        let (width, height) = inner_size_for_outer(2007.04, 1128.96, 15.0, 38.0);
        assert!((width - 1992.04).abs() < 0.01);
        assert!((height - 1090.96).abs() < 0.01);
    }
}
