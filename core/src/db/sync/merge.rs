//! 字段级 LWW（last-writer-wins）判定。
//!
//! **纯函数，不碰数据库**——因此可以脱离 SQLCipher 单测，不受「一进程一个
//! SQLCipher 测试」的约束。
//!
//! ## 判定规则（spec §6.3）
//!
//! 1. 远端 `lamport` 更大 → [`MergeDecision::TakeRemote`]
//! 2. 远端 `lamport` 更小 → [`MergeDecision::KeepLocal`]
//! 3. 相等：
//!    - 同一 `device_id` → 同源，[`MergeDecision::KeepLocal`]（幂等重放）
//!    - 不同 `device_id`：
//!      - **值相同** → 按 `device_id` 字典序决定，**不记冲突**
//!      - **值不同** → [`MergeDecision::Conflict`]
//!
//! ## 为什么「值相同时也按字典序取一个」
//!
//! 两台设备在同一个 lamport 上写了**相同**的值，语义上它们已经一致，
//! 但实现必须给出一个**确定性**的答案——否则 A 端认为「本地胜」、B 端也认为
//! 「本地胜」，两端都不更新，反而永远不知道对方已经一致了。用字典序
//! 破平能让所有端得到同一个结论。

use crate::db::sync::change_log::ChangeRow;

/// 合并结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeDecision {
    /// 采纳远端
    TakeRemote,
    /// 保留本地
    KeepLocal,
    /// 记录冲突，交人工解决
    Conflict,
}

/// 比较本地与远端的同一条变更（同 `entity_type` + `entity_uid` + `field`）。
pub fn resolve(local: &ChangeRow, remote: &ChangeRow) -> MergeDecision {
    if remote.lamport > local.lamport {
        return MergeDecision::TakeRemote;
    }
    if remote.lamport < local.lamport {
        return MergeDecision::KeepLocal;
    }

    // lamport 相等
    if remote.device_id == local.device_id {
        // 同源重放：远端带来的就是本地已经有的那条，不需要任何动作
        return MergeDecision::KeepLocal;
    }

    // 不同设备：值相同则不算冲突，按字典序确定性取一个
    if remote.new_value == local.new_value {
        return if remote.device_id > local.device_id {
            MergeDecision::TakeRemote
        } else {
            MergeDecision::KeepLocal
        };
    }

    MergeDecision::Conflict
}
