//! 快照与事件读写。**`events` 不得含 env 明文值（spec D6）。**
//!
//! 事务纪律：
//! - 自带事务的入口（[`insert_snapshot`]）供顶层调用；
//! - `*_tx` 变体供已在事务内的调用方复用（SQLite 不支持嵌套事务）。

use super::{db_err, Db};
use crate::error::{CoreError, ErrorCode};
use crate::history::{EventAction, EventResult, EventSource};
use crate::PathEntry;
use rusqlite::{params, OptionalExtension, Transaction};
use uuid::Uuid;

/// 快照类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotKind {
    /// 当前快照
    Current,
    /// 配置方案
    Profile,
    /// 备份
    Backup,
    /// 历史记录
    History,
}

impl SnapshotKind {
    /// 落库用的字符串表示。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Profile => "profile",
            Self::Backup => "backup",
            Self::History => "history",
        }
    }

    /// 从库内字符串还原；未知值按 `History` 处理（前向兼容）。
    fn from_str(s: &str) -> Self {
        match s {
            "current" => Self::Current,
            "profile" => Self::Profile,
            "backup" => Self::Backup,
            _ => Self::History,
        }
    }
}

/// 事件来源的落库字符串。
pub(crate) fn source_str(s: EventSource) -> &'static str {
    match s {
        EventSource::Gui => "gui",
        EventSource::Cli => "cli",
        EventSource::Import => "import",
        EventSource::Restore => "restore",
        EventSource::Migration => "migration",
        EventSource::Sync => "sync",
    }
}

/// 事件动作的落库字符串。
pub(crate) fn action_str(a: EventAction) -> &'static str {
    match a {
        EventAction::Save => "save",
        EventAction::Apply => "apply",
        EventAction::Restore => "restore",
        EventAction::Import => "import",
        EventAction::EnvSet => "envSet",
        EventAction::EnvAdd => "envAdd",
        EventAction::EnvRemove => "envRemove",
        EventAction::Undo => "undo",
        EventAction::Redo => "redo",
    }
}

/// 事件动作的落库字符串还原。
pub(crate) fn action_from_str(s: &str) -> EventAction {
    match s {
        "apply" => EventAction::Apply,
        "restore" => EventAction::Restore,
        "import" => EventAction::Import,
        "envSet" => EventAction::EnvSet,
        "envAdd" => EventAction::EnvAdd,
        "envRemove" => EventAction::EnvRemove,
        "undo" => EventAction::Undo,
        "redo" => EventAction::Redo,
        _ => EventAction::Save,
    }
}

/// 事件结果的落库字符串。
pub(crate) fn result_str(r: EventResult) -> &'static str {
    match r {
        EventResult::Success => "success",
        EventResult::Partial => "partial",
        EventResult::Failure => "failure",
    }
}

/// 事件结果的字符串还原。
pub(crate) fn result_from_str(s: &str) -> EventResult {
    match s {
        "partial" => EventResult::Partial,
        "failure" => EventResult::Failure,
        _ => EventResult::Success,
    }
}

/// 快照中的一行环境变量。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvValueRow {
    /// `system` / `user`
    pub hive: String,
    /// 变量名
    pub name: String,
    /// 注册表类型（`String` / `ExpandString` / `Unsupported`）
    pub kind: String,
    /// 明文值；`Unsupported` 为 `None`
    pub value: Option<String>,
    /// 采集时的并发校验摘要
    pub revision: String,
    /// **调用方传入值会被忽略** —— 写入前由 `env_var::is_sensitive` 重新计算（H2）
    pub sensitive: bool,
    /// 是否可通过通用通路恢复
    pub restorable: bool,
}

/// 待写入的新快照。
#[derive(Debug, Clone)]
pub struct NewSnapshot {
    /// 快照类别
    pub kind: SnapshotKind,
    /// 来源
    pub source: EventSource,
    /// 系统 PATH 条目（有序）
    pub system: Vec<PathEntry>,
    /// 用户 PATH 条目（有序）
    pub user: Vec<PathEntry>,
    /// 环境变量
    pub env: Vec<EnvValueRow>,
    /// 父快照 uid
    pub parent_uid: Option<String>,
}

