//! C→Rust 行为等价 golden 基线（F-10，Wave 2 Task 7）。
//!
//! 数据驱动：每个用例一个 JSON（`golden/` 目录，`include_str!` 读入），
//! `op` 字符串分派到具体函数调用。每个 `#[test]` 对应一个 JSON 用例，
//! 失败信息携带用例名与语义说明（见 JSON 的 `note` 字段）。
//!
//! **挂载位置说明**：本文件按控制者裁决位于 `core/src/registry/golden_tests.rs`，
//! 但 `mod` 声明挂在 [`super`]（`registry/path.rs`）下而非 `registry.rs`
//! 根 —— `split_path` / `join_path` / `validate_and_join_paths` /
//! `select_path_value_type` / `make_path_value` 是 `path.rs` 的私有 fn，
//! 只有其子模块可以 `use super::` 够到（brief W2-B3 的推理在 Task 4
//! registry 拆分后依然成立，只是宿主文件变了）。
//!
//! 覆盖六类行为中的五类（第 6 类延后，见 `golden/README.md`）：
//! 1. PATH 分割/空项/空白/重复（`split_path` / `join_path` / `clean_path_entries`）
//! 2. `REG_SZ` / `REG_EXPAND_SZ` 写回类型保持（`select_path_value_type` / `make_path_value`）
//! 3. 权限失败（`capabilities_for_with`，hive 写能力参数化注入）
//! 4. 备份格式契约（`backup.rs`；`backup_format_contract` 需读取注册表，仅读不写）
//! 5. profile / 导入导出 / 禁用项兼容性（`persist` 信封、`fs` 导入导出；
//!    `merge_hive` 用例在 `golden/merge_golden_tests.rs`，挂 `crate::disabled` 下）

use super::{
    clean_path_entries, join_path, make_path_value, select_path_value_type, split_path,
    validate_and_join_paths,
};
use crate::env_var::{capabilities_for_with, EnvValueKind};
use crate::path_entry::PathEntry;
use serde::Deserialize;
use winreg::enums::{RegType, REG_DWORD, REG_EXPAND_SZ, REG_SZ};
use winreg::types::FromRegValue;

/// 单个 golden 用例的 JSON 形状。`note` 不反序列化（serde 默认忽略未知字段），
/// 仅作为用例内的语义文档。
#[derive(Deserialize)]
struct GoldenCase {
    name: String,
    input: serde_json::Value,
    op: String,
    expect: serde_json::Value,
}

/// 把 JSON 里的 `EnvValueKind` 名映射为枚举（camelCase 命名，与 serde 序列化一致）。
fn parse_kind(s: &str) -> EnvValueKind {
    match s {
        "String" => EnvValueKind::String,
        "ExpandString" => EnvValueKind::ExpandString,
        "Unsupported" => EnvValueKind::Unsupported,
        other => panic!("未知 EnvValueKind: {other}"),
    }
}

/// 把 JSON 里的注册表类型名映射为 winreg `RegType`。
fn parse_reg_type(s: &str) -> RegType {
    match s {
        "REG_SZ" => REG_SZ,
        "REG_EXPAND_SZ" => REG_EXPAND_SZ,
        "REG_DWORD" => REG_DWORD,
        other => panic!("未知注册表类型: {other}"),
    }
}

/// 解析 `select_path_value_type` 的 `Option<RegType>` 输入（null → None）。
fn parse_opt_reg_type(v: &serde_json::Value) -> Option<RegType> {
    match v {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => Some(parse_reg_type(s)),
        other => panic!("existing 必须为字符串或 null，实际: {other}"),
    }
}

