use crate::domain::error::{CoreError, ErrorCode};
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
#[serde(rename_all = "camelCase")]
pub struct ToolGroup {
    pub dir: String,
    pub exists: bool,
    pub exes: Vec<String>,
    /// 该目录是否已在当前 PATH 中（winget/choco 等探测目录可能不在）
    pub in_path: bool,
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

/// 单个目录的枚举结果：`(PATH 优先级, 目录, 是否存在, 可执行文件名)`。
type DirScan = (usize, String, bool, Vec<String>);

/// 单次枚举 PATH 目录，最多使用 8 个扫描线程。
///
/// 任一工作线程 panic 都返回 `Err`：静默丢弃该分片会让冲突与工具清单
/// 同时缺一块，而调用方仍以为扫描成功。
fn enumerate_paths(paths: &[String]) -> Result<Vec<DirScan>, CoreError> {
    if paths.is_empty() {
        return Ok(vec![]);
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
            match handle.join() {
                Ok(partial) => results.extend(partial),
                Err(_) => {
                    return Err(CoreError::new(
                        ErrorCode::Internal,
                        "scan_paths",
                        "扫描线程异常终止，结果不完整",
                    ))
                }
            }
        }
        Ok(())
    })?;

    results.sort_by_key(|result| result.0);
    Ok(results)
}

/// winget / Chocolatey 的常见 shim 目录（仅返回存在的候选由调用方判断）。
fn package_manager_dirs() -> Vec<String> {
    let mut dirs = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(format!("{local}\\Microsoft\\WinGet\\Links"));
    }
    if let Ok(program_data) = std::env::var("PROGRAMDATA") {
        dirs.push(format!("{program_data}\\chocolatey\\bin"));
    }
    dirs
}

/// 只枚举一次目录，同时生成冲突和工具清单结果。
pub fn scan_paths(paths: Vec<String>, query: String) -> Result<ScanResult, CoreError> {
    let query_lower = query.to_lowercase();
    let mut map: HashMap<String, Vec<(usize, String)>> = HashMap::new();
    let mut tools = Vec::new();

    for (priority, dir, exists, exes) in enumerate_paths(&paths)? {
        for name in &exes {
            let locations = map.entry(name.to_lowercase()).or_default();
            // 同一目录在 PATH 里出现两次（`split_path` 刻意保留重复项）时，
            // 每个 exe 都会被当成「两处冲突」而两处 dir 完全相同 —— 纯误报。
            if !locations.iter().any(|(_, existing)| existing == &dir) {
                locations.push((priority, dir.clone()));
            }
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
            in_path: true,
        });
    }

    // winget / Chocolatey 的 shim 目录可能没有加入 PATH：单独探测并标记，
    // 让「装了但命令不可用」在分析面板里可见，而不是静默消失。
    let in_path_lower: Vec<String> = paths.iter().map(|p| p.to_ascii_lowercase()).collect();
    for dir in package_manager_dirs() {
        if in_path_lower.iter().any(|p| p == &dir.to_ascii_lowercase()) || !Path::new(&dir).is_dir()
        {
            continue;
        }
        let mut exes = list_exes(&dir);
        if !query_lower.is_empty() {
            exes.retain(|name| name.to_lowercase().contains(&query_lower));
        }
        exes.sort();
        tools.push(ToolGroup {
            dir,
            exists: true,
            exes,
            in_path: false,
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
pub fn scan_conflicts(paths: Vec<String>) -> Result<Vec<ConflictEntry>, CoreError> {
    scan_paths(paths, String::new()).map(|result| result.conflicts)
}

/// 扫描 PATH 中各目录提供的可执行文件。
pub fn scan_tools(paths: Vec<String>, query: String) -> Result<Vec<ToolGroup>, CoreError> {
    scan_paths(paths, query).map(|result| result.tools)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 建一个独立的临时目录并放入假可执行文件。
    ///
    /// 用 `tempfile::TempDir` 而非固定名字：测试并行时不会互相清理对方目录，
    /// 测试结束自动删除。
    fn make_temp_dir_with_exes(exe_names: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        for name in exe_names {
            fs::write(dir.path().join(name), b"fake").unwrap();
        }
        dir
    }

    #[test]
    fn scan_conflicts_no_duplicates() {
        let d1 = make_temp_dir_with_exes(&["a.exe"]);
        let d2 = make_temp_dir_with_exes(&["b.exe"]);
        let paths = vec![
            d1.path().to_string_lossy().to_string(),
            d2.path().to_string_lossy().to_string(),
        ];
        let conflicts = scan_conflicts(paths).unwrap();
        assert!(conflicts.is_empty());
    }

    #[test]
    fn scan_conflicts_detects_duplicate() {
        let d1 = make_temp_dir_with_exes(&["shared.exe"]);
        let d2 = make_temp_dir_with_exes(&["shared.exe"]);
        let paths = vec![
            d1.path().to_string_lossy().to_string(),
            d2.path().to_string_lossy().to_string(),
        ];
        let conflicts = scan_conflicts(paths).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].locations.len(), 2);
        assert_eq!(conflicts[0].locations[0].priority, 0);
        assert_eq!(conflicts[0].locations[1].priority, 1);
    }

    #[test]
    fn scan_tools_returns_groups() {
        let d1 = make_temp_dir_with_exes(&["tool.exe", "helper.bat"]);
        let paths = vec![d1.path().to_string_lossy().to_string()];
        let groups = scan_tools(paths, String::new()).unwrap();
        assert_eq!(groups.len(), 1);
        assert!(groups[0].exists);
        assert!(groups[0].exes.contains(&"helper.bat".to_string()));
        assert!(groups[0].exes.contains(&"tool.exe".to_string()));
    }

    #[test]
    fn scan_tools_with_query_filters() {
        let d1 = make_temp_dir_with_exes(&["apple.exe", "banana.exe"]);
        let paths = vec![d1.path().to_string_lossy().to_string()];
        let groups = scan_tools(paths, "apple".into()).unwrap();
        assert_eq!(groups[0].exes.len(), 1);
        assert_eq!(groups[0].exes[0], "apple.exe");
    }
}
