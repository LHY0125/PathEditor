//! CLI 子命令实现。每个文件对应一组顶层命令，只做「参数 → core 调用 → 输出」的转换。
//!
//! 本 crate 无任何安全判定逻辑：保留名、保护名单、`Unsupported` 类型、hive 写权限、
//! revision 校验全部由 `path_editor_core` 判定，这里只透传错误文本。

pub mod env;
pub mod import_export;
pub mod maintenance;
pub mod path;
pub mod profile;
pub mod scan;
pub mod sync;
