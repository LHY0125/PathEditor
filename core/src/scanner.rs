use std::collections::HashMap;
use std::fs;
use std::path::Path;

const EXECUTABLE_EXTENSIONS: &[&str] = &["exe", "bat", "cmd", "com", "ps1"];

#[derive(serde::Serialize, Clone)]
pub struct ConflictLocation {
    pub dir: String,
    pub priority: usize,
}

#[derive(serde::Serialize, Clone)]
pub struct ConflictEntry {
    pub name: String,
    pub locations: Vec<ConflictLocation>,
}

#[derive(serde::Serialize)]
pub struct ToolGroup {
    pub dir: String,
    pub exists: bool,
    pub exes: Vec<String>,
}

/// 扫描 PATH 中的可执行文件冲突
///
/// 遍历每个 PATH 目录，查找 .exe/.bat/.cmd/.com/.ps1 文件，
/// 标记出现在多个目录中的同名文件（后面的目录会被前面的「遮蔽」）

pub fn scan_conflicts(paths: Vec<String>) -> Result<Vec<ConflictEntry>, String> {
    // exe_name (小写) → [(priority, dir)]
    let mut map: HashMap<String, Vec<(usize, String)>> = HashMap::new();

    for (priority, dir) in paths.iter().enumerate() {
        let p = Path::new(dir);
        if !p.is_dir() {
            continue;
        }
        let entries = fs::read_dir(p).map_err(|e| format!("无法读取目录 {}: {}", dir, e))?;
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let name = fname.to_string_lossy();
            if let Some(ext) = Path::new(name.as_ref()).extension() {
                let ext_lower = ext.to_ascii_lowercase();
                if EXECUTABLE_EXTENSIONS.contains(&ext_lower.to_str().unwrap_or("")) {
                    let key = name.to_lowercase();
                    map.entry(key).or_default().push((priority, dir.clone()));
                }
            }
        }
    }

    let mut results: Vec<ConflictEntry> = map
        .into_iter()
        .filter(|(_, locs)| locs.len() >= 2)
        .map(|(name, locs)| ConflictEntry {
            name,
            locations: locs
                .into_iter()
                .map(|(priority, dir)| ConflictLocation { dir, priority })
                .collect(),
        })
        .collect();

    results.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(results)
}

/// 扫描 PATH 中各目录提供的可执行文件
///
/// query 非空时只返回文件名包含关键词的结果
pub fn scan_tools(paths: Vec<String>, query: String) -> Result<Vec<ToolGroup>, String> {
    let query_lower = query.to_lowercase();
    let mut groups: Vec<ToolGroup> = Vec::new();

    for dir in &paths {
        let p = Path::new(dir);
        if !p.is_dir() {
            groups.push(ToolGroup {
                dir: dir.clone(),
                exists: false,
                exes: vec![],
            });
            continue;
        }

        let entries = fs::read_dir(p).map_err(|e| format!("无法读取目录 {}: {}", dir, e))?;
        let mut exes: Vec<String> = Vec::new();

        for entry in entries.flatten() {
            let fname = entry.file_name();
            let name = fname.to_string_lossy();
            if let Some(ext) = Path::new(name.as_ref()).extension() {
                let ext_lower = ext.to_ascii_lowercase();
                if EXECUTABLE_EXTENSIONS.contains(&ext_lower.to_str().unwrap_or("")) {
                    if query_lower.is_empty() || name.to_lowercase().contains(&query_lower) {
                        exes.push(name.to_string());
                    }
                }
            }
        }

        exes.sort();
        groups.push(ToolGroup {
            dir: dir.clone(),
            exists: true,
            exes,
        });
    }

    Ok(groups)
}
