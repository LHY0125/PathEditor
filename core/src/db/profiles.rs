//! 配置方案（profile）元数据与版本，全部存于 DB。
//!
//! 承担标签/版本/最近使用/应用次数的查询能力。

use super::history::{insert_snapshot_tx, NewSnapshot};
use super::{db_err, Db};
use crate::error::{CoreError, ErrorCode};
use crate::PathEntry;
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
/// 嵌套会因 SQLite 不支持嵌套事务而失败。
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

    // ── 接线 ──
    //
    // 两条变更：配置元数据（整实体）+ 它的首个版本。
    //
    // **必须放在所有可能失败的检查之后**：上面 `NameExists` 分支会提前 `return`，
    // 若把记录放在它之前，就会留下「profile 没建成但变更记了」的孤儿行。
    // 整个函数在同一事务内，任一步失败都会整体回滚——但代码顺序仍应表达
    // 「先成功、再记账」的意图。
    let meta = serde_json::json!({
        "name": name,
        "description": description,
        "tags": tags,
        "snapshotUid": snapshot_uid,
    });
    crate::db::sync::change_log::record_change_tx(
        tx,
        crate::db::sync::change_log::NewChange {
            entity_type: "profile".to_string(),
            entity_uid: uid.clone(),
            field: None,
            old_value: String::new(),
            new_value: meta.to_string(),
        },
    )?;
    crate::db::sync::change_log::record_change_tx(
        tx,
        crate::db::sync::change_log::NewChange {
            entity_type: "profile_revision".to_string(),
            entity_uid: snapshot_uid.clone(),
            field: None,
            old_value: String::new(),
            new_value: serde_json::json!({ "profileUid": uid, "version": 1 }).to_string(),
        },
    )?;

    Ok(uid)
}

/// 同步包里的配置内容（接受侧子集）。
///
/// **不含** `applyCount` / `lastAppliedAt`：使用统计是本机数据，不跨设备同步
/// 。导出方送来的 `ProfileView` 会带这两个字段，
/// serde 默认忽略未知字段，所以可以直接反序列化。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileSyncContent {
    /// 配置名称
    pub name: String,
    /// 描述
    pub description: Option<String>,
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 创建时间（Unix 毫秒）
    pub created_at: i64,
    /// 更新时间（Unix 毫秒）
    pub updated_at: i64,
    /// CAS 版本号，同时是 revision 的版本号
    pub version: i64,
    /// 该版本指向的快照 uid
    pub snapshot_uid: Option<String>,
}

