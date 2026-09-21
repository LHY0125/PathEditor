//! 注册表访问统一入口。外部（gui/cli）一律经此路径引用，拆分对调用方不可见。
//!
//! 与 crate 根的 `crate::error` 区分：那是 F-06 的结构化 `CoreError`；
//! 本模块的 `conflict` 只承载注册表侧的冲突常量与构造。
//!
//! 子模块划分（W2 Task 4，纯搬家零行为变化）：
//! - [`path`]：PATH 读写、拆分/拼接/校验、清理
//! - [`access`]：hive 定位与写权限探测
//! - [`env_var`]：通用环境变量 CRUD（CoreError 形态）
//! - [`conflict`]：`[E_CONFLICT]` 常量与冲突错误构造

mod access;
mod conflict;
mod env_var;
mod path;

// ── 对外公开 API（gui/cli 经 `core::registry::*` 引用，路径保持不变）──
pub use access::can_write_user;
pub use conflict::conflict_message;
pub use env_var::{
    create_env_var, delete_env_var, delete_env_var_force, list_all_env_vars, reveal_env_var,
    update_env_var, update_env_var_force, validate_env_name, validate_env_value, WriteOutcome,
};
pub use path::{
    clean_path_entries, clean_paths, load_system_paths, load_user_paths, save_system_paths,
    save_user_paths,
};

// ── crate 内部共享（backup.rs / reg_store.rs / env_var.rs 等使用）──
pub(crate) use access::hive_location;
pub(crate) use path::{load_paths, SYS_REG_PATH, USER_REG_PATH};

// 供子模块内部互引的私有符号（`*_in_store` 系列、read/write 辅助、
// `conflict_error`/`ERR_CONFLICT` 等）保持各子模块内定义，经
// `super::conflict::` 等路径互引，不经根模块重导出。
