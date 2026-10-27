//! 平台适配层：所有直接触达外部世界的实现。
//!
//! 注册表读写、管理员探测、环境广播、文件系统、备份文件都在本层。
//! `registry` 与 `backup` 互相引用（写前备份 / 备份后回读），**必须同层**——
//! 这是本仓库唯一的层内强连通分量，拆开会产生 storage ↔ platform 环。

pub mod backup;
pub mod capabilities;
pub mod fs;
pub(crate) mod persist;
pub mod registry;
pub mod scanner;
pub mod system;
