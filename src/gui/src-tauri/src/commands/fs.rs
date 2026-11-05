use path_editor_core::platform::fs;
use path_editor_core::{CoreError, PathEntry};

#[tauri::command(async)]
pub fn import_file(path: String) -> Result<(Vec<PathEntry>, Vec<PathEntry>), CoreError> {
    let content = fs::read_text_file_scoped(&path)?;
    fs::import_paths(&path, &content)
}

#[tauri::command(async)]
pub fn export_path_entries(
    sys: Vec<PathEntry>,
    usr: Vec<PathEntry>,
    format: String,
) -> Result<String, CoreError> {
    fs::export_path_entries(&sys, &usr, &format)
}
