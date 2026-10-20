//! 配置方案（profile）元数据与版本，迁入 DB。
//!
//! 与 `core::profiles`（v5 文件版）并存：文件版保留为「导出 v5 JSON」的兼容通路，
//! DB 版承担标签/版本/最近使用/应用次数的查询能力（spec D8）。

use super::history::{insert_snapshot_tx, NewSnapshot};
use super::{db_err, Db};
use crate::error::{CoreError, ErrorCode};
use rusqlite::{params, OptionalExtension, Transaction};
use uuid::Uuid;

/// 配置方案的对外视图。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    /// 配置 uid
    pub uid: String,
    /// 名称
    pub name: String,
    /// 描述
    pub description: Option<String>,
    /// 标签
    pub tags: Vec<String>,
    /// 创建时间（Unix 毫秒）
    pub created_at: i64,
    /// 最后更新时间（Unix 毫秒）
    pub updated_at: i64,
    /// 最后应用时间（Unix 毫秒）
    pub last_applied_at: Option<i64>,
    /// 应用次数
    pub apply_count: i64,
    /// CAS 版本号
    pub version: i64,
    /// 该配置最新版本指向的快照 uid
    pub snapshot_uid: Option<String>,
}

/// 校验配置名。与文件版 `profiles::validate_profile_name` 保持同一套规则，
/// 但**不引入横线/斜杠** —— DB 版不受文件系统命名限制，只挡控制字符与空名。
fn validate_name(name: &str) -> Result<(), CoreError> {
    if name.trim().is_empty() {
        return Err(CoreError::new(
            ErrorCode::InvalidName,
            "save_profile",
            "配置名称不能为空",
        ));
    }
    if name.len() > 255 {
        return Err(CoreError::new(
            ErrorCode::InvalidName,
            "save_profile",
            "配置名称过长（最大 255 字符）",
        ));
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(CoreError::new(
            ErrorCode::InvalidName,
            "save_profile",
            "配置名称包含非法字符",
        ));
    }
    Ok(())
}

/// 保存配置方案：快照 + profile 行 + revision 行，**单事务**。
///
/// 快照用 [`insert_snapshot_tx`] 而非 `insert_snapshot` —— 后者自带事务，
/// 嵌套会因 SQLite 不支持嵌套事务而失败（计划核对 H6）。
///
/// 同名且未删除时**不覆盖**，返回 `NameExists`；调用方应改用更新语义。
pub fn save_profile(
    db: &mut Db,
    name: &str,
    description: Option<&str>,
    tags: &[String],
    snap: NewSnapshot,
) -> Result<String, CoreError> {
    validate_name(name)?;
    let now = chrono::Utc::now().timestamp_millis();
    db.with_transaction(|tx| save_profile_tx(tx, name, description, tags, snap, now))
}

/// 事务内保存配置（供「配置 + 其它业务行同一事务」的调用方复用）。
pub fn save_profile_tx(
    tx: &Transaction,
    name: &str,
    description: Option<&str>,
    tags: &[String],
    snap: NewSnapshot,
    now: i64,
) -> Result<String, CoreError> {
    let snapshot_uid = insert_snapshot_tx(tx, snap)?;
    let snapshot_id = snapshot_id_by_uid(tx, &snapshot_uid)?;

    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
    let uid = Uuid::now_v7().to_string();

    // 部分唯一索引 uq_profiles_name_active 会拒绝未删除的同名行，
    // 这里先用可读错误拦一层，避免把 SQLite 的约束错误直接抛给用户。
    let exists: Option<i64> = tx
        .query_row(
            "SELECT id FROM profiles WHERE name = ?1 AND deleted_at IS NULL",
            params![name],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| db_err("save_profile", e))?;
    if exists.is_some() {
        return Err(CoreError::new(
            ErrorCode::NameExists,
            "save_profile",
            format!("配置「{name}」已存在"),
        ));
    }

    tx.execute(
        "INSERT INTO profiles
           (uid, name, description, tags, created_at, updated_at, apply_count, version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5, 0, 1)",
        params![uid, name, description, tags_json, now],
    )
    .map_err(|e| db_err("insert_profile", e))?;
    let profile_id = tx.last_insert_rowid();

    tx.execute(
        "INSERT INTO profile_revisions (uid, profile_id, version, created_at, snapshot_id)
         VALUES (?1, ?2, 1, ?3, ?4)",
        params![Uuid::now_v7().to_string(), profile_id, now, snapshot_id],
    )
    .map_err(|e| db_err("insert_revision", e))?;

    Ok(uid)
}