/// 按**远端 uid** 落库一条配置（同步导入用）。
///
/// 保留远端 uid：bundle 内所有引用都指向它。`profile_revisions.uid` 由本地生成
/// （bundle 不引用 revision uid）；`(profile_id, version)` 用 upsert 更新 `snapshot_id`。
///
/// 已存在同 uid 的行时更新元数据并**复活**（`deleted_at = NULL`）——是否符合预期
/// 由调用方的 LWW 决定，本函数只负责按内容写入。
///
/// 悬空 `snapshotUid` 返回 `SyncFormat`，由调用方整体回滚。
///
/// 返回值：`true` 表示因为本地已有同名配置而**自动改名**了。
pub(crate) fn import_profile_content_tx(
    tx: &Transaction,
    uid: &str,
    content: &serde_json::Value,
    device_id: &str,
) -> Result<bool, CoreError> {
    let c: ProfileSyncContent = serde_json::from_value(content.clone()).map_err(|e| {
        CoreError::new(
            ErrorCode::SyncFormat,
            "import_profile",
            format!("配置内容无法解析: {e}"),
        )
    })?;

    // **外部输入跨信任边界**：包里的名称先过与本地保存同一套校验。
    //
    // 不做这一步的后果：空名 / 超长名 / 含控制字符的名称会写进 `profiles.name`
    // 并被 UI 渲染；与本地重名时空名还会变成 `" (来自 dev-x)"`。
    //
    // 错误码映射：`validate_name` 给的是 `InvalidName`（「用户输入错」的语义），
    // 而同步包里的非法名称属于「包格式错」，这里统一成 `SyncFormat` 整包回滚。
    validate_name(&c.name).map_err(|e| {
        CoreError::new(
            ErrorCode::SyncFormat,
            "import_profile",
            format!("配置名称不合法: {}", e.message),
        )
    })?;

    // 同名不同 uid：给本地行生成一个不冲突的名称。
    // 只影响本地行，包内 uid 保持不变。
    let name = unique_name_tx(tx, &c.name, uid, device_id)?;
    let renamed = name != c.name;

    let snapshot_id: Option<i64> = match &c.snapshot_uid {
        Some(s) => {
            let id: Option<i64> = tx
                .query_row("SELECT id FROM snapshots WHERE uid = ?1", params![s], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(|e| db_err("import_profile_snapshot", e))?;
            Some(id.ok_or_else(|| {
                CoreError::new(
                    ErrorCode::SyncFormat,
                    "import_profile",
                    format!("配置引用的快照不存在: {s}"),
                )
            })?)
        }
        None => None,
    };

    let existing: Option<i64> = tx
        .query_row(
            "SELECT id FROM profiles WHERE uid = ?1",
            params![uid],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| db_err("find_profile_uid", e))?;

    let tags_json = serde_json::to_string(&c.tags).unwrap_or_else(|_| "[]".to_string());
    let version = c.version.max(1);

    match existing {
        Some(id) => {
            tx.execute(
                "UPDATE profiles
                    SET name = ?1, description = ?2, tags = ?3, created_at = ?4,
                        updated_at = ?5, version = ?6, deleted_at = NULL
                  WHERE id = ?7",
                params![
                    name,
                    c.description,
                    tags_json,
                    c.created_at,
                    c.updated_at,
                    version,
                    id
                ],
            )
            .map_err(|e| db_err("import_profile_update", e))?;
            if let Some(sid) = snapshot_id {
                upsert_revision_tx(tx, id, version, c.updated_at, sid)?;
            }
        }
        None => {
            tx.execute(
                "INSERT INTO profiles
                   (uid, name, description, tags, created_at, updated_at, apply_count, version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
                params![
                    uid,
                    name,
                    c.description,
                    tags_json,
                    c.created_at,
                    c.updated_at,
                    version
                ],
            )
            .map_err(|e| db_err("import_profile_insert", e))?;
            let id = tx.last_insert_rowid();
            if let Some(sid) = snapshot_id {
                upsert_revision_tx(tx, id, version, c.updated_at, sid)?;
            }
        }
    }
    Ok(renamed)
}

/// 为导入的配置生成一个本机唯一的未删除名称。
///
/// 冲突时改为 `"{desired} (来自 {device_id 前 8 位})"`；仍冲突则追加 ` 2`、` 3`…
/// 只影响本地行，不改变包内 uid。
///
/// **按字符取设备 id 前缀**：`device_id` 来自外部包，可能是多字节字符，
/// `&device_id[..8]` 会在字符边界 panic。
fn unique_name_tx(
    tx: &Transaction,
    desired: &str,
    self_uid: &str,
    device_id: &str,
) -> Result<String, CoreError> {
    let taken = |name: &str| -> Result<bool, CoreError> {
        let n: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM profiles
                  WHERE name = ?1 AND deleted_at IS NULL AND uid <> ?2",
                params![name, self_uid],
                |r| r.get(0),
            )
            .map_err(|e| db_err("name_taken", e))?;
        Ok(n > 0)
    };

    if !taken(desired)? {
        return Ok(desired.to_string());
    }

    // 名称上限 255 字节（`validate_name`）；给后缀与「2」这类追加留出空间。
    let base_name = truncate_name(desired, 200);
    let suffix: String = device_id.chars().take(8).collect();
    let base = format!("{base_name} (来自 {suffix})");
    if !taken(&base)? {
        return Ok(base);
    }
    for i in 2..100 {
        let candidate = format!("{base} {i}");
        if !taken(&candidate)? {
            return Ok(candidate);
        }
    }
    Err(CoreError::new(
        ErrorCode::SyncFormat,
        "import_profile",
        "同名配置过多，无法生成唯一名称",
    ))
}

/// 按字节上限截断名称（**不切断 UTF-8 字符**）。
fn truncate_name(name: &str, max_bytes: usize) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if out.len() + ch.len_utf8() > max_bytes {
            break;
        }
        out.push(ch);
    }
    out
}

/// 插入/更新一条 revision（`(profile_id, version)` 唯一）。
fn upsert_revision_tx(
    tx: &Transaction,
    profile_id: i64,
    version: i64,
    created_at: i64,
    snapshot_id: i64,
) -> Result<(), CoreError> {
    tx.execute(
        "INSERT INTO profile_revisions (uid, profile_id, version, created_at, snapshot_id)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(profile_id, version) DO UPDATE SET snapshot_id = excluded.snapshot_id",
        params![
            Uuid::now_v7().to_string(),
            profile_id,
            version,
            created_at,
            snapshot_id
        ],
    )
    .map_err(|e| db_err("upsert_revision", e))?;
    Ok(())
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

/// 软删除配置：写墓碑，允许同名重建。
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
        if changed == 1 {
            // 墓碑也是一条变更（`field = "deleted_at"`）：远端据此知道该软删除。
            crate::db::sync::change_log::record_change_tx(
                tx,
                crate::db::sync::change_log::NewChange {
                    entity_type: "profile".to_string(),
                    entity_uid: uid.to_string(),
                    field: Some("deleted_at".to_string()),
                    old_value: "null".to_string(),
                    new_value: now.to_string(),
                },
            )?;
        }
        Ok(changed == 1)
    })
}

