//! `merge_hive` 的 golden 用例（F-10 第 5 类：禁用项兼容性）。
//!
//! `merge_hive` 是 [`super`]（`crate::disabled`）的私有 fn，因此本文件以
//! `#[cfg(test)] #[path = "registry/golden/merge_golden_tests.rs"]` 挂在
//! `disabled.rs` 下，与主 golden 模块（`registry/golden_tests.rs`，挂
//! `registry/path.rs` 下）分开。JSON 用例位于同目录（`golden/`）。

use super::merge_hive;
use crate::path_entry::PathEntry;
use serde::Deserialize;
use serde_json::Value;

/// merge 用例的 JSON 形状（`note` 不反序列化，仅作文档）。
#[derive(Deserialize)]
struct MergeGoldenCase {
    name: String,
    input: Value,
    op: String,
    expect: Value,
}

/// 执行 merge 用例：注册表当前值 + 旧版禁用字符串 + 持久化快照 → 合并结果。
fn run_merge_case(case: &MergeGoldenCase) {
    let name = &case.name;
    match case.op.as_str() {
        "merge_hive" => {
            let registry: Vec<String> = serde_json::from_value(case.input["registry"].clone())
                .unwrap_or_else(|e| panic!("{name}: input.registry 解析失败: {e}"));
            let legacy: Vec<String> = serde_json::from_value(case.input["legacy_disabled"].clone())
                .unwrap_or_else(|e| panic!("{name}: input.legacy_disabled 解析失败: {e}"));
            let snapshot: Vec<PathEntry> = serde_json::from_value(case.input["snapshot"].clone())
                .unwrap_or_else(|e| panic!("{name}: input.snapshot 解析失败: {e}"));
            let got = merge_hive(registry, legacy, snapshot);
            let expect: Vec<PathEntry> = serde_json::from_value(case.expect.clone())
                .unwrap_or_else(|e| panic!("{name}: expect 解析失败: {e}"));
            assert_eq!(got, expect, "golden 用例 {name} 失败");
        }
        other => panic!("golden 用例 {name}: 未实现的 op `{other}`"),
    }
}

macro_rules! merge_golden_test {
    ($fn_name:ident, $file:expr) => {
        #[test]
        fn $fn_name() {
            let raw = include_str!($file);
            let case: MergeGoldenCase = serde_json::from_str(raw)
                .unwrap_or_else(|e| panic!("解析 golden 用例 {} 失败: {}", $file, e));
            run_merge_case(&case);
        }
    };
}

merge_golden_test!(golden_merge_legacy_orphan, "merge_legacy_orphan.json");
merge_golden_test!(
    golden_merge_snapshot_order_wins,
    "merge_snapshot_order_wins.json"
);
merge_golden_test!(
    golden_merge_legacy_marks_registry,
    "merge_legacy_marks_registry.json"
);
