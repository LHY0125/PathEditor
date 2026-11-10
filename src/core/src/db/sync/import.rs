//! 导入：解密验证 → 单事务应用 → 冲突记录。
//!
//! ## 三条不可协商的约束
//!
//! 1. **整体回滚**：任何一步失败都不产生半应用状态。
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
use crate::db::sync::change_log::{
    change_exists_tx, current_device_id, record_remote_change_tx, ChangeRow, RemoteChange,
};
use crate::db::sync::conflict::record_conflict_tx;
use crate::db::sync::merge::{resolve, MergeDecision};
use crate::db::Db;
use crate::domain::error::{CoreError, ErrorCode};
use crate::domain::history::EventSource;

/// 导入结果。
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    /// 实际应用的变更数（**真正落库**的条数）
    pub applied: u32,
    /// 因已存在而跳过的变更数（幂等）
    pub skipped: u32,
    /// 记录的冲突数
    pub conflicts: u32,
    /// 本版本还不支持落库、因而**未应用也未记录**的变更数
    ///
    /// 这些变更**不写入 `changes`**：写了会让 uid 被占用，
    /// 等到真正支持该类型时同一个包再也无法重放。
    /// 界面必须把这个数字显示出来，否则用户会以为「全部同步成功」。
    ///
    /// 起对已知类型恒为 0：白名单里的类型全部可落库。字段保留是为了
    /// 将来新增类型时不必再改一次报告形状。
    pub skipped_unsupported: u32,
    /// 因与本地配置同名而**自动改名**的条数
    ///
    /// 改名只影响本地行，包内 uid 不变；界面与 CLI 都要显式说出来，
    /// 否则用户会发现「我的配置名字变了」却找不到原因。
    pub renamed_profiles: u32,
}

/// 当前版本能真正写入数据库的实体类型。
///
/// `profile_revision` / `path_entry` / `env_value` 的 `content` 为 `null`
/// （内容随父实体），`apply_entity_tx` 对它们是显式 no-op。
const APPLICABLE_ENTITY_TYPES: &[&str] = &[
    "snapshot",
    "profile",
    "profile_revision",
    "path_entry",
    "env_value",
];

/// 导入时的依赖优先级：数值小的先应用。
///
/// `profile` 的 `content.snapshotUid` 引用快照，必须等快照落库后才能解析。
/// `sort_by_key` 是**稳定排序**，同类型内部维持包内原有顺序。
fn entity_priority(entity_type: &str) -> u8 {
    match entity_type {
        "snapshot" => 0,
        "profile" => 1,
        "profile_revision" => 2,
        "path_entry" => 3,
        "env_value" => 4,
        _ => 5,
    }
}

/// 包格式允许出现的实体类型。
///
/// 当前与 [`APPLICABLE_ENTITY_TYPES`] **内容一致**。
/// 保留两个常量是**有意**的：将来新增已知类型时，先在白名单里接受，
/// 再实现落库——分离让「格式合法、但本版本还落不了库」这个中间态有地方表达，
/// 而不是被迫二选一。
const KNOWN_ENTITY_TYPES: &[&str] = &[
    "snapshot",
    "profile",
    "profile_revision",
    "path_entry",
    "env_value",
];

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

/// 同步包体积上限。超过即拒绝：`open()` 之前就要把整个文件读进内存，
/// 不设上限时一个超大文件足以让进程 OOM（`.pebak` 早已有 1 MiB 上限）。
const MAX_BUNDLE_BYTES: u64 = 64 * 1024 * 1024;
/// 单个包允许的变更条数上限（与体积上限互补：小而多的条目同样能撑爆内存）。
const MAX_BUNDLE_CHANGES: usize = 100_000;

/// 远端 `seq` 上限。留出 2 的余量：`next_seq` 还要在本地做两次递增，
/// 不能把 `i64::MAX` 和 `i64::MAX - 1` 留给远端包。
const MAX_INBOUND_SEQ: i64 = i64::MAX - 2;

