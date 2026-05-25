use serde::Serialize;

/// 传给前端的统一错误类型（保留供未来使用，当前命令返回 Result<T, String>）
#[derive(Debug, Serialize)]
pub struct AppError {
    pub message: String,
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError {
            message: s.to_string(),
        }
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError { message: s }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError {
            message: format!("IO 错误: {}", e),
        }
    }
}
