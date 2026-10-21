//! 维护任务（spec §4.2 第 10 条、§5.3、§6.5.2）。
//!
//! 两类职责：
//!
//! 1. **保留策略**：`changes` 中「已导出且超过 90 天」的变更、`sync_conflicts` 中
//!    「已解决且超过 30 天」的记录会被删除。待导出队列（`exported_at IS NULL`）
//!    永远不删——那里面是会永久丢失的变更。
//! 2. **遗留明文备份清理**：把 v5 时代的 `env_backup_*.json` 转成 DPAPI 加密的
//!    `.pebak`，**回读校验通过后**才删除明文。

use std::path::{Path, PathBuf};

use super::{db_err, Db};
use crate::backup::EnvBackupPayload;
use crate::db::crypto::KeyMaterial;
use crate::error::CoreError;

/// 已导出 `changes` 的保留期：90 天。
pub const CHANGES_RETENTION_MS: i64 = 90 * 24 * 60 * 60 * 1000;
/// 已解决 `sync_conflicts` 的保留期：30 天。
pub const CONFLICTS_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// 清理结果。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PruneReport {
    /// 删除的 changes 行数
    pub changes_deleted: u32,
    /// 删除的 sync_conflicts 行数
    pub conflicts_deleted: u32,
}

/// `changes` 的删除谓词。
///
/// `applied_at > 0` 排除「从未落地」的行；`exported_at IS NOT NULL` 排除待导出队列。
/// 任务本身产生的本地变更在 `record_change_tx` 里就把 `applied_at` 写成当时时间，
/// 因此本地导出过的变更同样会到期清理。
const CHANGES_PREDICATE: &str = "exported_at IS NOT NULL AND applied_at > 0 AND applied_at < ?1";

/// `sync_conflicts` 的删除谓词：只删**已解决**且到期的。
const CONFLICTS_PREDICATE: &str = "resolved_at IS NOT NULL AND resolved_at < ?1";

/// 执行清理，返回删除行数。
///
/// **绝不删除 `exported_at IS NULL`**：那是待导出队列，删了就永久丢变更。
pub fn prune(db: &mut Db, now_ms: i64) -> Result<PruneReport, CoreError> {
    db.with_transaction(|tx| {
        let changes_deleted = tx
            .execute(
                &format!("DELETE FROM changes WHERE {CHANGES_PREDICATE}"),
                rusqlite::params![now_ms - CHANGES_RETENTION_MS],
            )
            .map_err(|e| db_err("prune_changes", e))?;
        let conflicts_deleted = tx
            .execute(
                &format!("DELETE FROM sync_conflicts WHERE {CONFLICTS_PREDICATE}"),
                rusqlite::params![now_ms - CONFLICTS_RETENTION_MS],
            )
            .map_err(|e| db_err("prune_conflicts", e))?;
        Ok(PruneReport {
            changes_deleted: changes_deleted as u32,
            conflicts_deleted: conflicts_deleted as u32,
        })
    })
}

/// dry-run：只统计将被删除的行数，**不写库**。
pub fn prune_plan(db: &Db, now_ms: i64) -> Result<PruneReport, CoreError> {
    let changes_deleted: i64 = db.query_row_with(
        &format!("SELECT COUNT(*) FROM changes WHERE {CHANGES_PREDICATE}"),
        rusqlite::params![now_ms - CHANGES_RETENTION_MS],
        |r| r.get(0),
    )?;
    let conflicts_deleted: i64 = db.query_row_with(
        &format!("SELECT COUNT(*) FROM sync_conflicts WHERE {CONFLICTS_PREDICATE}"),
        rusqlite::params![now_ms - CONFLICTS_RETENTION_MS],
        |r| r.get(0),
    )?;
    Ok(PruneReport {
        changes_deleted: changes_deleted as u32,
        conflicts_deleted: conflicts_deleted as u32,
    })
}

/// 以当前时间执行清理（GUI 启动序列用）。
///
/// 时间源收在 core：GUI crate 没有直接依赖 `chrono`，也不该为一次清理新增依赖。
pub fn prune_now(db: &mut Db) -> Result<PruneReport, CoreError> {
    prune(db, chrono::Utc::now().timestamp_millis())
}

