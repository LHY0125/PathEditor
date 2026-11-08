// TS 端 src/core/import-export.ts 仅保留为历史测试夹具；GUI/CLI 运行时统一使用本模块。
//! PATH 导入/导出与文件读写；格式解析在 [`formats`]，文件系统边界在 [`io`]。

use crate::domain::error::{CoreError, ErrorCode};
use crate::domain::path_entry::PathEntry;

mod formats;
mod io;

pub(crate) use io::ensure_not_system_dir;
pub use io::{
    atomic_write, read_text_file, read_text_file_scoped, validate_sync_path, write_export_file,
};

use formats::{escape_csv_field, import_csv, import_json, import_txt};
#[cfg(test)]
use formats::{export_paths, parse_csv_line, sanitize_entries};

/// 导入路径文件（JSON / CSV / TXT），返回 (系统条目, 用户条目)
pub fn import_paths(
    path: &str,
    content: &str,
) -> Result<(Vec<PathEntry>, Vec<PathEntry>), CoreError> {
    let ext = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let ext = ext.to_string_lossy();

    let result = match ext.as_ref() {
        "json" => import_json(content),
        "csv" => import_csv(content),
        "txt" => import_txt(content),
        _ => {
            return Err(CoreError::new(
                ErrorCode::InvalidValue,
                "import_paths",
                format!("不支持的格式: .{}", ext),
            ))
        }
    };
    result.map_err(|m| CoreError::new(ErrorCode::Parse, "import_paths", m))
}

