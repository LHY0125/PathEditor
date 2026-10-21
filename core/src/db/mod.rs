//! SQLCipher 数据库句柄。
//!
//! **所有写事务 `BEGIN IMMEDIATE`**（spec D14：WAL 不代替业务并发控制）；
//! 读-改-写必须带 `version` CAS。

pub mod backups;
pub mod compare;
pub mod crypto;
#[cfg(windows)]
pub mod dacl;
pub mod history;
pub mod history_sink;
pub mod migrate_v5;
pub mod profiles;
pub mod refs;
pub mod scan_history;
pub mod schema;
pub mod search;
pub mod stats;
pub mod sync;
pub mod v5_export;

#[cfg(windows)]
pub mod dpapi;

use crate::error::{CoreError, ErrorCode};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::path::Path;

/// 数据库句柄。由 GUI / CLI 启动时创建并显式传入服务层，**不使用全局单例**。
pub struct Db {
    conn: Connection,
}

impl Db {
    /// 用 32 字节十六进制密钥打开（或创建）加密库。
    ///
    /// 打开序列与 M0 探针（`probes/v6-sqlcipher/src/lib.rs`）已验证的顺序一致：
    /// 先只设 key，再**做一次只读查询**让 SQLCipher 用 HMAC 校验密钥，
    /// 最后才设 PRAGMA —— 否则错误密钥会先被 `journal_mode=WAL` 的写操作触发，
    /// 报出的错误语义模糊。
    pub fn open_encrypted(path: &Path, key_hex: &str) -> Result<Self, CoreError> {
        if key_hex.len() != 64 || !key_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CoreError::new(
                ErrorCode::Crypto,
                "db_open",
                "数据库密钥必须是 32 字节（64 个十六进制字符）",
            ));
        }
        let conn = Connection::open(path).map_err(|e| db_err("open", e))?;
        // SQLCipher 的 key PRAGMA 必须用双引号包住 x'...'，不能用参数绑定。
        conn.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";"))
            .map_err(|e| db_err("key", e))?;
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|e| {
            CoreError::new(
                ErrorCode::Crypto,
                "db_open",
                format!("密钥错误或数据库损坏: {e}"),
            )
        })?;
        // cipher_memory_security 默认 OFF：M0 实测 ON 在受限进程里 VirtualLock 失败，
        // 且 ON/OFF 性能无显著差异（M0 结果 §四）。
        conn.execute_batch(
            "PRAGMA cipher_memory_security = OFF;
             PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|e| db_err("pragma", e))?;
        Ok(Self { conn })
    }

    /// 只读版本号（spec B2：启动序列中**不得**写 `user_version`）。
    pub fn schema_version(&self) -> Result<u32, CoreError> {
        self.conn
            .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .map_err(|e| db_err("user_version", e))
    }

    /// 建库/迁移到当前版本。**只有这里写 `user_version`。**
    ///
    /// 库版本高于程序支持时拒绝启动（文件由更新版本写入），避免旧代码误解新格式。
    pub fn migrate_to_current(&mut self) -> Result<(), CoreError> {
        let current = self.schema_version()?;
        if current > schema::CURRENT_SCHEMA_VERSION {
            return Err(CoreError::new(
                ErrorCode::Migration,
                "migrate",
                format!(
                    "数据库版本 {current} 高于程序支持的 {}，请升级 PathEditor",
                    schema::CURRENT_SCHEMA_VERSION
                ),
            ));
        }
        if current == schema::CURRENT_SCHEMA_VERSION {
            return Ok(());
        }
        self.with_transaction(|tx| {
            if current < 1 {
                tx.execute_batch(schema::DDL_V1)
                    .map_err(|e| db_err("ddl_v1", e))?;
                // J-7：建库即写 app_state 单行。否则 `set_current_snapshot` 的
                // CAS 永远匹配 0 行、恒返回 false（spec §十二 的 CAS 验收不可达）。
                tx.execute(
                    schema::SEED_APP_STATE_SQL,
                    rusqlite::params![uuid::Uuid::now_v7().to_string()],
                )
                .map_err(|e| db_err("seed_app_state", e))?;
            }
            // v2：M2 的派生表（scan_runs / var_references / daily_stats / search_fts）
            if current < 2 {
                tx.execute_batch(schema::DDL_V2)
                    .map_err(|e| db_err("ddl_v2", e))?;
            }
            // v3：M3 的同步表（changes / sync_conflicts）
            if current < 3 {
                tx.execute_batch(schema::DDL_V3)
                    .map_err(|e| db_err("ddl_v3", e))?;
            }
            // 版本号写在**所有分支之后**：v1→v2 的升级路径同样要落版本，
            // 写在 `if current < 1` 内部会让升级后 user_version 仍停在 1，
            // 下次启动重复执行 DDL_V2（表已存在 → 报错）。
            //
            // 用 execute_batch 而非 pragma_update：后者在事务内的生效行为
            // 依赖 rusqlite 版本，PRAGMA 语句本身是确定生效的。
            tx.execute_batch(&format!(
                "PRAGMA user_version = {}",
                schema::CURRENT_SCHEMA_VERSION
            ))
            .map_err(|e| db_err("set_version", e))?;
            Ok(())
        })
    }

    /// 表是否存在（测试/诊断用）。
    pub fn table_exists(&self, name: &str) -> Result<bool, CoreError> {
        let n: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [name],
                |r| r.get(0),
            )
            .map_err(|e| db_err("table_exists", e))?;
        Ok(n > 0)
    }

    /// 在 `BEGIN IMMEDIATE` 事务内执行闭包。
    ///
    /// SQLite 不支持嵌套事务：调用方若已在事务内，必须改用对应的 `*_tx` 变体
    /// （如 [`crate::db::history::insert_snapshot_tx`]），不要再次调用本方法。
    pub fn with_transaction<T>(
        &mut self,
        f: impl FnOnce(&Transaction) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| db_err("begin", e))?;
        let out = f(&tx)?;
        tx.commit().map_err(|e| db_err("commit", e))?;
        Ok(out)
    }

    /// 只读查询单值（测试/诊断用，集成测试需要）。
    pub fn query_row_with<T, P, F>(&self, sql: &str, params: P, f: F) -> Result<T, CoreError>
    where
        P: rusqlite::Params,
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        self.conn
            .query_row(sql, params, f)
            .map_err(|e| db_err("query_row", e))
    }

    /// 只读查询单值，**无行时返回 `Ok(None)`**。
    ///
    /// 不能直接对 [`Self::query_row_with`] 的返回值调 `.optional()` ——
    /// 后者是 `rusqlite::OptionalExtension` 为 `Result<T, rusqlite::Error>`
    /// 提供的扩展，错误类型换成 `CoreError` 后就不再适用。
    pub fn query_row_optional<T, P, F>(
        &self,
        sql: &str,
        params: P,
        f: F,
    ) -> Result<Option<T>, CoreError>
    where
        P: rusqlite::Params,
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        use rusqlite::OptionalExtension as _;
        self.conn
            .query_row(sql, params, f)
            .optional()
            .map_err(|e| db_err("query_row", e))
    }

    /// 预处理语句（只读列表查询用）。
    pub(crate) fn prepare(&self, sql: &str) -> Result<rusqlite::Statement<'_>, CoreError> {
        self.conn.prepare(sql).map_err(|e| db_err("prepare", e))
    }
}

