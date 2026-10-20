//! v5 遗留文件的一次性迁移（spec §7）。
//!
//! **幂等**：以 `migration_imports(source_path, content_hash)` 判重（spec B3），
//! 迁移可重跑而不会产生重复快照。
//!
//! **不动原件**：只读 + 登记导入记录；`.v5.bak` 归档与删除留给 M4（spec §7.4）。
//!
//! 迁移范围（spec §7.2）：
//! - `disabled.json` → `snapshots(kind=current)` + `path_entries`
//! - `profiles/*.json` → `profiles` + `profile_revisions` + `snapshots`
//! - `env_backup_*.json` → `snapshots(kind=backup)` + `env_values`（排除 Path）
//! - `path_backup_*.txt` → `snapshots(kind=backup)` + `path_entries`
//!
//! **pending 补写不在本模块**：spec §7.1 把「先 `retry_pending_path_state`」
//! 列为**启动序列**的第一步，由调用方（GUI/CLI 启动路径）在调用本函数前完成。
//! 刻意不在此处调用，原因有二：
//! 1. `retry_pending_path_state` 读写的是 `~/.patheditor/` 下的**真实**文件
//!    （`disabled.rs` 的 `#[cfg(test)]` 临时路径只对同 crate 单元测试生效，
//!    集成测试链接的是非 test 构建的 lib）——在迁移函数里调用会让
//!    `core/tests/db_migrate_v5.rs` 触碰用户真实数据；
//! 2. 本函数只应操作显式传入的 `home`，不产生隐式的全局副作用。

use super::history::{insert_snapshot_tx, EnvValueRow, NewSnapshot, SnapshotKind};
use super::profiles::save_profile_tx;
use super::{db_err, Db};
use crate::backup::read_env_backup;
use crate::error::{CoreError, ErrorCode};
use crate::history::{
    EventAction, EventResult, EventSource, HistoryEvent, HistorySink, NoopHistory,
};
use crate::persist::Versioned;
use crate::profiles::ProfileData;
use crate::PathEntry;
use serde::Deserialize;
use std::path::Path;

/// 迁移结果统计。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// 本次实际导入的文件数
    pub imported: usize,
    /// 因已导入而跳过的文件数（幂等重跑）
    pub skipped: usize,
    /// 解析/读取失败而跳过的文件数（单个坏文件不阻断整体）
    pub failed: usize,
}

/// 执行一次全量迁移（不记录事件）。
pub fn migrate_legacy(db: &mut Db, home: &Path) -> Result<MigrationReport, CoreError> {
    migrate_legacy_with(db, home, &NoopHistory)
}

/// 执行迁移并上报事件（GUI/CLI 注入 `DbHistory` 时使用）。
///
/// **调用方必须先完成 pending 补写**（spec §7.1 启动序列第一步），见模块文档。
pub fn migrate_legacy_with(
    db: &mut Db,
    home: &Path,
    history: &dyn HistorySink,
) -> Result<MigrationReport, CoreError> {
    let mut report = MigrationReport::default();

    import_disabled(db, home, &mut report)?;
    import_profiles(db, home, &mut report)?;
    import_env_backups(db, home, &mut report)?;
    import_path_backups(db, home, &mut report)?;

    // 标记已完成 v5 迁移（app_state 可能尚无行，用 UPSERT 建立单行状态）
    db.with_transaction(|tx| {
        tx.execute(
            "INSERT INTO app_state (id, current_snapshot_id, version, migrated_v5, device_id, lamport)
             VALUES (1, (SELECT id FROM snapshots ORDER BY created_at DESC, id DESC LIMIT 1),
                     1, 1, ?1, 0)
             ON CONFLICT(id) DO UPDATE SET migrated_v5 = 1",
            rusqlite::params![uuid::Uuid::now_v7().to_string()],
        )
        .map_err(|e| db_err("mark_migrated", e))?;
        Ok(())
    })?;

    // 迁移整体作为一次事件（不含明文）
    let _ = history.record(HistoryEvent {
        source: EventSource::Migration,
        action: EventAction::Import,
        hive: None,
        target: None,
        changed_fields: vec![format!(
            "imported={} skipped={} failed={}",
            report.imported, report.skipped, report.failed
        )],
        before_revision: None,
        after_revision: None,
        result: if report.failed > 0 {
            EventResult::Partial
        } else {
            EventResult::Success
        },
    });

    Ok(report)
}

