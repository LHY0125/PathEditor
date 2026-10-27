//! 环境变量备份的采集、写入、轮换与列表。

use super::config::{env_backup_dir, env_backup_keep};
use crate::domain::env_var::{is_reserved, revision_of, EnvHive, EnvValueKind};
use crate::domain::error::{CoreError, ErrorCode};
use crate::platform::persist::{Versioned, PERSIST_SCHEMA_VERSION};
use crate::platform::registry::hive_location;
use crate::platform::registry::store::{EnvHiveStore, WinregHive};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use winreg::types::FromRegValue;

/// 备份中的单个环境变量条目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvBackupVar {
    /// 注册表返回的原始大小写变量名
    pub name: String,
    /// 注册表类型；备份中只出现 String / ExpandString
    pub kind: EnvValueKind,
    /// 完整明文值
    pub value: String,
    /// 采集时的并发校验摘要（FNV-1a 64，16 位十六进制）
    pub revision: String,
}

/// 按 hive 分组的备份内容。
///
/// 加 `rename_all = "camelCase"` 与 `EnvVarSnapshot`（`core/src/domain/env_var.rs:127`）
/// 及本文件的 `EnvBackupPayload` 保持风格一致；`system` / `user` 两个单字字段
/// 在此宏下不变形，无功能影响。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvBackupHives {
    #[serde(default)]
    pub system: Vec<EnvBackupVar>,
    #[serde(default)]
    pub user: Vec<EnvBackupVar>,
}

/// env 备份文件的载荷（由 `persist::Versioned` 信封包裹后落盘）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvBackupPayload {
    /// 采集时刻（Unix 毫秒），口径同 `EnvVarSnapshot.capturedAt`
    pub captured_at: i64,
    pub hives: EnvBackupHives,
}

/// 采集单个 hive 中全部可恢复的环境变量。
///
/// 排除两类变量：保留名（`Path` 由专用通路管理）与 `Unsupported` 类型
/// （本就不可写，备份了也无法通过通用通路恢复）。
///
/// **：`Unsupported` 必须「跳过该变量」，不得让整次采集失败。** 因此本函数
/// 用 `store.get_raw` + `from_reg_type` 自己判类型，而**不能**用 `read_env_var`
/// ——后者（`core/src/platform/registry/env_edit.rs:30-36`）对 `Unsupported` 直接返回
/// `Err`，配上调用方的 `?` 会让「机器上存在一个 `REG_DWORD` 变量」变成
/// 「每次写前备份都失败」。写法与 `list_env_vars_in_store`
/// （`core/src/platform/registry/env_edit.rs:116-199`）保持一致。
///
/// # Returns
/// - `Ok(Vec<EnvBackupVar>)` — 该 hive 的可恢复变量，含明文与 revision
/// - `Err(CoreError)` — 枚举失败（`Io`）或读取/解码失败（`Io`/`Parse`）；
///   **不含** `UnsupportedType`
pub(crate) fn collect_hive_vars_in_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
) -> Result<Vec<EnvBackupVar>, CoreError> {
    let (_, _, label) = hive_location(hive);
    let with_hive = |code: ErrorCode, msg: String| {
        let mut e = CoreError::new(code, "collect_env_backup", msg);
        e.hive = Some(hive);
        e
    };

    let names = store.enum_names().map_err(|e| {
        with_hive(
            e.code,
            format!("读取{}环境变量列表失败: {}", label, e.message),
        )
    })?;

    let mut vars = Vec::new();
    for name in names {
        if is_reserved(&name) {
            continue;
        }
        let raw = store.get_raw(&name).map_err(|e| {
            with_hive(
                e.code,
                format!("读取{}环境变量 {} 失败: {}", label, name, e.message),
            )
            .with_target(hive, &name)
        })?;
        let kind = EnvValueKind::from_reg_type(raw.vtype.clone());
        if !kind.is_writable() {
            continue; // Unsupported：跳过该变量，**不**使整次采集失败
        }
        let value = String::from_reg_value(&raw).map_err(|e| {
            with_hive(
                ErrorCode::Parse,
                format!("无法解码{}环境变量 {}: {}", label, name, e),
            )
            .with_target(hive, &name)
        })?;
        // 先算 revision 再移动 raw.vtype；`RegType` 非 Copy（winreg 0.52）。
        vars.push(EnvBackupVar {
            revision: revision_of(&name, raw.vtype, &value),
            name,
            kind,
            value,
        });
    }
    Ok(vars)
}