/// 读取同步包文件，并在**读入内存之前**拒绝超限文件。
fn read_bundle_file(path: &Path) -> Result<Vec<u8>, CoreError> {
    let size = std::fs::metadata(path)
        .map_err(|e| {
            CoreError::new(
                ErrorCode::Io,
                "import",
                format!("读取同步包元数据失败: {}", e.kind()),
            )
        })?
        .len();
    if size > MAX_BUNDLE_BYTES {
        return Err(CoreError::new(
            ErrorCode::SyncFormat,
            "import",
            format!(
                "同步包过大（{} MiB，上限 {} MiB）",
                size / (1024 * 1024),
                MAX_BUNDLE_BYTES / (1024 * 1024)
            ),
        ));
    }
    std::fs::read(path).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "import",
            format!("读取同步包失败: {}", e.kind()),
        )
    })
}

/// 已解密并通过静态校验的同步包。
struct OpenedBundle {
    header: BundleHeader,
    payload: InboundPayload,
    base_lamport: i64,
}

/// 解密同步包，并完成 header ↔ payload 交叉校验、条数上限、实体白名单与时钟钳制。
fn open_bundle(passphrase: &str, bytes: &[u8]) -> Result<OpenedBundle, CoreError> {
    // ② header 与 payload 交叉校验：自述条数与实际不符说明包被拼接/截断过
    let (header, payload_bytes) = open(passphrase, bytes)?;
    if header.schema_version != crate::db::schema::CURRENT_SCHEMA_VERSION {
        return Err(CoreError::new(
            ErrorCode::SyncFormat,
            "import",
            format!(
                "同步包数据库结构版本 {} 与当前 {} 不匹配，请两端同时升级",
                header.schema_version,
                crate::db::schema::CURRENT_SCHEMA_VERSION
            ),
        ));
    }
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
    if payload.changes.len() > MAX_BUNDLE_CHANGES {
        return Err(CoreError::new(
            ErrorCode::SyncFormat,
            "import",
            format!(
                "同步包条目过多（{}，上限 {}）",
                payload.changes.len(),
                MAX_BUNDLE_CHANGES
            ),
        ));
    }

    // header 在明文里（只受 AEAD 保护），持有口令的对端可以把 `baseLamport`
    // 顶到 `i64::MAX`；沿用后本地 `+1` 会溢出（SQLite 转 REAL → 类型错误），
    // 此后本地记账永久失败。钳制到一半，留出足够的单调增长空间。
    let base_lamport = header.base_lamport.clamp(0, i64::MAX / 2);
    if base_lamport != header.base_lamport {
        log::warn!(
            "同步包的 baseLamport {} 超出合理范围，已钳制为 {}",
            header.base_lamport,
            base_lamport
        );
    }
    if header.device_id.trim().is_empty() {
        return Err(CoreError::new(
            ErrorCode::SyncFormat,
            "import",
            "同步包缺少 deviceId",
        ));
    }

    // ③ 实体类型白名单、设备一致性、时钟/序号边界与 profile 形状。
    //    这些字段都在密文/AAD 保护范围内，但持有口令的对端仍可构造非法值。
    for c in &payload.changes {
        if !KNOWN_ENTITY_TYPES.contains(&c.entity_type.as_str()) {
            return Err(CoreError::new(
                ErrorCode::SyncFormat,
                "import",
                format!("未知的实体类型: {}", c.entity_type),
            ));
        }
        if c.device_id != header.device_id {
            return Err(CoreError::new(
                ErrorCode::SyncFormat,
                "import",
                format!(
                    "变更设备 {} 与 header 设备 {} 不一致",
                    c.device_id, header.device_id
                ),
            ));
        }
        if c.lamport < 0 || c.lamport > base_lamport {
            return Err(CoreError::new(
                ErrorCode::SyncFormat,
                "import",
                format!("变更 lamport {} 超出允许范围 0..={base_lamport}", c.lamport),
            ));
        }
        if c.seq < 0 || c.seq > MAX_INBOUND_SEQ {
            return Err(CoreError::new(
                ErrorCode::SyncFormat,
                "import",
                format!("变更 seq {} 超出允许范围 0..={MAX_INBOUND_SEQ}", c.seq),
            ));
        }
        // profile 必须带字段名：字段级 LWW 的值无法从整实体猜测。
        if c.entity_type == "profile" {
            let Some(field) = c.field.as_deref() else {
                return Err(CoreError::new(
                    ErrorCode::SyncFormat,
                    "import",
                    "profile 变更必须带字段名",
                ));
            };
            if !["name", "description", "tags", "content", "deleted_at"].contains(&field) {
                return Err(CoreError::new(
                    ErrorCode::SyncFormat,
                    "import",
                    format!("未知的配置字段: {field}"),
                ));
            }
            // 墓碑必须是「无内容」的删除意图；带 content 的 deleted_at 是
            // 自相矛盾的形状，必须拒绝，否则会走整实体应用并清空删除标记。
            if field == "deleted_at" && c.content.is_some() {
                return Err(CoreError::new(
                    ErrorCode::SyncFormat,
                    "import",
                    "profile 墓碑不能携带 content",
                ));
            }
        }
    }

    Ok(OpenedBundle {
        header,
        payload,
        base_lamport,
    })
}

