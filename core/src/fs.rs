/// 读取文本文件内容（供前端原生对话框选择文件后使用）
pub fn read_text_file(path: &str) -> Result<String, String> {
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
    Ok((data.system, data.user))
}

fn import_csv(content: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let mut sys = Vec::new();
    let mut usr = Vec::new();
    for line in content.lines() {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() >= 2 {
            match fields[0].trim() {
                "system" | "sys" => sys.push(fields[1].trim().to_string()),
                "user" | "usr" => usr.push(fields[1].trim().to_string()),
                _ => {}
            }
        }
    }
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
    if paths.is_empty() {
        return Err("TXT 文件中未找到路径".into());
    }
    // TXT 格式全部导入为用户路径
    Ok((vec![], paths))
}

/// 导出 PATH 为指定格式字符串
pub fn export_paths(sys: &[String], usr: &[String], format: &str) -> String {
    match format {
        "json" => {
            let data = serde_json::json!({
                "version": "5.0.0",
                "timestamp": chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                "system": sys,
                "user": usr,
            });
            serde_json::to_string_pretty(&data).unwrap_or_default()
        }
        "csv" => {
            let mut out = String::from("type,path\n");
            for p in sys {
                out.push_str(&format!("system,{}\n", p));
            }
            for p in usr {
                out.push_str(&format!("user,{}\n", p));
            }
            out
        }
        _ => {
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
            out
        }
    }
}
