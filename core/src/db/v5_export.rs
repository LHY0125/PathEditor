//! v5 配置导出（M1 欠账 P2-2）：DB → v5 文件格式的退路。
//!
//! **为什么需要它**：v6 的配置存在 SQLCipher 库里，v5 的配置是
//! `~/.patheditor/profiles/*.json`。用户降级到 v5 或需要在旧版本间迁移时，
//! 必须有一个把库里的配置写回 v5 格式的出口。
//!
//! **输出必须能被 [`crate::profiles::load_profile_from`] 读回**——
//! 后者是 v5 侧的官方解析入口（读文件 → 解信封 → 版本迁移）。
//! 如果这里自己拼 JSON 却没有 `schemaVersion` 信封，v5 会把它当「无版本头的旧格式」，
//! 虽然也能读，但语义上不是 v5 当前格式。

use super::{profiles, Db};
use crate::error::{CoreError, ErrorCode};
use crate::persist::{Versioned, PERSIST_SCHEMA_VERSION};
use crate::profiles::ProfileData;

/// 把一个 DB 配置导出成 v5 `ProfileData` 的 JSON 字符串。
///
/// 返回的字符串**带 `Versioned` 信封**（`{"schemaVersion":1, ...ProfileData}`），
/// 与 v5 的 `save_profile` 写出的格式一致。
///
/// # Errors
/// - `NotFound`：配置 uid 不存在，或该配置没有指向任何快照
/// - `Database`：读库失败
pub fn export_profile_v5(db: &Db, uid: &str) -> Result<String, CoreError> {
    let Some(view) = profiles::get_profile(db, uid)? else {
        return Err(CoreError::new(
            ErrorCode::NotFound,
            "export_profile_v5",
            format!("配置不存在: {uid}"),
        ));
    };

    let Some(snapshot_uid) = view.snapshot_uid.clone() else {
        // 配置存在但没有快照：v5 格式要求 sys/user 两个数组，
        // 而「没有快照」无法区分「空配置」与「数据缺失」，因此按 NotFound 报错
        // 而不是导出两个空数组——后者会让用户以为配置真的为空。
        return Err(CoreError::new(
            ErrorCode::NotFound,
            "export_profile_v5",
            format!("配置 {} 没有关联的快照，无法导出", view.name),
        ));
    };

    let snap = crate::db::history::get_snapshot(db, &snapshot_uid, false)?;

    // v5 的时间字段是本地时间字符串 `YYYY-MM-DDTHH:MM:SS`（见 profiles::save_profile），
    // 库里是 Unix 毫秒。转换失败时回落为空串——时间不影响配置内容。
    let created = millis_to_v5_time(view.created_at);
    let modified = millis_to_v5_time(view.updated_at);

    let data = ProfileData {
        name: view.name,
        sys: snap.system,
        user: snap.user,
        created,
        modified,
    };

    serde_json::to_string_pretty(&Versioned {
        schema_version: PERSIST_SCHEMA_VERSION,
        inner: data,
    })
    .map_err(|e| {
        CoreError::new(
            ErrorCode::Internal,
            "export_profile_v5",
            format!("序列化失败: {e}"),
        )
    })
}

/// Unix 毫秒 → v5 的本地时间字符串。
///
/// v5 用 `chrono::Local::now().format("%Y-%m-%dT%H:%M:%S")`，
/// 这里保持一致，让导出的文件与 v5 自己写出的文件在时间字段上同构。
fn millis_to_v5_time(ms: i64) -> String {
    use chrono::{Local, TimeZone};
    match Local.timestamp_millis_opt(ms).single() {
        Some(dt) => dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        None => String::new(),
    }
}