/// 按 `op` 分派执行单个用例。所有断言失败信息都带用例名。
fn run_case(case: &GoldenCase) {
    let name = &case.name;
    match case.op.as_str() {
        // ── 第 1 类：PATH 分割 / 拼接 / 清理 ──
        "split_path" => {
            let raw = case.input["raw"]
                .as_str()
                .unwrap_or_else(|| panic!("{name}: 缺 input.raw"));
            let got = split_path(raw);
            let expect: Vec<String> = serde_json::from_value(case.expect.clone())
                .unwrap_or_else(|e| panic!("{name}: expect 解析失败: {e}"));
            assert_eq!(got, expect, "golden 用例 {name} 失败");
        }
        "join_path" => {
            let paths: Vec<String> = serde_json::from_value(case.input["paths"].clone())
                .unwrap_or_else(|e| panic!("{name}: input.paths 解析失败: {e}"));
            let got = join_path(&paths);
            assert_eq!(
                got,
                case.expect.as_str().unwrap_or_default(),
                "golden 用例 {name} 失败"
            );
        }
        "validate_and_join_paths_error" => {
            let paths: Vec<String> = if let Some(pad) = case.input["pad_utf16_chars"].as_u64() {
                vec!["C:\\a".to_string(), "a".repeat(pad as usize)]
            } else {
                serde_json::from_value(case.input["paths"].clone())
                    .unwrap_or_else(|e| panic!("{name}: input.paths 解析失败: {e}"))
            };
            let label = case.input["label"].as_str().unwrap_or("测试");
            let err = match validate_and_join_paths(&paths, label) {
                Ok(v) => panic!("{name}: 应拒绝却成功，结果: {v:?}"),
                Err(e) => e,
            };
            if let Some(contains) = case.expect["contains"].as_array() {
                for s in contains {
                    let s = s.as_str().unwrap_or_default();
                    assert!(
                        err.contains(s),
                        "golden 用例 {name}: 错误 `{err}` 应包含 `{s}`"
                    );
                }
            }
            if let Some(prefix) = case.expect["label_prefix"].as_str() {
                assert!(
                    err.starts_with(prefix),
                    "golden 用例 {name}: 错误 `{err}` 应以 `{prefix}` 开头"
                );
            }
        }
        "clean_path_entries" => {
            let entries: Vec<PathEntry> = serde_json::from_value(case.input["entries"].clone())
                .unwrap_or_else(|e| panic!("{name}: input.entries 解析失败: {e}"));
            let (kept, removed) = clean_path_entries(entries);
            let expect_kept: Vec<PathEntry> = serde_json::from_value(case.expect["kept"].clone())
                .unwrap_or_else(|e| panic!("{name}: expect.kept 解析失败: {e}"));
            let expect_removed: Vec<String> =
                serde_json::from_value(case.expect["removed_paths"].clone())
                    .unwrap_or_else(|e| panic!("{name}: expect.removed_paths 解析失败: {e}"));
            let removed_paths: Vec<String> = removed.into_iter().map(|e| e.path).collect();
            assert_eq!(kept, expect_kept, "golden 用例 {name}: kept 不符");
            assert_eq!(
                removed_paths, expect_removed,
                "golden 用例 {name}: removed 不符"
            );
        }

        // ── 第 2 类：REG_SZ / REG_EXPAND_SZ 写回类型保持 ──
        "select_path_value_type" => {
            let existing = parse_opt_reg_type(&case.input["existing"]);
            let got = select_path_value_type(existing);
            let expect = parse_reg_type(case.expect.as_str().unwrap_or_default());
            assert_eq!(got, expect, "golden 用例 {name} 失败");
        }
        "make_path_value" => {
            let value = case.input["value"].as_str().unwrap_or_default();
            let vtype = parse_reg_type(case.input["vtype"].as_str().unwrap_or_default());
            let raw = make_path_value(value, vtype);
            if let Some(expect_vtype) = case.expect["vtype"].as_str() {
                assert_eq!(
                    raw.vtype,
                    parse_reg_type(expect_vtype),
                    "golden 用例 {name}: vtype 不符"
                );
            }
            if let Some(decoded) = case.expect["decoded"].as_str() {
                let got = String::from_reg_value(&raw)
                    .unwrap_or_else(|e| panic!("{name}: RegValue 解码失败: {e}"));
                assert_eq!(got, decoded, "golden 用例 {name}: 解码文本不符");
            }
            if case.expect["even_byte_len"].as_bool() == Some(true) {
                assert_eq!(
                    raw.bytes.len() % 2,
                    0,
                    "golden 用例 {name}: 字节数应为偶数（UTF-16LE）"
                );
            }
            if case.expect["ends_with_utf16_nul"].as_bool() == Some(true) {
                assert_eq!(
                    &raw.bytes[raw.bytes.len() - 2..],
                    &[0, 0],
                    "golden 用例 {name}: 应以 UTF-16 NUL 终止"
                );
            }
        }

        // ── 第 3 类：权限失败（hive 写能力注入） ──
        "capabilities_for_with" => {
            let writable = case.input["writable"].as_bool().unwrap_or_default();
            let var_name = case.input["name"].as_str().unwrap_or_default();
            let kind = parse_kind(case.input["kind"].as_str().unwrap_or_default());
            let (can_edit, can_delete) = capabilities_for_with(writable, var_name, kind);
            assert_eq!(
                can_edit,
                case.expect["can_edit"].as_bool().unwrap_or_default(),
                "golden 用例 {name}: can_edit 不符"
            );
            assert_eq!(
                can_delete,
                case.expect["can_delete"].as_bool().unwrap_or_default(),
                "golden 用例 {name}: can_delete 不符"
            );
        }

        // ── 第 4 类：备份格式契约（仅读注册表，不写） ──
        "backup_rejects_dir" => {
            let dir = case.input["custom_dir"].as_str().unwrap_or_default();
            let err = crate::backup::backup_registry(Some(dir.to_string()))
                .expect_err(&format!("golden 用例 {name}: 应拒绝系统目录"));
            let contains = case.expect["error_contains"].as_str().unwrap_or_default();
            assert!(
                err.contains(contains),
                "golden 用例 {name}: 错误 `{err}` 应包含 `{contains}`"
            );
        }
        "backup_format_contract" => {
            let dir = std::env::temp_dir().join("patheditor_golden_backup_fmt");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir)
                .unwrap_or_else(|e| panic!("{name}: 创建临时目录失败: {e}"));
            let filepath = crate::backup::backup_registry(Some(dir.to_string_lossy().into_owned()))
                .unwrap_or_else(|e| {
                    panic!("golden 用例 {name}: backup_registry 失败: {e}（本用例需可读取注册表，仅读不写）")
                });
            let content = std::fs::read_to_string(&filepath)
                .unwrap_or_else(|e| panic!("{name}: 读取备份文件失败: {e}"));
            let header = case.expect["header_prefix"].as_str().unwrap_or_default();
            assert!(
                content.starts_with(header),
                "golden 用例 {name}: 备份应以 `{header}` 开头"
            );
            for section in case.expect["sections"].as_array().unwrap_or(&vec![]) {
                let s = section.as_str().unwrap_or_default();
                assert!(
                    content.contains(s),
                    "golden 用例 {name}: 备份应包含 section `{s}`"
                );
            }
            let filename = std::path::Path::new(&filepath)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            assert!(
                filename.starts_with("path_backup_") && filename.ends_with(".txt"),
                "golden 用例 {name}: 备份文件名 `{filename}` 应为 path_backup_*.txt 形态"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }

        // ── 第 5 类：profile / 导入导出兼容性 ──
        "load_profile_legacy_v1" => {
            // 直接走 profiles.rs 同款持久化原语：Versioned 信封反序列化 + migrate。
            // 不触临时文件，行为与 read_profile_file 等价。
            let json = serde_json::to_string(&case.input["profile_json"])
                .unwrap_or_else(|e| panic!("{name}: 序列化 input 失败: {e}"));
            let versioned: crate::persist::Versioned<crate::profiles::ProfileData> =
                serde_json::from_str(&json)
                    .unwrap_or_else(|e| panic!("{name}: profile_json 反序列化失败: {e}"));
            match crate::persist::migrate(versioned, "配置文件") {
                Ok(data) => {
                    let expect_sys: Vec<PathEntry> =
                        serde_json::from_value(case.expect["sys"].clone())
                            .unwrap_or_else(|e| panic!("{name}: expect.sys 解析失败: {e}"));
                    let expect_user: Vec<PathEntry> =
                        serde_json::from_value(case.expect["user"].clone())
                            .unwrap_or_else(|e| panic!("{name}: expect.user 解析失败: {e}"));
                    assert_eq!(
                        data.name,
                        case.expect["name"].as_str().unwrap_or_default(),
                        "golden 用例 {name}: name 不符"
                    );
                    assert_eq!(data.sys, expect_sys, "golden 用例 {name}: sys 不符");
                    assert_eq!(data.user, expect_user, "golden 用例 {name}: user 不符");
                }
                Err(e) => {
                    let expect_code = case.expect["error_code"].as_str().unwrap_or_else(|| {
                        panic!("golden 用例 {name}: migrate 意外失败且无 error_code 期望: {e}")
                    });
                    assert_eq!(
                        serde_json::to_value(e.code).unwrap_or_default(),
                        serde_json::json!(expect_code),
                        "golden 用例 {name}: 错误码不符（按 camelCase 序列化判定）"
                    );
                    let contains = case.expect["message_contains"].as_str().unwrap_or_default();
                    assert!(
                        e.message.contains(contains),
                        "golden 用例 {name}: 错误 `{}` 应包含 `{contains}`",
                        e.message
                    );
                }
            }
        }
        "import_paths" => {
            let filename = case.input["filename"].as_str().unwrap_or_default();
            let content = serde_json::to_string(&case.input["content"])
                .unwrap_or_else(|e| panic!("{name}: 序列化 input.content 失败: {e}"));
            let (sys, usr) = crate::fs::import_paths(filename, &content)
                .unwrap_or_else(|e| panic!("golden 用例 {name}: 导入失败: {e}"));
            let expect_sys: Vec<PathEntry> = serde_json::from_value(case.expect["system"].clone())
                .unwrap_or_else(|e| panic!("{name}: expect.system 解析失败: {e}"));
            let expect_usr: Vec<PathEntry> = serde_json::from_value(case.expect["user"].clone())
                .unwrap_or_else(|e| panic!("{name}: expect.user 解析失败: {e}"));
            assert_eq!(sys, expect_sys, "golden 用例 {name}: system 不符");
            assert_eq!(usr, expect_usr, "golden 用例 {name}: user 不符");
        }
        "export_path_entries_csv" => {
            let sys: Vec<PathEntry> = serde_json::from_value(case.input["system"].clone())
                .unwrap_or_else(|e| panic!("{name}: input.system 解析失败: {e}"));
            let usr: Vec<PathEntry> = serde_json::from_value(case.input["user"].clone())
                .unwrap_or_else(|e| panic!("{name}: input.user 解析失败: {e}"));
            let out = crate::fs::export_path_entries(&sys, &usr, "csv")
                .unwrap_or_else(|e| panic!("golden 用例 {name}: 导出失败: {e}"));
            let expect_lines: Vec<String> = serde_json::from_value(case.expect["lines"].clone())
                .unwrap_or_else(|e| panic!("{name}: expect.lines 解析失败: {e}"));
            let got_lines: Vec<&str> = out.lines().collect();
            assert_eq!(got_lines, expect_lines, "golden 用例 {name}: 导出行不符");
        }

        other => panic!("golden 用例 {name}: 未实现的 op `{other}`"),
    }
}

