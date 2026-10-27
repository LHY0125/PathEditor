//! 导出：收集待导出变更 → 构建 payload → 加密 → 写文件 → 标记已导出。
//!
//! ## 顺序不是风格问题
//!
//! ```text
//! 正确：收集 → 加密 → 写文件 →（成功后）标记 exported_at
//! 错误：收集 → 加密 → 标记 exported_at → 写文件
//! ```
//!
//! 后者在写文件失败时，变更**既没导出成功、又不在待导出队列里了**——永久丢失，
//! 而且不会报错（`mark_exported` 成功了）。这是本次实现里唯一无法靠类型系统
//! 表达的约束，只能靠测试构造「写文件失败」这条少见路径来守。

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::db::sync::bundle::{seal, BundleHeader};
use crate::db::sync::change_log::{current_lamport, mark_exported, pending_export, ChangeRow};
use crate::db::Db;
use crate::domain::error::{CoreError, ErrorCode};

/// 导出结果。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    /// 本次导出的变更条数
    pub change_count: u32,
    /// 写出的文件路径
    pub path: PathBuf,
}

/// bundle 里的一条变更。
///
/// **禁止出现本地整数主键**：`id` 是各设备自己的自增列，跨设备毫无意义。
/// 所有引用一律用 uid（`uid` / `deviceId` / `entityUid`）。
///
/// ## `content` 是对补充
///
/// `Change` 只有 `oldValue` / `newValue` 两个自由文本字段，**不足以
/// 让导入方重建实体**：变更日志只知道「某快照被创建了」，不知道它里面有哪些
/// PATH 条目。若只导出变更行，接收端解密成功后只能记一条 `changes` 行，
/// 却无法写出对应的 `snapshots` / `path_entries`——同步「成功」但什么都没同步。
///
/// 因此导出时把**实体当前内容**一并带上（`content`）。它是导出瞬间的快照，
/// 天然自包含；导入方无需回查。代价是同一实体多次变更会重复携带内容，
/// 由去重逻辑在导入侧按 `entityUid` 合并（见 `import.rs`）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BundleChange<'a> {
    uid: &'a str,
    device_id: &'a str,
    seq: i64,
    lamport: i64,
    entity_type: &'a str,
    entity_uid: &'a str,
    field: Option<&'a str>,
    old_value: Option<&'a str>,
    new_value: Option<&'a str>,
    created_at: i64,
    /// 实体当前内容（`null` 表示实体已不存在，例如已删除）
    content: Option<serde_json::Value>,
}

impl<'a> BundleChange<'a> {
    fn new(c: &'a ChangeRow, content: Option<serde_json::Value>) -> Self {
        Self {
            uid: &c.uid,
            device_id: &c.device_id,
            seq: c.seq,
            lamport: c.lamport,
            entity_type: &c.entity_type,
            entity_uid: &c.entity_uid,
            field: c.field.as_deref(),
            old_value: c.old_value.as_deref(),
            new_value: c.new_value.as_deref(),
            created_at: c.created_at,
            content,
        }
    }
}

/// 读一条变更对应的实体内容。
///
/// 返回 `None` 表示实体已不存在（软删除或从未成功写入）——导入方据此判断
/// 是「墓碑」还是「内容缺失」。**查询失败也必须整体失败**：静默跳过会让
/// 接收端收到一条没有内容的变更，同步看起来成功但数据丢了。
fn entity_content(
    db: &Db,
    entity_type: &str,
    entity_uid: &str,
) -> Result<Option<serde_json::Value>, CoreError> {
    match entity_type {
        "snapshot" => {
            // `reveal = true`：敏感值必须同步过去（DB 与 bundle 都已加密）。
            // 掩码只发生在 UI 展示层，不在同步层。
            match crate::db::history::get_snapshot(db, entity_uid, true) {
                Ok(v) => Ok(Some(serde_json::to_value(&v).map_err(|e| {
                    CoreError::new(ErrorCode::Sync, "export", format!("快照序列化失败: {e}"))
                })?)),
                // 快照被软删/不存在 → 无内容可带，但变更本身仍然有效
                Err(e) if e.code == ErrorCode::NotFound => Ok(None),
                Err(e) => Err(e),
            }
        }
        "profile" => {
            let view = crate::db::profiles::get_profile(db, entity_uid)?;
            match view {
                Some(v) => Ok(Some(serde_json::to_value(&v).map_err(|e| {
                    CoreError::new(ErrorCode::Sync, "export", format!("配置序列化失败: {e}"))
                })?)),
                None => Ok(None),
            }
        }
        // `profile_revision` 与 `path_entry` / `env_value` 随父实体走：
        // 它们的内容已经包含在对应 `snapshot` / `profile` 的 content 里。
        _ => Ok(None),
    }
}