/// 快照的对外视图。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotView {
    /// 快照 uid
    pub uid: String,
    /// 创建时间（Unix 毫秒）
    pub created_at: i64,
    /// 快照类别
    pub kind: String,
    /// 系统 PATH
    pub system: Vec<PathEntry>,
    /// 用户 PATH
    pub user: Vec<PathEntry>,
    /// 环境变量（敏感值按 `reveal` 参数掩码）
    pub env: Vec<EnvValueRow>,
}

/// 事件列表筛选条件。默认 `limit = 50`、`offset = 0`（spec §3.2.1）。
#[derive(Debug, Clone)]
pub struct EventFilter {
    /// 本页条数（1–500）
    pub limit: u32,
    /// 偏移
    pub offset: u32,
    /// 按动作筛选
    pub action: Option<String>,
    /// 按来源筛选
    pub source: Option<String>,
    /// 按 hive 筛选（`system` / `user`）——P1-1
    pub hive: Option<String>,
    /// 按操作目标筛选（变量名等）——P1-1
    pub target: Option<String>,
    /// 按结果筛选（`success` / `partial` / `failure`）——P1-1
    pub result: Option<String>,
    /// 时间范围起点（含，Unix 毫秒）——P1-1
    pub from_ms: Option<i64>,
    /// 时间范围终点（含，Unix 毫秒）——P1-1
    pub to_ms: Option<i64>,
}

impl Default for EventFilter {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
            action: None,
            source: None,
            hive: None,
            target: None,
            result: None,
            from_ms: None,
            to_ms: None,
        }
    }
}

/// 单条事件的对外视图。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventView {
    /// 事件 uid
    pub uid: String,
    /// 发生时间（Unix 毫秒）
    pub created_at: i64,
    /// 来源
    pub source: String,
    /// 动作
    pub action: EventAction,
    /// 涉及 hive
    pub hive: Option<String>,
    /// 操作目标
    pub target: Option<String>,
    /// 结果
    pub result: EventResult,
    /// 本次操作产生的快照 uid（J-8）。
    ///
    /// `None` 表示该事件没有关联快照（例如纯查询类事件），或快照已被删除。
    /// 前端据此决定是否允许点击查看详情。
    pub snapshot_uid: Option<String>,
}

/// 一页事件。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPage {
    /// 本页事件
    pub items: Vec<EventView>,
    /// 满足条件的**总行数**（不是本页条数，H1）
    pub total: u32,
}

/// 插入快照 + 条目，返回快照 uid。
///
/// **不写 `events`** —— 事件统一由 `HistorySink` 负责，避免同一操作产生两条
/// 事件（计划核对 B2）。
pub fn insert_snapshot(db: &mut Db, snap: NewSnapshot) -> Result<String, CoreError> {
    db.with_transaction(|tx| insert_snapshot_tx(tx, snap))
}

/// 事务内插入快照；供配置保存等需要「快照 + 业务行同一事务」的调用方复用（H6）。
pub fn insert_snapshot_tx(tx: &Transaction, snap: NewSnapshot) -> Result<String, CoreError> {
    let uid = Uuid::now_v7().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let hash = content_hash(&snap);

    // 父快照 uid → 本地整数主键（bundle 里一律用 uid，库里用 id）
    let parent_id: Option<i64> = match &snap.parent_uid {
        Some(p) => tx
            .query_row("SELECT id FROM snapshots WHERE uid = ?1", params![p], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| db_err("find_parent", e))?,
        None => None,
    };

    tx.execute(
        "INSERT INTO snapshots
           (uid, created_at, source, kind, content_hash, parent_id, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?2)",
        params![
            uid,
            now,
            source_str(snap.source),
            snap.kind.as_str(),
            hash,
            parent_id
        ],
    )
    .map_err(|e| db_err("insert_snapshot", e))?;
    let id = tx.last_insert_rowid();

    for (hive, entries) in [("system", &snap.system), ("user", &snap.user)] {
        for (i, e) in entries.iter().enumerate() {
            tx.execute(
                "INSERT INTO path_entries (snapshot_id, hive, position, path, enabled)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, hive, i as i64, e.path, e.enabled as i64],
            )
            .map_err(|e| db_err("insert_path", e))?;
        }
    }

    for v in &snap.env {
        // D13：Path 永不进入 env_values（DDL 的 CHECK 也会拒绝，这里提前给出可读错误）
        if v.name.eq_ignore_ascii_case("path") {
            return Err(CoreError::new(
                ErrorCode::InvalidName,
                "insert_snapshot",
                "Path 走专用通路，不得写入 env_values",
            ));
        }
        // H2：sensitive 以 core 判定为准，忽略调用方传值。
        let sensitive = crate::env_var::is_sensitive(&v.name);
        // D13：Unsupported 只存元数据
        let value = if v.restorable {
            v.value.as_deref()
        } else {
            None
        };
        tx.execute(
            "INSERT INTO env_values
               (snapshot_id, hive, name, kind, value, revision, sensitive, restorable)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                v.hive,
                v.name,
                v.kind,
                value,
                v.revision,
                sensitive as i64,
                v.restorable as i64
            ],
        )
        .map_err(|e| db_err("insert_env", e))?;
    }

    Ok(uid)
}