/// 导入同步包。成功返回统计，失败**整体回滚**。
pub fn import_bundle(
    db: &mut Db,
    passphrase: &str,
    path: &Path,
) -> Result<ImportReport, CoreError> {
    // 空口令等于不加密：在此拒绝，GUI/CLI 不得各写一份。
    if passphrase.is_empty() {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "import_bundle",
            "同步口令不能为空",
        ));
    }
    // ① 读文件 + 解密验证（失败时不产生任何写入）
    let bytes = read_bundle_file(path)?;
    let OpenedBundle {
        header,
        payload,
        base_lamport,
    } = open_bundle(passphrase, &bytes)?;

    let header_device = header.device_id.clone();

    // 远端包不能声称来自本机：否则伪造的 `seq` 会污染本地 `next_seq`。
    let local_device = current_device_id(db)?;
    if header_device == local_device {
        return Err(CoreError::new(
            ErrorCode::SyncFormat,
            "import",
            "同步包声称来自本机设备，已拒绝（请勿导入本机导出的包）",
        ));
    }

    // ④ 依赖排序：`sort_by_key` 是稳定排序，同类型内部维持包内原有顺序。
    //
    // `profile.content.snapshotUid` 指向同一包里的快照；不排序时，
    // 若导出方把 profile 写在 snapshot 前面，接收端会因「引用的快照不存在」
    // 整包回滚。
    let mut ordered: Vec<&InboundChange> = payload.changes.iter().collect();
    ordered.sort_by_key(|c| entity_priority(&c.entity_type));

    // ⑤ 单事务应用
    db.with_transaction(|tx| {
        let mut report = ImportReport::default();
        // 同一包子内按 entityUid 去重：同一实体只应用一次。
        //
        // 导出侧给每条变更附的都是**导出瞬间的当前内容**，所以同一实体
        // 多次出现时各条 content 相同；取第一条与取最后一条等价。这里只是
        // 避免重复写实体，不承诺「后到覆盖」。
        let mut seen_entities: HashSet<(String, String, String)> = HashSet::new();
        // profile 的字段级去重：同一实体可以带多条不同字段的变更。
        let mut seen_fields: HashSet<(String, String, String)> = HashSet::new();

        for c in &ordered {
            // 幂等：这个 uid 已经落过库就跳过
            if change_exists_tx(tx, &c.uid)? {
                report.skipped += 1;
                continue;
            }

            // 白名单里的类型在已**全部**可落库，本分支当前不可达。
            //
            // 保留它是为了将来：新增已知类型但还没实现落库时，必须走
            // 「不应用、不记录、单独计数」——记了就把 uid 占住，等真正支持
            // 该类型时同一个包会因为「uid 已存在」被永久跳过。
            if !APPLICABLE_ENTITY_TYPES.contains(&c.entity_type.as_str()) {
                report.skipped_unsupported += 1;
                continue;
            }

            // LWW 的维度：
            // - `snapshot`：整实体（内容不可字段化）
            // - `profile`：字段级（name / description / tags / content / deleted_at）
            // - 其余：`field` 维度
            //
            // 墓碑是实体级：删除必须同时压过所有字段，否则旧内容会在删除后
            // 「复活」。
            let local = match c.entity_type.as_str() {
                "snapshot" => latest_for_entity_tx(tx, &c.entity_type, &c.entity_uid)?,
                "profile" => {
                    let field = c.field.as_deref().ok_or_else(|| {
                        CoreError::new(ErrorCode::SyncFormat, "import", "profile 变更必须带字段名")
                    })?;
                    if field == "deleted_at" {
                        latest_for_entity_tx(tx, &c.entity_type, &c.entity_uid)?
                    } else {
                        latest_profile_field_tx(tx, &c.entity_uid, field)?
                    }
                }
                _ => latest_for_tx(tx, &c.entity_type, &c.entity_uid, c.field.as_deref())?,
            };

            // 墓碑优先：本地删除晚于（或等于）远端字段编辑时，编辑不得复活配置。
            // 严格小于才放行；平局保守保留删除，不制造「删除 vs 编辑」冲突。
            if c.entity_type == "profile" {
                if let Some(field) = c.field.as_deref() {
                    if field != "deleted_at" {
                        if let Some(tomb) = latest_profile_tombstone_tx(tx, &c.entity_uid)? {
                            if tomb.lamport >= c.lamport {
                                report.skipped += 1;
                                continue;
                            }
                        }
                    }
                }
            }
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
                        c.content.as_ref(),
                    )?;
                    report.conflicts += 1;
                    // 冲突时不写实体，也不写变更行：用户做出选择后由
                    // `resolve_conflict` 追加一条 lamport 更高的变更，
                    // 并按这里留下的 `remote_content` 应用远端实体。
                    continue;
                }
                MergeDecision::KeepLocal => {
                    report.skipped += 1;
                    continue;
                }
                MergeDecision::TakeRemote => {}
            }

            // 墓碑：`content = null` **且** field 明确是 `deleted_at`。
            //
            // 只有这两个条件同时成立才是删除意图；其它 `content = null` 只是
            // 「这条变更没有实体内容」（例如 `profile_revision` / `path_entry`）。
            //
            // 与内容变更共用 `seen_entities` 去重：同一实体在一包里只处理一次。
            if c.content.is_none() && c.field.as_deref() == Some("deleted_at") {
                let key = (
                    c.entity_type.clone(),
                    c.entity_uid.clone(),
                    "deleted_at".to_string(),
                );
                if seen_entities.insert(key) {
                    apply_tombstone_tx(tx, &c.entity_type, &c.entity_uid)?;
                }
            }

            // 应用实体内容（只对带 content 的类型动手）。
            //
            // profile 的新包按字段逐条应用；同一 (实体, 字段) 在一包里只应用
            // 一次。导出侧保证 content 是当前值，重复条目的内容一致。
            if let Some(content) = &c.content {
                let field_key = c.field.as_deref().unwrap_or("");
                let renamed = if c.entity_type == "profile"
                    && !field_key.is_empty()
                    && field_key != "deleted_at"
                {
                    let key = (
                        c.entity_type.clone(),
                        c.entity_uid.clone(),
                        field_key.to_string(),
                    );
                    if seen_fields.insert(key) {
                        crate::db::profiles::apply_profile_field_tx(
                            tx,
                            &c.entity_uid,
                            field_key,
                            content,
                            &c.device_id,
                        )?
                    } else {
                        false
                    }
                } else {
                    let key = (c.entity_type.clone(), c.entity_uid.clone(), String::new());
                    seen_entities.insert(key)
                        && apply_entity_tx(
                            tx,
                            &c.entity_type,
                            &c.entity_uid,
                            content,
                            &c.device_id,
                        )?
                };
                if renamed {
                    report.renamed_profiles += 1;
                }
            }

            record_remote_change_tx(
                tx,
                RemoteChange {
                    uid: &c.uid,
                    device_id: &c.device_id,
                    seq: c.seq,
                    lamport: c.lamport,
                    entity_type: &c.entity_type,
                    entity_uid: &c.entity_uid,
                    field: c.field.as_deref(),
                    old_value: c.old_value.as_deref(),
                    new_value: c.new_value.as_deref(),
                },
            )?;
            report.applied += 1;
        }

        // ⑥ 把本地时钟提到至少「导出方的时钟」——只升不降
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

