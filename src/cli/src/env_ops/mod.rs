//! 环境变量子命令实现：参数转换、core 调用与输出格式化。
//!
//! 本模块**不实现任何安全判定**（保留 / 保护 / 敏感 / 权限 / revision 校验），
//! 全部由 `path_editor_core` 负责，此处仅透传错误文本。
//!
//! 子模块：
//! - [`value`]：值输入通道与并发选项解析
//! - [`render`]：列表、表格与 dry-run 预览渲染
//! - [`cmd`]：各 `env` 子命令实现

mod cmd;
mod render;
mod value;

pub(crate) use cmd::*;

#[cfg(test)]
mod tests;
