use path_editor_core::scanner;

#[tauri::command]
pub fn scan_conflicts(paths: Vec<String>) -> Result<Vec<scanner::ConflictEntry>, String> { scanner::scan_conflicts(paths) }
#[tauri::command]
pub fn scan_tools(paths: Vec<String>, query: String) -> Result<Vec<scanner::ToolGroup>, String> { scanner::scan_tools(paths, query) }
