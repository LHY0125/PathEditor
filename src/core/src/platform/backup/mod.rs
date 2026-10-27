//! 注册表备份：环境变量 `.pebak` 备份/恢复、PATH 文本备份与轮换。
//!
//! 子模块：
//! - [`config`]：备份目录与保留份数
//! - [`env`]：环境变量备份的采集、写入、轮换与列表
//! - [`path`]：PATH 备份与轮换
//! - [`restore`]：环境变量备份的预览与恢复

mod config;
mod env;
mod path;
mod restore;

pub use config::{env_backup_dir, get_appdata_dir};
pub use env::*;
pub use path::*;
pub use restore::*;

/// env 备份默认保留份数。
///
/// 全量快照单文件约几十 KB；20 份在正常使用下是几百 KB 量级。
/// 写操作密集的用户可调大——注意 README 已提示备份含明文敏感值，
/// 保留份数越大暴露面越大。
pub const ENV_BACKUP_KEEP: usize = 20;

/// path 备份默认保留份数。
///
/// `path_backup_*.txt` 单文件约 2.3 KB，历史上无轮换，实测已累积约 250 份。
/// 可被 `~/.patheditor/config.ini` 的 `path_backup_keep` 覆盖（回落语义同 `env_backup_keep`）。
pub const PATH_BACKUP_KEEP: usize = 20;

#[cfg(test)]
mod tests;
