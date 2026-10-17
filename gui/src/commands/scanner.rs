use path_editor_core::scanner;

#[tauri::command]
pub fn scan_paths(paths: Vec<String>, query: String) -> Result<scanner::ScanResult, String> {
    scanner::scan_paths(paths, query)
}