/// 导出 PathEntry 为指定格式字符串，保留 enabled 状态。
pub fn export_path_entries(
    sys: &[PathEntry],
    usr: &[PathEntry],
    format: &str,
) -> Result<String, CoreError> {
    match format {
        "json" => {
            let data = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "timestamp": chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                "system": sys,
                "user": usr,
            });
            Ok(serde_json::to_string_pretty(&data).expect("JSON 序列化 Value 不应失败"))
        }
        "csv" => {
            let mut out = String::from("type,path,enabled\n");
            for entry in sys {
                out.push_str(&format!(
                    "system,{},{}\n",
                    escape_csv_field(&entry.path),
                    entry.enabled
                ));
            }
            for entry in usr {
                out.push_str(&format!(
                    "user,{},{}\n",
                    escape_csv_field(&entry.path),
                    entry.enabled
                ));
            }
            Ok(out)
        }
        "txt" => {
            let mut out = String::new();
            if !sys.is_empty() {
                out.push_str(&format!("# 系统 PATH ({})\n", sys.len()));
                for entry in sys {
                    out.push_str(&format!("{}\n", entry.path));
                }
            }
            if !usr.is_empty() {
                out.push_str(&format!("# 用户 PATH ({})\n", usr.len()));
                for entry in usr {
                    out.push_str(&format!("{}\n", entry.path));
                }
            }
            Ok(out)
        }
        _ => Err(CoreError::new(
            ErrorCode::InvalidValue,
            "export_path_entries",
            format!("不支持的导出格式: {}", format),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> PathEntry {
        PathEntry {
            path: path.into(),
            enabled: true,
        }
    }

    fn entry_disabled(path: &str) -> PathEntry {
        PathEntry {
            path: path.into(),
            enabled: false,
        }
    }

    #[test]
    fn import_json_valid() {
        let json = r#"{"system": [{"path": "C:\\sys1"}, {"path": "C:\\sys2"}], "user": [{"path": "D:\\usr1"}]}"#;
        let (sys, usr) = import_json(json).unwrap();
        assert_eq!(sys.len(), 2);
        assert_eq!(sys[0].path, "C:\\sys1");
        assert!(sys[0].enabled);
        assert_eq!(sys[1].path, "C:\\sys2");
        assert_eq!(usr.len(), 1);
        assert_eq!(usr[0].path, "D:\\usr1");
    }

    #[test]
    fn import_json_empty_arrays() {
        let (sys, usr) = import_json(r#"{"system": [], "user": []}"#).unwrap();
        assert!(sys.is_empty() && usr.is_empty());
    }

    #[test]
    fn import_json_disabled_entry() {
        let json = r#"{"system": [{"path": "C:\\on", "enabled": true}, {"path": "C:\\off", "enabled": false}]}"#;
        let (sys, _) = import_json(json).unwrap();
        assert_eq!(sys.len(), 2);
        assert!(sys[0].enabled);
        assert!(!sys[1].enabled);
    }

    #[test]
    fn import_json_missing_fields() {
        let (sys, usr) = import_json(r#"{}"#).unwrap();
        assert!(sys.is_empty() && usr.is_empty());
    }

    #[test]
    fn import_csv_valid() {
        let csv = "type,path\nsystem,C:\\sys1\nuser,D:\\usr1\n";
        let (sys, usr) = import_csv(csv).unwrap();
        assert_eq!(sys.len(), 1);
        assert_eq!(sys[0].path, "C:\\sys1");
        assert!(sys[0].enabled);
        assert_eq!(usr.len(), 1);
        assert_eq!(usr[0].path, "D:\\usr1");
        assert!(usr[0].enabled);
    }

    #[test]
    fn import_csv_with_bom() {
        let csv = "\u{FEFF}type,path\nsystem,C:\\sys1\n";
        let (sys, _) = import_csv(csv).unwrap();
        assert_eq!(sys[0].path, "C:\\sys1");
    }

    #[test]
    fn import_csv_empty() {
        assert!(import_csv("type,path\n").is_err());
    }

    #[test]
    fn import_csv_alternate_type_names() {
        let csv = "type,path\nsys,D:\\a\nusr,D:\\b\n";
        let (sys, usr) = import_csv(csv).unwrap();
        assert_eq!(sys[0].path, "D:\\a");
        assert_eq!(usr[0].path, "D:\\b");
    }

    #[test]
    fn import_csv_reads_enabled_column() {
        let csv = "type,path,enabled\nsystem,C:\\ok,true\nsystem,C:\\disabled,false\n";
        let (sys, _) = import_csv(csv).unwrap();
        assert_eq!(sys.len(), 2);
        assert_eq!(sys[0].path, "C:\\ok");
        assert!(sys[0].enabled);
        assert_eq!(sys[1].path, "C:\\disabled");
        assert!(!sys[1].enabled);
    }

    #[test]
    fn import_csv_enabled_defaults_true() {
        // 2 列格式（无 enabled 列）默认为 true
        let csv = "type,path\nsystem,C:\\x\n";
        let (sys, _) = import_csv(csv).unwrap();
        assert!(sys[0].enabled);
    }

    #[test]
    fn export_json_roundtrip() {
        let sys = vec!["C:\\a".into()];
        let usr: Vec<String> = vec![];
        let exported = export_paths(&sys, &usr, "json").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&exported).unwrap();
        assert_eq!(parsed["system"][0]["path"], "C:\\a");
        assert_eq!(parsed["system"][0]["enabled"], true);
    }

    #[test]
    fn export_csv_includes_enabled_column() {
        let sys = vec!["C:\\a".into()];
        let usr = vec!["D:\\b".into()];
        let exported = export_paths(&sys, &usr, "csv").unwrap();
        assert!(exported.starts_with("type,path,enabled"));
        assert!(exported.contains("system,C:\\a,true"));
        assert!(exported.contains("user,D:\\b,true"));
    }

    #[test]
    fn export_txt_roundtrip() {
        let sys = vec!["C:\\a".into()];
        let usr = vec!["D:\\b".into()];
        let exported = export_paths(&sys, &usr, "txt").unwrap();
        assert!(exported.contains("C:\\a") && exported.contains("D:\\b"));
    }

    #[test]
    fn export_invalid_format_errors() {
        assert!(export_paths(&[], &[], "xml").is_err());
    }

    #[test]
    fn import_paths_detects_format() {
        let (sys, _) = import_paths("test.csv", "type,path\nsystem,C:\\x\n").unwrap();
        assert_eq!(sys[0].path, "C:\\x");
    }

    #[test]
    fn import_paths_txt_to_user() {
        let (sys, usr) = import_paths("test.txt", "C:\\x\nD:\\y\n").unwrap();
        assert!(sys.is_empty());
        assert_eq!(usr.len(), 2);
        assert_eq!(usr[0].path, "C:\\x");
        assert_eq!(usr[1].path, "D:\\y");
    }

    #[test]
    fn read_text_file_rejects_non_whitelisted_ext() {
        let result = read_text_file("C:\\Windows\\System32\\evil.dll");
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("不支持的文件类型"));
    }

    #[test]
    fn read_text_file_rejects_no_ext() {
        let result = read_text_file("/etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn read_text_file_scoped_allows_temp_import() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("scoped_import.json");
        std::fs::write(&path, "{\"system\":[]}").unwrap();
        let result = read_text_file_scoped(path.to_string_lossy().as_ref());
        assert!(result.is_ok());
    }

    #[test]
    fn write_export_file_rejects_system_dirs_and_writes_normal_paths() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("out.txt");

        let normal = write_export_file(path.to_str().unwrap(), "content");
        assert!(normal.is_ok(), "普通目录必须可写: {normal:?}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "content");

        for blocked in ["C:\\Windows\\out.txt", "C:/Program Files/out.txt"] {
            let result = write_export_file(blocked, "x");
            assert!(
                matches!(&result, Err(e) if e.code == ErrorCode::InvalidValue),
                "系统目录必须被拒绝（{blocked}）: {result:?}"
            );
        }
    }

    #[test]
    fn sanitize_entries_filters_null_byte_paths() {
        let entries = vec![entry("C:\\safe"), entry("C:\\bad\0path")];
        let result = sanitize_entries(entries);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "C:\\safe");
    }

    #[test]
    fn import_csv_filters_semicolon_paths() {
        let csv = "type,path\nsystem,C:\\good\nsystem,C:\\bad;path\n";
        let (sys, _) = import_csv(csv).unwrap();
        assert_eq!(sys.len(), 1);
        assert_eq!(sys[0].path, "C:\\good");
    }

    #[test]
    fn import_txt_trims_and_filters() {
        let txt = "  C:\\trimmed  \n\nC:\\bad\0path\n# comment\n";
        let (_, usr) = import_txt(txt).unwrap();
        assert_eq!(usr.len(), 1);
        assert_eq!(usr[0].path, "C:\\trimmed");
    }

    #[test]
    fn sanitize_entries_removes_empty_after_trim() {
        let result = sanitize_entries(vec![entry("  "), entry("C:\\ok")]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "C:\\ok");
    }

    #[test]
    fn sanitize_entries_preserves_enabled_flag() {
        let result = sanitize_entries(vec![entry_disabled("C:\\keep")]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "C:\\keep");
        assert!(!result[0].enabled);
    }

    #[test]
    fn parse_csv_line_basic() {
        assert_eq!(parse_csv_line("a,b,c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_csv_line_quoted_comma() {
        assert_eq!(
            parse_csv_line(r#"system,"C:\Program Files, Inc\bin""#),
            vec!["system", r#"C:\Program Files, Inc\bin"#]
        );
    }

    #[test]
    fn parse_csv_line_escaped_quotes() {
        assert_eq!(
            parse_csv_line(r#"system,"He said ""hello""""#),
            vec!["system", r#"He said "hello""#]
        );
    }

    #[test]
    fn import_csv_quoted_comma_path() {
        let csv = "type,path\nsystem,\"C:\\Program Files, Inc\\bin\"\n";
        let (sys, _) = import_csv(csv).unwrap();
        assert_eq!(sys[0].path, "C:\\Program Files, Inc\\bin");
    }

    #[test]
    fn csv_roundtrip_preserves_enabled() {
        let csv = "type,path,enabled\nsystem,C:\\on,true\nsystem,C:\\off,false\n";
        let (sys, _) = import_csv(csv).unwrap();
        assert_eq!(sys.len(), 2);
        assert!(sys[0].enabled);
        assert!(!sys[1].enabled);
    }
}