/// 把 `changes` 的一行映射成 [`ChangeRow`]。
///
/// [`latest_for_tx`] 与 [`latest_for_entity_tx`] 共用同一份列顺序，
/// 避免两处各自维护、将来加了列只改一半。
fn map_change_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ChangeRow> {
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
    use rusqlite::OptionalExtension as _;
    let out = match field {
        Some(f) => tx.query_row(
            &sql,
            rusqlite::params![entity_type, entity_uid, f],
            map_change_row,
        ),
        None => tx.query_row(
            &sql,
            rusqlite::params![entity_type, entity_uid],
            map_change_row,
        ),
    }
    .optional()
    .map_err(|e| crate::db::db_err("latest_for_tx", e))?;
    Ok(out)
}

/// 取 profile 某字段的最新本地变更。
fn latest_profile_field_tx(
    tx: &rusqlite::Transaction,
    entity_uid: &str,
    field: &str,
) -> Result<Option<ChangeRow>, CoreError> {
    use rusqlite::OptionalExtension as _;

    tx.query_row(
        "SELECT uid, device_id, seq, lamport, origin, entity_type, entity_uid,
                field, old_value, new_value, created_at, applied_at, exported_at
           FROM changes
          WHERE entity_type = 'profile' AND entity_uid = ?1 AND field = ?2
          ORDER BY lamport DESC, device_id DESC
          LIMIT 1",
        rusqlite::params![entity_uid, field],
        map_change_row,
    )
    .optional()
    .map_err(|e| crate::db::db_err("latest_profile_field", e))
}

