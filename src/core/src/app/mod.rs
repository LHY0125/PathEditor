//! 应用编排层：把平台适配与持久化组合成 GUI / CLI 直接调用的用例。
//!
//! 本层是唯一允许同时依赖 `platform` 与 `db` 的地方。

pub mod bootstrap;
pub mod disabled;
pub mod service;