/// 采集两个 hive 的完整备份载荷。
///
/// # Returns
/// - `Ok(EnvBackupPayload)` — system 与 user 两个 hive 的可恢复变量
/// - `Err(CoreError)` — 任一 hive 采集失败即整体失败（沿用 fail-fast：
///   不产出「成功但不完整」的备份）
pub fn collect_env_backup() -> Result<EnvBackupPayload, CoreError> {
    let sys_store = WinregHive::open(EnvHive::System, false)?;
    let usr_store = WinregHive::open(EnvHive::User, false)?;
    let system = collect_hive_vars_in_store(&sys_store, EnvHive::System)?;
    let user = collect_hive_vars_in_store(&usr_store, EnvHive::User)?;
    Ok(EnvBackupPayload {
        captured_at: Local::now().timestamp_millis(),
        hives: EnvBackupHives { system, user },
    })
}

/// 把备份载荷写入目录，随后执行保留策略轮换。
///
/// 文件名为 `env_backup_<YYYYMMDD>_<HHMMSS>_<毫秒3位>.json`，时间戳格式与
/// PATH 备份一致，便于用户在同一目录中找到全部备份。
///
/// **顺序：先写新文件、再轮换。** 目录稳态恰好等于保留份数 `keep`：
/// `keep` 份 → 写入 → `keep+1` 份 → 轮换删 1 份 → `keep` 份。
/// 若反过来「先轮换、后写入」，`keep` 份时轮换无可删、写入后变成 `keep+1` 份，
/// 目录会长期稳定在 `keep+1` 份——与验收标准 11 的端到端口径不符。
///
/// **新文件必被保留**：`rotate_env_backups` 按文件名降序排列后从 `keep` 处开始
/// 删，丢弃的是字典序最小（最旧）的一端；刚写入的文件时间戳最大，永不落入删除集。
/// （例外：`keep = 0` 时用户明确要求零保留，新文件也会被删，见实现说明。）
///
/// # Returns
/// - `Ok(PathBuf)` — 写入的备份文件路径。是 `dir.join(...)` 的**原样结果**：
///   `dir` 是相对路径时返回值也是相对路径，**不保证绝对**（`backup_env_vars()`
///   传入的 `env_backup_dir()` 通常绝对，但可被 `PATHEDITOR_BACKUP_DIR` 设成
///   相对路径）。调用方若需要绝对路径须自行解析。
/// - `Err(CoreError)` — 目录创建、写文件或轮换失败（code=`Io`）；
///   另有一条序列化失败路径返回 code=`Internal`（正常载荷不会走到）。
///   **注意轮换失败这一路**：此时新备份文件**已经在磁盘上**，返回值只表示
///   「保留策略未能执行」，**不等于**备份未落盘。调用方措辞须按此理解。
pub fn write_env_backup_to(dir: &Path, payload: &EnvBackupPayload) -> Result<PathBuf, CoreError> {
    // 优先写 DPAPI 加密的 `.pebak`。只有拿不到密钥时（降级模式/
    // 密钥文件不可用）才回落明文 `.json` —— 那条路径**必须**留下明确日志，
    // 因为它是「磁盘上新增了一个明文敏感文件」。
    match crate::db::crypto::load_or_create_key(&crate::db::crypto::config_dir()) {
        Ok(material) => write_env_backup_to_with_key(dir, payload, &material),
        Err(e) => {
            log::warn!(
                "无法获取加密密钥（{}），本次回落为明文 .json 备份。\
                 这是降级路径，建议尽快修复密钥问题。",
                e.message
            );
            write_env_backup_to_with_keep(dir, payload, env_backup_keep())
        }
    }
}

