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

/// 扫描单个目录中的可执行文件名
fn list_exes(dir: &str) -> Vec<String> {
    let p = Path::new(dir);
    if !p.is_dir() {
        return vec![];
    }
    let mut exes: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(p) {
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let name = fname.to_string_lossy();
            if let Some(ext) = Path::new(name.as_ref()).extension() {
                let ext_lower = ext.to_ascii_lowercase();
                if EXECUTABLE_EXTENSIONS.contains(&ext_lower.to_str().unwrap_or("")) {
                    exes.push(name.to_string());
                }
            }
        }
    }
    exes
}

/// 扫描 PATH 中的可执行文件冲突
///
/// 并行遍历每个 PATH 目录，查找 .exe/.bat/.cmd/.com/.ps1 文件，
/// 标记出现在多个目录中的同名文件（后面的目录会被前面的「遮蔽」）
pub fn scan_conflicts(paths: Vec<String>) -> Result<Vec<ConflictEntry>, String> {
    let results: Vec<(usize, String, Vec<String>)> = std::thread::scope(|s| {
        let handles: Vec<_> = paths
            .iter()
            .enumerate()
            .map(|(priority, dir)| s.spawn(move || (priority, dir.clone(), list_exes(dir))))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().map_err(|e| format!("扫描线程失败: {:?}", e)))
            .collect::<Result<Vec<_>, _>>()
    })
    .map_err(|e| format!("线程扫描失败: {}", e))?;

    // 合并: exe_name (小写) → [(priority, dir)]
    let mut map: HashMap<String, Vec<(usize, String)>> = HashMap::new();
    for (priority, dir, exes) in results {
        for name in exes {
            map.entry(name.to_lowercase())
                .or_default()
                .push((priority, dir.clone()));
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
/// query 非空时只返回文件名包含关键词的结果。各目录并行扫描。
pub fn scan_tools(paths: Vec<String>, query: String) -> Result<Vec<ToolGroup>, String> {
    let query_lower = query.to_lowercase();

    // 并行扫描各目录
    let dir_results: Vec<(String, Option<Vec<String>>)> = std::thread::scope(|s| {
        let handles: Vec<_> = paths
            .iter()
            .map(|dir| {
                s.spawn(move || {
                    let p = Path::new(dir);
                    if !p.is_dir() {
                        return (dir.clone(), None);
                    }
                    let exes = list_exes(dir);
                    (dir.clone(), Some(exes))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().map_err(|e| format!("扫描线程失败: {:?}", e)))
            .collect::<Result<Vec<_>, _>>()
    })
    .map_err(|e| format!("线程扫描失败: {}", e))?;

    let mut groups: Vec<ToolGroup> = Vec::new();
    for (dir, opt_exes) in dir_results {
        match opt_exes {
            None => {
                groups.push(ToolGroup {
                    dir,
                    exists: false,
                    exes: vec![],
                });
            }
            Some(mut exes) => {
                if !query_lower.is_empty() {
                    exes.retain(|name| name.to_lowercase().contains(&query_lower));
                }
                exes.sort();
                groups.push(ToolGroup {
                    dir,
                    exists: true,
                    exes,
                });
            }
        }
    }

    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TempDirGuard(std::path::PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    impl std::ops::Deref for TempDirGuard {
        type Target = std::path::PathBuf;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    fn make_temp_dir_with_exes(prefix: &str, exe_names: &[&str]) -> TempDirGuard {
        let dir = std::env::temp_dir().join(format!("patheditor_test_{}", prefix));
        let _ = fs::remove_dir_all(&dir); // 清理残留
        fs::create_dir_all(&dir).unwrap();
        for name in exe_names {
            fs::write(dir.join(name), b"fake").unwrap();
        }
        TempDirGuard(dir)
    }

    #[test]
    fn scan_conflicts_no_duplicates() {
        let d1 = make_temp_dir_with_exes("c_a", &["a.exe"]);
        let d2 = make_temp_dir_with_exes("c_b", &["b.exe"]);
        let paths = vec![
            d1.to_string_lossy().to_string(),
            d2.to_string_lossy().to_string(),
        ];
        let conflicts = scan_conflicts(paths).unwrap();
        assert!(conflicts.is_empty());
    }

    #[test]
    fn scan_conflicts_detects_duplicate() {
        let d1 = make_temp_dir_with_exes("c_dup1", &["shared.exe"]);
        let d2 = make_temp_dir_with_exes("c_dup2", &["shared.exe"]);
        let paths = vec![
            d1.to_string_lossy().to_string(),
            d2.to_string_lossy().to_string(),
        ];
        let conflicts = scan_conflicts(paths).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].locations.len(), 2);
        assert_eq!(conflicts[0].locations[0].priority, 0);
        assert_eq!(conflicts[0].locations[1].priority, 1);
    }

    #[test]
    fn scan_tools_returns_groups() {
        let d1 = make_temp_dir_with_exes("t_a", &["tool.exe", "helper.bat"]);
        let paths = vec![d1.to_string_lossy().to_string()];
        let groups = scan_tools(paths, String::new()).unwrap();
        assert_eq!(groups.len(), 1);
        assert!(groups[0].exists);
        assert!(groups[0].exes.contains(&"helper.bat".to_string()));
        assert!(groups[0].exes.contains(&"tool.exe".to_string()));
    }

    #[test]
    fn scan_tools_with_query_filters() {
        let d1 = make_temp_dir_with_exes("t_q", &["apple.exe", "banana.exe"]);
        let paths = vec![d1.to_string_lossy().to_string()];
        let groups = scan_tools(paths, "apple".into()).unwrap();
        assert_eq!(groups[0].exes.len(), 1);
        assert_eq!(groups[0].exes[0], "apple.exe");
    }
}
