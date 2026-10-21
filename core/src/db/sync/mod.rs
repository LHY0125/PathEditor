//! 跨设备同步（M3）。
//!
//! 子模块职责：
//!
//! - [`change_log`]：`changes` 表的写入与查询（本设备单调序号 + 逻辑时钟）
//! - [`bundle`]：同步包的加密格式（Argon2id 派生 + ChaCha20-Poly1305）
//! - [`export`]：收集待导出变更 → 加密 → 写文件
//! - [`import`]：解密验证 → 单事务应用 → 冲突记录
//! - [`merge`]：字段级 LWW 判定（纯函数，不碰数据库）
//! - [`conflict`]：`sync_conflicts` 读写与解决
//!
//! **同步不写注册表**：导入只改 DB，用户要生效必须走既有的
//! `apply_path_snapshot` / `apply_profile`。

pub mod bundle;
pub mod change_log;
pub mod conflict;
pub mod export;
pub mod import;
pub mod merge;