/// 用**显式传入的密钥**写加密备份。
///
/// 这是给测试用的入口：生产 `write_env_backup_to` 从 `config_dir()` 取密钥，
/// 而测试无法安全地重定向那个目录（`dirs::home_dir()` 在 Windows 上读
/// `USERPROFILE`，设 `HOME` 无效）。
///
/// **不要**在测试里调用 `write_env_backup_to` 来验证 `.pebak` 行为 ——
/// 它会写出真实的 `~/.patheditor/keys/db.key.dpapi`。
pub fn write_env_backup_to_with_key(
    dir: &Path,
    payload: &EnvBackupPayload,
    material: &crate::db::crypto::KeyMaterial,
) -> Result<PathBuf, CoreError> {
    let path = crate::db::backup_index::write_env_backup_pebak(dir, payload, material)?;
    rotate_env_backups(dir, env_backup_keep())?;
    log::info!("env 备份已加密保存到: {}", path.display());
    Ok(path)
}

/// [`write_env_backup_to`] 的实现体，保留份数由调用方显式注入。
///
/// 拆出这一层是为了让测试能显式指定 `keep` 断言端到端稳态，而**不必**去写
/// 用户真实的 `~/.patheditor/config.ini`（`config_file_path()` 不受
/// `PATHEDITOR_BACKUP_DIR` 重定向）。生产路径只有 `write_env_backup_to` 一个入口。
pub(crate) fn write_env_backup_to_with_keep(
    dir: &Path,
    payload: &EnvBackupPayload,
    keep: usize,
) -> Result<PathBuf, CoreError> {
    std::fs::create_dir_all(dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "backup_env_vars",
            format!("无法创建备份目录 {}: {}", dir.display(), e),
        )
    })?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    let filepath = dir.join(format!("env_backup_{}.json", timestamp));

    let versioned = Versioned {
        schema_version: PERSIST_SCHEMA_VERSION,
        inner: payload.clone(),
    };
    let json = serde_json::to_string_pretty(&versioned).map_err(|e| {
        CoreError::new(
            ErrorCode::Internal,
            "backup_env_vars",
            format!("序列化备份失败: {e}"),
        )
    })?;

    std::fs::write(&filepath, json).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "backup_env_vars",
            format!("无法写入备份文件 {}: {}", filepath.display(), e),
        )
    })?;

    // 写入后才轮换，目录稳态恰为 keep 份（见 write_env_backup_to 的顺序说明）。
    rotate_env_backups(dir, keep)?;

    log::info!("env 备份已保存到: {}", filepath.display());
    Ok(filepath)
}

/// 保留策略轮换：只删除**本功能生成**的旧备份，保留最近 `keep` 份。
///
/// 授权范围（用户明确授权的例外）：
/// 1. 只匹配文件名 `env_backup_*.json`；
/// 2. 只在给定目录内操作（自定义备份目录不参与轮换）；
/// 3. 只删最旧的、超出 `keep` 之外的文件；
/// 4. 绝不删除 `.txt` PATH 备份、`.bak`、`.corrupt-*` 及目录中任何其他文件。
///
/// `keep = 0` 是合法输入，表示用户明确要求零保留，此时本函数会删空所有候选
/// 文件（含刚写入的那份）。`config.ini` 的 `env_backup_keep = 0` 即可触发。
///
/// # Returns
/// - `Ok(Vec<PathBuf>)` — 被删除的文件路径（供测试与日志核对）
/// - `Err(CoreError)` — 目录枚举失败（code=`Io`）；单个文件删除失败只记 warn，不影响其他
pub(crate) fn rotate_env_backups(dir: &Path, keep: usize) -> Result<Vec<PathBuf>, CoreError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    // 只收集本功能生成的备份：前缀 env_backup_ + 后缀 .json 或 .pebak，
    // `.bak` / `.corrupt-*` 因后缀不同天然被排除。
    // 新备份写 `.pebak`，遗留 `.json` 仍参与轮换（否则旧明文永不过期）。
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "rotate_env_backups",
            format!("枚举备份目录 {} 失败: {}", dir.display(), e),
        )
    })?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("env_backup_") {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with(".json") && !lower.ends_with(".pebak") {
            continue;
        }
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        candidates.push((name, path));
    }

    if candidates.len() <= keep {
        return Ok(Vec::new());
    }

    // 文件名内嵌时间戳，字典序即时间序；降序排列后丢弃最旧的。
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    let mut removed = Vec::new();
    for (_, path) in candidates.into_iter().skip(keep) {
        match std::fs::remove_file(&path) {
            Ok(()) => {
                log::info!("已轮换删除旧备份: {}", path.display());
                removed.push(path);
            }
            // 删除失败不中止轮换：留着旧文件无害，报错反而阻断后续写入。
            Err(e) => log::warn!("删除旧备份 {} 失败: {}", path.display(), e),
        }
    }
    Ok(removed)
}