/// 取 profile 的最新墓碑（`field = "deleted_at"`）。
fn latest_profile_tombstone_tx(
    tx: &rusqlite::Transaction,
    entity_uid: &str,
) -> Result<Option<ChangeRow>, CoreError> {
    use rusqlite::OptionalExtension as _;

    tx.query_row(
        "SELECT uid, device_id, seq, lamport, origin, entity_type, entity_uid,
                field, old_value, new_value, created_at, applied_at, exported_at
           FROM changes
          WHERE entity_type = 'profile' AND entity_uid = ?1
            AND field = 'deleted_at'
          ORDER BY lamport DESC, device_id DESC
          LIMIT 1",
        rusqlite::params![entity_uid],
        map_change_row,
    )
    .optional()
    .map_err(|e| crate::db::db_err("latest_profile_tombstone", e))
}

/// 取某实体**最新的一条变更（不限 field）**。
///
/// `snapshot` 与 profile 墓碑用实体级 LWW：墓碑必须压过更早的字段变更，
/// 否则旧内容会在删除之后「复活」。
fn latest_for_entity_tx(
    tx: &rusqlite::Transaction,
    entity_type: &str,
    entity_uid: &str,
) -> Result<Option<ChangeRow>, CoreError> {
    use rusqlite::OptionalExtension as _;

    tx.query_row(
        "SELECT uid, device_id, seq, lamport, origin, entity_type, entity_uid,
                field, old_value, new_value, created_at, applied_at, exported_at
           FROM changes
          WHERE entity_type = ?1 AND entity_uid = ?2
          ORDER BY lamport DESC, device_id DESC
          LIMIT 1",
        rusqlite::params![entity_type, entity_uid],
        map_change_row,
    )
    .optional()
    .map_err(|e| crate::db::db_err("latest_entity", e))
}

