//! 历史记录接缝：服务层通过 `HistorySink` 上报事件，不直接依赖数据库。
//!
//! 这样 `core::app::service::save_path_with_sidecar` 无需知道 `Db` 的存在：
//! 生产注入 `DbHistory`，测试注入 `NoopHistory`，既有测试零改动。

use crate::domain::env_var::EnvHive;
use crate::domain::error::CoreError;

/// 事件来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventSource {
    /// 图形界面
    #[default]
    Gui,
    /// 命令行
    Cli,
    /// 文件导入
    Import,
    /// 快照恢复
    Restore,
    /// 迁移 / 基线
    Migration,
    /// 同步包导入
    Sync,
}

/// 事件动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventAction {
    /// 保存 PATH
    #[default]
    Save,
    /// 应用配置方案
    Apply,
    /// 恢复历史快照
    Restore,
    /// 导入文件
    Import,
    /// 修改环境变量
    EnvSet,
    /// 新增环境变量
    EnvAdd,
    /// 删除环境变量
    EnvRemove,
    /// 撤销
    Undo,
    /// 重做
    Redo,
}

/// 事件结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventResult {
    /// 完全成功
    #[default]
    Success,
    /// 部分 hive 成功
    Partial,
    /// 失败
    Failure,
}

/// 一次可记录的操作。**不得包含环境变量明文值**。
#[derive(Debug, Clone, Default)]
pub struct HistoryEvent {
    /// 事件来源
    pub source: EventSource,
    /// 事件动作
    pub action: EventAction,
    /// 涉及的 hive；两个 hive 都动过时为 `None`
    pub hive: Option<EnvHive>,
    /// 操作目标（变量名等）；无单一目标时为 `None`
    pub target: Option<String>,
    /// 只放字段名/摘要，禁止明文值。
    pub changed_fields: Vec<String>,
    /// 变更前 revision
    pub before_revision: Option<String>,
    /// 变更后 revision
    pub after_revision: Option<String>,
    /// 操作结果
    pub result: EventResult,
    /// 本次操作产生的快照 uid。
    ///
    /// 整改：没有它，历史面板点击事件后无法定位到快照——只能拿事件 uid
    /// 去查快照，必然 `NotFound`，右侧详情恒空。
    /// 由服务层在写完快照后回填；`None` 表示该操作没有产生快照（如纯查询类事件）。
    pub snapshot_uid: Option<String>,
}

/// 历史写入端口。实现方负责持久化；调用方不感知存储形态。
///
/// **：不要求 Send + Sync** —— `DbHistory` 持有内部可变句柄，
/// 同步调用即可。将来若需跨线程，改为 `Mutex<Db>` 并重新评估。
pub trait HistorySink {
    /// 记录一次事件。失败不应影响主流程（调用方只记日志）。
    fn record(&self, event: HistoryEvent) -> Result<(), CoreError>;
}

/// 无操作实现：测试与「未启用历史」场景使用。
pub struct NoopHistory;

impl HistorySink for NoopHistory {
    fn record(&self, _event: HistoryEvent) -> Result<(), CoreError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_history_accepts_any_event() {
        let sink = NoopHistory;
        let event = HistoryEvent {
            source: EventSource::Gui,
            action: EventAction::Save,
            hive: Some(EnvHive::User),
            target: None,
            changed_fields: vec!["path_entries".to_string()],
            before_revision: None,
            after_revision: Some("abc".to_string()),
            result: EventResult::Success,
            snapshot_uid: None,
        };
        assert!(sink.record(event).is_ok());
    }
}