/// 备份文件列表项（不含内容）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvBackupInfo {
    /// 文件名
    pub file: String,
    /// 条目路径（`list_env_backups_in` 收的目录拼接文件名）。
    ///
    /// **不保证是绝对路径**：入参目录是相对路径时这里也是相对路径。
    /// 默认目录通常绝对（`~/.patheditor/backups/`），但 `PATHEDITOR_BACKUP_DIR`
    /// 若被设成相对路径则不是。调用方需绝对路径时须自行解析。
    pub path: String,
    /// 文件名去掉 `env_backup_` 前缀与扩展名（`.json` / `.pebak`）后的中缀。
    ///
    /// 本工具自己生成的文件形如 `env_backup_<YYYYMMDD>_<HHMMSS>_<毫秒3位>.json`，
    /// 此时中缀即时间戳；但本字段只做前后缀裁剪、**不校验格式**——对
    /// `env_backup_x.json` 这类命名合规但非本工具产物的文件，得到的是任意中缀
    /// （如 `"x"`）。调用方**不得**假定它一定是 `YYYYMMDD_HHMMSS_mmm`。
    /// 排序按中缀字典序，对上面的规范命名等同于时间序。
    pub timestamp: String,
    /// 文件字节数
    pub size_bytes: u64,
    /// 备份中的变量条数。
    ///
    /// - `.pebak`：从**文件头**读出；
    /// - `.json`：**恒为 0**——列表不解析内容，一旦解析，
    ///   单个损坏文件就会让列表整体失败。
    pub variable_count: u64,
    /// 是否为 DPAPI 加密的 `.pebak`。
    pub encrypted: bool,
    /// 文件格式：`"pebak"` / `"json"`。
    pub format: String,
}

/// 备份文件大小上限：正常备份几十 KB，超过 1 MiB 说明不是本工具产物。
const MAX_BACKUP_FILE_BYTES: u64 = 1024 * 1024;

/// 列出目录中的 env 备份，按时间倒序。**只枚举目录与 stat，不解析内容**，
/// 因此单个损坏文件不会让列表整体失败。
///
/// # Returns
/// - `Ok(Vec<EnvBackupInfo>)` — 备份列表，最新在前
/// - `Err(CoreError)` — 目录枚举失败（code=`Io`）；目录不存在时返回空列表
///
/// 单条 `stat` 失败的条目会被**静默跳过**（不报错、不记 warn）：列表的韧性优先，
/// 一个读取不到的条目不应让整份列表失败。
pub fn list_env_backups_in(dir: &Path) -> Result<Vec<EnvBackupInfo>, CoreError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "list_env_backups",
            format!("枚举备份目录 {} 失败: {}", dir.display(), e),
        )
    })?;

    let mut infos = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // 列表同时覆盖 `.pebak`（新）与 `.json`
        let lower = name.to_ascii_lowercase();
        if !name.starts_with("env_backup_") {
            continue;
        }
        let (encrypted, ext_len) = if lower.ends_with(".pebak") {
            (true, ".pebak".len())
        } else if lower.ends_with(".json") {
            (false, ".json".len())
        } else {
            continue;
        };
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        // 中缀 = 去掉前缀与后缀后的部分（时间戳）
        let stem = &name[..name.len() - ext_len];
        let timestamp = stem.trim_start_matches("env_backup_").to_string();
        // `.pebak` 的变量数从**文件头**读；`.json` 仍不解析。
        let variable_count = if encrypted {
            crate::db::backup_index::read_pebak_header(&path)
                .map(|h| h.variable_count as u64)
                .unwrap_or(0)
        } else {
            0
        };
        infos.push(EnvBackupInfo {
            file: name,
            path: path.to_string_lossy().into_owned(),
            timestamp,
            size_bytes: meta.len(),
            variable_count,
            encrypted,
            format: if encrypted { "pebak" } else { "json" }.to_string(),
        });
    }

    infos.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(infos)
}