/// 遗留明文备份清理结果。
#[derive(Debug, Default)]
pub struct CleanupReport {
    /// 已转换（dry-run 时是「将转换」）的 `(json, pebak)` 对；dry-run 时 pebak 为空路径
    pub converted: Vec<(PathBuf, PathBuf)>,
    /// 跳过项与原因
    pub skipped: Vec<(PathBuf, String)>,
    /// 是否为 dry-run
    pub dry_run: bool,
}

/// 把遗留 `env_backup_*.json` 转成 `.pebak` 后删除明文。
///
/// **顺序不可颠倒**：先写 `.pebak` 并回读校验，成功后才删除 `.json`。
/// 任一失败都保留原文件——这是 spec §5.3 / §十二 的「转换成功才删除」。
///
/// 解析**不使用** `backup::read_env_backup`：后者在解析失败时会经
/// `persist::parse_error_quarantined` 把文件重命名为 `.corrupt-<ts>`，
/// 清理命令不能在 dry-run 时改动用户文件。这里直接反序列化 + [`crate::persist::migrate`]。
///
/// `dry_run = true` 时不使用 `material`；只想预览的调用方应改用
/// [`scan_legacy_env_backups`]，免得为一次 dry-run 去创建密钥文件。
pub fn cleanup_legacy_env_backups(
    dir: &Path,
    material: &KeyMaterial,
    dry_run: bool,
) -> Result<CleanupReport, CoreError> {
    cleanup_legacy_impl(dir, if dry_run { None } else { Some(material) })
}

/// dry-run：只解析并列出将被转换的文件，**不写、不删、不需要密钥**。
///
/// 与 [`cleanup_legacy_env_backups`] 共用同一份解析与筛选逻辑，区别只在
/// `material = None`——这保证「预览」与「执行」看的是同一批文件。
pub fn scan_legacy_env_backups(dir: &Path) -> Result<CleanupReport, CoreError> {
    cleanup_legacy_impl(dir, None)
}

/// 转换实现。`material` 为 `None` 即 dry-run。
///
/// **类型上就不可能「忘了传密钥却真的去写」**：写路径只在 `Some` 分支里。
fn cleanup_legacy_impl(
    dir: &Path,
    material: Option<&KeyMaterial>,
) -> Result<CleanupReport, CoreError> {
    let dry_run = material.is_none();
    let mut report = CleanupReport {
        dry_run,
        ..Default::default()
    };
    if !dir.is_dir() {
        return Ok(report);
    }

    let entries = std::fs::read_dir(dir).map_err(|e| {
        CoreError::new(
            crate::error::ErrorCode::Io,
            "cleanup_legacy",
            format!("读取 {} 失败: {e}", dir.display()),
        )
    })?;

    for entry in entries {
        let path = entry
            .map_err(|e| {
                CoreError::new(
                    crate::error::ErrorCode::Io,
                    "cleanup_legacy",
                    format!("读取目录项失败: {e}"),
                )
            })?
            .path();
        if !is_legacy_env_backup(&path) {
            continue;
        }

        let payload = match read_legacy_backup_no_quarantine(&path) {
            Ok(p) => p,
            Err(e) => {
                report
                    .skipped
                    .push((path, format!("无法解析: {}", e.message)));
                continue;
            }
        };

        let Some(material) = material else {
            // dry-run：只报告「会转换哪些」，pebak 路径未知（写入时刻才定名）
            report.converted.push((path, PathBuf::new()));
            continue;
        };

        let pebak = match crate::db::backups::write_env_backup_pebak(dir, &payload, material) {
            Ok(p) => p,
            Err(e) => {
                report
                    .skipped
                    .push((path, format!("写加密备份失败: {}", e.message)));
                continue;
            }
        };
        // **回读校验**：写成功不等于写对了（DPAPI 作用域、磁盘满都可能让文件不可读）。
        if let Err(e) = crate::db::backups::read_env_backup_any(&pebak, material) {
            report
                .skipped
                .push((path, format!("加密备份回读失败，已保留明文: {}", e.message)));
            continue;
        }
        if let Err(e) = std::fs::remove_file(&path) {
            report.skipped.push((path, format!("删除明文失败: {e}")));
            continue;
        }
        report.converted.push((path, pebak));
    }
    Ok(report)
}