/// 记录一次「配置被应用」：`apply_count + 1`、更新 `last_applied_at`。
///
/// **不写 `events`** —— 事件由 `HistorySink` 统一负责。
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

/// 按**名称**读取未删除的配置（UI 用名称而非 uid 作标识）。
pub fn get_profile_by_name(db: &Db, name: &str) -> Result<Option<ProfileView>, CoreError> {
    Ok(list_profiles(db)?.into_iter().find(|p| p.name == name))
}

/// 按 uid 软删除（UI 用名称，故另提供 [`delete_profile_by_name`]）。
pub fn delete_profile_by_name(db: &mut Db, name: &str) -> Result<bool, CoreError> {
    let Some(p) = get_profile_by_name(db, name)? else {
        return Ok(false);
    };
    delete_profile(db, &p.uid)
}

/// 按名称记录一次「配置被应用」。
pub fn mark_profile_applied_by_name(db: &mut Db, name: &str) -> Result<bool, CoreError> {
    let Some(p) = get_profile_by_name(db, name)? else {
        return Ok(false);
    };
    mark_profile_applied(db, &p.uid)
}

/// 重命名配置。
///
/// 同名（未删除）已存在时返回 `NameExists` —— 与 `save_profile` 同一口径，
/// 不静默覆盖。改名**不产生新 revision**（内容没变，只是标签变了）。
pub fn rename_profile(db: &mut Db, old_name: &str, new_name: &str) -> Result<bool, CoreError> {
    validate_name(new_name)?;
    if old_name == new_name {
        return Ok(false);
    }
    let now = chrono::Utc::now().timestamp_millis();
    db.with_transaction(|tx| {
        let taken: Option<i64> = tx
            .query_row(
                "SELECT id FROM profiles WHERE name = ?1 AND deleted_at IS NULL",
                params![new_name],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| db_err("rename_profile", e))?;
        if taken.is_some() {
            return Err(CoreError::new(
                ErrorCode::NameExists,
                "rename_profile",
                format!("配置「{new_name}」已存在"),
            ));
        }
        let changed = tx
            .execute(
                "UPDATE profiles SET name = ?1, updated_at = ?2, version = version + 1
                 WHERE name = ?3 AND deleted_at IS NULL",
                params![new_name, now, old_name],
            )
            .map_err(|e| db_err("rename_profile", e))?;
        if changed == 1 {
            // 改名按 `name` 字段记，实体 uid 取自改名后的行——
            // bundle 里引用一律用 uid，这里必须还原成本地 uid 才能被远端对上。
            let uid: String = tx
                .query_row(
                    "SELECT uid FROM profiles WHERE name = ?1 AND deleted_at IS NULL",
                    params![new_name],
                    |r| r.get(0),
                )
                .map_err(|e| db_err("rename_profile_uid", e))?;
            crate::db::sync::change_log::record_change_tx(
                tx,
                crate::db::sync::change_log::NewChange {
                    entity_type: "profile".to_string(),
                    entity_uid: uid,
                    field: Some("name".to_string()),
                    old_value: serde_json::Value::String(old_name.to_string()).to_string(),
                    new_value: serde_json::Value::String(new_name.to_string()).to_string(),
                },
            )?;
        }
        Ok(changed == 1)
    })
}

/// 配置的 PATH 条目：`(system, user)`。
///
/// 提取成别名是为了避开 clippy 的 `type_complexity`（嵌套 `Vec` 的元组）。
pub type ProfilePaths = (Vec<PathEntry>, Vec<PathEntry>);

/// 读取配置最新版本对应的快照 PATH 条目（供 UI 预览与应用）。
///
/// 返回 `(system, user)`；配置不存在或没有 revision 时返回 `None`。
pub fn profile_paths(db: &Db, name: &str) -> Result<Option<ProfilePaths>, CoreError> {
    let Some(p) = get_profile_by_name(db, name)? else {
        return Ok(None);
    };
    let Some(snapshot_uid) = p.snapshot_uid else {
        return Ok(None);
    };
    let view = crate::db::history::get_snapshot(db, &snapshot_uid, false)?;
    Ok(Some((view.system, view.user)))
}