/// 列出默认备份目录中的 env 备份。
///
/// # Returns
/// - `Ok(Vec<EnvBackupInfo>)` — 备份列表，最新在前
/// - `Err(CoreError)` — 目录枚举失败
pub fn list_env_backups() -> Result<Vec<EnvBackupInfo>, CoreError> {
    list_env_backups_in(&env_backup_dir())
}

/// 校验用户给定的备份文件路径。
///
/// 规则：扩展名必须为 `.json`；必须位于默认备份目录之内，**或**文件名以
/// `env_backup_` 开头；文件必须存在且不超过 1 MiB。
///
/// # Returns
/// - `Ok(PathBuf)` — 校验通过的路径，**原样返回**（不做 canonicalize / 绝对化）：
///   入参是相对路径时返回值仍是相对路径。调用方若需要绝对路径须自行解析；
///   校验与后续读取之间 cwd 变化导致的 TOCTOU 属**已知未覆盖项**
///   （单线程 CLI 下为理论问题，本波不解决）。
/// - `Err(CoreError)` — 路径非法（code=`InvalidValue`）或文件不存在/过大（code=`NotFound`/`InvalidValue`）
pub fn validate_backup_path(path: &str) -> Result<PathBuf, CoreError> {
    let p = PathBuf::from(path);
    let file_name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| {
            CoreError::new(
                ErrorCode::InvalidValue,
                "restore_env_backup",
                format!("备份路径非法: {path}"),
            )
        })?;

    // 允许 `.pebak`（DPAPI 加密）与遗留 `.json`（明文）。
    if p.extension()
        .map(|e| e != "json" && e != "pebak")
        .unwrap_or(true)
    {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "restore_env_backup",
            format!("备份文件必须是 .json 或 .pebak: {path}"),
        ));
    }

    let in_backup_dir = p
        .parent()
        .map(|parent| parent == env_backup_dir())
        .unwrap_or(false);
    if !in_backup_dir && !file_name.starts_with("env_backup_") {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "restore_env_backup",
            format!("备份文件必须位于备份目录内或以 env_backup_ 开头: {path}"),
        ));
    }

    let meta = std::fs::metadata(&p).map_err(|e| {
        CoreError::new(
            ErrorCode::NotFound,
            "restore_env_backup",
            format!("无法读取备份文件 {path}: {e}"),
        )
    })?;
    if meta.len() > MAX_BACKUP_FILE_BYTES {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "restore_env_backup",
            format!(
                "备份文件过大（{} 字节，上限 {}），不是本工具生成的备份",
                meta.len(),
                MAX_BACKUP_FILE_BYTES
            ),
        ));
    }
    Ok(p)
}

/// 一次写操作前的备份结果。
///
/// 备份是 **best-effort**：失败不使写入失败，但必须被调用方看见 ——
/// 因此写入口把它作为返回值的一部分（`WriteOutcome`），而不是只记一条日志
/// （CLI 未初始化 logger，core 的 `log::warn!` 在 CLI 下会被丢弃）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupOutcome {
    /// 备份已写入，携带文件路径
    Created(PathBuf),
    /// 本次操作无需备份
    Skipped,
    /// 备份失败（不阻断写入），携带原因
    Failed(String),
}

/// 采集两个 hive 并落盘一份 env 备份（公开入口，供 CLI `env backup` 与写前自动备份使用）。
///
/// # Returns
/// - `Ok(PathBuf)` — 备份文件路径。**不保证绝对**：由 `env_backup_dir()` 决定，
///   该目录可被 `PATHEDITOR_BACKUP_DIR` 设成相对路径。需要绝对路径须自行解析。
/// - `Err(CoreError)` — 采集或落盘失败
pub fn backup_env_vars() -> Result<PathBuf, CoreError> {
    backup_env_vars_in(&env_backup_dir())
}

/// [`backup_env_vars`] 的显式目录版本（测试注入 `TempDir`）。
///
/// 备份目录由调用方给出，因此不读取 `PATHEDITOR_BACKUP_DIR`；
/// 保留份数仍按 `config.ini` 的 `env_backup_keep`（或默认值）决定。
pub fn backup_env_vars_in(dir: &Path) -> Result<PathBuf, CoreError> {
    let payload = collect_env_backup()?;
    write_env_backup_to(dir, &payload)
}