/// 每个golden 用例一个 `#[test]`：JSON 路径相对本文件（`core/src/registry/`）。
macro_rules! golden_test {
    ($fn_name:ident, $file:expr) => {
        #[test]
        fn $fn_name() {
            let raw = include_str!($file);
            let case: GoldenCase = serde_json::from_str(raw)
                .unwrap_or_else(|e| panic!("解析 golden 用例 {} 失败: {}", $file, e));
            run_case(&case);
        }
    };
}

// ── 第 1 类：PATH 分割 / 空项 / 空白 / 重复 ──
golden_test!(
    golden_split_trims_and_drops_empty,
    "golden/split_trims_and_drops_empty.json"
);
golden_test!(golden_split_empty_string, "golden/split_empty_string.json");
golden_test!(
    golden_split_keeps_duplicates,
    "golden/split_keeps_duplicates.json"
);
golden_test!(
    golden_join_trims_and_drops_empty,
    "golden/join_trims_and_drops_empty.json"
);
golden_test!(
    golden_validate_rejects_null_byte,
    "golden/validate_rejects_null_byte.json"
);
golden_test!(
    golden_validate_rejects_oversize,
    "golden/validate_rejects_oversize.json"
);
golden_test!(
    golden_clean_dedupes_case_insensitive,
    "golden/clean_dedupes_case_insensitive.json"
);
golden_test!(
    golden_clean_keeps_percent_paths,
    "golden/clean_keeps_percent_paths.json"
);

