use path_editor_core::fs;
use path_editor_core::{CoreError, PathEntry};

#[tauri::command]
pub fn read_text_file(path: &str) -> Result<String, CoreError> {
    fs::read_text_file_scoped(path)
}

#[tauri::command]
pub fn import_file(path: &str) -> Result<(Vec<PathEntry>, Vec<PathEntry>), CoreError> {
    let content = fs::read_text_file_scoped(path)?;
    fs::import_paths(path, &content)
}

#[tauri::command]
pub fn export_path_entries(
    sys: Vec<PathEntry>,
    usr: Vec<PathEntry>,
    format: String,
) -> Result<String, CoreError> {
    fs::export_path_entries(&sys, &usr, &format)
}
