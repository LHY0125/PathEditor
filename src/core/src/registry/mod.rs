//! 注册表访问统一入口。外部（gui/cli）一律经此路径引用，拆分对调用方不可见。
//!
//! 与 crate 根的 `crate::error` 区分：那是结构化 `CoreError`；
//! 本模块的 `conflict` 只承载注册表侧的冲突常量与构造。
//!
//! 子模块划分：
//! - [`path`]：PATH 读写、拆分/拼接/校验、清理
//! - [`access`]：hive 定位与写权限探测
//! - [`env_edit`]：通用环境变量 CRUD（CoreError 形态）
//! - [`conflict`]：`[E_CONFLICT]` 常量与冲突错误构造

mod access;
mod conflict;
mod env_edit;
mod path;
pub(crate) mod store;

// ── 对外公开 API（gui/cli 经 `core::registry::*` 引用，路径保持不变）──
pub use access::can_write_user;
pub use conflict::conflict_message;
pub use env_edit::{
    create_env_var, delete_env_var, delete_env_var_force, env_hive_writable, list_all_env_vars,
    preview_env_change, reveal_env_var, update_env_var, update_env_var_force, validate_env_name,
    validate_env_value, EnvChangePreview, WriteOutcome,
};
pub use path::{
    clean_path_entries, clean_paths, load_system_paths, load_user_paths, save_system_paths,
    save_user_paths,
};

// ── crate 内部共享（backup.rs / store.rs / env_var.rs 等使用）──
pub(crate) use access::hive_location;
// 恢复执行（`backup.rs`）需要逐变量调用这三个写函数，且**保护名单 / 类型 /
// 名称合法性判定只在它们内部实现一处。必须经根模块重导出：
// 子模块 `mod env_edit;` 是私有的，`crate::registry::env_edit::X` 会报
// `error[E0603]: module env_edit is private`。
pub(crate) use env_edit::{
    create_env_var_in_store, delete_env_var_force_in_store, update_env_var_force_in_store,
};
pub(crate) use path::{load_paths, SYS_REG_PATH, USER_REG_PATH};

// 其余子模块内部符号（`update_env_var_in_store`、`read/write_env_var` 辅助、
// `conflict_error`/`ERR_CONFLICT` 等）保持各子模块内定义，经
// `super::conflict::` 等路径互引，不经根模块重导出。
