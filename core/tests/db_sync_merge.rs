//! 字段级 LWW 判定的四种情况。
//!
//! 本文件测试**纯函数**，不建库——因此不受 SQLCipher 栈溢出约束，
//! 但仍遵守一文件一测试（见 M0 §5.3）。
use path_editor_core::db::sync::change_log::ChangeRow;
use path_editor_core::db::sync::merge::{resolve, MergeDecision};

/// 构造一条变更。只用 `lamport` / `device_id` / `new_value` 三个维度，
/// 其余字段填与判定无关的常量。
fn change(lamport: i64, device: &str, value: &str) -> ChangeRow {
    ChangeRow {
        uid: format!("{device}-{lamport}-{value}"),
        device_id: device.to_string(),
        seq: lamport,
        lamport,
        origin: "remote".to_string(),
        entity_type: "profile".to_string(),
        entity_uid: "p1".to_string(),
        field: Some("name".to_string()),
        old_value: None,
        new_value: Some(value.to_string()),
        created_at: 0,
        applied_at: 0,
        exported_at: None,
    }
}

#[test]
fn lww_prefers_higher_lamport_then_device_id() {
    // ① 远端 lamport 更高 → 采纳远端
    assert_eq!(
        resolve(&change(1, "a", "x"), &change(2, "b", "y")),
        MergeDecision::TakeRemote,
        "高 lamport 必须胜"
    );

    // ② 远端 lamport 更低 → 保留本地
    assert_eq!(
        resolve(&change(2, "a", "x"), &change(1, "b", "y")),
        MergeDecision::KeepLocal,
        "低 lamport 必须负"
    );

    // ③ 同 lamport、不同 device、**值相同** → 不冲突，按 device_id 字典序确定性地取一个
    assert_eq!(
        resolve(&change(5, "aaa", "same"), &change(5, "bbb", "same")),
        MergeDecision::TakeRemote,
        "同 lamport 且值相同时，device_id 字典序大的胜（确定性，避免各端结论不一）"
    );
    assert_eq!(
        resolve(&change(5, "bbb", "same"), &change(5, "aaa", "same")),
        MergeDecision::KeepLocal,
        "本地 device_id 更大时必须保留本地"
    );

    // ④ 同 lamport、同 device → 同源，保持本地（不产生冲突）
    assert_eq!(
        resolve(&change(5, "a", "x"), &change(5, "a", "y")),
        MergeDecision::KeepLocal,
        "同一设备同一 lamport 视为同源"
    );

    // ⑤ 真冲突：同 lamport、不同 device、值不同
    assert_eq!(
        resolve(&change(5, "aaa", "x"), &change(5, "bbb", "y")),
        MergeDecision::Conflict,
        "同 lamport + 不同 device + 值不同才是冲突"
    );
    // 冲突判定与 device_id 的大小无关：换一边仍然是冲突
    assert_eq!(
        resolve(&change(5, "bbb", "y"), &change(5, "aaa", "x")),
        MergeDecision::Conflict,
        "冲突是对称的——不因本地 device_id 更大就静默取胜"
    );

    // ⑥ 两边 new_value 都为空（例如都是墓碑）视为相同，不冲突
    assert_eq!(
        resolve(&change(5, "aaa", ""), &change(5, "bbb", "")),
        MergeDecision::TakeRemote,
        "空值相同不算冲突"
    );
}
