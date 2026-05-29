use path_editor_core::fs;

#[tauri::command]
pub fn read_text_file(path: &str) -> Result<String, String> {
    fs::read_text_file(path)
}