/// `disabled.json` → `snapshots(kind=current)`。
///
/// **不调用 `disabled::load_path_snapshot()`**：那个函数会读真实注册表并把
/// 禁用状态合并进来（`disabled.rs:212`），迁移阶段我们只想要文件本身的内容。
fn import_disabled(
    db: &mut Db,
    home: &Path,
    report: &mut MigrationReport,
) -> Result<(), CoreError> {
    let path = home.join("disabled.json");
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read(&path)
        .map_err(|e| CoreError::new(ErrorCode::Io, "migrate_disabled", format!("读取失败: {e}")))?;
    let hash = blake3::hash(&raw).to_hex().to_string();
    if already_imported(db, &path, &hash)? {
        report.skipped += 1;
        return Ok(());
    }

    // 用 Versioned 信封解析：无 schemaVersion 的旧文件按 v1 读（persist.rs:37）
    let versioned: Versioned<LegacyDisabledState> = match serde_json::from_slice(&raw) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("disabled.json 解析失败，跳过迁移: {e}");
            report.failed += 1;
            return Ok(());
        }
    };
    let state = versioned.inner;

    let snap = NewSnapshot {
        kind: SnapshotKind::Current,
        source: EventSource::Migration,
        system: state.system_snapshot,
        user: state.user_snapshot,
        env: Vec::new(),
        parent_uid: None,
    };
    db.with_transaction(|tx| {
        insert_snapshot_tx(tx, snap)?;
        record_import(tx, &path, &hash)
    })?;
    report.imported += 1;
    Ok(())
}

/// `profiles/*.json` → `profiles` + `profile_revisions` + `snapshots`。
fn import_profiles(
    db: &mut Db,
    home: &Path,
    report: &mut MigrationReport,
) -> Result<(), CoreError> {
    let dir = home.join("profiles");
    if !dir.exists() {
        return Ok(());
    }
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("无法枚举配置目录 {}: {e}", dir.display());
            return Ok(());
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("json"))
            .unwrap_or(false)
        {
            continue;
        }
        let raw = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("读取配置 {} 失败，跳过: {e}", path.display());
                report.failed += 1;
                continue;
            }
        };
        let hash = blake3::hash(&raw).to_hex().to_string();
        if already_imported(db, &path, &hash)? {
            report.skipped += 1;
            continue;
        }

        // B3：正常路径带 schemaVersion 信封；无版本头的旧格式按 v1 读
        let data: ProfileData = match serde_json::from_slice::<Versioned<ProfileData>>(&raw) {
            Ok(v) => v.inner,
            Err(e) => {
                log::warn!("配置 {} 解析失败，跳过: {e}", path.display());
                report.failed += 1;
                continue;
            }
        };

        let snap = NewSnapshot {
            kind: SnapshotKind::Profile,
            source: EventSource::Migration,
            system: data.sys,
            user: data.user,
            env: Vec::new(),
            parent_uid: None,
        };
        let now = chrono::Utc::now().timestamp_millis();
        let result = db.with_transaction(|tx| {
            let r = save_profile_tx(tx, &data.name, None, &[], snap, now);
            match r {
                Ok(_) => {
                    record_import(tx, &path, &hash)?;
                    Ok(true)
                }
                // 同名配置已存在（例如用户先手动建过）：跳过而非整体失败。
                // 这是迁移期可接受的冲突，M4 的清理阶段再统一处理。
                Err(e) if e.code == ErrorCode::NameExists => {
                    log::warn!(
                        "配置「{}」已存在于库中，跳过 {}: {}",
                        data.name,
                        path.display(),
                        e.message
                    );
                    Ok(false)
                }
                Err(e) => Err(e),
            }
        })?;
        if result {
            report.imported += 1;
        } else {
            report.skipped += 1;
        }
    }
    Ok(())
}

/// `env_backup_*.json` → `snapshots(kind=backup)` + `env_values`。
fn import_env_backups(
    db: &mut Db,
    home: &Path,
    report: &mut MigrationReport,
) -> Result<(), CoreError> {
    let dir = home.join("backups");
    if !dir.exists() {
        return Ok(());
    }
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("env_backup_") || !name.ends_with(".json") {
            continue;
        }
        let raw = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("读取备份 {} 失败，跳过: {e}", path.display());
                report.failed += 1;
                continue;
            }
        };
        let hash = blake3::hash(&raw).to_hex().to_string();
        if already_imported(db, &path, &hash)? {
            report.skipped += 1;
            continue;
        }

        let payload = match read_env_backup(&path) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("备份 {} 解析失败，跳过: {}", path.display(), e.message);
                report.failed += 1;
                continue;
            }
        };

        // D13：Path 走专用通路，不得进入 env_values；
        // Unsupported 只存元数据（value=None、restorable=false）。
        let mut env = Vec::new();
        for (hive, vars) in [
            ("system", &payload.hives.system),
            ("user", &payload.hives.user),
        ] {
            for v in vars {
                if v.name.eq_ignore_ascii_case("path") {
                    continue;
                }
                let restorable = !matches!(v.kind, crate::env_var::EnvValueKind::Unsupported);
                env.push(EnvValueRow {
                    hive: hive.to_string(),
                    name: v.name.clone(),
                    kind: match v.kind {
                        crate::env_var::EnvValueKind::String => "String",
                        crate::env_var::EnvValueKind::ExpandString => "ExpandString",
                        crate::env_var::EnvValueKind::Unsupported => "Unsupported",
                    }
                    .to_string(),
                    // Unsupported 不存值（即使备份里有字符串形式）
                    value: if restorable {
                        Some(v.value.clone())
                    } else {
                        None
                    },
                    revision: v.revision.clone(),
                    sensitive: false, // 写入层会按 is_sensitive 重新计算（H2）
                    restorable,
                });
            }
        }

        let snap = NewSnapshot {
            kind: SnapshotKind::Backup,
            source: EventSource::Migration,
            system: Vec::new(),
            user: Vec::new(),
            env,
            parent_uid: None,
        };
        db.with_transaction(|tx| {
            insert_snapshot_tx(tx, snap)?;
            record_import(tx, &path, &hash)
        })?;
        report.imported += 1;
    }
    Ok(())
}

