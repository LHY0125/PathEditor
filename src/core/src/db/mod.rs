//! SQLite 数据库句柄（不加密）。
//!
//! **所有写事务 `BEGIN IMMEDIATE`**；
//! 读-改-写必须带 `version` CAS。
//!
//! **安全边界文件**（改动需走安全审查，等同 `db/security/` 的职责）：
//! `crypto.rs`（本机盐与 `.pebak` 密钥材料）、`dpapi.rs`（DPAPI 加解密）、
//! `dacl.rs`（盐文件访问控制，仅 Windows）。
//!
//! 这三个文件刻意留在 `db/` 顶层而不收进 `db/security/`——`db::crypto` 前缀在源码里
//! 有近 60 处引用，加一层只会让路径变长。本清单是那次取舍的补偿措施（spec D4）。
//!
//! **数据库文件本身不加密**：`~/.patheditor/patheditor.db` 是标准 SQLite。
//! 需要加密的只剩 `.pebak` 备份（DPAPI）与同步 bundle（Argon2id + AEAD）。

pub mod audit_export;
pub mod backup_index;
pub mod compare;
pub mod crypto;
#[cfg(windows)]
pub mod dacl;
pub mod derived;
pub mod health;
pub mod history;
pub mod history_sink;
pub mod maintenance;
pub mod profiles;
pub mod refs;
pub mod scan_history;
pub mod schema;
pub mod search;
pub mod stats;
pub mod sync;

#[cfg(windows)]
pub mod dpapi;

use crate::domain::error::{CoreError, ErrorCode};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 数据库句柄。由 GUI / CLI 启动时创建并显式传入服务层，**不使用全局单例**。
pub struct Db {
    conn: Connection,
    /// 打开时的文件路径（迁移前备份与健康面板需要）。
    path: PathBuf,
}

impl Db {
    /// 打开（或创建）普通 SQLite 库。
    ///
    /// 数据库不加密。旧版 SQLCipher 库的文件头不是
    /// SQLite magic，这里**直接拒绝**并提示用户删除，不做任何转换——
    /// 静默重建空库会掩盖历史丢失。
    pub fn open(path: &Path) -> Result<Self, CoreError> {
        if !has_sqlite_header(path)? {
            return Err(CoreError::new(
                ErrorCode::Database,
                "db_open",
                format!(
                    "检测到旧版加密数据库 {}（SQLCipher），当前版本不再支持。\
                     请删除或重命名该文件（连同同目录的 patheditor.db-wal、patheditor.db-shm）后重启。\
                     PATH 编辑不受影响；历史/同步/检索会从空库重建。",
                    path.display()
                ),
            ));
        }
        let conn = Connection::open(path).map_err(|e| db_err("open", e))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|e| db_err("pragma", e))?;
        Ok(Self {
            conn,
            path: path.to_path_buf(),
        })
    }

    /// 数据库文件路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 只读版本号。
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
        // 迁移前留一份可回退副本：事务只防「迁移中途失败」，
        // 防不了「迁移逻辑错误但成功提交」。备份失败则中止迁移（宁可不开机也不裸迁）。
        self.backup_before_migration(current)?;
        self.with_transaction(|tx| {
            // **事务内重读**：GUI 与 CLI 可能同时首次迁移/升级。`BEGIN IMMEDIATE`
            // 已经拿到写锁，此刻读到的值才是真正的起点；沿用事务外的值会让后到者
            // 按过期版本重跑 DDL（`table ... already exists`），库本身却是好的。
            let current: u32 = tx
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .map_err(|e| db_err("read_version", e))?;
            if current >= schema::CURRENT_SCHEMA_VERSION {
                return Ok(());
            }
            if current < 1 {
                tx.execute_batch(schema::DDL_V1)
                    .map_err(|e| db_err("ddl_v1", e))?;
                // 建库即写 app_state 单行。否则 `set_current_snapshot` 的
                // CAS 永远匹配 0 行、恒返回 false。
                tx.execute(
                    schema::SEED_APP_STATE_SQL,
                    rusqlite::params![uuid::Uuid::now_v7().to_string()],
                )
                .map_err(|e| db_err("seed_app_state", e))?;
            }
            // v2：派生表（scan_runs / var_references / daily_stats / search_fts）
            if current < 2 {
                tx.execute_batch(schema::DDL_V2)
                    .map_err(|e| db_err("ddl_v2", e))?;
            }
            // v3：同步表（changes / sync_conflicts）
            if current < 3 {
                tx.execute_batch(schema::DDL_V3)
                    .map_err(|e| db_err("ddl_v3", e))?;
            }
            // v4：冲突保留远端内容 + 查询索引
            if current < 4 {
                tx.execute_batch(schema::DDL_V4)
                    .map_err(|e| db_err("ddl_v4", e))?;
            }
            // v5：快照固定标记（保留策略的保护位）
            if current < 5 {
                tx.execute_batch(schema::DDL_V5)
                    .map_err(|e| db_err("ddl_v5", e))?;
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

    /// 迁移前把当前库复制为 `<文件名>.pre-v{旧版本}.bak`，最多保留 3 份。
    ///
    /// 用 SQLite Online Backup API 而不是文件复制：WAL 模式下直接拷主文件
    /// 可能拿到不含最新事务的半成品。
    fn backup_before_migration(&self, from_version: u32) -> Result<(), CoreError> {
        if from_version == 0 {
            // 全新库没有需要保护的数据
            return Ok(());
        }
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                CoreError::new(
                    ErrorCode::Migration,
                    "pre_migration_backup",
                    "数据库路径无文件名",
                )
            })?;
        let dest = self
            .path
            .with_file_name(format!("{file_name}.pre-v{from_version}.bak"));
        let tmp = dest.with_extension("bak.tmp");
        if tmp.exists() {
            let _ = std::fs::remove_file(&tmp);
        }
        {
            let mut dest_conn = Connection::open(&tmp).map_err(|e| db_err("backup_open", e))?;
            let backup = rusqlite::backup::Backup::new(&self.conn, &mut dest_conn)
                .map_err(|e| db_err("backup_init", e))?;
            backup
                .run_to_completion(64, Duration::from_millis(10), None)
                .map_err(|e| db_err("backup_copy", e))?;
        }
        std::fs::rename(&tmp, &dest).map_err(|e| {
            CoreError::new(
                ErrorCode::Migration,
                "pre_migration_backup",
                format!("写迁移备份失败: {e}"),
            )
        })?;
        self.rotate_migration_backups(file_name);
        Ok(())
    }

    /// 只保留最近的 3 份迁移前备份；删除失败只告警，不影响迁移。
    fn rotate_migration_backups(&self, file_name: &str) {
        let Some(dir) = self.path.parent() else {
            return;
        };
        let prefix = format!("{file_name}.pre-v");
        let mut backups: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with(&prefix) || !name.ends_with(".bak") {
                    return None;
                }
                let modified = entry
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                Some((modified, entry.path()))
            })
            .collect();
        backups.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
        for (_, path) in backups.into_iter().skip(3) {
            if let Err(e) = std::fs::remove_file(&path) {
                log::warn!("删除旧迁移备份失败 {}: {e}", path.display());
            }
        }
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

    /// 直接执行一段 SQL（`VACUUM` / `wal_checkpoint` 等不能在事务内跑的语句用）。
    pub fn execute_batch(&self, sql: &str) -> Result<(), CoreError> {
        self.conn
            .execute_batch(sql)
            .map_err(|e| db_err("execute_batch", e))
    }

    /// 预处理语句（只读列表查询用）。
    pub(crate) fn prepare(&self, sql: &str) -> Result<rusqlite::Statement<'_>, CoreError> {
        self.conn.prepare(sql).map_err(|e| db_err("prepare", e))
    }
}

