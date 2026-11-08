//! PATH 导入/导出的格式解析与序列化（纯字符串，不碰文件系统）。

#[cfg(test)]
use crate::domain::error::CoreError;
use crate::domain::path_entry::PathEntry;

/// 过滤导入条目：去除空白、排除 null 字节和分号（PATH 分隔符冲突）
pub(super) fn sanitize_entries(entries: Vec<PathEntry>) -> Vec<PathEntry> {
    entries
        .into_iter()
        .map(|e| PathEntry {
            path: e.path.trim().to_string(),
            enabled: e.enabled,
        })
        .filter(|e| !e.path.is_empty() && !e.path.contains('\0') && !e.path.contains(';'))
        .collect()
}

pub(super) fn import_json(content: &str) -> Result<(Vec<PathEntry>, Vec<PathEntry>), String> {
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
    let into_entries = |items: Vec<ImportItem>| -> Vec<PathEntry> {
        items
            .into_iter()
            .map(|i| PathEntry {
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
pub(super) fn parse_csv_line(line: &str) -> Vec<String> {
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

pub(super) fn import_csv(content: &str) -> Result<(Vec<PathEntry>, Vec<PathEntry>), String> {
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
            let entry = PathEntry { path, enabled };
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

pub(super) fn import_txt(content: &str) -> Result<(Vec<PathEntry>, Vec<PathEntry>), String> {
    let entries: Vec<PathEntry> = content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|path| PathEntry {
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

pub(crate) fn escape_csv_field(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// 导出字符串路径列表；兼容旧 CLI 调用，默认全部启用。
#[cfg(test)]
pub(super) fn export_paths(
    sys: &[String],
    usr: &[String],
    format: &str,
) -> Result<String, CoreError> {
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
    super::export_path_entries(&sys, &usr, format)
}
