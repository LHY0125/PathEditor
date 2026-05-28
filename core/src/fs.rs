/// 读取文本文件内容（供前端原生对话框选择文件后使用）

pub fn read_text_file(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("无法读取文件: {}", e))
}
