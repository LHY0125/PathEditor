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

#[derive(serde::Serialize)]
pub struct ScanResult {
    pub conflicts: Vec<ConflictEntry>,
    pub tools: Vec<ToolGroup>,
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

/// 单次枚举 PATH 目录，最多使用 8 个扫描线程。
fn enumerate_paths(paths: &[String]) -> Vec<(usize, String, bool, Vec<String>)> {
    if paths.is_empty() {
        return vec![];
    }

    let workers = 8usize.min(paths.len());
    let chunk_size = paths.len().div_ceil(workers);
    let mut results = Vec::new();

    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for (chunk_index, chunk) in paths.chunks(chunk_size).enumerate() {
            let start = chunk_index * chunk_size;
            handles.push(scope.spawn(move || {
                chunk
                    .iter()
                    .enumerate()
                    .map(|(offset, dir)| {
                        let exists = Path::new(dir).is_dir();
                        let exes = if exists { list_exes(dir) } else { vec![] };
                        (start + offset, dir.clone(), exists, exes)
                    })
                    .collect::<Vec<_>>()
            }));
        }
        for handle in handles {
            if let Ok(partial) = handle.join() {
                results.extend(partial);
            }
        }
    });

    results.sort_by_key(|result| result.0);
    results
}

/// 只枚举一次目录，同时生成冲突和工具清单结果。
pub fn scan_paths(paths: Vec<String>, query: String) -> Result<ScanResult, String> {
    let query_lower = query.to_lowercase();
    let mut map: HashMap<String, Vec<(usize, String)>> = HashMap::new();
    let mut tools = Vec::new();

    for (priority, dir, exists, exes) in enumerate_paths(&paths) {
        for name in &exes {
            map.entry(name.to_lowercase())
                .or_default()
                .push((priority, dir.clone()));
        }

        let mut filtered = exes;
        if !query_lower.is_empty() {
            filtered.retain(|name| name.to_lowercase().contains(&query_lower));
        }
        filtered.sort();
        tools.push(ToolGroup {
            dir,
            exists,
            exes: filtered,
        });
    }

    let mut conflicts: Vec<ConflictEntry> = map
        .into_iter()
        .filter(|(_, locations)| locations.len() >= 2)
        .map(|(name, locations)| ConflictEntry {
            name,
            locations: locations
                .into_iter()
                .map(|(priority, dir)| ConflictLocation { dir, priority })
                .collect(),
        })
        .collect();
    conflicts.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(ScanResult { conflicts, tools })
}

/// 扫描 PATH 中的可执行文件冲突。
pub fn scan_conflicts(paths: Vec<String>) -> Result<Vec<ConflictEntry>, String> {
    scan_paths(paths, String::new()).map(|result| result.conflicts)
}

/// 扫描 PATH 中各目录提供的可执行文件。
pub fn scan_tools(paths: Vec<String>, query: String) -> Result<Vec<ToolGroup>, String> {
    scan_paths(paths, query).map(|result| result.tools)
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
