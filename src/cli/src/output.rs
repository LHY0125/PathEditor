//! CLI 的 JSON 输出出口。
//!
//! 序列化失败只会来自 `Serialize` 实现违反契约。此时打印错误并以 1 退出，
//! 避免在输出半截 JSON 后仍以成功码返回。

use crate::runtime::exit_err;

/// 以 pretty JSON 打印一个可序列化值。
pub(crate) fn print_json<T: serde::Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => println!("{text}"),
        Err(err) => exit_err(&format!("JSON 序列化失败: {err}")),
    }
}