/// 统一把 `rusqlite::Error` 包装为 `CoreError`。
///
/// 刻意写成**二元函数**而非返回闭包：`map_err` 需要一元 `FnOnce(E) -> F`，
/// 直接传 `db_err` 会类型不匹配（计划核对 B1）。
pub(crate) fn db_err(op: &'static str, e: rusqlite::Error) -> CoreError {
    CoreError::new(ErrorCode::Database, op, format!("{op} 失败: {e}"))
}

/// 用默认路径与 DPAPI 密钥打开数据库。
///
/// 失败时返回错误 —— **GUI 据此进入降级模式**（spec D16 fail-open），
/// CLI 对需要 DB 的命令返回 exit 5（Crypto）或 1（Database）。
///
/// **不静默重建空库**：密钥丢失/DPAPI 不可用时必须明确报错，否则会静默丢失历史。
pub fn open_default() -> Result<Db, CoreError> {
    let home = crypto::config_dir();
    let material = crypto::load_or_create_key(&home)?;

    // 启动时校验密钥文件 DACL（spec §5.1：过宽只告警，**不自动修改**——
    // 避免误伤用户自建的目录权限）。
    #[cfg(windows)]
    {
        let key_path = home.join("keys").join("db.key.dpapi");
        match dacl::key_dacl_is_restricted(&key_path) {
            Ok(true) => {}
            Ok(false) => log::warn!(
                "密钥文件 {} 的权限过宽（存在非当前用户/SYSTEM 的授权项）。\
                 其它用户可能删除或替换它，导致历史永久不可读。\
                 建议确保只有当前用户可访问该文件。",
                key_path.display()
            ),
            // 读取 ACL 失败不影响打开库：这是告警而非门禁
            Err(e) => log::warn!("无法校验密钥文件权限: {}", e.message),
        }
    }

    let path = home.join("patheditor.db");
    Db::open_encrypted(&path, &crypto::key_hex(&material))
}

/// 打开默认库并确保 schema 与 v5 迁移就绪（CLI 用）。
///
/// 与 GUI 的 `setup_database` 等价，但**不做 pending 补写**——那是 GUI 启动
/// 序列的职责（`retry_pending_path_state` 会写 `disabled.json`，CLI 单次命令
/// 不该有这种副作用）。
///
/// 失败返回 `Err`：CLI 对需要 DB 的命令应据此报错退出（spec §8）。
pub fn open_and_migrate() -> Result<Db, CoreError> {
    let mut db = open_default()?;
    db.migrate_to_current()?;
    let home = crypto::config_dir();
    // 迁移失败不阻断：库仍可用于新记录，旧文件等下次重试
    if let Err(e) = migrate_v5::migrate_legacy(&mut db, &home) {
        log::warn!("v5 文件迁移失败（继续）: {}", e.message);
    }
    Ok(db)
}