/// 内容哈希：**必须区分 hive**（H3）。
///
/// `system=[A]` 与 `user=[A]` 在 PATH 语义上完全不同，不能哈希相同。
/// 每个字段之间插入互不相同的分隔字节，避免「拼接歧义」碰撞
/// （`"system"+"C:"` vs `"systemC:"`）。
fn content_hash(snap: &NewSnapshot) -> String {
    let mut h = blake3::Hasher::new();
    for (hive, entries) in [("system", &snap.system), ("user", &snap.user)] {
        h.update(hive.as_bytes());
        h.update(&[0xff]);
        // 条目数也参与哈希，避免 [a,b] 与 [a] + 空 hive 的边界歧义
        h.update(&(entries.len() as u64).to_le_bytes());
        for e in entries.iter() {
            h.update(e.path.as_bytes());
            h.update(&[e.enabled as u8, 0xfe]);
        }
    }
    // 环境变量按名字排序后哈希：同一集合的插入顺序不应改变快照身份
    let mut env: Vec<&EnvValueRow> = snap.env.iter().collect();
    env.sort_by(|a, b| (&a.hive, &a.name).cmp(&(&b.hive, &b.name)));
    for v in env {
        h.update(v.hive.as_bytes());
        h.update(&[0xfd]);
        h.update(v.name.as_bytes());
        h.update(&[0xfc]);
        if let Some(val) = &v.value {
            h.update(val.as_bytes());
        }
        h.update(&[0xfb]);
    }
    h.finalize().to_hex().to_string()
}

