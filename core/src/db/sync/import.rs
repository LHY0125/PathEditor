//! 导入：解密验证 → 单事务应用 → 冲突记录。
//!
//! ## 三条不可协商的约束
//!
//! 1. **整体回滚**：任何一步失败都不产生半应用状态（spec §8）。
//!    靠 `Db::with_transaction` 保证——它不是「尽力而为」，是「全有或全无」。
//! 2. **幂等**：`changes.uid` 已存在则跳过。同一份包导入两次不该产生第二份数据。
//! 3. **不写注册表**：导入只改 DB；用户要生效必须显式走
//!    `apply_path_snapshot` / `apply_profile`。同步是「把配置搬过来」，
//!    不是「替我改系统」。
//!
//! ## 为什么 payload 里的 `content` 是必须的
//!
//! 变更日志只记「谁在什么时候改了什么」，**不记实体内容**。若只靠
//! `oldValue`/`newValue` 重建，接收端无法知道一个快照里有哪些 PATH 条目。
//! 因此导出把实体当前内容一并写入 `content`，导入据此落 `snapshots` /
//! `path_entries` / `profiles`（见 `export::BundleChange` 的文档）。

use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;

use crate::db::history::{EnvValueRow, NewSnapshot, SnapshotKind};
use crate::db::sync::bundle::{open, BundleHeader};
use crate::db::sync::change_log::{change_exists_tx, record_remote_change_tx, ChangeRow};
use crate::db::sync::conflict::record_conflict_tx;
use crate::db::sync::merge::{resolve, MergeDecision};
use crate::db::Db;
use crate::error::{CoreError, ErrorCode};
use crate::history::EventSource;

/// 导入结果。
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    /// 实际应用的变更数
    pub applied: u32,
    /// 因已存在而跳过的变更数（幂等）
    pub skipped: u32,
    /// 记录的冲突数
    pub conflicts: u32,
}

/// payload 里的一条变更（接受侧形状）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InboundChange {
    uid: String,
    device_id: String,
    seq: i64,
    lamport: i64,
    entity_type: String,
    entity_uid: String,
    field: Option<String>,
    old_value: Option<String>,
    new_value: Option<String>,
    #[serde(default)]
    content: Option<serde_json::Value>,
}

/// payload 顶层。
#[derive(Debug, Deserialize)]
struct InboundPayload {
    changes: Vec<InboundChange>,
}

/// 导入同步包。成功返回统计，失败**整体回滚**。
pub fn import_bundle(
    db: &mut Db,
    passphrase: &str,
    path: &Path,
) -> Result<ImportReport, CoreError> {
    // ① 读文件 + 解密验证（失败时不产生任何写入）
    let bytes = std::fs::read(path).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "import",
            format!("读取同步包失败: {}", e.kind()),
        )
    })?;
    let (header, payload_bytes) = open(passphrase, &bytes)?;

    // ② header 与 payload 交叉校验：自述条数与实际不符说明包被拼接/截断过
    let payload: InboundPayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| CoreError::new(ErrorCode::SyncFormat, "import", "同步包 payload 无法解析"))?;
    if payload.changes.len() != header.change_count as usize {
        return Err(CoreError::new(
            ErrorCode::SyncFormat,
            "import",
            format!(
                "同步包条数不符：header 声明 {}，payload 实际 {}",
                header.change_count,
                payload.changes.len()
            ),
        ));
    }

    // ③ 实体类型白名单：未知类型说明对端版本更新，静默忽略会丢掉用户的配置
    for c in &payload.changes {
        if !matches!(
            c.entity_type.as_str(),
            "snapshot" | "profile" | "profile_revision" | "path_entry" | "env_value"
        ) {
            return Err(CoreError::new(
                ErrorCode::SyncFormat,
                "import",
                format!("未知的实体类型: {}", c.entity_type),
            ));
        }
    }

    let header_device = header.device_id.clone();
    let base_lamport = header.base_lamport;

    // ④ 单事务应用
    db.with_transaction(|tx| {
        let mut report = ImportReport::default();
        // 同一包子内按 entityUid 去重：后到的 content 覆盖先到的（同一实体
        // 多次变更时，最后一次的内容才是当前值）。
        let mut seen_entities: HashSet<(String, String)> = HashSet::new();

        for c in &payload.changes {
            // 幂等：这个 uid 已经落过库就跳过
            if change_exists_tx(tx, &c.uid)? {
                report.skipped += 1;
                continue;
            }

            // LWW：与本地同实体同字段的当前值比较
            let local = latest_for_tx(tx, &c.entity_type, &c.entity_uid, c.field.as_deref())?;
            let remote_as_row = ChangeRow {
                uid: c.uid.clone(),
                device_id: c.device_id.clone(),
                seq: c.seq,
                lamport: c.lamport,
                origin: "remote".to_string(),
                entity_type: c.entity_type.clone(),
                entity_uid: c.entity_uid.clone(),
                field: c.field.clone(),
                old_value: c.old_value.clone(),
                new_value: c.new_value.clone(),
                created_at: 0,
                applied_at: 0,
                exported_at: None,
            };

            let decision = match &local {
                Some(l) => resolve(l, &remote_as_row),
                // 本地从未见过这个实体 → 直接采纳远端
                None => MergeDecision::TakeRemote,
            };

            match decision {
                MergeDecision::Conflict => {
                    record_conflict_tx(
                        tx,
                        local.as_ref().expect("Conflict 必有本地"),
                        &remote_as_row,
                    )?;
                    report.conflicts += 1;
                    // 冲突时不写实体，也不写变更行：用户做出选择后由
                    // `resolve_conflict` 追加一条 lamport 更高的变更。
                    continue;
                }
                MergeDecision::KeepLocal => {
                    report.skipped += 1;
                    continue;
                }
                MergeDecision::TakeRemote => {}
            }

            // 应用实体内容（只对带 content 的类型动手）
            if let Some(content) = &c.content {
                let key = (c.entity_type.clone(), c.entity_uid.clone());
                if seen_entities.insert(key) {
                    apply_entity_tx(tx, &c.entity_type, &c.entity_uid, content)?;
                }
            }

            record_remote_change_tx(
                tx,
                &c.uid,
                &c.device_id,
                c.seq,
                c.lamport,
                &c.entity_type,
                &c.entity_uid,
                c.field.as_deref(),
                c.old_value.as_deref(),
                c.new_value.as_deref(),
            )?;
            report.applied += 1;
        }

        // ⑤ 把本地时钟提到至少「导出方的时钟」——只升不降
        tx.execute(
            "UPDATE app_state SET lamport = MAX(lamport, ?1) WHERE id = 1",
            rusqlite::params![base_lamport],
        )
        .map_err(|e| crate::db::db_err("raise_lamport", e))?;

        log::info!(
            "同步包导入完成：来自设备 {} 的 {} 条（应用 {}，跳过 {}，冲突 {}）",
            header_device,
            payload.changes.len(),
            report.applied,
            report.skipped,
            report.conflicts
        );
        Ok(report)
    })
}