/// 列出未删除的配置，按 `updated_at DESC` 排序。
pub fn list_profiles(db: &Db) -> Result<Vec<ProfileView>, CoreError> {
    let mut stmt = db.prepare(
        "SELECT p.uid, p.name, p.description, p.tags, p.created_at, p.updated_at,
                p.last_applied_at, p.apply_count, p.version,
                (SELECT s.uid FROM profile_revisions r
                   JOIN snapshots s ON s.id = r.snapshot_id
                  WHERE r.profile_id = p.id
                  ORDER BY r.version DESC LIMIT 1)
         FROM profiles p
         WHERE p.deleted_at IS NULL
         ORDER BY p.updated_at DESC, p.id DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let tags_json: Option<String> = r.get(3)?;
            Ok(ProfileView {
                uid: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                tags: tags_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
                    .unwrap_or_default(),
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
                last_applied_at: r.get(6)?,
                apply_count: r.get(7)?,
                version: r.get(8)?,
                snapshot_uid: r.get(9)?,
            })
        })
        .map_err(|e| db_err("list_profiles", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("list_profiles", e))?;
    Ok(rows)
}

/// 按 uid 读取单个配置；不存在或已删除返回 `None`。
pub fn get_profile(db: &Db, uid: &str) -> Result<Option<ProfileView>, CoreError> {
    Ok(list_profiles(db)?.into_iter().find(|p| p.uid == uid))
}

/// 软删除配置：写墓碑，允许同名重建（spec B5 的部分唯一索引）。
///
/// 幂等：已删除时返回 `Ok(false)`，不报错。
pub fn delete_profile(db: &mut Db, uid: &str) -> Result<bool, CoreError> {
    let now = chrono::Utc::now().timestamp_millis();
    db.with_transaction(|tx| {
        let changed = tx
            .execute(
                "UPDATE profiles SET deleted_at = ?1, updated_at = ?1, version = version + 1
                 WHERE uid = ?2 AND deleted_at IS NULL",
                params![now, uid],
            )
            .map_err(|e| db_err("delete_profile", e))?;
        Ok(changed == 1)
    })
}

/// 记录一次「配置被应用」：`apply_count + 1`、更新 `last_applied_at`。
///
/// **不写 `events`** —— 事件由 `HistorySink` 统一负责（与 Task 4 的 B2 口径一致）。
pub fn mark_profile_applied(db: &mut Db, uid: &str) -> Result<bool, CoreError> {
    let now = chrono::Utc::now().timestamp_millis();
    db.with_transaction(|tx| {
        let changed = tx
            .execute(
                "UPDATE profiles
                 SET apply_count = apply_count + 1, last_applied_at = ?1,
                     updated_at = ?1, version = version + 1
                 WHERE uid = ?2 AND deleted_at IS NULL",
                params![now, uid],
            )
            .map_err(|e| db_err("mark_profile_applied", e))?;
        Ok(changed == 1)
    })
}

/// 用 uid 查本地整数主键。
fn snapshot_id_by_uid(tx: &Transaction, uid: &str) -> Result<i64, CoreError> {
    tx.query_row(
        "SELECT id FROM snapshots WHERE uid = ?1",
        params![uid],
        |r| r.get(0),
    )
    .map_err(|e| db_err("find_snapshot", e))
}