/// 分页查询事件，按 `created_at DESC, id DESC` 排序（服务端固定排序）。
///
/// `total` 是**独立的 count 查询**，不能等于本页条数（H1）。
pub fn list_events(db: &Db, filter: &EventFilter) -> Result<EventPage, CoreError> {
    let limit = filter.limit.clamp(1, 500) as i64;
    let offset = filter.offset as i64;

    // WHERE 条件在两处必须完全一致，否则 total 与实际可分页内容不符。
    // 必须带 `e.` 前缀：`snapshots` 也有 `source` 列，JOIN 后裸列名是歧义的。
    // P1-1：新增 hive / target / result / 时间范围四个维度（spec §6.1）。
    const WHERE: &str = "WHERE (?1 IS NULL OR e.action = ?1)
           AND (?2 IS NULL OR e.source = ?2)
           AND (?3 IS NULL OR e.hive = ?3)
           AND (?4 IS NULL OR e.target = ?4)
           AND (?5 IS NULL OR e.result = ?5)
           AND (?6 IS NULL OR e.created_at >= ?6)
           AND (?7 IS NULL OR e.created_at <= ?7)";

    // 参数顺序必须与 WHERE 里的 ?1..?7 一一对应
    let filter_params = params![
        filter.action,
        filter.source,
        filter.hive,
        filter.target,
        filter.result,
        filter.from_ms,
        filter.to_ms
    ];

    let total: i64 = db
        .query_row_with(
            &format!("SELECT count(*) FROM events e {WHERE}"),
            filter_params,
            |r| r.get(0),
        )
        .map_err(|e| CoreError::new(e.code, "list_events", e.message))?;

    let mut stmt = db.prepare(&format!(
        "SELECT e.uid, e.created_at, e.source, e.action, e.hive, e.target, e.result, s.uid
         FROM events e LEFT JOIN snapshots s ON s.id = e.snapshot_id {WHERE}
         ORDER BY e.created_at DESC, e.id DESC
         LIMIT ?8 OFFSET ?9"
    ))?;
    // 参数顺序：?1..?7 是 WHERE 的筛选条件，?8/?9 是分页
    let page_params = params![
        filter.action,
        filter.source,
        filter.hive,
        filter.target,
        filter.result,
        filter.from_ms,
        filter.to_ms,
        limit,
        offset
    ];
    let items = stmt
        .query_map(page_params, |r| {
            let action_s: String = r.get(3)?;
            let result_s: String = r.get(6)?;
            Ok(EventView {
                uid: r.get(0)?,
                created_at: r.get(1)?,
                source: r.get(2)?,
                action: action_from_str(&action_s),
                hive: r.get(4)?,
                target: r.get(5)?,
                result: result_from_str(&result_s),
                snapshot_uid: r.get(7)?,
            })
        })
        .map_err(|e| db_err("list_events", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("list_events", e))?;

    Ok(EventPage {
        items,
        total: total as u32,
    })
}

/// 按 uid 读取快照视图。
///
/// `reveal = false` 时，`sensitive = 1` 的环境变量值被置为 `None`（spec §3.2.1-3）。
pub fn get_snapshot(db: &Db, uid: &str, reveal: bool) -> Result<SnapshotView, CoreError> {
    let (id, created_at, kind): (i64, i64, String) = db
        .query_row_with(
            "SELECT id, created_at, kind FROM snapshots WHERE uid = ?1 AND deleted_at IS NULL",
            params![uid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| {
            if e.code == ErrorCode::Database {
                CoreError::new(
                    ErrorCode::NotFound,
                    "get_snapshot",
                    format!("快照不存在: {uid}"),
                )
            } else {
                e
            }
        })?;

    load_snapshot_view(
        db,
        id,
        uid,
        created_at,
        SnapshotKind::from_str(&kind),
        reveal,
    )
}

/// 读取 `app_state.current_snapshot_id` 指向的快照；无当前快照时返回 `None`。
///
/// 敏感值恒掩码（当前快照的明文请走 `reveal_env_var` / `list_all_env_vars`）。
pub fn current_snapshot(db: &Db) -> Result<Option<SnapshotView>, CoreError> {
    let row: Option<(i64, String)> = db
        .query_row_optional(
            "SELECT s.id, s.uid
             FROM app_state a JOIN snapshots s ON s.id = a.current_snapshot_id
             WHERE a.id = 1 AND s.deleted_at IS NULL",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| CoreError::new(e.code, "current_snapshot", e.message))?;

    let Some((id, uid)) = row else {
        return Ok(None);
    };
    // created_at / kind 从同一行取，避免二次查询带来的不一致窗口
    let (created_at, kind): (i64, String) = db
        .query_row_with(
            "SELECT created_at, kind FROM snapshots WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| CoreError::new(e.code, "current_snapshot", e.message))?;
    load_snapshot_view(
        db,
        id,
        &uid,
        created_at,
        SnapshotKind::from_str(&kind),
        false,
    )
    .map(Some)
}

/// 把 `app_state.current_snapshot_id` 指向给定 uid（含 `version` CAS）。
///
/// 返回 `false` 表示 CAS 失败（并发修改），调用方应重新读取后重试。
pub fn set_current_snapshot(
    db: &mut Db,
    uid: &str,
    expected_version: i64,
) -> Result<bool, CoreError> {
    db.with_transaction(|tx| {
        let id: Option<i64> = tx
            .query_row(
                "SELECT id FROM snapshots WHERE uid = ?1",
                params![uid],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| db_err("set_current", e))?;
        let id = id.ok_or_else(|| {
            CoreError::new(
                ErrorCode::NotFound,
                "set_current",
                format!("快照不存在: {uid}"),
            )
        })?;
        let changed = tx
            .execute(
                "UPDATE app_state
                 SET current_snapshot_id = ?1, version = version + 1
                 WHERE id = 1 AND version = ?2",
                params![id, expected_version],
            )
            .map_err(|e| db_err("set_current", e))?;
        Ok(changed == 1)
    })
}

/// 读取 `app_state.version`（CAS 用）。
pub fn app_state_version(db: &Db) -> Result<i64, CoreError> {
    db.query_row_with("SELECT version FROM app_state WHERE id = 1", [], |r| {
        r.get(0)
    })
    .map_err(|e| CoreError::new(e.code, "app_state_version", e.message))
}

/// 显式 reveal 一个历史快照里的 env 变量值（spec §3.2.1，M1 欠账 P2-1）。
///
/// **为什么是独立函数而不是 `get_snapshot(reveal=true)`**：
/// 掩码必须是默认行为；「看明文」应当是一次**显式、可审计**的动作，
/// 而不是某个批量查询的副作用。GUI 侧对应一条独立的 IPC 命令。
///
/// 返回 `NotFound` 的三种情况，调用方无需区分细节（都表示「没有可 reveal 的值」）：
/// - 快照不存在；
/// - 该 `(hive, name)` 在快照里不存在；
/// - 变量 `restorable = 0`（`Unsupported` 类型，库里 `value` 恒为 NULL）。
///
/// 敏感变量**同样返回明文**——调用方已经通过「调用本函数」表达了显式意图，
/// 再拦一次只会让「显示」按钮点了没反应。掩码的责任在读路径（[`get_snapshot`]）。
pub fn reveal_env_value(db: &Db, uid: &str, hive: &str, name: &str) -> Result<String, CoreError> {
    let row: Option<Option<String>> = db
        .query_row_optional(
            "SELECT e.value
               FROM env_values e
               JOIN snapshots s ON s.id = e.snapshot_id
              WHERE s.uid = ?1 AND s.deleted_at IS NULL
                AND e.hive = ?2 AND e.name = ?3 AND e.restorable = 1",
            params![uid, hive, name],
            |r| r.get::<_, Option<String>>(0),
        )
        .map_err(|e| CoreError::new(e.code, "reveal_env_value", e.message))?;

    match row {
        Some(Some(v)) => Ok(v),
        // 行存在但 value 为 NULL：Data 不一致（restorable=1 却没值），按 NotFound 处理
        Some(None) | None => Err(CoreError::new(
            ErrorCode::NotFound,
            "reveal_env_value",
            format!("历史快照中没有可读取的变量值: {hive}\\{name}"),
        )),
    }
}

/// 组装快照视图（PATH 条目 + 环境变量）。
fn load_snapshot_view(
    db: &Db,
    id: i64,
    uid: &str,
    created_at: i64,
    kind: SnapshotKind,
    reveal: bool,
) -> Result<SnapshotView, CoreError> {
    let mut stmt = db.prepare(
        "SELECT hive, path, enabled FROM path_entries
         WHERE snapshot_id = ?1 ORDER BY hive, position",
    )?;
    let rows = stmt
        .query_map(params![id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                PathEntry {
                    path: r.get(1)?,
                    enabled: r.get::<_, i64>(2)? != 0,
                },
            ))
        })
        .map_err(|e| db_err("load_paths", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("load_paths", e))?;

    let mut system = Vec::new();
    let mut user = Vec::new();
    for (hive, entry) in rows {
        if hive == "system" {
            system.push(entry);
        } else {
            user.push(entry);
        }
    }

    let mut stmt = db.prepare(
        "SELECT hive, name, kind, value, revision, sensitive, restorable
         FROM env_values WHERE snapshot_id = ?1 ORDER BY hive, name",
    )?;
    let env = stmt
        .query_map(params![id], |r| {
            let sensitive: i64 = r.get(5)?;
            let raw: Option<String> = r.get(3)?;
            Ok(EnvValueRow {
                hive: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                // 掩码只在读取层做，库里始终保存真实值（DB 本身已加密）
                value: if reveal || sensitive == 0 { raw } else { None },
                revision: r.get(4)?,
                sensitive: sensitive != 0,
                restorable: r.get::<_, i64>(6)? != 0,
            })
        })
        .map_err(|e| db_err("load_env", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| db_err("load_env", e))?;

    Ok(SnapshotView {
        uid: uid.to_string(),
        created_at,
        kind: kind.as_str().to_string(),
        system,
        user,
        env,
    })
}
