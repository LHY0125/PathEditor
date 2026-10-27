//! 领域契约层：跨层共享的类型、错误码与端口（trait）。
//!
//! 本层**零 OS、零 IO、零数据库依赖**：不碰注册表、不碰文件系统、不打开 SQLite。
//! 只允许依赖 `serde` 与 `winreg::enums` 的类型常量。

pub mod env_var;
pub mod error;
pub mod history;
pub mod path_entry;