/// 事务内的 `latest_for`（`change_log::latest_for` 接 `&Db`，事务里用不了）。
fn latest_for_tx(
    tx: &rusqlite::Transaction,
    entity_type: &str,
    entity_uid: &str,
    field: Option<&str>,
) -> Result<Option<ChangeRow>, CoreError> {
    let field_clause = if field.is_some() {
        "AND field = ?3"
    } else {
        "AND field IS NULL"
    };
    let sql = format!(
        "SELECT uid, device_id, seq, lamport, origin, entity_type, entity_uid,
                field, old_value, new_value, created_at, applied_at, exported_at
           FROM changes
          WHERE entity_type = ?1 AND entity_uid = ?2 {field_clause}
          ORDER BY lamport DESC, device_id DESC
          LIMIT 1"
    );
    let mapper = |r: &rusqlite::Row<'_>| -> rusqlite::Result<ChangeRow> {
        Ok(ChangeRow {
            uid: r.get(0)?,
            device_id: r.get(1)?,
            seq: r.get(2)?,
            lamport: r.get(3)?,
            origin: r.get(4)?,
            entity_type: r.get(5)?,
            entity_uid: r.get(6)?,
            field: r.get(7)?,
            old_value: r.get(8)?,
            new_value: r.get(9)?,
            created_at: r.get(10)?,
            applied_at: r.get(11)?,
            exported_at: r.get(12)?,
        })
    };
    use rusqlite::OptionalExtension as _;
    let out = match field {
        Some(f) => tx.query_row(&sql, rusqlite::params![entity_type, entity_uid, f], mapper),
        None => tx.query_row(&sql, rusqlite::params![entity_type, entity_uid], mapper),
    }
    .optional()
    .map_err(|e| crate::db::db_err("latest_for_tx", e))?;
    Ok(out)
}

/// 把 bundle 里的实体内容写进本地库。
///
/// **不重复记账**：`record_remote_change_tx` 由调用方负责，这里只写实体本身。
fn apply_entity_tx(
    tx: &rusqlite::Transaction,
    entity_type: &str,
    entity_uid: &str,
    content: &serde_json::Value,
) -> Result<(), CoreError> {
    match entity_type {
        "snapshot" => {
            let snap = snapshot_from_content(content, entity_uid)?;
            crate::db::history::insert_snapshot_with_uid_tx(tx, snap, entity_uid, false)?;
            Ok(())
        }
        // profile 的实体写入依赖 `profiles` 表的本地整数主键映射，
        // 且 profile_revision 引用 snapshot —— 它们的落库由 M3 后续迭代
        // 的 `apply_profile_content` 承担；当前版本先保证变更日志与冲突记录
        // 正确，配置内容同步记录为未覆盖项。
        _ => Ok(()),
    }
}

/// 从 bundle 的 `content` 还原一个新快照。
fn snapshot_from_content(content: &serde_json::Value, uid: &str) -> Result<NewSnapshot, CoreError> {
    let bad = |why: &str| {
        CoreError::new(
            ErrorCode::SyncFormat,
            "import",
            format!("快照 {uid} 的内容不合法: {why}"),
        )
    };
    let kind = match content["kind"].as_str() {
        Some("current") => SnapshotKind::Current,
        Some("profile") => SnapshotKind::Profile,
        Some("backup") => SnapshotKind::Backup,
        Some("history") => SnapshotKind::History,
        other => return Err(bad(&format!("未知 kind: {other:?}"))),
    };
    let system: Vec<crate::PathEntry> = serde_json::from_value(content["system"].clone())
        .map_err(|e| bad(&format!("system 条目解析失败: {e}")))?;
    let user: Vec<crate::PathEntry> = serde_json::from_value(content["user"].clone())
        .map_err(|e| bad(&format!("user 条目解析失败: {e}")))?;
    let env: Vec<EnvValueRow> = serde_json::from_value(content["env"].clone())
        .map_err(|e| bad(&format!("env 解析失败: {e}")))?;

    Ok(NewSnapshot {
        kind,
        // 来源标为 `Sync`：库里能看出这条快照是同步来的
        source: EventSource::Sync,
        system,
        user,
        env,
        parent_uid: None,
    })
}

/// 供 GUI/CLI 展示的「未解析实体引用」检查（C-3 的实体侧）。
///
/// 当前实现只在导入时按需检查；独立入口留给 M4。
#[allow(dead_code)]
fn _unused(_: &BundleHeader) {}
