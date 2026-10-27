//! 平台适配层：所有直接触达外部世界的实现。
//!
//! 注册表读写、管理员探测、环境广播、文件系统、备份文件都在本层。
//! `registry` 与 `backup` 互相引用（写前备份 / 备份后回读），**必须同层**——
//! 这是本仓库唯一的层内强连通分量（SCC-1），拆开会让 `platform` 内部出现环；
//! 跨层的 `db` ↔ `platform::backup`（SCC-2）已登记为白名单例外。

pub mod backup;
pub mod capabilities;
pub mod fs;
pub(crate) mod persist;
pub mod registry;
pub mod scanner;
pub mod system;
