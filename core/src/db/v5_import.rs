//! v5 配置导入（M1 欠账 P2-2 的导入侧）：`profiles/*.json` → DB。
//!
//! 读取复用 [`crate::profiles::load_profile_from`]——它是 v5 侧唯一的解析入口，
//! 带 `schemaVersion` 信封与损坏隔离语义，不在这里复制。
//!
//! 与迁移期的一次性导入（`migrate_v5`）不同：这是**用户显式触发**的单文件导入，
//! 用于「从备份里捞回一个配置」。

use super::{profiles, Db};
use crate::db::history::{NewSnapshot, SnapshotKind};
use crate::error::CoreError;
use crate::history::EventSource;
use std::path::Path;

/// 把一个 v5 配置文件导入 DB，返回新配置的 uid。
///
/// 名称冲突返回 `NameExists`（与 `save_profile` 同口径），由用户先改名再导入——
/// 这与同步导入的「自动改名」是**有意的不同**：用户亲手指定了一个文件名，
/// 再悄悄改掉它的名字会让「我导入了什么」变得不可预期。
///
/// v5 的 `created` / `modified` 是展示字段，不写回 DB（导入时间即创建时间）。
pub fn import_profile_v5(db: &mut Db, path: &Path) -> Result<String, CoreError> {
    let data = crate::profiles::load_profile_from(path)?;
    let snap = NewSnapshot {
        kind: SnapshotKind::Current,
        source: EventSource::Import,
        system: data.sys,
        user: data.user,
        env: vec![],
        parent_uid: None,
    };
    profiles::save_profile(db, &data.name, None, &[], snap)
}
