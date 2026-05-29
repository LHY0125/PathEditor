// 注意：TS 端 src/core/import-export.ts 有对应的导入导出实现，
// 前端使用 TS 版（需 ImportDialog 交互），CLI 使用 Rust 版，修改时需同步两端。

/// 过滤导入路径：去除空白、排除 null 字节和分号（PATH 分隔符冲突）
fn sanitize_paths(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty() && !p.contains('\0') && !p.contains(';'))
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

/// 导入路径文件（JSON / CSV / TXT），返回 (系统路径, 用户路径)
pub fn import_paths(path: &str, content: &str) -> Result<(Vec<String>, Vec<String>), String> {
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

fn import_json(content: &str) -> Result<(Vec<String>, Vec<String>), String> {
    #[derive(serde::Deserialize)]
    struct ImportData {
        #[serde(default)]
        system: Vec<String>,
        #[serde(default)]
        user: Vec<String>,
    }
    let data: ImportData =
        serde_json::from_str(content).map_err(|e| format!("JSON 解析失败: {}", e))?;
    Ok((sanitize_paths(data.system), sanitize_paths(data.user)))
}

fn import_csv(content: &str) -> Result<(Vec<String>, Vec<String>), String> {
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
            // 跳过 header 行 "type,path"
            let fields: Vec<&str> = trimmed.split(',').collect();
            if fields.len() >= 2 {
                let c0 = fields[0].trim().to_lowercase();
                let c1 = fields[1].trim().to_lowercase();
                if c0 == "type" && c1 == "path" {
                    continue;
                }
            }
        }

        let fields: Vec<&str> = trimmed.split(',').collect();
        if fields.len() >= 2 {
            match fields[0].trim().to_lowercase().as_str() {
                "system" | "sys" => sys.push(fields[1].trim().to_string()),
                "user" | "usr" => usr.push(fields[1].trim().to_string()),
                _ => {
                    log::warn!("import_csv: 无法识别的类型字段，已跳过: {trimmed}");
                }
            }
        } else {
            log::warn!("import_csv: 格式不正确（缺逗号），已跳过: {trimmed}");
        }
    }
    let sys = sanitize_paths(sys);
    let usr = sanitize_paths(usr);
    if sys.is_empty() && usr.is_empty() {
        return Err("CSV 文件中未找到有效路径".into());
    }
    Ok((sys, usr))
}

fn import_txt(content: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let paths: Vec<String> = content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let paths = sanitize_paths(paths);
    if paths.is_empty() {
        return Err("TXT 文件中未找到路径".into());
    }
    // TXT 格式全部导入为用户路径
    Ok((vec![], paths))
}

/// 导出 PATH 为指定格式字符串
pub fn export_paths(sys: &[String], usr: &[String], format: &str) -> Result<String, String> {
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
            let mut out = String::from("type,path\n");
            for p in sys {
                out.push_str(&format!("system,{}\n", p));
            }
            for p in usr {
                out.push_str(&format!("user,{}\n", p));
            }
            Ok(out)
        }
        "txt" => {
            let mut out = String::new();
            if !sys.is_empty() {
                out.push_str(&format!("# 系统 PATH ({})\n", sys.len()));
                for p in sys {
                    out.push_str(&format!("{}\n", p));
                }
            }
            if !usr.is_empty() {
                out.push_str(&format!("# 用户 PATH ({})\n", usr.len()));
                for p in usr {
                    out.push_str(&format!("{}\n", p));
                }
            }
            Ok(out)
        }
        _ => Err(format!("不支持的导出格式: {}", format)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_json_valid() {
        let json = r#"{"system": ["C:\\sys1", "C:\\sys2"], "user": ["D:\\usr1"]}"#;
        let (sys, usr) = import_json(json).unwrap();
        assert_eq!(sys, vec!["C:\\sys1", "C:\\sys2"]);
        assert_eq!(usr, vec!["D:\\usr1"]);
    }

    #[test]
    fn import_json_empty_arrays() {
        let (sys, usr) = import_json(r#"{"system": [], "user": []}"#).unwrap();
        assert!(sys.is_empty() && usr.is_empty());
    }

    #[test]
    fn import_json_missing_fields() {
        let (sys, usr) = import_json(r#"{}"#).unwrap();
        assert!(sys.is_empty() && usr.is_empty());
    }

    #[test]
    fn import_csv_valid() {
        let csv = "type,path\nsystem,C:\\sys1\nuser,D:\\usr1\n";
        let (sys, _usr) = import_csv(csv).unwrap();
        assert_eq!(sys, vec!["C:\\sys1"]);
        assert_eq!(_usr, vec!["D:\\usr1"]);
    }

    #[test]
    fn import_csv_with_bom() {
        let csv = "\u{FEFF}type,path\nsystem,C:\\sys1\n";
        let (sys, _) = import_csv(csv).unwrap();
        assert_eq!(sys, vec!["C:\\sys1"]);
    }

    #[test]
    fn import_csv_empty() {
        assert!(import_csv("type,path\n").is_err());
    }

    #[test]
    fn import_csv_alternate_type_names() {
        let csv = "type,path\nsys,D:\\a\nusr,D:\\b\n";
        let (sys, usr) = import_csv(csv).unwrap();
        assert_eq!(sys, vec!["D:\\a"]);
        assert_eq!(usr, vec!["D:\\b"]);
    }

    #[test]
    fn export_json_roundtrip() {
        let sys = vec!["C:\\a".into()];
        let usr: Vec<String> = vec![];
        let exported = export_paths(&sys, &usr, "json").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&exported).unwrap();
        assert_eq!(parsed["system"][0], "C:\\a");
    }

    #[test]
    fn export_csv_roundtrip() {
        let sys = vec!["C:\\a".into()];
        let usr = vec!["D:\\b".into()];
        let exported = export_paths(&sys, &usr, "csv").unwrap();
        assert!(exported.contains("system,C:\\a"));
        assert!(exported.contains("user,D:\\b"));
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
        assert_eq!(sys, vec!["C:\\x"]);
    }

    #[test]
    fn import_paths_txt_to_user() {
        let (sys, usr) = import_paths("test.txt", "C:\\x\nD:\\y\n").unwrap();
        assert!(sys.is_empty());
        assert_eq!(usr, vec!["C:\\x", "D:\\y"]);
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
    fn import_json_filters_null_byte_paths() {
        // sanitize_paths 作为额外防线
        let paths = vec!["C:\\safe".into(), "C:\\bad\0path".into()];
        assert_eq!(sanitize_paths(paths), vec!["C:\\safe"]);
    }

    #[test]
    fn import_csv_filters_semicolon_paths() {
        let csv = "type,path\nsystem,C:\\good\nsystem,C:\\bad;path\n";
        let (sys, _) = import_csv(csv).unwrap();
        assert_eq!(sys, vec!["C:\\good"]);
    }

    #[test]
    fn import_txt_trims_and_filters() {
        let txt = "  C:\\trimmed  \n\nC:\\bad\0path\n# comment\n";
        let (_, usr) = import_txt(txt).unwrap();
        assert_eq!(usr, vec!["C:\\trimmed"]);
    }

    #[test]
    fn sanitize_paths_removes_empty_after_trim() {
        let result = sanitize_paths(vec!["  ".into(), "C:\\ok".into()]);
        assert_eq!(result, vec!["C:\\ok"]);
    }
}
