// TS 端 src/core/import-export.ts 仅保留为历史测试夹具；GUI/CLI 运行时统一使用本模块。

use crate::path_entry::PathEntry;
use crate::profiles::ProfilePathEntry;

/// 过滤导入条目：去除空白、排除 null 字节和分号（PATH 分隔符冲突）
fn sanitize_entries(entries: Vec<ProfilePathEntry>) -> Vec<ProfilePathEntry> {
    entries
        .into_iter()
        .map(|e| ProfilePathEntry {
            path: e.path.trim().to_string(),
            enabled: e.enabled,
        })
        .filter(|e| !e.path.is_empty() && !e.path.contains('\0') && !e.path.contains(';'))
        .collect()
}

/// 原子写入：先写临时文件，再 rename 覆盖
pub fn atomic_write(path: &std::path::Path, content: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// 读取文本文件内容（供前端原生对话框选择文件后使用）
/// 仅允许 .json / .csv / .txt 扩展名，防止任意文件读取
pub fn read_text_file(path: &str) -> Result<String, String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "json" | "csv" | "txt") {
        return Err(format!(
            "不支持的文件类型: .{}（仅允许 .json/.csv/.txt）",
            ext
        ));
    }
    std::fs::read_to_string(path).map_err(|e| format!("无法读取文件: {}", e))
}

/// 读取导入文件时限制在用户目录、临时目录或当前工作目录内。
pub fn read_text_file_scoped(path: &str) -> Result<String, String> {
    let requested = std::path::Path::new(path);
    let canonical = std::fs::canonicalize(requested).map_err(|e| format!("无法读取文件: {}", e))?;

    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home);
    }
    roots.push(std::env::temp_dir());
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }

    let allowed = roots.into_iter().any(|root| {
        std::fs::canonicalize(root)
            .map(|canonical_root| canonical.starts_with(canonical_root))
            .unwrap_or(false)
    });
    if !allowed {
        return Err("仅允许读取用户目录、临时目录或当前工作目录中的导入文件".into());
    }

    read_text_file(path)
}

/// 导入路径文件（JSON / CSV / TXT），返回 (系统条目, 用户条目)
pub fn import_paths(
    path: &str,
    content: &str,
) -> Result<(Vec<ProfilePathEntry>, Vec<ProfilePathEntry>), String> {
    let ext = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let ext = ext.to_string_lossy();

    match ext.as_ref() {
        "json" => import_json(content),
        "csv" => import_csv(content),
        "txt" => import_txt(content),
        _ => Err(format!("不支持的格式: .{}", ext)),
    }
}

fn import_json(content: &str) -> Result<(Vec<ProfilePathEntry>, Vec<ProfilePathEntry>), String> {
    #[derive(serde::Deserialize)]
    struct ImportItem {
        path: String,
        #[serde(default = "default_true")]
        enabled: bool,
    }
    fn default_true() -> bool {
        true
    }

    #[derive(serde::Deserialize)]
    struct ImportData {
        #[serde(default)]
        system: Vec<ImportItem>,
        #[serde(default)]
        user: Vec<ImportItem>,
    }
    let data: ImportData =
        serde_json::from_str(content).map_err(|e| format!("JSON 解析失败: {}", e))?;
    let into_entries = |items: Vec<ImportItem>| -> Vec<ProfilePathEntry> {
        items
            .into_iter()
            .map(|i| ProfilePathEntry {
                path: i.path,
                enabled: i.enabled,
            })
            .collect()
    };
    Ok((
        sanitize_entries(into_entries(data.system)),
        sanitize_entries(into_entries(data.user)),
    ))
}

/// 解析 CSV 行，支持引号包裹的字段（RFC 4180 子集）
/// 与测试夹具 src/core/import-export.ts 的兼容语义保持一致。
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next(); // 跳过转义引号
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == ',' {
            fields.push(current);
            current = String::new();
        } else {
            current.push(ch);
        }
    }
    fields.push(current);
    fields
}

