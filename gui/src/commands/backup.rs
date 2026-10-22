use super::db_state::{with_db_mut, DbState};
use path_editor_core::backup;
use path_editor_core::backup::{EnvBackupInfo, RestoreOutcome, RestorePreview};
use path_editor_core::db::backups::{self, BackupView};
use path_editor_core::error::CoreError;
use tauri::State;

#[tauri::command]
pub fn backup_registry(custom_dir: Option<String>) -> Result<String, String> {
    backup::backup_registry(custom_dir)
}

/// 立即创建一份环境变量备份，返回备份文件路径。
///
/// # Returns
/// - `Ok(String)` — 备份文件路径。**不保证绝对**：由 `env_backup_dir()` 决定，
/// 该目录可被 `PATHEDITOR_BACKUP_DIR` 设成相对路径（见 core 侧文档）。
/// - `Err(CoreError)` — 采集或落盘失败（code=`Io`，或对未支持的注册表类型为
/// `UnsupportedType`）
#[tauri::command]
pub fn backup_env_vars() -> Result<String, CoreError> {
    backup::backup_env_vars().map(|path| path.to_string_lossy().into_owned())
}

/// 列出已有的环境变量备份（Rust 侧按时间倒序，**不解析内容**）。
///
/// # Returns
/// - `Ok(Vec<EnvBackupInfo>)` — 备份列表，最新在前；目录不存在时为空列表
/// - `Err(CoreError)` — 目录枚举失败（code=`Io`）
#[tauri::command]
pub fn list_env_backups() -> Result<Vec<EnvBackupInfo>, CoreError> {
    backup::list_env_backups()
}

/// 计算备份相对当前注册表的差异（**纯读**，不写任何内容）。
///
/// 路径来源校验必须先于读取：core 的 `preview_restore_file` 自身**不校验**，
/// 对不可解析的文件会经 persist 层把它重命名隔离为 `<file>.corrupt-<ts>`。
/// 因此这里第一行就调 `validate_backup_path`，判定规则全部在 core，
/// 本命令只做参数转发（GUI/CLI 均不得复制该规则）。
///
/// # Returns
/// - `Ok(RestorePreview)` — 差异摘要
/// - `Err(CoreError)` — 路径非法（`InvalidValue`/`NotFound`）、读取或解析失败
/// （`Io`/`Parse`）、打开注册表键或枚举失败（`PermissionDenied`/`Io`）
#[tauri::command]
pub fn preview_env_backup(file: String) -> Result<RestorePreview, CoreError> {
    let path = backup::validate_backup_path(&file)?;
    backup::preview_restore_file(&path)
}

/// 从备份文件恢复环境变量。
///
/// 与 [`preview_env_backup`] 同样先做路径校验（core 侧 `restore_env_backup_from`
/// 内部还会再校验一次，这里是同源调用，不构成第二套规则）。
///
/// **权限提示**：未提权时写方式打开 HKLM 必然失败，本命令恒返回
/// code=`permissionDenied` —— 即使备份只含用户 hive 变量。前端据此提示需要
/// 管理员权限，不得绕过 core 的权限判定。
///
/// # Returns
/// - `Ok(RestoreOutcome)` — 恢复结果（含逐条失败）
/// - `Err(CoreError)` — 路径非法、读取失败、打开 hive 失败（`PermissionDenied`），
/// 或 `force = false` 时检测到冲突（code=`Conflict`，注册表零改动）
#[tauri::command]
pub fn restore_env_backup(file: String, force: bool) -> Result<RestoreOutcome, CoreError> {
    let path = backup::validate_backup_path(&file)?;
    backup::restore_env_backup_from(&path, force)
}

/// 列出备份索引。
///
/// 每次调用前先增量索引：只在启动时索引会漏掉「启动后新做的备份」。
/// `index_backups` 按 `file_path` 去重，重复调用幂等，成本只是目录 stat。
#[tauri::command]
pub fn backup_list(state: State<'_, DbState>) -> Result<Vec<BackupView>, CoreError> {
    with_db_mut(&state, "backup_list", |db| {
        let dir = path_editor_core::backup::env_backup_dir();
        // 索引失败不阻断列表：已有索引行仍然有价值
        if let Err(e) = backups::index_backups(db, &dir) {
            log::warn!("备份索引刷新失败（展示已有索引）: {}", e.message);
        }
        backups::list_backups(db)
    })
}
