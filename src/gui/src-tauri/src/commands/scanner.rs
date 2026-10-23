use path_editor_core::scanner;
use path_editor_core::CoreError;

#[tauri::command]
pub fn scan_paths(paths: Vec<String>, query: String) -> Result<scanner::ScanResult, CoreError> {
    scanner::scan_paths(paths, query)
}
