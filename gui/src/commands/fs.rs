use path_editor_core::fs;
use path_editor_core::PathEntry;

#[tauri::command]
pub fn read_text_file(path: &str) -> Result<String, String> {
    fs::read_text_file_scoped(path)
}

#[tauri::command]
pub fn import_file(path: &str) -> Result<(Vec<PathEntry>, Vec<PathEntry>), String> {
    let content = fs::read_text_file_scoped(path)?;
    fs::import_paths(path, &content)
}

#[tauri::command]
pub fn export_path_entries(
    sys: Vec<PathEntry>,
    usr: Vec<PathEntry>,
    format: String,
) -> Result<String, String> {
    fs::export_path_entries(&sys, &usr, &format)
}