fn import_csv(content: &str) -> Result<(Vec<ProfilePathEntry>, Vec<ProfilePathEntry>), String> {
    let mut sys = Vec::new();
    let mut usr = Vec::new();
    let mut first = true;
    for line in content.lines() {
        let mut trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 处理 UTF-8 BOM（仅首行）
        if first {
            first = false;
            if let Some(stripped) = trimmed.strip_prefix('\u{FEFF}') {
                trimmed = stripped;
            }
            // 跳过 header 行，兼容 type,path 和 type,path,enabled 两种格式
            let header_fields = parse_csv_line(trimmed);
            if header_fields.len() >= 2 {
                let c0 = header_fields[0].trim().to_lowercase();
                let c1 = header_fields[1].trim().to_lowercase();
                if c0 == "type" && c1 == "path" {
                    continue;
                }
            }
        }

        let fields = parse_csv_line(trimmed);
        if fields.len() >= 2 {
            let path = fields[1].trim().to_string();
            let enabled = if fields.len() >= 3 {
                fields[2].trim().to_lowercase() != "false"
            } else {
                true
            };
            let entry = ProfilePathEntry { path, enabled };
            match fields[0].trim().to_lowercase().as_str() {
                "system" | "sys" => sys.push(entry),
                "user" | "usr" => usr.push(entry),
                _ => {
                    log::warn!("import_csv: 无法识别的类型字段，已跳过: {trimmed}");
                }
            }
        } else {
            log::warn!("import_csv: 格式不正确（缺逗号），已跳过: {trimmed}");
        }
    }
    let sys = sanitize_entries(sys);
    let usr = sanitize_entries(usr);
    if sys.is_empty() && usr.is_empty() {
        return Err("CSV 文件中未找到有效路径".into());
    }
    Ok((sys, usr))
}

fn import_txt(content: &str) -> Result<(Vec<ProfilePathEntry>, Vec<ProfilePathEntry>), String> {
    let entries: Vec<ProfilePathEntry> = content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|path| ProfilePathEntry {
            path,
            enabled: true,
        })
        .collect();
    let entries = sanitize_entries(entries);
    if entries.is_empty() {
        return Err("TXT 文件中未找到路径".into());
    }
    // TXT 格式全部导入为用户路径
    Ok((vec![], entries))
}

fn escape_csv_field(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// 导出 PathEntry 为指定格式字符串，保留 enabled 状态。
pub fn export_path_entries(
    sys: &[PathEntry],
    usr: &[PathEntry],
    format: &str,
) -> Result<String, String> {
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
        _ => Err(format!("不支持的导出格式: {}", format)),
    }
}

/// 导出字符串路径列表；兼容旧 CLI 调用，默认全部启用。
pub fn export_paths(sys: &[String], usr: &[String], format: &str) -> Result<String, String> {
    let sys = sys
        .iter()
        .map(|path| PathEntry {
            path: path.clone(),
            enabled: true,
        })
        .collect::<Vec<_>>();
    let usr = usr
        .iter()
        .map(|path| PathEntry {
            path: path.clone(),
            enabled: true,
        })
        .collect::<Vec<_>>();
    export_path_entries(&sys, &usr, format)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> ProfilePathEntry {
        ProfilePathEntry {
            path: path.into(),
            enabled: true,
        }
    }

    fn entry_disabled(path: &str) -> ProfilePathEntry {
        ProfilePathEntry {
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
        assert!(result.unwrap_err().contains("不支持的文件类型"));
    }

    #[test]
    fn read_text_file_rejects_no_ext() {
        let result = read_text_file("/etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn read_text_file_scoped_allows_temp_import() {
        let path = std::env::temp_dir().join("patheditor_scoped_import.json");
        std::fs::write(&path, "{\"system\":[]}").unwrap();
        let result = read_text_file_scoped(path.to_string_lossy().as_ref());
        assert!(result.is_ok());
        let _ = std::fs::remove_file(path);
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