/// 统一把 `rusqlite::Error` 包装为 `CoreError`。
///
/// 刻意写成**二元函数**而非返回闭包：`map_err` 需要一元 `FnOnce(E) -> F`，
/// 直接传 `db_err` 会类型不匹配。
pub(crate) fn db_err(op: &'static str, e: rusqlite::Error) -> CoreError {
    CoreError::new(ErrorCode::Database, op, format!("{op} 失败: {e}"))
}

/// 文件是否以 SQLite 的 16 字节 magic 开头。
///
/// 文件不存在或为空时返回 `true`（可以新建）；读失败按 IO 错误上抛。
/// 判定放在这里而不是靠 `rusqlite` 的首条语句报错，是为了能给旧版加密库
/// 一条可操作的提示，而不是混在「数据库损坏」里。
fn has_sqlite_header(path: &Path) -> Result<bool, CoreError> {
    use std::io::Read as _;
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(e) => {
            return Err(CoreError::new(
                ErrorCode::Io,
                "db_open",
                format!("读取数据库文件失败: {e}"),
            ))
        }
    };
    let mut buf = [0u8; 16];
    let n = f.read(&mut buf).map_err(|e| {
        CoreError::new(ErrorCode::Io, "db_open", format!("读取数据库文件失败: {e}"))
    })?;
    Ok(n == 0 || &buf[..n] == b"SQLite format 3\0")
}

/// 用默认路径打开（或创建）数据库。
///
/// 失败时返回错误 —— **GUI 据此进入降级模式**，CLI 对需要 DB 的命令返回 exit 1。
///
/// **不静默重建空库**：文件存在但不是 SQLite（旧版加密库）或已损坏时必须明确报错。
pub fn open_default() -> Result<Db, CoreError> {
    let path = crypto::data_home().join("patheditor.db");
    Db::open(&path)
}

/// 打开**已存在**的默认库；不存在时返回 `Ok(None)`。
///
/// 与 [`open_default`] 的区别：不存在时**不创建**空库。
/// CLI 的 PATH 命令据此决定「有 DB 就用、没有就降级」。
pub fn open_existing() -> Result<Option<Db>, CoreError> {
    let path = crypto::data_home().join("patheditor.db");
    if !path.exists() {
        return Ok(None);
    }
    open_default().map(Some)
}

/// 打开默认库并确保 schema 就绪（CLI 用）。
///
/// 与 GUI 的 `setup_database` 等价，但**不做 pending 补写**——那是 GUI 启动
/// 序列的职责，CLI 单次命令不该有这种副作用。
///
/// 失败返回 `Err`：CLI 对需要 DB 的命令应据此报错退出。
pub fn open_and_migrate() -> Result<Db, CoreError> {
    let mut db = open_default()?;
    db.migrate_to_current()?;
    Ok(db)
}