/// 把 bundle 里的实体内容写进本地库。
///
/// **不重复记账**：`record_remote_change_tx` 由调用方负责，这里只写实体本身。
///
/// 返回 `true` 表示这条内容因同名而**自动改名**（目前只有 `profile` 会产生）。
pub(crate) fn apply_entity_tx(
    tx: &rusqlite::Transaction,
    entity_type: &str,
    entity_uid: &str,
    content: &serde_json::Value,
    device_id: &str,
) -> Result<bool, CoreError> {
    match entity_type {
        "snapshot" => {
            use rusqlite::OptionalExtension as _;
            // 幂等：同一 uid 已经存在时不再 INSERT（`snapshots.uid` 是 UNIQUE）。
            // 触发场景：保留策略把 `changes` 清掉后重放同一个包——变更行不在了，
            // 但实体还在。旧实现直接撞 UNIQUE 让整包以 `Database` 回滚。
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT id FROM snapshots WHERE uid = ?1",
                    [entity_uid],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| crate::db::db_err("find_snapshot", e))?;
            let id = match existing {
                Some(id) => id,
                None => {
                    let snap = snapshot_from_content(content, entity_uid)?;
                    // 远端快照的时间戳是事实：丢掉它会把历史时间线与统计日
                    // 改写成「导入当天」（profile 侧一直保留远端时间戳）。
                    let created_at = content.get("createdAt").and_then(|v| v.as_i64());
                    crate::db::history::insert_snapshot_with_uid_tx(
                        tx, snap, entity_uid, false, created_at,
                    )?;
                    tx.query_row(
                        "SELECT id FROM snapshots WHERE uid = ?1",
                        [entity_uid],
                        |r| r.get(0),
                    )
                    .map_err(|e| crate::db::db_err("find_snapshot", e))?
                }
            };
            // 导入的快照同样要进检索索引；否则「同步过来就搜不到」。
            crate::db::search::index_snapshot_tx(tx, id)?;
            Ok(false)
        }
        // 配置：保留远端 uid，`snapshotUid` 解析成本地快照主键。
        // `device_id` 只用于同名改名的后缀。
        "profile" => {
            let renamed =
                crate::db::profiles::import_profile_content_tx(tx, entity_uid, content, device_id)?;
            // 配置写入后同步更新索引（新建/改名/改描述都会变）。
            crate::db::search::index_profile_tx(tx, entity_uid)?;
            Ok(renamed)
        }
        // 内容随父实体：变更本身已落 `changes`，这里不单独写实体。
        "profile_revision" | "path_entry" | "env_value" => Ok(false),
        // 防御性分支：调用方（`import_bundle`）已按 `APPLICABLE_ENTITY_TYPES`
        // 提前跳过不支持的类型，这里**不应该**再被命中。
        //
        // 返回错误而不是 `Ok(())`：将来有人往可落库列表里加类型却忘了在这里
        // 实现，会**立刻炸**而不是静默吞掉—— 就是这么来的。
        other => Err(CoreError::new(
            ErrorCode::SyncFormat,
            "import",
            format!("内部错误：实体类型 {other} 未实现落库"),
        )),
    }
}

/// 应用墓碑：按 uid 软删除实体。
///
/// 只支持有 `deleted_at` 列的实体；其它类型说明对端用了本版本没有的语义，
/// 返回 `SyncFormat` 整包回滚，而不是静默忽略——静默忽略会让「删除」变成
/// 「什么也没发生」，比报错更难排查。
///
/// `WHERE ... AND deleted_at IS NULL` 让重复墓碑成为幂等的 no-op。
pub(crate) fn apply_tombstone_tx(
    tx: &rusqlite::Transaction,
    entity_type: &str,
    entity_uid: &str,
) -> Result<(), CoreError> {
    let now = chrono::Utc::now().timestamp_millis();
    let sql = match entity_type {
        "profile" => {
            "UPDATE profiles SET deleted_at = ?1, updated_at = ?1, version = version + 1
              WHERE uid = ?2 AND deleted_at IS NULL"
        }
        "snapshot" => {
            "UPDATE snapshots SET deleted_at = ?1, updated_at = ?1
              WHERE uid = ?2 AND deleted_at IS NULL"
        }
        other => {
            return Err(CoreError::new(
                ErrorCode::SyncFormat,
                "import",
                format!("实体类型 {other} 不支持墓碑"),
            ))
        }
    };
    tx.execute(sql, rusqlite::params![now, entity_uid])
        .map_err(|e| crate::db::db_err("apply_tombstone", e))?;
    // 删除同样要反映到检索索引，否则搜出来是死引用。
    match entity_type {
        "profile" => crate::db::search::remove_profile_index_tx(tx, entity_uid)?,
        "snapshot" => crate::db::search::remove_snapshot_index_tx(tx, entity_uid)?,
        _ => {}
    }
    Ok(())
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