// ── 第 2 类：REG_SZ / REG_EXPAND_SZ 写回类型保持 ──
golden_test!(golden_type_keeps_reg_sz, "golden/type_keeps_reg_sz.json");
golden_test!(
    golden_type_defaults_expand_sz,
    "golden/type_defaults_expand_sz.json"
);
golden_test!(
    golden_make_value_utf16_nul,
    "golden/make_value_utf16_nul.json"
);
golden_test!(golden_make_value_reg_sz, "golden/make_value_reg_sz.json");

// ── 第 3 类：权限失败（hive 写能力注入） ──
golden_test!(
    golden_caps_writable_allows,
    "golden/caps_writable_allows.json"
);
golden_test!(
    golden_caps_nonwritable_denies,
    "golden/caps_nonwritable_denies.json"
);
golden_test!(
    golden_caps_protected_denies,
    "golden/caps_protected_denies.json"
);
golden_test!(
    golden_caps_reserved_denies,
    "golden/caps_reserved_denies.json"
);

// ── 第 4 类：备份格式契约 ──
golden_test!(
    golden_backup_format_contract,
    "golden/backup_format_contract.json"
);
golden_test!(
    golden_backup_rejects_system_dir,
    "golden/backup_rejects_system_dir.json"
);

// ── 第 5 类：profile / 导入导出兼容性（merge_hive 部分见 merge_golden_tests.rs） ──
golden_test!(golden_profile_legacy_v1, "golden/profile_legacy_v1.json");
golden_test!(
    golden_profile_future_version_rejected,
    "golden/profile_future_version_rejected.json"
);
golden_test!(
    golden_import_json_sanitizes,
    "golden/import_json_sanitizes.json"
);
golden_test!(golden_export_csv_enabled, "golden/export_csv_enabled.json");