/// 文件名是否形如本工具生成的 `env_backup_YYYYMMDD_HHMMSS_mmm.json`。
///
/// **不能只匹配 `env_backup_*.json`**：那个前缀可能命中用户自己放的文件
/// （例如 `env_backup_x.json`），清理命令不得碰它们——口径与 `backup.rs` 的轮换一致。
fn is_legacy_env_backup(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Some(stem) = name
        .strip_prefix("env_backup_")
        .and_then(|s| s.strip_suffix(".json"))
    else {
        return false;
    };
    let parts: Vec<&str> = stem.split('_').collect();
    parts.len() == 3
        && parts[0].len() == 8
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].len() == 6
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[2].len() == 3
        && parts[2].chars().all(|c| c.is_ascii_digit())
}

/// 不做损坏隔离的遗留备份解析。
///
/// 与 `backup::read_env_backup` 同口径（`Versioned` 信封 + `persist::migrate`），
/// 唯一区别是解析失败时**只返回错误、不动文件**。
fn read_legacy_backup_no_quarantine(path: &Path) -> Result<EnvBackupPayload, CoreError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        CoreError::new(
            crate::error::ErrorCode::Io,
            "cleanup_legacy",
            format!("读取 {} 失败: {e}", path.display()),
        )
    })?;
    let versioned: crate::persist::Versioned<EnvBackupPayload> = serde_json::from_str(&content)
        .map_err(|e| {
            CoreError::new(
                crate::error::ErrorCode::Parse,
                "cleanup_legacy",
                format!("解析 {} 失败: {e}", path.display()),
            )
        })?;
    crate::persist::migrate(versioned, "env 备份")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::{EnvBackupHives, EnvBackupVar};
    use crate::db::crypto::load_or_create_key;
    use crate::env_var::EnvValueKind;
    use crate::persist::{Versioned, PERSIST_SCHEMA_VERSION};

    /// 此文件只有一个测试：它用 DPAPI、不建 SQLCipher，因此可以放在单元测试模块里。
    #[test]
    fn legacy_json_is_converted_to_pebak_before_deletion() {
        let dir = tempfile::TempDir::new().unwrap();
        let material = load_or_create_key(dir.path()).unwrap();

        let payload = EnvBackupPayload {
            captured_at: 1_700_000_000_000,
            hives: EnvBackupHives {
                system: vec![EnvBackupVar {
                    name: "TOKEN".to_string(),
                    kind: EnvValueKind::String,
                    value: "secret".to_string(),
                    revision: "0123456789abcdef".to_string(),
                }],
                user: vec![],
            },
        };
        let legacy = dir.path().join("env_backup_20260101_000000_000.json");
        std::fs::write(
            &legacy,
            serde_json::to_string(&Versioned {
                schema_version: PERSIST_SCHEMA_VERSION,
                inner: payload.clone(),
            })
            .unwrap(),
        )
        .unwrap();

        // dry-run 不得写文件、不得删除
        let plan = cleanup_legacy_env_backups(dir.path(), &material, true).unwrap();
        assert_eq!(plan.converted.len(), 1);
        assert!(legacy.exists(), "dry-run 不得删除");

        // CLI 的预览走 `scan_legacy_env_backups`（不加载密钥）：两者必须看同一批文件
        let scanned = scan_legacy_env_backups(dir.path()).unwrap();
        assert!(scanned.dry_run);
        assert_eq!(
            scanned.converted.len(),
            plan.converted.len(),
            "预览与 dry-run 必须给出同一份计划"
        );

        let report = cleanup_legacy_env_backups(dir.path(), &material, false).unwrap();
        assert_eq!(report.converted.len(), 1);
        assert_eq!(report.skipped.len(), 0, "转换不应产生跳过项");
        assert!(!legacy.exists(), "转换成功后删除明文");

        let (_, pebak) = &report.converted[0];
        let read_back = crate::db::backups::read_env_backup_any(pebak, &material).unwrap();
        assert_eq!(read_back, payload, ".pebak 必须可回读");
    }
}