/// `path_backup_*.txt` → `snapshots(kind=backup)` + `path_entries`。
///
/// 文本格式（`backup.rs:1253`）：
/// ```text
/// PathEditor Backup - <时间>
///
/// [System PATH]
/// <path>
/// ...
/// [User PATH]
/// <path>
/// ...
/// ```
fn import_path_backups(
    db: &mut Db,
    home: &Path,
    report: &mut MigrationReport,
) -> Result<(), CoreError> {
    let dir = home.join("backups");
    if !dir.exists() {
        return Ok(());
    }
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("path_backup_") || !name.ends_with(".txt") {
            continue;
        }
        let raw = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("读取 PATH 备份 {} 失败，跳过: {e}", path.display());
                report.failed += 1;
                continue;
            }
        };
        let hash = blake3::hash(&raw).to_hex().to_string();
        if already_imported(db, &path, &hash)? {
            report.skipped += 1;
            continue;
        }

        let text = String::from_utf8_lossy(&raw);
        let (system, user) = parse_path_backup_text(&text);
        let snap = NewSnapshot {
            kind: SnapshotKind::Backup,
            source: EventSource::Migration,
            system,
            user,
            env: Vec::new(),
            parent_uid: None,
        };
        db.with_transaction(|tx| {
            insert_snapshot_tx(tx, snap)?;
            record_import(tx, &path, &hash)
        })?;
        report.imported += 1;
    }
    Ok(())
}

/// 解析 `path_backup_*.txt` 的两段内容。
fn parse_path_backup_text(text: &str) -> (Vec<PathEntry>, Vec<PathEntry>) {
    let mut system = Vec::new();
    let mut user = Vec::new();
    let mut section = "";
    for line in text.lines() {
        let trimmed = line.trim();
        match trimmed {
            "[System PATH]" => {
                section = "system";
                continue;
            }
            "[User PATH]" => {
                section = "user";
                continue;
            }
            _ => {}
        }
        if trimmed.is_empty() || trimmed.starts_with("PathEditor Backup") {
            continue;
        }
        let entry = PathEntry {
            path: trimmed.to_string(),
            enabled: true,
        };
        match section {
            "system" => system.push(entry),
            "user" => user.push(entry),
            _ => {}
        }
    }
    (system, user)
}

/// 该文件是否已按此内容导入过（幂等判据，spec B3）。
fn already_imported(db: &Db, path: &Path, hash: &str) -> Result<bool, CoreError> {
    let key = path.to_string_lossy().to_string();
    let n: i64 = db
        .query_row_with(
            "SELECT count(*) FROM migration_imports WHERE source_path = ?1 AND content_hash = ?2",
            rusqlite::params![key, hash],
            |r| r.get(0),
        )
        .map_err(|e| CoreError::new(e.code, "already_imported", e.message))?;
    Ok(n > 0)
}

/// 登记一条导入记录（必须在同一事务内）。
fn record_import(tx: &rusqlite::Transaction, path: &Path, hash: &str) -> Result<(), CoreError> {
    tx.execute(
        "INSERT OR IGNORE INTO migration_imports (source_path, content_hash, imported_at)
         VALUES (?1, ?2, ?3)",
        rusqlite::params![
            path.to_string_lossy(),
            hash,
            chrono::Utc::now().timestamp_millis()
        ],
    )
    .map_err(|e| db_err("record_import", e))?;
    Ok(())
}

/// `disabled.json` 的 v5 结构（与 `disabled::DisabledState` 同形，
/// 但那个类型是私有的，这里独立定义以免为迁移把私有类型提为 `pub`）。
#[derive(Debug, Deserialize, Default)]
struct LegacyDisabledState {
    #[serde(default, rename = "systemSnapshot")]
    system_snapshot: Vec<PathEntry>,
    #[serde(default, rename = "userSnapshot")]
    user_snapshot: Vec<PathEntry>,
}