/// payload 顶层结构。
#[derive(Debug, Serialize)]
struct BundlePayload<'a> {
    changes: Vec<BundleChange<'a>>,
}

/// 导出同步包。成功返回写出的路径与条数。
///
/// **无待导出变更时也写出一个空包**：这与「同步中心显示 0 条」一致，
/// 且让「导出成功」这个操作始终产出文件（用户不必区分两种成功）。
pub fn export_bundle(
    db: &mut Db,
    passphrase: &str,
    path: &Path,
) -> Result<ExportReport, CoreError> {
    // 空口令等于不加密：在此拒绝，GUI/CLI 不得各写一份。
    if passphrase.is_empty() {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "export_bundle",
            "同步口令不能为空",
        ));
    }
    // ① 收集
    let pending = pending_export(db)?;
    let change_count = u32::try_from(pending.len())
        .map_err(|_| CoreError::new(ErrorCode::Sync, "export", "待导出变更过多"))?;

    // ② 构建 payload + header
    //
    // 每条变更都带上实体的**当前内容**，使 bundle 自包含（见 `BundleChange`
    // 的文档：`oldValue`/`newValue` 不足以重建实体）。
    let mut changes = Vec::with_capacity(pending.len());
    for c in &pending {
        let content = entity_content(db, &c.entity_type, &c.entity_uid)?;
        changes.push(BundleChange::new(c, content));
    }
    let payload = BundlePayload { changes };
    let payload_bytes = serde_json::to_vec(&payload).map_err(|e| {
        CoreError::new(
            ErrorCode::Sync,
            "export",
            format!("payload 序列化失败: {e}"),
        )
    })?;

    // `device_id` 取本机：所有待导出变更都是 `origin='local'`，device_id 相同
    let device_id = pending
        .first()
        .map(|c| c.device_id.clone())
        .unwrap_or_else(|| local_device_id(db).unwrap_or_else(|_| "local".to_string()));
    let header = BundleHeader {
        schema_version: crate::db::schema::CURRENT_SCHEMA_VERSION,
        device_id,
        exported_at: chrono::Utc::now().timestamp_millis(),
        // `base_lamport` 是**导出时**的时钟，导入方据此把自己提升到至少这个值
        base_lamport: current_lamport(db)?,
        change_count,
    };

    // ③ 加密
    let sealed = seal(passphrase, &header, &payload_bytes)?;

    // ④ 写文件 —— **必须成功之后才标记**
    std::fs::write(path, &sealed).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "export",
            format!("写入同步包失败: {}", e.kind()),
        )
    })?;

    // ⑤ 标记（同一事务内批量更新）
    let uids: Vec<String> = pending.into_iter().map(|c| c.uid).collect();
    mark_exported(db, &uids, header.exported_at)?;

    Ok(ExportReport {
        change_count,
        path: path.to_path_buf(),
    })
}

/// 读本机 `device_id`（`origin='local'` 行存在时优先用行里的值）。
fn local_device_id(db: &Db) -> Result<String, CoreError> {
    db.query_row_with(
        "SELECT COALESCE((SELECT device_id FROM app_state WHERE id = 1), 'local')",
        [],
        |r| r.get::<_, String>(0),
    )
}
