use crate::env_var::{is_reserved, revision_of, EnvHive, EnvValueKind};
use crate::error::{CoreError, ErrorCode};
use crate::persist::{Versioned, PERSIST_SCHEMA_VERSION};
use crate::reg_store::{EnvHiveStore, WinregHive};
// 经 registry 根 re-export —— 不能写 crate::registry::env_var::X（E0603）。
// 三个 `*_in_store` 是恢复执行的写入口：保护名单 / 类型 / 名称合法性判定
// 只在它们内部实现一处（设计文档 §S6），恢复层不得复制判定规则。
use crate::registry::{
    self, create_env_var_in_store, delete_env_var_force_in_store, hive_location,
    update_env_var_force_in_store, SYS_REG_PATH, USER_REG_PATH,
};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::types::FromRegValue;

/// env 备份默认保留份数（设计文档 D6）。
///
/// 全量快照单文件约几十 KB；20 份在正常使用下是几百 KB 量级。
/// 写操作密集的用户可调大——注意 README 已提示备份含明文敏感值，
/// 保留份数越大暴露面越大。
pub const ENV_BACKUP_KEEP: usize = 20;

/// 配置文件路径：`~/.patheditor/config.ini`。
///
/// 放用户目录而非 exe 同目录（设计文档 §配置文件）：CLI/GUI 是两份独立 exe，
/// scoop 升级换目录、NSIS 装 Program Files 需提权——只有用户目录能让双 exe
/// 共享、升级不丢、免提权。
fn config_file_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("config.ini")
}

/// 判断「读配置失败」是否应当 `log::warn!` 提示，而不是静默回落。
///
/// 口径（设计文档 §解析与回落行为 表格把两种情形分开列）：
/// - `NotFound` —— 文件不存在是首次使用的正常情形，**静默**回落（且不得创建文件）；
/// - 其它（`PermissionDenied`、路径指向目录、I/O 错误等）—— 用户可修复的异常，
///   **必须 warn**，否则配置写坏了在 CLI 下完全不可见（spec §K2 已实证 CLI 未初始化
///   logger，core 的 warn 会被丢弃，但仍须按 GUI/服务侧能收到的口径发出）。
fn should_warn_on_read_error(e: &std::io::Error) -> bool {
    e.kind() != std::io::ErrorKind::NotFound
}

/// 读取 env 备份保留份数；任何异常一律回落 [`ENV_BACKUP_KEEP`]。
///
/// 回落情形（设计文档 §解析与回落行为）：文件不存在 / 键不存在 / 值非整数 /
/// 值为负数 / 值空 / 文件不可读。**不报错、不中止备份**。
/// 读取**无副作用**：文件不存在时不创建。
///
/// 落日志口径：值非法与文件不可读**各记一次 `log::warn!`**；
/// 文件不存在是首次使用的正常情形，**静默回落**（见 [`should_warn_on_read_error`]）。
///
/// 手写极简 INI 解析（`key = value`，`;` 或 `#` 起始为注释），不引入新依赖——
/// 单键配置不值得拉一个 crate，与项目「手写 FNV-1a 而不引 sha2」的先例一致。
fn read_env_backup_keep(path: &Path) -> usize {
    let fallback = ENV_BACKUP_KEEP;
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            // 不存在 → 静默；不可读 → 必须可见（两种情形不得合并处理）。
            if should_warn_on_read_error(&e) {
                log::warn!(
                    "config.ini 不可读（{}: {}），回落默认 {}",
                    path.display(),
                    e,
                    fallback
                );
            }
            return fallback;
        }
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "env_backup_keep" {
            continue;
        }
        match value.trim().parse::<usize>() {
            Ok(n) => return n,
            Err(_) => {
                log::warn!(
                    "config.ini 的 env_backup_keep 值非法（{}），回落默认 {}",
                    value.trim(),
                    fallback
                );
                return fallback;
            }
        }
    }
    fallback
}

/// 从真实配置文件读取保留份数。
fn env_backup_keep() -> usize {
    read_env_backup_keep(&config_file_path())
}

/// 备份根目录。`PATHEDITOR_BACKUP_DIR` 存在时用它（测试隔离用），
/// 否则回落 `~/.patheditor/backups/`。
fn backup_base_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("PATHEDITOR_BACKUP_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("backups")
}

/// env 备份目录的公开访问器。
pub fn env_backup_dir() -> PathBuf {
    backup_base_dir()
}

/// 获取备份目录路径
pub fn get_appdata_dir() -> String {
    backup_base_dir().to_string_lossy().to_string()
}

/// 备份中的单个环境变量条目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvBackupVar {
    /// 注册表返回的原始大小写变量名
    pub name: String,
    /// 注册表类型；备份中只出现 String / ExpandString
    pub kind: EnvValueKind,
    /// 完整明文值（设计文档 D2：备份的价值在于能还原，故不打码）
    pub value: String,
    /// 采集时的并发校验摘要（FNV-1a 64，16 位十六进制）
    pub revision: String,
}

/// 按 hive 分组的备份内容。
///
/// 加 `rename_all = "camelCase"` 与 `EnvVarSnapshot`（`core/src/env_var.rs:127`）
/// 及本文件的 `EnvBackupPayload` 保持风格一致；`system` / `user` 两个单字字段
/// 在此宏下不变形，无功能影响（核对轮一致性观察）。
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
/// **B1：`Unsupported` 必须「跳过该变量」，不得让整次采集失败。** 因此本函数
/// 用 `store.get_raw` + `from_reg_type` 自己判类型，而**不能**用 `read_env_var`
/// ——后者（`core/src/registry/env_var.rs:30-36`）对 `Unsupported` 直接返回
/// `Err`，配上调用方的 `?` 会让「机器上存在一个 `REG_DWORD` 变量」变成
/// 「每次写前备份都失败」。写法与 `list_env_vars_in_store`
/// （`core/src/registry/env_var.rs:116-199`）保持一致。
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
            ErrorCode::Io,
            format!("读取{}环境变量列表失败: {}", label, e),
        )
    })?;

    let mut vars = Vec::new();
    for name in names {
        if is_reserved(&name) {
            continue;
        }
        let raw = store.get_raw(&name).map_err(|e| {
            with_hive(
                ErrorCode::Io,
                format!("读取{}环境变量 {} 失败: {}", label, name, e),
            )
            .with_target(hive, &name)
        })?;
        let kind = EnvValueKind::from_reg_type(raw.vtype.clone());
        if !kind.is_writable() {
            continue; // Unsupported：跳过该变量，**不**使整次采集失败（B1）
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
/// - `Err(CoreError)` — 任一 hive 采集失败即整体失败（沿用 F-04 的 fail-fast：
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
/// 目录会长期稳定在 `keep+1` 份——与 spec 验收标准 11 的端到端口径不符。
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
    write_env_backup_to_with_keep(dir, payload, env_backup_keep())
}

/// [`write_env_backup_to`] 的实现体，保留份数由调用方显式注入。
///
/// 拆出这一层是为了让测试能显式指定 `keep` 断言端到端稳态，而**不必**去写
/// 用户真实的 `~/.patheditor/config.ini`（`config_file_path()` 不受
/// `PATHEDITOR_BACKUP_DIR` 重定向）。生产路径只有 `write_env_backup_to` 一个入口。
fn write_env_backup_to_with_keep(
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
/// 授权范围（设计文档 §S1，用户 2026-09-21 明确授权的例外）：
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
fn rotate_env_backups(dir: &Path, keep: usize) -> Result<Vec<PathBuf>, CoreError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    // 只收集本功能生成的备份：前缀 env_backup_ + 后缀 .json，
    // `.bak` / `.corrupt-*` 因后缀不同天然被排除。
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
        if !name.starts_with("env_backup_") || !name.ends_with(".json") {
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

/// 备份文件列表项（不含内容，见设计文档 §S5）。
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
    /// 文件名去掉 `env_backup_` 前缀与 `.json` 后缀后的中缀。
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
    /// **恒为 0：列表不解析内容（S5 / J4 裁断）** —— 变量数只能靠解析 JSON 得到，
    /// 一旦解析，单个损坏文件就会让列表整体失败。字段保留是为了保持契约稳定，
    /// 调用方**不得**把它当作真实计数使用。
    pub variable_count: u64,
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
        if !name.starts_with("env_backup_") || !name.ends_with(".json") {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let timestamp = name
            .trim_start_matches("env_backup_")
            .trim_end_matches(".json")
            .to_string();
        infos.push(EnvBackupInfo {
            file: name,
            path: path.to_string_lossy().into_owned(),
            timestamp,
            size_bytes: meta.len(),
            variable_count: 0, // 不解析内容：见 S5
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

/// 校验用户给定的备份文件路径（设计文档 §S4）。
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

    if p.extension().map(|e| e != "json").unwrap_or(true) {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "restore_env_backup",
            format!("备份文件必须是 .json: {path}"),
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
/// 备份是 **best-effort**：失败不使写入失败（设计文档 K2），但必须被调用方看见 ——
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
    let payload = collect_env_backup()?;
    let dir = env_backup_dir();
    write_env_backup_to(&dir, &payload)
}

/// 恢复差异的类型。
///
/// 当前 [`diff_one_hive`] 只产出 [`RestoreChangeKind::Added`] /
/// [`RestoreChangeKind::Removed`] / [`RestoreChangeKind::Conflict`] 三种；
/// [`RestoreChangeKind::Modified`] 的不可达原因见该变体自身的说明。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RestoreChangeKind {
    /// 备份中有、注册表中没有 → 将新建
    Added,
    /// 两边都有但值不同 → 将覆盖
    ///
    /// **当前 [`diff_one_hive`] 不产出本变体**，因此 `RestorePreview.modified` 恒为 0：
    /// `revision_of` 是对 `name + vtype + value` 取值的纯函数，同名同类型下
    /// revision 不同**当且仅当**值不同 —— 「值变了」与「备份已过期」在差异计算里
    /// 是**同一个条件**，后者按行为契约一律判为 [`RestoreChangeKind::Conflict`]
    /// （设计文档 K3：revision 不一致即冲突）。
    ///
    /// 保留本变体是为了维持 spec 声明的类型形状，以及恢复执行侧对
    /// `Modified | Conflict` 的合并处理；不要为消除「未使用变体」而删除它。
    Modified,
    /// 注册表中有、备份中没有 → 将删除（最不可逆）
    Removed,
    /// 备份中的 revision 与注册表当前值不符 → 备份后被外部修改
    Conflict,
}

/// 单条恢复差异。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreChange {
    /// 该差异所属的 hive
    pub hive: EnvHive,
    /// 变量名，**保留来源侧的原始大小写**：`Added` / `Conflict` 用备份中的名字，
    /// `Removed` 用注册表枚举返回的名字（见行为契约第 5、6 条）
    pub name: String,
    /// 差异类型
    pub kind: RestoreChangeKind,
}

/// 恢复差异预览（供 CLI `--dry-run` 与 GUI 确认弹窗消费）。
///
/// 四个计数都是 `changes` 的按类型计数，因此
/// `added + modified + removed + conflicts == changes.len()` 恒成立。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    /// 全部差异。**已排序**，顺序固定为：先 user hive、后 system hive；
    /// 同一 hive 内按 `added → modified → removed → conflict`，再按变量名
    /// （忽略大小写）升序。等价输入必须产出等价顺序 —— 这是
    /// `--dry-run --json` 可复现的前提，排序规则见 [`sort_changes`]。
    pub changes: Vec<RestoreChange>,
    /// 新增数量
    pub added: usize,
    /// 修改数量。**恒为 0**：见 [`RestoreChangeKind::Modified`]
    pub modified: usize,
    /// 删除数量（最不可逆的一部分）
    pub removed: usize,
    /// 冲突数量（备份后被外部修改）
    pub conflicts: usize,
}

/// 读取并校验备份文件。
///
/// # Returns
/// - `Ok(EnvBackupPayload)` — 校验通过的内容
/// - `Err(CoreError)` — 读取失败（code=`Io`）、解析失败（code=`Parse`，
///   损坏文件会被 `persist` 隔离为 `<file>.corrupt-<ts>`）、版本过高（code=`Parse`）
///
/// **不做路径来源校验**（扩展名 / 是否在备份目录内）：那是
/// [`validate_backup_path`] 的职责。本函数只负责「读出并验版本」。
pub fn read_env_backup(path: &Path) -> Result<EnvBackupPayload, CoreError> {
    let versioned = crate::persist::read_versioned_file::<EnvBackupPayload>(path, "env 备份")?;
    crate::persist::migrate(versioned, "env 备份")
}

/// `changes` 的 hive 排序键：**user 在前、system 在后**，与
/// [`RestorePreview::changes`] 声明的顺序一致。
///
/// 刻意不 `derive(Ord)` 到 [`EnvHive`] 上：枚举声明序是 `System` 在前，
/// 用派生序会让「user 先于 system」的既有契约被静默反转。排序键在这里显式写出。
fn hive_rank(hive: EnvHive) -> u8 {
    match hive {
        EnvHive::User => 0,
        EnvHive::System => 1,
    }
}

/// `changes` 的差异类型排序键：新增 → 修改 → 删除 → 冲突。
///
/// 前三者按「实际会写入的动作」排：新增最轻、删除最不可逆，故删除靠后便于阅读时
/// 落在末尾附近。**冲突排在最后**（不是删除）—— 它是唯一可能让整次恢复中止的类别，
/// 放在末尾便于在 `--dry-run --json` 输出里一眼扫到「为什么没写成」。
/// 顺序本身无安全含义，只要是**确定的**即可（可复现是这些键的全部目的）。
fn kind_rank(kind: RestoreChangeKind) -> u8 {
    match kind {
        RestoreChangeKind::Added => 0,
        RestoreChangeKind::Modified => 1,
        RestoreChangeKind::Removed => 2,
        RestoreChangeKind::Conflict => 3,
    }
}

/// 差异列表的可复现排序（原地）。
///
/// **为什么必须排序**：`diff_one_hive` 的 `Removed` 批次由 `HashMap` 迭代产出，
/// `HashMap` 默认用 `RandomState`，因此**同一份注册表在同一台机器上，两次运行
/// 会得到不同的 `Removed` 顺序** —— `env restore --dry-run --json` 原样打印
/// `changes`，于是同一输入产出不同 JSON（用户可见的不可复现，Task 5 复审 Minor）。
///
/// **排序键：`(hive, kind, name)`**，其中 `name` 一律**小写化后**比较
/// （`to_ascii_lowercase`）：`diff_one_hive` 已按大小写不敏感语义把变量名配成
/// 「同一个」，用原始大小写作键会让 `Windir` 与 `windir` 这类同义名落在不同区间。
/// 小写键相同时再比原始名 —— 同 hive 同 kind 下小写键相同的两个条目属于同一变量，
/// 本不会同时出现；这一层只是让比较成为严格全序，结果不依赖比较器的调用顺序。
///
/// **排序不改变任何计数**：`added` / `modified` / `removed` / `conflicts` 都在排序后
/// 按遍历计数，与顺序无关（已完成排序后再计数，见 [`preview_restore_in_stores`]）。
fn sort_changes(changes: &mut [RestoreChange]) {
    changes.sort_by(|a, b| {
        hive_rank(a.hive)
            .cmp(&hive_rank(b.hive))
            .then_with(|| kind_rank(a.kind).cmp(&kind_rank(b.kind)))
            .then_with(|| {
                a.name
                    .to_ascii_lowercase()
                    .cmp(&b.name.to_ascii_lowercase())
            })
            .then_with(|| a.name.cmp(&b.name))
    });
}

/// 计算备份相对当前注册表的差异（不写入任何内容）。**存储可注入版本，仅 core 内部用。**
///
/// revision 比对口径与写通路一致：备份记录的 revision 与注册表当前值的 revision
/// 不同即说明备份之后被改动过（设计文档 K3），记 `Conflict`。
///
/// **`changes` 已排序**（见 [`sort_changes`]）：user hive 在前、system 在后，
/// 同 hive 内按差异类型再按变量名（忽略大小写）稳定排序。排序不计入任何计数，
/// 只影响顺序 —— 因此 `--dry-run --json` 的输出对同一注册表可复现。
///
/// # Returns
/// - `Ok(RestorePreview)` — 差异摘要
/// - `Err(CoreError)` — 枚举任一 hive 的环境变量失败（code=`Io`）
pub(crate) fn preview_restore_in_stores(
    sys: &dyn EnvHiveStore,
    usr: &dyn EnvHiveStore,
    payload: &EnvBackupPayload,
) -> Result<RestorePreview, CoreError> {
    let mut changes = Vec::new();
    diff_one_hive(usr, EnvHive::User, &payload.hives.user, &mut changes)?;
    diff_one_hive(sys, EnvHive::System, &payload.hives.system, &mut changes)?;

    sort_changes(&mut changes);

    let added = changes
        .iter()
        .filter(|c| c.kind == RestoreChangeKind::Added)
        .count();
    let modified = changes
        .iter()
        .filter(|c| c.kind == RestoreChangeKind::Modified)
        .count();
    let removed = changes
        .iter()
        .filter(|c| c.kind == RestoreChangeKind::Removed)
        .count();
    let conflicts = changes
        .iter()
        .filter(|c| c.kind == RestoreChangeKind::Conflict)
        .count();

    Ok(RestorePreview {
        changes,
        added,
        modified,
        removed,
        conflicts,
    })
}

/// 计算单个 hive 的差异，追加到 `out`。
///
/// **行为契约（每条都必须成立）**：
/// 1. 当前注册表中的**保留名**（`Path`，用 [`is_reserved`] 判定）与 **`Unsupported`
///    类型**不参与差异计算 —— 既不产生 `Removed`，也不产生其他任何变体；
/// 2. 备份中有、当前无 → `Added`；
/// 3. 备份的 revision 与当前值的 revision **相同** → 不进差异（无变化）；
/// 4. 备份的 revision 与当前**不同** → `Conflict`；
/// 5. 当前有、备份无 → `Removed`，且 `Removed` 条目必须携带**注册表返回的原始大小写
///    名字**（不能是小写化的比较键）；
/// 6. 变量名比较**忽略大小写**（Windows 注册表语义），但输出保留原始大小写。
///
/// **不判定保护名单**：保护名单变量照常出现在差异中，拒绝发生在恢复执行的写函数内
/// （保护名单在 core 写函数中是唯一真相源，此处再判一次等于双份规则）。
/// 代价是差异列表可能含实际写不进去的变量 —— 由恢复结果如实报告，比提前隐藏更诚实。
///
/// **读取失败的条目被跳过**：枚举成功但单条 `get_raw` 失败（例如枚举与读取之间
/// 该值被其他进程删除）时，该变量既不算无变化也不算 `Removed`，而是完全不进差异。
/// 这是刻意的韧性取舍：单条读取失败不应让整份预览失败；而它若同时存在于备份中，
/// 会因 `current` 里没有对应键而落 `Added`，由执行阶段的「同名已存在」如实报错。
///
/// # Returns
/// - `Ok(())` — 差异已追加到 `out`
/// - `Err(CoreError)` — 枚举该 hive 的环境变量失败（code=`Io`）
fn diff_one_hive(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    backup_vars: &[EnvBackupVar],
    out: &mut Vec<RestoreChange>,
) -> Result<(), CoreError> {
    let (_, _, label) = hive_location(hive);

    // 当前可写变量表：小写比较键 → (注册表原始名, revision)。
    // 原始名用于 `Removed` 差异的展示（注册表返回的大小写不保证与写入时一致），
    // 小写键用于契约第 6 条的大小写不敏感比较。
    let mut current: HashMap<String, (String, String)> = HashMap::new();
    let names = store.enum_names().map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "preview_restore",
            format!("读取{}环境变量列表失败: {}", label, e),
        )
        .with_hive(hive)
    })?;
    for name in names {
        if is_reserved(&name) {
            continue; // 契约第 1 条：保留名由专用 PATH 通路管理
        }
        let Ok(raw) = store.get_raw(&name) else {
            continue; // 单条读取失败：跳过，不让整份预览失败
        };
        // 契约第 1 条：Unsupported 既不恢复，也不构成删除差异。
        if !EnvValueKind::from_reg_type(raw.vtype.clone()).is_writable() {
            continue;
        }
        let Ok(value) = String::from_reg_value(&raw) else {
            continue; // 解码失败同样跳过，理由同上
        };
        let revision = revision_of(&name, raw.vtype, &value);
        current.insert(name.to_ascii_lowercase(), (name, revision));
    }

    // 契约第 2、3、4、6 条：遍历备份侧，用大小写不敏感的比较键查当前值。
    let mut seen: HashSet<String> = HashSet::new();
    for var in backup_vars {
        let key = var.name.to_ascii_lowercase();
        seen.insert(key.clone());
        match current.get(&key) {
            // 备份有、当前无 → 新增（用备份中的原始名）
            None => out.push(RestoreChange {
                hive,
                name: var.name.clone(),
                kind: RestoreChangeKind::Added,
            }),
            // revision 相同 → 值相同，无变化，不进差异
            Some((_, current_revision)) if current_revision == &var.revision => {}
            // revision 不同 → 备份后已被外部修改
            Some(_) => out.push(RestoreChange {
                hive,
                name: var.name.clone(),
                kind: RestoreChangeKind::Conflict,
            }),
        }
    }

    // 契约第 5 条：当前有、备份无 → 删除，且用注册表返回的原始名（不是小写比较键）。
    for (key, (original_name, _)) in &current {
        if seen.contains(key) {
            continue;
        }
        out.push(RestoreChange {
            hive,
            name: original_name.clone(),
            kind: RestoreChangeKind::Removed,
        });
    }

    Ok(())
}

/// 计算备份相对当前注册表的差异（不写入任何内容）。
///
/// **不含 force 参数** —— force 只在执行层影响「冲突是否中止」，
/// 差异计算本身与 force 无关（核对轮 E3 裁断）。
///
/// 注册表按**只读**方式打开（`WinregHive::open(hive, false)`），本函数不写入任何内容。
///
/// # Returns
/// - `Ok(RestorePreview)` — 差异摘要
/// - `Err(CoreError)` — 打开注册表键失败（code=`Io`/`PermissionDenied`）或枚举失败（code=`Io`）
pub fn preview_restore(payload: &EnvBackupPayload) -> Result<RestorePreview, CoreError> {
    let sys = WinregHive::open(EnvHive::System, false)?;
    let usr = WinregHive::open(EnvHive::User, false)?;
    preview_restore_in_stores(&sys, &usr, payload)
}

/// 从文件读取备份后计算差异（GUI 确认弹窗用）。
///
/// # Returns
/// - `Ok(RestorePreview)` — 差异摘要
/// - `Err(CoreError)` — 读取/解析备份失败（见 [`read_env_backup`]），
///   或打开注册表键/枚举失败（见 [`preview_restore`]）
pub fn preview_restore_file(path: &Path) -> Result<RestorePreview, CoreError> {
    let payload = read_env_backup(path)?;
    preview_restore(&payload)
}

/// 恢复执行的结果。单变量失败**不中止**整体（best-effort），但必须逐条记录。
///
/// serde 用 camelCase（与 `RestorePreview` / `EnvVarSnapshot` 一致），经 Tauri IPC
/// 出境时前端拿到 `{ applied, skipped, failures }`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    /// 成功写入的变量数（新增 + 修改 + 删除）
    pub applied: usize,
    /// 被跳过、既未写也未计入 `failures` 的变量数。
    ///
    /// **恒为 0**：当前没有任何差异类型会走到「跳过」——`Conflict` 在默认模式下
    /// 中止整次恢复（不进这里），在 `--force` 下按 `Modified` 覆盖写入；
    /// 其余三类都必然尝试写入。保留该字段是因为它的形状由 spec 与 Task 7/8 的
    /// 消费端（CLI `--json`、GUI 命令）共同约定，删掉会破坏跨任务契约。
    /// **不要为消除「恒零字段」而删除它。**
    pub skipped: usize,
    /// 逐条失败原因，格式 `[{hive标签}] {变量名}: {错误文本}`；空表示全部成功。
    ///
    /// 这里承载的是**用户可读的展示文本**；程序分支若需判定失败类型，
    /// 必须回到 `CoreError.code`（项目硬约束：不得匹配 message 文本）。
    pub failures: Vec<String>,
}

/// 在备份载荷中按 hive 与变量名查找条目（**忽略大小写**）。
///
/// 忽略大小写与 [`diff_one_hive`] 的配对语义一致：差异条目里的名字可能来自
/// 备份侧（`Added` / `Modified` / `Conflict`）也可能来自注册表侧（`Removed`），
/// 而两侧名字大小写不保证相同。`Removed` 不需要查备份侧，故无匹配也正常；
/// 其余三类若查不到，说明差异列表与载荷不自洽 —— 由调用方记为失败，不静默跳过。
fn find_backup_var<'a>(
    payload: &'a EnvBackupPayload,
    hive: EnvHive,
    name: &str,
) -> Option<&'a EnvBackupVar> {
    let vars = match hive {
        EnvHive::System => &payload.hives.system,
        EnvHive::User => &payload.hives.user,
    };
    vars.iter().find(|v| v.name.eq_ignore_ascii_case(name))
}

/// 执行恢复：逐变量写入，存储与备份文件路径均可注入（供测试）。
///
/// # 写入通路（设计文档 §S6，**不得改动**）
///
/// 恢复**不使用** `EnvHiveStore::set_raw` 直接写裸值 —— 那会绕过保护名单与类型
/// 判定。必须按差异类型逐变量调用：
/// - `Added` → [`create_env_var_in_store`]
/// - `Modified` / `Conflict` → [`update_env_var_force_in_store`]
/// - `Removed` → [`delete_env_var_force_in_store`]
///
/// 也就是说，**保护名单 / 保留名 / 类型可写性 / 名称与值合法性全部由这三个
/// core 写函数判定**，本函数不做任何第二次判定（判定只在 core 一处）。
/// `--force` 只豁免 revision 校验（`*_force_*` 的既有语义），**不豁免**上述判定。
///
/// # 冲突语义（设计文档 K3）
///
/// 默认模式（`force == false`）下 `preview.conflicts > 0` 即返回
/// `Err(CoreError{code: Conflict})`，且**注册表零改动** —— 中止发生在任何写入
/// 之前，不存在「写了一半才发现冲突」的中间状态。退出码 3 由 CLI 层从 `code`
/// 映射（`CoreError::exit_code`），不在本函数。
///
/// `force == true` 时 `Conflict` 按 `Modified` 处理（写入覆盖）。注意
/// [`RestoreChangeKind::Modified`] 当前**不可达**（见该变体文档），但 `match` 仍
/// 必须同时覆盖两个变体 —— 这正是该变体被保留的原因。
///
/// # 失败语义
///
/// - **单变量失败不中止整体**：`Err` 被记入 `RestoreOutcome.failures`，继续处理下一条。
/// - **单 hive 失败不影响另一 hive**：差异列表本身带 hive 标记，两个 hive 的条目
///   各自独立写入，一个 hive 的变量全数失败不会阻止另一个 hive 的写入。
/// - 但**差异计算阶段的失败是整体的**：`preview_restore_in_stores` 对任一 hive
///   枚举失败即返回 `Err`，此时恢复在写入前就中止 —— 这是刻意的，宁可什么都不做，
///   也不留下「半个 hive 已改、另一个没动」的注册表。
///
/// # 空备份（**后果已被显式固定，不是疏漏**）
///
/// `EnvBackupHives` 的 `system` / `user` 带 `#[serde(default)]`（Task 1 的产物，
/// 本任务不得改动），因此形如 `{"capturedAt":1,"hives":{}}` 的备份会反序列化成功
/// 并得到**两个空列表**。此时当前注册表里的每个可写变量都会被判 `Removed`，
/// 即「恢复一个空备份」等价于**删除当前全部环境变量**。
///
/// **本任务裁定：不在恢复层加防护。** 理由：
/// 1. 与既有 `env remove --force` 同构：两者都能造成同等幅度的破坏，也都靠
///    「用户显式指定目标 + 自行复核」把关，而不是在 core 里猜测用户意图；
/// 2. 在恢复层加「两 hive 皆空即拒绝」会引入**第二个**「什么备份算合法」的规则源，
///    与「判定只在 core 一处」的硬约束冲突（E5 裁断同源），且没有 natural owner ——
///    真正的疑点（文件是否被截断/损坏）无法在恢复层可靠判定。
///
/// **默认模式不构成对空备份清空的保护**（勿按直觉推断）：空备份产生**零冲突**
/// （备份侧没有任何条目，`diff_one_hive` 无从产出 `Conflict`），而下面的 K3 中止
/// 条件恰是 `preview.conflicts > 0` —— 因此它**根本不会触发**，默认模式会照常删光。
/// 唯一的事前可见性来自 `--dry-run`（由 CLI/GUI 展示层调用，是**选择性**的、
/// 不是默认路径）。
///
/// 另一件必须说清的事：**恢复足以区分「无事可做」与「删光一切」** ——
/// 已填充但值全同的备份产生零差异（revision 相同时 `diff_one_hive` 不 push，
/// `seen` 又抑制了 `Removed`），于是 `changes.len() == 0`、零写入、也不广播。
/// 所以「两 hive 皆空」不是一次寻常恢复，而是本特性**能造成的最大幅度动作**；
/// 它有真实的误用风险（把截断但仍可解析的文件当备份恢复）。
/// 这正是本条裁定必须被显式固定、而不是隐式存在的原因。
///
/// 代价因此是真实的：该行为由 `restore_empty_backup_removes_all_current_variables`
/// 与 `restore_empty_backup_without_force_also_removes_all` 两个测试**钉住**，
/// 且 CLI/GUI 的展示层必须把「删除」数量显著呈现（spec 验收标准 12/13）。
/// **若将来要加防护，这两个测试会先红 —— 那是有意的，它强制语义变更被显式确认。**
///
/// **本函数不产生新备份**：否则每次恢复都新增一份文件，与保留策略互相吞噬
/// （设计文档明文要求）。恢复前的手工兜底提示由 CLI/GUI 层打印，也不自动执行。
///
/// # Returns
/// - `Ok(RestoreOutcome)` — 恢复结果（可能含逐条失败）
/// - `Err(CoreError)` — 读取/解析备份失败，差异计算失败（code=`Io`），
///   或**默认模式下检测到冲突**（code=`Conflict`，注册表零改动）
fn restore_in_stores(
    sys: &dyn EnvHiveStore,
    usr: &dyn EnvHiveStore,
    path: &Path,
    force: bool,
) -> Result<RestoreOutcome, CoreError> {
    let payload = read_env_backup(path)?;
    let preview = preview_restore_in_stores(sys, usr, &payload)?;

    // K3：默认模式下任何冲突都中止，且不做任何写入。
    // 位置很关键——必须早于下面第一个写调用，否则会留下部分写入。
    if !force && preview.conflicts > 0 {
        let names: Vec<&str> = preview
            .changes
            .iter()
            .filter(|c| c.kind == RestoreChangeKind::Conflict)
            .map(|c| c.name.as_str())
            .collect();
        return Err(CoreError::new(
            ErrorCode::Conflict,
            "restore_env_backup",
            format!(
                "备份后有 {} 个变量被外部修改，恢复已中止（未做任何改动）: {}。确认要覆盖请加 --force",
                names.len(),
                names.join(", ")
            ),
        ));
    }

    let mut outcome = RestoreOutcome {
        applied: 0,
        skipped: 0,
        failures: Vec::new(),
    };

    for change in &preview.changes {
        let store: &dyn EnvHiveStore = match change.hive {
            EnvHive::System => sys,
            EnvHive::User => usr,
        };

        // 三个分支都必须先用备份侧的值/类型，再交给 core 写函数做判定。
        // 未知差异类型一律记失败（宁可响亮地失败，也不静默漏掉一个变量）。
        let result: Result<(), CoreError> = match change.kind {
            RestoreChangeKind::Added => {
                match find_backup_var(&payload, change.hive, &change.name) {
                    Some(var) => {
                        create_env_var_in_store(store, change.hive, &var.name, &var.value, var.kind)
                    }
                    None => Err(inconsistent_change(&change.name)),
                }
            }
            // Conflict 在 force 模式下按 Modified 处理；非 force 模式已在上面整批中止，
            // 因此走到这里时这两个变体的语义完全相同。
            RestoreChangeKind::Modified | RestoreChangeKind::Conflict => {
                match find_backup_var(&payload, change.hive, &change.name) {
                    Some(var) => {
                        update_env_var_force_in_store(store, change.hive, &var.name, &var.value)
                    }
                    None => Err(inconsistent_change(&change.name)),
                }
            }
            // Removed 不查备份侧：该变量本就「注册表有、备份无」。
            RestoreChangeKind::Removed => {
                delete_env_var_force_in_store(store, change.hive, &change.name)
            }
        };

        match result {
            Ok(()) => outcome.applied += 1,
            Err(e) => outcome.failures.push(format!(
                "[{}] {}: {}",
                // hive 标签取 hive_location 的第 3 项（"系统"/"用户"），
                // 与其它错误文案同源，不另写一套标签。
                hive_location(change.hive).2,
                change.name,
                e.message
            )),
        }
    }

    Ok(outcome)
}

/// [`restore_in_stores`] 的失败文本：[`RestoreChange`] 与备份载荷不自洽。
///
/// 只在「差异说该变量来自备份、但载荷里查不到」时产生。当前实现不可达
/// （差异条目唯一的来源就是载荷本身，且比较忽略大小写），保留它是为了让
/// 类型系统逼着调用方处理 `None`，而不是 `.expect()` 掉一个潜在的数据不一致。
fn inconsistent_change(name: &str) -> CoreError {
    CoreError::new(
        ErrorCode::Internal,
        "restore_env_backup",
        format!("内部不一致：差异条目 {name} 在备份载荷中找不到对应变量"),
    )
}

/// 恢复结束后是否应广播 `WM_SETTINGCHANGE`。**门控条件只有一处**，见下。
///
/// 抽成具名谓词是为了让这个分支**可被测试**：真实广播走
/// `crate::system::broadcast_env_change()`，那是一次 Win32 `SendMessageTimeoutW`
/// 调用（生产约 4s，且无观察点），core 单测里既不该触发也无法观测。
/// 因此本文件测的是**决策**（[`restore_noop_does_not_broadcast`]），
/// Win32 调用本身不在 core 单测覆盖范围内 —— 这是如实登记的边界，不是遗漏。
///
/// 判据是 `applied > 0`：只有确实改动过注册表才需要惊动已运行的进程。
/// 零差异（`applied == 0`）意味着什么都没变，广播纯属噪声。
fn restore_should_broadcast(outcome: &RestoreOutcome) -> bool {
    outcome.applied > 0
}

/// 从备份文件恢复环境变量（公开入口，供 CLI `env restore` 与 GUI 命令使用）。
///
/// 先做来源校验（[`validate_backup_path`]：扩展名、目录/文件名前缀、大小上限），
/// 再以**可写方式**打开两个 hive 执行恢复。**不产生新备份**（见 [`restore_in_stores`]）；
/// 恢复前的手工兜底提示由调用方打印。
///
/// 恢复按 hive 独立执行（单变量失败记入 `failures` 而不中止整体），但两个 hive 的
/// 注册表键都是**先打开、后写入**：任一 hive 打不开（例如系统 hive 需要管理员权限）
/// 都会在写入前整体失败，不会留下「一个 hive 改了、另一个没改」的状态。
///
/// # Returns
/// - `Ok(RestoreOutcome)` — 恢复结果（含逐条失败）
/// - `Err(CoreError)` — 路径非法（`InvalidValue`/`NotFound`）、读取或解析备份失败
///   （`Io`/`Parse`）、打开 hive 失败（`PermissionDenied`/`Io`）、差异计算失败（`Io`），
///   或默认模式下检测到冲突（`Conflict`，注册表零改动）
pub fn restore_env_backup_from(path: &Path, force: bool) -> Result<RestoreOutcome, CoreError> {
    let verified = validate_backup_path(&path.to_string_lossy())?;
    let sys = WinregHive::open(EnvHive::System, true)?;
    let usr = WinregHive::open(EnvHive::User, true)?;
    let outcome = restore_in_stores(&sys, &usr, &verified, force)?;
    // 广播只在确实写入过时发出：无差异的恢复不应惊动已运行的进程（判据见谓词）。
    if restore_should_broadcast(&outcome) {
        crate::system::broadcast_env_change();
    }
    Ok(outcome)
}

/// 备份当前注册表中的系统 PATH 和用户 PATH
/// 在保存前调用，备份的是注册表中的当前值（保存前的状态）
pub fn backup_registry(custom_dir: Option<String>) -> Result<String, String> {
    let backup_dir = match custom_dir {
        Some(ref dir) if !dir.is_empty() => {
            let p = std::path::PathBuf::from(dir);
            let normalized = dir.replace('/', "\\").to_lowercase();
            if normalized.starts_with("c:\\windows\\")
                || normalized.starts_with("c:\\program files\\")
            {
                return Err("不允许备份到系统目录".into());
            }
            p
        }
        _ => backup_base_dir(),
    };

    std::fs::create_dir_all(&backup_dir).map_err(|e| format!("无法创建备份目录: {}", e))?;

    // 读取当前注册表中的值（保存前的旧值）
    let sys_paths = registry::load_paths(HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统")?;
    let user_paths = registry::load_paths(HKEY_CURRENT_USER, USER_REG_PATH, "用户")?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    let filename = format!("path_backup_{}.txt", timestamp);
    let filepath = backup_dir.join(&filename);

    let mut content = String::new();
    content.push_str(&format!(
        "PathEditor Backup - {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    content.push_str("\n[System PATH]\n");
    for path in &sys_paths {
        content.push_str(&format!("{}\n", path));
    }
    content.push_str("\n[User PATH]\n");
    for path in &user_paths {
        content.push_str(&format!("{}\n", path));
    }

    std::fs::write(&filepath, &content).map_err(|e| format!("无法写入备份文件: {}", e))?;

    let result = filepath.to_string_lossy().to_string();
    log::info!("备份已保存到: {}", result);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_var::{revision_of, EnvHive, EnvValueKind};
    // `MemoryHive` 定义在 `reg_store::memory`（`#[cfg(test)] pub(crate) mod`），
    // 未经根 re-export —— brief 写的 `crate::reg_store::MemoryHive` 不成立。
    use crate::reg_store::memory::MemoryHive;
    // `RegType` 经 `winreg::enums::*` 已在文件顶部导入，测试模块经 `use super::*` 可见。
    use winreg::enums::{REG_DWORD, REG_EXPAND_SZ, REG_MULTI_SZ, REG_SZ};

    #[test]
    fn get_appdata_dir_returns_non_empty() {
        assert!(!get_appdata_dir().is_empty());
    }

    #[test]
    fn backup_registry_with_custom_dir() {
        let dir = std::env::temp_dir().join("patheditor_test_backup_custom");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let result = backup_registry(Some(dir.to_string_lossy().to_string()));
        // 可能因无权限读取注册表而失败，但不应 panic
        if let Ok(path) = result {
            assert!(path.contains("patheditor_test_backup_custom"));
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn backup_registry_default_dir_no_panic() {
        // 验证不传参时不会 panic
        let _ = backup_registry(None);
    }

    /// 采集必须排除保留名（Path）与 Unsupported 类型，并携带明文与 revision。
    ///
    /// B1：**先断言 Ok** —— 遇到 Unsupported 必须跳过该变量而非让整次采集失败。
    #[test]
    fn collect_hive_vars_excludes_reserved_and_unsupported() {
        let hive = MemoryHive::new(true);
        hive.seed("JAVA_HOME", "C:\\jdk17", REG_SZ);
        hive.seed("API_TOKEN", "secret-value", REG_SZ);
        hive.seed("Path", "C:\\Windows", REG_EXPAND_SZ); // 保留名，须排除
        hive.seed("SomeDword", "1", REG_DWORD); // Unsupported，须跳过（不是报错）

        // 关键：必须 Ok。若实现走 read_env_var，这里会 Err 而失败。
        let vars = collect_hive_vars_in_store(&hive, EnvHive::User)
            .expect("遇到 Unsupported 类型必须跳过该变量，而不是整次采集失败");

        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"JAVA_HOME"), "普通变量必须被采集");
        assert!(
            names.contains(&"API_TOKEN"),
            "敏感变量也必须采集（备份要能还原）"
        );
        assert!(!names.contains(&"Path"), "保留名必须排除");
        assert!(!names.contains(&"SomeDword"), "Unsupported 类型必须被跳过");

        let java = vars.iter().find(|v| v.name == "JAVA_HOME").unwrap();
        assert_eq!(java.value, "C:\\jdk17", "备份必须携带明文值");
        assert_eq!(java.kind, EnvValueKind::String);
        assert_eq!(java.revision.len(), 16, "revision 是 16 位十六进制摘要");
    }

    /// B1 回归：Unsupported 只在被跳过时出现，**不得**使同一 hive 的其他变量丢失。
    #[test]
    fn collect_hive_vars_keeps_others_when_unsupported_present() {
        let hive = MemoryHive::new(true);
        hive.seed("A_DWORD", "1", REG_DWORD);
        hive.seed("B_NORMAL", "b", REG_SZ);
        hive.seed("C_DWORD", "2", REG_DWORD);
        hive.seed("D_NORMAL", "d", REG_EXPAND_SZ);

        let vars = collect_hive_vars_in_store(&hive, EnvHive::User).expect("必须跳过而非失败");
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();

        assert_eq!(vars.len(), 2, "两个 Unsupported 被跳过，两个正常变量保留");
        assert!(names.contains(&"B_NORMAL"));
        assert!(names.contains(&"D_NORMAL"));
    }

    /// 采集的 revision 必须与 core 的 revision_of 同口径（恢复时据此判冲突）。
    #[test]
    fn collect_hive_vars_revision_matches_revision_of() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "value-1", REG_SZ);

        let vars = collect_hive_vars_in_store(&hive, EnvHive::User).expect("采集失败");
        let expected = revision_of("MY_VAR", REG_SZ, "value-1");

        assert_eq!(vars[0].revision, expected);
    }

    /// 备份目录可用环境变量重定向，供测试隔离；未设置时回落 ~/.patheditor/backups。
    fn with_temp_backup_dir<F: FnOnce(&std::path::Path)>(f: F) {
        // 环境变量是进程级的，与其他触碰它的测试互斥
        let _guard = crate::persist::test_persist_lock();
        let dir =
            std::env::temp_dir().join(format!("patheditor_test_env_backup_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("PATHEDITOR_BACKUP_DIR", &dir);
        f(&dir);
        std::env::remove_var("PATHEDITOR_BACKUP_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `PATHEDITOR_BACKUP_DIR` 必须真正把备份目录整体重定向（否则测试隔离失效，
    /// 所有备份测试会写到用户的真实 `~/.patheditor/backups/`）。
    #[test]
    fn backup_dir_is_redirected_by_env_var() {
        with_temp_backup_dir(|dir| {
            assert_eq!(env_backup_dir(), dir, "备份目录必须被重定向到测试临时目录");
            assert_eq!(
                get_appdata_dir(),
                dir.to_string_lossy(),
                "PATH 备份目录访问器也必须继承同一重定向"
            );
        });
    }

    /// 重定向变量为空时必须回落到默认目录，而不是把备份写到进程当前工作目录。
    #[test]
    fn backup_dir_empty_env_var_falls_back_to_default() {
        let _guard = crate::persist::test_persist_lock();
        let default_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".patheditor")
            .join("backups");

        std::env::set_var("PATHEDITOR_BACKUP_DIR", "");
        let redirected = env_backup_dir();
        std::env::remove_var("PATHEDITOR_BACKUP_DIR");

        assert_eq!(redirected, default_dir, "空值必须回落默认目录");
        assert!(!redirected.as_os_str().is_empty());
    }

    fn sample_payload() -> EnvBackupPayload {
        EnvBackupPayload {
            captured_at: 1_758_400_000_000,
            hives: EnvBackupHives {
                system: vec![],
                user: vec![EnvBackupVar {
                    name: "JAVA_HOME".into(),
                    kind: EnvValueKind::String,
                    value: "C:\\jdk17".into(),
                    revision: "a1b2c3d4e5f60718".into(),
                }],
            },
        }
    }

    /// 落盘文件必须带 schemaVersion 信封，且能被读回（往返一致）。
    #[test]
    fn write_env_backup_round_trips_through_versioned_envelope() {
        with_temp_backup_dir(|dir| {
            let path = write_env_backup_to(dir, &sample_payload()).expect("写备份失败");
            assert!(path.exists());
            assert!(
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("env_backup_"),
                "文件名必须以 env_backup_ 开头"
            );
            assert_eq!(path.extension().unwrap(), "json");

            let content = std::fs::read_to_string(&path).unwrap();
            assert!(
                content.contains("\"schemaVersion\""),
                "必须带 schemaVersion 信封（复用 persist::Versioned）"
            );
            assert!(content.contains("JAVA_HOME"));

            let back: Versioned<EnvBackupPayload> = serde_json::from_str(&content).unwrap();
            assert_eq!(back.inner, sample_payload());
        });
    }

    /// S1 对抗性测试：轮换只删自己生成的 env_backup_*.json，其他文件一律不动。
    ///
    /// **本测试必须真正发生删除**：25 份备份全部唯一，`keep=20` 时轮换必然删 5 份。
    /// 若文件名重复导致候选数 ≤ keep，轮换会提前返回、一份不删——测试就退化成
    /// 空断言，无法发现「误删外部文件」的 bug。故此处额外断言删除确实发生。
    #[test]
    fn rotate_env_backups_never_deletes_foreign_files() {
        with_temp_backup_dir(|dir| {
            // 25 份互不相同的正常备份（超过保留数 20），确保轮换真的删除
            for i in 0..25 {
                let name = format!("env_backup_202601{:02}_120000_000.json", i);
                std::fs::write(dir.join(&name), format!("{{\"n\":{i}}}")).unwrap();
            }
            // 干扰文件：PATH 备份、.bak、损坏隔离、用户自放文件
            std::fs::write(dir.join("path_backup_20260920_152446_043.txt"), "PATH").unwrap();
            // 名字刻意排在待删区间最前（字典序最小）：若过滤条件漏掉 `.ends_with(".json")`
            // 这一半，本文件会被当成候选删掉，断言才会真正失败。
            std::fs::write(dir.join("env_backup_20250101_000000_000.json.bak"), "BAK").unwrap();
            // 另一个 .bak 落在保留区间内（新于 20 份中最旧的一份），
            // 验证「保留区间内的外部文件同样不会被删」。
            std::fs::write(dir.join("env_backup_old.json.bak"), "BAK").unwrap();
            std::fs::write(dir.join("disabled.json.corrupt-20260920-120000000"), "C").unwrap();
            std::fs::write(dir.join("我的笔记.txt"), "note").unwrap();
            std::fs::write(dir.join("env_backup_note.md"), "md").unwrap();

            let removed = rotate_env_backups(dir, ENV_BACKUP_KEEP).expect("轮换失败");

            // 前置条件：轮换确实删了东西，下面的「未删除」断言才有意义
            assert_eq!(
                removed.len(),
                5,
                "25 份候选 - keep 20 份 = 应删 5 份；未删除说明测试空转"
            );
            // 删除目标必须全部是本功能生成的备份，一个外部文件都不许碰
            for path in &removed {
                let name = path.file_name().unwrap().to_string_lossy();
                assert!(
                    name.starts_with("env_backup_") && name.ends_with(".json"),
                    "删除了非本功能生成的文件: {name}"
                );
            }

            assert!(
                dir.join("path_backup_20260920_152446_043.txt").exists(),
                "PATH 备份不得删除"
            );
            assert!(
                dir.join("env_backup_20250101_000000_000.json.bak").exists(),
                ".bak 不得删除"
            );
            assert!(
                dir.join("env_backup_old.json.bak").exists(),
                ".bak 不得删除（保留区间内）"
            );
            assert!(
                dir.join("disabled.json.corrupt-20260920-120000000")
                    .exists(),
                "损坏隔离文件不得删除"
            );
            assert!(dir.join("我的笔记.txt").exists(), "无关文件不得删除");
            assert!(
                dir.join("env_backup_note.md").exists(),
                "非 .json 的同前缀文件不得删除"
            );
        });
    }

    /// 轮换后 env_backup_*.json 恰好保留 keep 份。
    #[test]
    fn rotate_env_backups_keeps_exactly_keep_files() {
        with_temp_backup_dir(|dir| {
            for i in 0..25 {
                std::fs::write(
                    dir.join(format!("env_backup_202601{:02}_120000_000.json", i)),
                    "{}",
                )
                .unwrap();
            }
            rotate_env_backups(dir, ENV_BACKUP_KEEP).expect("轮换失败");

            let remaining = std::fs::read_dir(dir)
                .unwrap()
                .flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.starts_with("env_backup_") && n.ends_with(".json")
                })
                .count();
            assert_eq!(remaining, ENV_BACKUP_KEEP);
        });
    }

    /// 端到端回归：经写入路径之后，目录稳态**恰好**等于 keep 份。
    ///
    /// 这才是 spec 验收标准 11（「轮换保留 config.ini 的 env_backup_keep 份」）的
    /// 口径——用户看到的是目录里有多少份，不是 `rotate_env_backups` 单次调用的
    /// 结果。函数级测试看不到「先轮换、后写入」把稳态推高到 `keep + 1` 的偏差，
    /// 必须走完整写入路径整体断言。
    #[test]
    fn write_env_backup_steady_state_is_exactly_keep_files() {
        with_temp_backup_dir(|dir| {
            let count = |dir: &std::path::Path| {
                std::fs::read_dir(dir)
                    .unwrap()
                    .flatten()
                    .filter(|e| {
                        let n = e.file_name().to_string_lossy().into_owned();
                        n.starts_with("env_backup_") && n.ends_with(".json")
                    })
                    .count()
            };
            const KEEP: usize = 5;

            // 预置远超 keep 份的旧备份
            for i in 0..25 {
                std::fs::write(
                    dir.join(format!("env_backup_202601{:02}_120000_000.json", i)),
                    "{}",
                )
                .unwrap();
            }
            assert_eq!(count(dir), 25, "预置基线");

            let mut written_paths = Vec::new();
            for round in 0..4 {
                let written = write_env_backup_to_with_keep(dir, &sample_payload(), KEEP)
                    .expect("写备份失败");
                assert!(
                    written.exists(),
                    "第 {round} 轮：刚写入的备份必须保留，不得被自己轮换掉"
                );
                assert_eq!(
                    count(dir),
                    KEEP,
                    "第 {round} 轮写入后应为恰好 {KEEP} 份；若为 {} 说明轮换与写入顺序颠倒",
                    KEEP + 1
                );
                written_paths.push(written);
                // 文件名时间戳精度到毫秒；错开 20ms 确保每轮都产生新文件
                // （Windows 计时器粒度约 15.6ms），否则同名覆盖会让本测试失去意义。
                std::thread::sleep(std::time::Duration::from_millis(20));
            }

            // 前置条件：四轮确实各写了不同文件，稳态断言才覆盖「keep+1 → keep」回缩
            let unique: std::collections::HashSet<_> = written_paths.iter().collect();
            assert_eq!(unique.len(), 4, "四轮写入应产生 4 个不同文件，否则测试空转");

            // 最新的一份必须仍在（时间戳最大，永不落入删除集）
            let newest = written_paths.last().unwrap();
            assert!(newest.exists(), "最新备份必须保留");
        });
    }

    /// 测试用日志捕获器：把 `log::warn!` 的消息收进静态缓冲，供断言。
    ///
    /// 存在的理由：`read_env_backup_keep` 的「不可读要 warn、不存在不 warn」这一
    /// 要求**只能通过真实 log 输出观察**。分类函数的单测能证明分类结果正确，却
    /// 证明不了 `read_env_backup_keep` 确实调用了它——把 warn 整段删掉，那些测试
    /// 依然全绿。故此处按 `log` crate 的全局 logger 接口接一个最小捕获器。
    struct CapturingLogger;

    static CAPTURED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

    impl log::Log for CapturingLogger {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.level() <= log::Level::Warn
        }
        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                // 捕获器是全局的，而 cargo 测试并行跑：锁被 poisoned 时取内层数据
                // 继续，避免别的测试 panic 后本测试连锁失败（同 persist::test_persist_lock）。
                let mut buf = match CAPTURED.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                buf.push(record.args().to_string());
            }
        }
        fn flush(&self) {}
    }

    /// 安装捕获器（全局 logger 只能设置一次，已设置则忽略错误）。
    fn install_capture_logger() {
        static LOGGER: CapturingLogger = CapturingLogger;
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Warn);
    }

    /// 清空捕获缓冲，隔离本用例的日志。
    fn clear_captured() {
        let mut buf = match CAPTURED.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        buf.clear();
    }

    /// 取捕获缓冲的守卫（poisoned 时取内层数据）。
    ///
    /// 返回守卫而非克隆：`Vec<String>` 无共享所有权，调用方借守卫即可 filter/any，
    /// 无拷贝；且守卫在语句末即释放，不会跨 `clear_captured()` 持锁。
    fn captured() -> std::sync::MutexGuard<'static, Vec<String>> {
        match CAPTURED.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// **「不可读要 warn、不存在不 warn」——直接断言真实 log 输出。**
    ///
    /// 这是对该要求在集成点上的唯一有效验证：把 `read_env_backup_keep` 里的
    /// warn 分支删掉、或退回「NotFound 与不可读合并处理」的老写法，本测试都会
    /// 失败（后者表现为不可读时一条 warn 都没有）。
    ///
    /// 断言一律**按本用例的临时路径过滤**：捕获器是全局的、cargo 测试并行跑，
    /// 别的测试（如 `config_unreadable_path_falls_back_without_panic`）也会发出
    /// 同样含「不可读」字样的日志。若只匹配关键字，情形 1 的「必须静默」断言会
    /// 被邻居的日志污染成假失败。
    #[test]
    fn read_error_warns_only_when_unreadable() {
        install_capture_logger();
        // 路径含进程 id 与专用前缀，保证与其它测试的临时路径不重叠
        let dir =
            std::env::temp_dir().join(format!("patheditor_cfg_warn_probe_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dir_marker = dir.to_string_lossy().into_owned();

        // 情形 1：文件不存在 → 必须静默（首次使用的正常情形，不得刷警告）
        let missing = dir.join("config.ini");
        clear_captured();
        assert_eq!(read_env_backup_keep(&missing), ENV_BACKUP_KEEP);
        let missing_msgs: Vec<String> = captured()
            .iter()
            .filter(|m| m.contains("不可读") && m.contains(&dir_marker))
            .cloned()
            .collect();
        assert!(
            missing_msgs.is_empty(),
            "文件不存在必须静默回落，不得记 warn；实测: {missing_msgs:?}"
        );

        // 情形 2：路径指向目录（不可读）→ 必须 warn，且消息含该路径
        clear_captured();
        assert_eq!(read_env_backup_keep(&dir), ENV_BACKUP_KEEP);
        let all_msgs: Vec<String> = captured().clone();
        let unreadable_msgs: Vec<&String> = all_msgs
            .iter()
            .filter(|m| m.contains("不可读") && m.contains(&dir_marker))
            .collect();
        assert!(
            !unreadable_msgs.is_empty(),
            "文件不可读必须记一条含该路径的 warn；实测全部日志: {all_msgs:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 读配置失败的分类（`:43` 那处曾把「不存在」与「不可读」合并处理）：
    /// `NotFound` 静默回落，其它错误必须 warn。
    ///
    /// 这里直接调 `should_warn_on_read_error` 断言分类结果——这是 `read_env_backup_keep`
    /// 中**真实执行**的那行判定，不是 mock。`read_to_string` 的错误构造依赖 OS，
    /// 无法在测试里造出任意 `ErrorKind`，故分类逻辑本身被抽成纯函数以便断言。
    #[test]
    fn read_error_kind_separates_missing_from_unreadable() {
        let missing = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert!(
            !should_warn_on_read_error(&missing),
            "文件不存在是首次使用的正常情形，必须静默回落（且不得创建文件）"
        );

        for kind in [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::IsADirectory,
            std::io::ErrorKind::InvalidData,
            std::io::ErrorKind::Other,
        ] {
            assert!(
                should_warn_on_read_error(&std::io::Error::from(kind)),
                "{kind:?} 属于「不可读」，必须 warn 而非静默"
            );
        }
    }

    /// 「文件整体不可读」回落默认值，且**不 panic、无副作用**。
    ///
    /// 用**路径指向目录**构造不可读：`read_to_string` 对目录返回非 `NotFound` 的
    /// 错误（本机实测 Windows 为 `PermissionDenied`，os error 5），从而真实走进
    /// 非 `NotFound` 分支。不用 chmod——Windows 上语义不同且 CI 是 Windows。
    ///
    /// 前置断言保证本测试不空转：若某平台上目录读取竟然返回 `NotFound`，
    /// 该断言会失败并提醒改用别的构造手段，而不是让测试悄悄退化成
    /// 「又一次覆盖了不存在的情形」。
    #[test]
    fn config_unreadable_path_falls_back_without_panic() {
        let dir =
            std::env::temp_dir().join(format!("patheditor_cfg_unreadable_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // 前置条件：指向目录必须产生非 NotFound 错误，否则下面断言的不是目标分支
        let probe = std::fs::read_to_string(&dir).expect_err("读取目录必须失败");
        assert_ne!(
            probe.kind(),
            std::io::ErrorKind::NotFound,
            "目录读取返回 NotFound，本测试未覆盖「不可读」分支；需改用其它构造手段"
        );

        // 被测行为：不 panic、回落默认值
        assert_eq!(read_env_backup_keep(&dir), ENV_BACKUP_KEEP);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 配置文件缺失 → 回落默认值，且**不创建文件**。
    #[test]
    fn config_missing_returns_default_and_creates_nothing() {
        let _guard = crate::persist::test_persist_lock();
        let dir =
            std::env::temp_dir().join(format!("patheditor_cfg_missing_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("config.ini");

        assert_eq!(read_env_backup_keep(&cfg), ENV_BACKUP_KEEP);
        assert!(!cfg.exists(), "读配置不得有副作用");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 五种回落情形（键缺失 / 非整数 / 负数 / 空值 / 未知键）都返回默认值而不是报错。
    #[test]
    fn config_invalid_values_fall_back_to_default() {
        let dir =
            std::env::temp_dir().join(format!("patheditor_cfg_invalid_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("config.ini");

        for body in [
            "; 只有注释\n",            // 键缺失
            "env_backup_keep = abc\n", // 非整数
            "env_backup_keep = -5\n",  // 负数
            "env_backup_keep =\n",     // 空值
            "other_key = 1\n",         // 未知键
        ] {
            std::fs::write(&cfg, body).unwrap();
            assert_eq!(
                read_env_backup_keep(&cfg),
                ENV_BACKUP_KEEP,
                "回落失败: {body:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 合法值被采纳；`;` 与 `#` 注释、行内两侧空格都能正确解析。
    #[test]
    fn config_reads_valid_value_with_comments_and_spaces() {
        let dir = std::env::temp_dir().join(format!("patheditor_cfg_valid_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("config.ini");

        std::fs::write(&cfg, "; 注释行\n# 另一种注释\nenv_backup_keep   =   7   \n").unwrap();
        assert_eq!(read_env_backup_keep(&cfg), 7);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 列表只枚举目录、不解析内容：损坏文件不得让列表整体失败（S5）。
    #[test]
    fn list_env_backups_survives_corrupt_file() {
        with_temp_backup_dir(|dir| {
            std::fs::write(
                dir.join("env_backup_20260901_120000_000.json"),
                "not json at all",
            )
            .unwrap();
            std::fs::write(
                dir.join("env_backup_20260902_120000_000.json"),
                "{\"broken\":",
            )
            .unwrap();
            // 干扰文件不得出现在列表里
            std::fs::write(dir.join("path_backup_20260920_152446_043.txt"), "PATH").unwrap();
            std::fs::write(dir.join("env_backup_x.json.bak"), "BAK").unwrap();

            let list = list_env_backups_in(dir).expect("损坏文件不得使列表失败");

            assert_eq!(list.len(), 2, "只有两个 env_backup_*.json");
            assert!(list.iter().all(|i| i.file.starts_with("env_backup_")));
        });
    }

    /// 列表按时间倒序（最新在前）。
    #[test]
    fn list_env_backups_sorted_newest_first() {
        with_temp_backup_dir(|dir| {
            for ts in [
                "20260901_120000_000",
                "20260903_120000_000",
                "20260902_120000_000",
            ] {
                std::fs::write(dir.join(format!("env_backup_{ts}.json")), "{}").unwrap();
            }
            let list = list_env_backups_in(dir).unwrap();
            assert_eq!(list[0].file, "env_backup_20260903_120000_000.json");
            assert_eq!(list[2].file, "env_backup_20260901_120000_000.json");
        });
    }

    /// S4：路径校验拒绝非 .json、非备份目录、超大文件。
    #[test]
    fn validate_backup_path_rejects_invalid() {
        with_temp_backup_dir(|dir| {
            assert!(
                validate_backup_path("C:\\Windows\\System32\\config\\SAM").is_err(),
                "非 .json 必须拒绝"
            );
            assert!(
                validate_backup_path("C:\\some\\other\\file.json").is_err(),
                "既不在备份目录也不带 env_backup_ 前缀必须拒绝"
            );

            // 带前缀但超大 → 拒绝
            let big = dir.join("env_backup_huge.json");
            std::fs::write(&big, vec![b'x'; 1024 * 1024 + 1]).unwrap();
            assert!(
                validate_backup_path(&big.to_string_lossy()).is_err(),
                "超过 1 MiB 必须拒绝"
            );

            // 正常文件 → 通过
            let ok = dir.join("env_backup_20260901_120000_000.json");
            std::fs::write(&ok, "{}").unwrap();
            assert!(validate_backup_path(&ok.to_string_lossy()).is_ok());
        });
    }

    /// S4 逐规则隔离：每条拒绝规则都必须**单独可证伪**。
    ///
    /// 上面那条测试里，「非 .json 必须拒绝」与「既不在备份目录也不带前缀必须拒绝」
    /// 两条断言各自被**另一条规则**兜住了：`SAM` 没有扩展名，但去掉扩展名规则后
    /// 它仍会被来源规则拒绝；`C:\some\other\file.json` 不存在，去掉来源规则后仍会
    /// 被存在性规则拒绝。删掉任一条规则，那两条断言照样通过——是自证的断言，
    /// 测不到东西。
    ///
    /// 本测试为每条规则各造一个**只有该规则能拒**的输入：其余规则的前置条件全满足，
    /// 删掉对应规则即变 `Ok`。
    #[test]
    fn validate_backup_path_rejects_each_rule_in_isolation() {
        with_temp_backup_dir(|dir| {
            // 规则 1（扩展名）单独生效：文件在备份目录内、存在、体积正常，
            // 唯一的问题是没有 .json 扩展名。
            let wrong_ext = dir.join("env_backup_20260901_120000_000.txt");
            std::fs::write(&wrong_ext, "{}").unwrap();
            assert!(
                validate_backup_path(&wrong_ext.to_string_lossy()).is_err(),
                "目录内的非 .json 文件必须被扩展名规则拒绝（其余三条规则均满足）"
            );

            // 规则 2（来源）单独生效：文件存在、体积正常、扩展名为 .json，
            // 唯一的问题是既不在备份目录内、又没有 env_backup_ 前缀。
            // **必须真实存在**——否则会被存在性规则兜住而失去隔离性。
            let foreign = std::env::temp_dir().join(format!(
                "patheditor_foreign_backup_{}.json",
                std::process::id()
            ));
            std::fs::write(&foreign, "{}").unwrap();
            let foreign_result = validate_backup_path(&foreign.to_string_lossy());
            let _ = std::fs::remove_file(&foreign);
            assert!(
                foreign_result.is_err(),
                "备份目录之外、无 env_backup_ 前缀的 .json 必须被来源规则拒绝\
                 （其余三条规则均满足）"
            );

            // 规则 3（存在性）单独生效：路径在备份目录内、带前缀、扩展名为 .json，
            // 唯一的问题是文件不存在。
            let missing = dir.join("env_backup_20260902_120000_000.json");
            assert!(
                !missing.exists(),
                "前置条件：该文件必须不存在，否则本用例测的不是存在性规则"
            );
            assert!(
                validate_backup_path(&missing.to_string_lossy()).is_err(),
                "不存在的备份文件必须被存在性规则拒绝（其余三条规则均满足）"
            );
        });
    }

    /// 目录不存在不是错误：返回空列表（首次使用时的正常情形）。
    #[test]
    fn list_env_backups_missing_dir_returns_empty() {
        let missing = std::env::temp_dir().join(format!(
            "patheditor_no_such_backup_dir_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&missing);

        let list = list_env_backups_in(&missing).expect("目录不存在必须返回空列表，而不是报错");
        assert!(list.is_empty());
    }

    /// 公开入口 `list_env_backups()` 必须枚举被重定向的默认目录。
    #[test]
    fn list_env_backups_public_entry_uses_default_dir() {
        with_temp_backup_dir(|dir| {
            std::fs::write(dir.join("env_backup_20260901_120000_000.json"), "{}").unwrap();

            let list = list_env_backups().expect("列表失败");
            assert_eq!(
                list.len(),
                1,
                "公开入口必须走默认（被重定向的）备份目录，而不是别处"
            );
            assert_eq!(list[0].file, "env_backup_20260901_120000_000.json");
        });
    }

    /// 构造只含 user hive 的备份载荷（其余测试都只需要 user 侧）。
    fn payload_with(user: Vec<EnvBackupVar>) -> EnvBackupPayload {
        EnvBackupPayload {
            captured_at: 0,
            hives: EnvBackupHives {
                system: vec![],
                user,
            },
        }
    }

    /// 构造一个备份条目，revision 由本条目的 name/type/value 自洽算出 ——
    /// 即「备份时该变量就是这个值」。
    fn backup_var(name: &str, value: &str, vtype: RegType) -> EnvBackupVar {
        // 先算 revision 再移动 vtype；`RegType` 非 Copy（同 collect_hive_vars_in_store）。
        let revision = revision_of(name, vtype.clone(), value);
        EnvBackupVar {
            name: name.into(),
            kind: EnvValueKind::from_reg_type(vtype),
            value: value.into(),
            revision,
        }
    }

    /// 差异三类（新增 / 冲突 / 删除）都要被识别，无变化的变量不进差异。
    ///
    /// **本测试原先断言 `modified == 1`，与契约第 4 条互斥**（见
    /// `preview_marks_conflict_when_revision_differs` 与 `RestoreChangeKind::Modified`
    /// 的注释）：`revision_of` 对 name+type+value 取值，故「值变了」与「备份已过期」
    /// 是同一条件，一律判 `Conflict`。改判后本测试的 `CHANGED` 走的就是 `Conflict`
    /// 分支，而 `SAME` 仍覆盖「revision 相同 → 无变化」这条。
    #[test]
    fn preview_classifies_added_conflict_removed() {
        let hive = MemoryHive::new(true);
        hive.seed("SAME", "v", REG_SZ); // 与备份一致，无变化
        hive.seed("CHANGED", "new-value", REG_SZ); // 备份里是旧值 → revision 不符 → Conflict
        hive.seed("EXTRA", "x", REG_SZ); // 备份里没有 → Removed

        let payload = payload_with(vec![
            backup_var("SAME", "v", REG_SZ),
            backup_var("CHANGED", "old-value", REG_SZ),
            backup_var("BRAND_NEW", "n", REG_SZ),
        ]);

        let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

        assert_eq!(preview.added, 1, "BRAND_NEW 应记为新增");
        assert_eq!(
            preview.conflicts, 1,
            "CHANGED 的 revision 与当前不符，应记为冲突"
        );
        assert_eq!(preview.removed, 1, "EXTRA 应记为删除");
        // Modified 当前不可达（见枚举文档）；钉住这一点，避免将来误以为它是活路径。
        assert_eq!(
            preview.modified, 0,
            "Modified 不可达：值变化与备份过期是同一条件，一律判 Conflict"
        );
        assert_eq!(
            preview.changes.len(),
            3,
            "恰好三条差异（新增/冲突/删除），无变化的不进列表"
        );
        assert!(
            !preview.changes.iter().any(|c| c.name == "SAME"),
            "无变化的变量不进差异"
        );
        // 三条差异的 kind 逐一钉住，避免「计数对但分类错」蒙混过关
        let kind_of = |name: &str| {
            preview
                .changes
                .iter()
                .find(|c| c.name == name)
                .map(|c| c.kind)
                .expect("差异缺失")
        };
        assert_eq!(kind_of("BRAND_NEW"), RestoreChangeKind::Added);
        assert_eq!(kind_of("CHANGED"), RestoreChangeKind::Conflict);
        assert_eq!(kind_of("EXTRA"), RestoreChangeKind::Removed);
    }

    /// K3 / 契约第 4 条：备份中的 revision 与当前不一致时记为 Conflict。
    ///
    /// **可证伪性**：这里走的是 `Conflict` 分支而非「无变化」分支——当前值是
    /// `"changed-by-other-tool"`，备份的 revision 由 `"backed-up-value"` 算出，
    /// 两者必然不同（`revision_of` 对 value 取值）。若把契约第 4 条改成
    /// 「revision 不同也不进差异」，`conflicts` 会掉到 0 而本测试失败。
    #[test]
    fn preview_marks_conflict_when_revision_differs() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "changed-by-other-tool", REG_SZ);

        let payload = payload_with(vec![backup_var("MY_VAR", "backed-up-value", REG_SZ)]);

        let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

        assert_eq!(preview.conflicts, 1, "外部改动必须被识别为冲突");
        assert_eq!(preview.added, 0, "该变量当前存在，不得同时记为新增");
        assert_eq!(preview.removed, 0, "该变量在备份中存在，不得记为删除");
        assert_eq!(preview.changes[0].kind, RestoreChangeKind::Conflict);
        assert_eq!(preview.changes[0].name, "MY_VAR");
        assert_eq!(preview.changes[0].hive, EnvHive::User);
    }

    /// 契约第 3 条：两端 revision 一致时该变量**完全不出现在差异里**。
    ///
    /// 与上一条构成配对——同一形状的输入，只有 revision 是否相符这一个变量，
    /// 一条要求进差异、一条要求不进，两条合起来才真正钉住契约第 3、4 条。
    #[test]
    fn preview_omits_unchanged_variable() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "same-value", REG_SZ);

        // revision 由同一份 name+type+value 算出，与注册表当前值一致
        let payload = payload_with(vec![backup_var("MY_VAR", "same-value", REG_SZ)]);

        let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

        assert_eq!(preview.changes.len(), 0, "值未变的变量必须零差异");
        assert_eq!(preview.added, 0);
        assert_eq!(preview.conflicts, 0);
        assert_eq!(preview.removed, 0);
    }

    /// 契约第 1 条（Unsupported 半边）：`Unsupported` 类型的当前值不构成「删除」差异。
    ///
    /// **必须放进 `REG_MULTI_SZ`，不能只用 `REG_DWORD`**：`REG_DWORD` 的解码
    /// （`String::from_reg_value` 对非字符串类型返回 `Err`）会先把该变量滤掉，
    /// 于是「跳过 Unsupported」这条判定**删掉也照样通过** —— 断言被另一条守卫
    /// 兜住，不可证伪（变异验证第 2 项实测如此）。`REG_MULTI_SZ` 能被解码成字符串
    /// （winreg 把多字符串按 `\n` 连接），只有 `is_writable()` 这一条能拦下它，
    /// 断言才真正钉在契约第 1 条上。
    ///
    /// 两种类型都保留：`REG_MULTI_SZ` 提供可证伪性，`REG_DWORD` 覆盖实际最常见
    /// 的非字符串类型。
    #[test]
    fn preview_ignores_unsupported_current_vars() {
        let hive = MemoryHive::new(true);
        hive.seed("SomeDword", "1", REG_DWORD); // 非字符串类型，解码阶段即被滤掉
        hive.seed("SomeMultiSz", "a", REG_MULTI_SZ); // 可解码，只有 is_writable 能拦住

        let preview =
            preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload_with(vec![]))
                .unwrap();

        assert_eq!(
            preview.removed, 0,
            "Unsupported 变量的存在不构成「删除」差异"
        );
        assert_eq!(preview.changes.len(), 0, "Unsupported 不得产生任何变体");
    }

    /// 契约第 1 条（保留名半边）：注册表中的 `Path` 不参与差异计算。
    ///
    /// 用 `path`（小写）作输入是有意的：`is_reserved` 忽略大小写，而若实现
    /// 误把保留名判定写成精确匹配 `"Path"`，本用例会失败。
    ///
    /// **可证伪性**：去掉实现里的 `is_reserved` 跳过，`path` 就会以原始名落进
    /// `Removed`，`removed` 由 0 变 1 而失败。
    #[test]
    fn preview_ignores_reserved_current_vars() {
        let hive = MemoryHive::new(true);
        hive.seed("path", "C:\\Windows", REG_EXPAND_SZ); // 保留名，须排除
        hive.seed("KEEP_ME", "v", REG_SZ); // 普通变量，用来证明不是「整表被跳过」

        let payload = payload_with(vec![backup_var("KEEP_ME", "v", REG_SZ)]);

        let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

        assert_eq!(preview.removed, 0, "保留名不得构成删除差异");
        assert_eq!(preview.changes.len(), 0, "保留名不得产生任何变体");
        // 前置条件：KEEP_ME 确实在两表中且值一致（否则本测试空转）
        assert!(
            !preview
                .changes
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case("path")),
            "保留名不得出现在差异中"
        );
    }

    /// 契约第 5 条：`Removed` 必须携带**注册表返回的原始大小写名字**，
    /// **不能**是小写化的比较键。
    ///
    /// **可证伪性**：注册表里存的是 `MixedCaseVar`（而非全大写），若实现把
    /// 小写键直接当作展示名输出（`mixedcasevar`），断言 `== "MixedCaseVar"` 即失败。
    /// 用全大写名（如 `EXTRA`）测这一条是**测不到的**——那种输入下原始名与
    /// 小写键只在大小写上不同的话仍可通过，故此处刻意用混合大小写。
    #[test]
    fn preview_removed_entry_keeps_registry_original_case() {
        let hive = MemoryHive::new(true);
        hive.seed("MixedCaseVar", "x", REG_SZ); // 备份里没有 → 应判 Removed

        let preview =
            preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload_with(vec![]))
                .unwrap();

        assert_eq!(preview.removed, 1, "该变量应被判为删除");
        let removed = &preview.changes[0];
        assert_eq!(removed.kind, RestoreChangeKind::Removed);
        assert_eq!(
            removed.name, "MixedCaseVar",
            "Removed 必须用注册表返回的原始大小写，而不是小写比较键（mixedcasevar）"
        );
        assert_ne!(
            removed.name,
            removed.name.to_ascii_lowercase(),
            "前置条件：该名字本身必须含大写，否则本测试区分不出大小写保留"
        );
    }

    /// 契约第 6 条：变量名比较**忽略大小写**，两个方向都要成立。
    ///
    /// - 备份写 `MixedCaseVar`、注册表存 `mixedcasevar` → 应认出是**同一个**变量
    ///   （不进 Added），且因 revision 相同而**零差异**；
    /// - 备份写 `SAMEVAR`、注册表存 `samevar` → 同上。
    ///
    /// **可证伪性**：把实现里的 `to_ascii_lowercase()` 换成直接比较名字（区分大小写），
    /// `current.get(&key)` 会查不到 → 该变量被判 `Added`，`added` 由 0 变 1 而失败。
    #[test]
    fn preview_matches_names_case_insensitively() {
        let hive = MemoryHive::new(true);
        hive.seed("mixedcasevar", "v1", REG_SZ);
        hive.seed("samevar", "v2", REG_SZ);

        let payload = payload_with(vec![
            backup_var("MixedCaseVar", "v1", REG_SZ),
            backup_var("SAMEVAR", "v2", REG_SZ),
        ]);

        let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

        assert_eq!(
            preview.added, 0,
            "仅大小写不同的同名变量必须被认出是同一个，不得判为新增"
        );
        assert_eq!(preview.removed, 0, "反向同理：不得因大小写差异判为删除");
        assert_eq!(preview.conflicts, 0, "值一致，不得判为冲突");
        assert_eq!(preview.changes.len(), 0, "两端大小写不同但值相同 → 零差异");
    }

    /// 契约第 5 与第 6 条的组合：大小写不敏感配对后，**未配对的**那个才判删除，
    /// 且仍用注册表原始名。避免「配对成功」的断言掩盖「未配对时用了小写键」。
    #[test]
    fn preview_pairs_case_insensitively_and_reports_unmatched_original() {
        let hive = MemoryHive::new(true);
        hive.seed("PairedVar", "same", REG_SZ); // 备份里有（不同大小写）→ 无差异
        hive.seed("OrphanVar", "x", REG_SZ); // 备份里没有 → Removed

        let payload = payload_with(vec![backup_var("PAIREDVAR", "same", REG_SZ)]);

        let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

        assert_eq!(preview.removed, 1, "只有未配对的 OrphanVar 应判删除");
        assert_eq!(preview.added, 0, "PAIREDVAR 已配对，不得判新增");
        assert_eq!(
            preview.changes[0].name, "OrphanVar",
            "删除项须用注册表原始名"
        );
    }

    /// 两个 hive 都要被扫描，且差异各自带上正确的 hive 标记。
    ///
    /// **可证伪性**：把 `preview_restore_in_stores` 里对 system 的调用删掉，
    /// 本测试的 `added` 会从 1 掉到 0（system 侧那条没了）而失败。
    #[test]
    fn preview_covers_both_hives_and_tags_each_change() {
        let sys = MemoryHive::new(true);
        let usr = MemoryHive::new(true);
        usr.seed("USER_ONLY", "u", REG_SZ);

        let payload = EnvBackupPayload {
            captured_at: 0,
            hives: EnvBackupHives {
                system: vec![backup_var("SYS_NEW", "s", REG_SZ)],
                user: vec![backup_var("USER_ONLY", "u", REG_SZ)],
            },
        };

        let preview = preview_restore_in_stores(&sys, &usr, &payload).unwrap();

        assert_eq!(preview.added, 1);
        assert_eq!(preview.removed, 0);
        let change = preview
            .changes
            .iter()
            .find(|c| c.name == "SYS_NEW")
            .expect("缺 SYS_NEW");
        assert_eq!(
            change.hive,
            EnvHive::System,
            "system 侧的差异必须带 System 标记"
        );
    }

    /// 枚举失败必须上报为错误（code=`Io`），而不是静默产出空差异。
    ///
    /// **判定只认 `code`**（项目硬约束），不得匹配 message 文本。
    #[test]
    fn preview_reports_enumeration_failure() {
        let mut hive = MemoryHive::new(true);
        hive.fail_enum = true;

        let err = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload_with(vec![]))
            .expect_err("枚举失败必须上报");

        assert_eq!(err.code, ErrorCode::Io, "枚举失败的错误码必须是 Io");
    }

    /// 读取并校验备份文件：正常文件往返一致，损坏文件判 `Parse` 并隔离。
    #[test]
    fn read_env_backup_round_trips_and_quarantines_corrupt() {
        with_temp_backup_dir(|dir| {
            let payload = payload_with(vec![backup_var("JAVA_HOME", "C:\\jdk17", REG_SZ)]);
            let path = write_env_backup_to(dir, &payload).expect("写备份失败");

            let back = read_env_backup(&path).expect("读备份失败");
            assert_eq!(back, payload, "读回的载荷必须与写入的一致");

            // 损坏文件：读失败、code=Parse，且被隔离成 .corrupt-*
            let corrupt = dir.join("env_backup_20260101_000000_000.json");
            std::fs::write(&corrupt, "{\"broken\":").unwrap();
            let err = read_env_backup(&corrupt).expect_err("损坏文件必须报错");
            assert_eq!(err.code, ErrorCode::Parse, "损坏文件的错误码必须是 Parse");
            assert!(!corrupt.exists(), "损坏文件必须被隔离（原名不再存在）");
        });
    }

    /// `preview_restore_file` 必须是「读文件 + 算差异」的组合入口。
    ///
    /// 因为绑定的是真实注册表（`preview_restore` 无存储注入），这里只断言
    /// 「链路可用」：写一份备份再读，必须 `Ok`。差异内容本身由注入版测试覆盖。
    /// 用只读打开注册表，不写入任何内容。
    #[test]
    fn preview_restore_file_reads_and_previews() {
        with_temp_backup_dir(|dir| {
            let path = write_env_backup_to(dir, &payload_with(vec![])).expect("写备份失败");
            // 空备份：差异只可能来自真实注册表里「有而备份无」的变量，
            // 无论多少都必须是 Ok（不是 Err）。
            let preview = preview_restore_file(&path).expect("读文件并预览必须成功");
            assert_eq!(
                preview.added, 0,
                "空备份不可能产生新增（备份侧没有任何条目）"
            );
            assert_eq!(preview.modified, 0, "Modified 不可达");
            assert_eq!(
                preview.added + preview.modified + preview.removed + preview.conflicts,
                preview.changes.len(),
                "四个计数之和必须等于差异条数"
            );
        });
    }

    // ── Task 6：恢复执行 ──

    /// 构造一个**未配对**的备份条目，revision 必然与注册表当前值不符。
    ///
    /// 与 [`backup_var`]（自洽，配对成功）相对：本函数让备份条目的 revision
    /// 由 `revision_value` 算出，而条目携带 `stored_value`，于是
    /// `revision_of(name, vtype, stored_value) != revision` 恒成立
    /// —— `diff_one_hive` 必然判 `Conflict`。这是唯一能让保存的值与
    /// revision 不一致的构造方式，也就是「备份已过期」的真实形态。
    fn stale_backup_var(
        name: &str,
        stored_value: &str,
        revision_value: &str,
        kind: EnvValueKind,
    ) -> EnvBackupVar {
        let vtype = match kind {
            EnvValueKind::String => REG_SZ,
            _ => REG_EXPAND_SZ,
        };
        EnvBackupVar {
            name: name.into(),
            kind,
            value: stored_value.into(),
            revision: revision_of(name, vtype, revision_value),
        }
    }

    /// `outcome.failures` 里是否存在一条点名 `name` 的条目。
    ///
    /// 失败文本采用 core 全库统一的 `"[{hive标签}] {name}: {message}"` 形态，
    /// 因此「点名」可直接由包含 `name` 判定。**失败的具体原因不由本函数判定** ——
    /// 判定只认 `CoreError.code`，不得匹配 message 文本；「该输入确实走进了
    /// 哪个错误分支」由独立的前置条件测试（如
    /// `windir_seed_really_hits_protected_branch`）钉住。
    fn failure_names(outcome: &RestoreOutcome, name: &str) -> bool {
        outcome.failures.iter().any(|f| f.contains(name))
    }

    /// 前置条件核对：确认「保护名单」这一取值真的走进 `ErrorCode::Protected` 分支。
    ///
    /// 存在的理由（断言可证伪性）：`RestoreOutcome.failures` 只承载一条文本，
    /// 任何原因（类型不支持、revision 冲突、名称非法）都会让变量出现在 failures 里。
    /// 若取值不当，测试会「通过」却根本没测到保护名单。这里用一个**独立于恢复路径**
    /// 的同名调用直接取出 `CoreError.code`，把「该输入确实命中保护名单」钉死。
    ///
    /// **覆盖恢复会走的全部三个写函数**：保护名单判定在
    /// `create_env_var_in_store` / `update_env_var_force_in_store` /
    /// `delete_env_var_force_in_store` 里是**三处独立代码**。只核对一条
    /// 会漏掉另外两条 —— 那正是「测试看似通过但测不到东西」的典型形态。
    ///
    /// 可证伪性：把 `windir` 从 `PROTECTED_NAMES`（`core/src/env_var.rs`）删掉，
    /// 三个断言全部失败。
    #[test]
    fn windir_seed_really_hits_protected_branch() {
        let hive = MemoryHive::new(true);
        hive.seed("windir", "C:\\Windows", REG_SZ);

        let created = create_env_var_in_store(
            &hive,
            EnvHive::User,
            "windir",
            "C:\\evil",
            EnvValueKind::String,
        )
        .expect_err("windir 必须被拒绝（create 分支的前置条件）");
        assert_eq!(
            created.code,
            ErrorCode::Protected,
            "create 分支走到的必须是保护名单，而不是类型/名称等其它分支；实际: {created:?}"
        );

        let updated = update_env_var_force_in_store(&hive, EnvHive::User, "windir", "C:\\evil")
            .expect_err("windir 必须被拒绝（update 分支的前置条件）");
        assert_eq!(
            updated.code,
            ErrorCode::Protected,
            "update 分支走到的必须是保护名单；实际: {updated:?}"
        );

        let deleted = delete_env_var_force_in_store(&hive, EnvHive::User, "windir")
            .expect_err("windir 必须被拒绝（delete 分支的前置条件）");
        assert_eq!(
            deleted.code,
            ErrorCode::Protected,
            "delete 分支走到的必须是保护名单；实际: {deleted:?}"
        );
    }

    /// K3：默认模式遇到冲突必须中止，且**不做任何写入**（注册表零改动）。
    ///
    /// 可证伪性：
    /// - 删掉 `restore_in_stores` 里的冲突中止分支 → 本函数返回 `Ok`，`expect_err` 失败；
    /// - 把中止**放在写入之后**（先写后判）→ `NEW_ONE` 会被创建，`!contains` 断言失败。
    #[test]
    fn restore_aborts_on_conflict_without_writing() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("MY_VAR", "changed-by-other-tool", REG_SZ);
            hive.seed("UNTOUCHED", "keep-me", REG_SZ);

            // MY_VAR：注册表值≠备份值 → Conflict。
            // NEW_ONE：备份有、注册表无 → Added（若实现「先写后判」就会写进去）。
            let payload = payload_with(vec![
                stale_backup_var("MY_VAR", "backed-up", "backed-up", EnvValueKind::String),
                backup_var("NEW_ONE", "n", REG_SZ),
            ]);
            let path = write_env_backup_to(dir, &payload).unwrap();

            let err = restore_in_stores(&MemoryHive::new(true), &hive, &path, false)
                .expect_err("冲突必须中止");
            assert_eq!(err.code, ErrorCode::Conflict, "中止的错误码必须是 Conflict");

            assert_eq!(
                String::from_reg_value(&hive.get_raw("MY_VAR").unwrap()).unwrap(),
                "changed-by-other-tool",
                "冲突时不得写入"
            );
            assert!(
                !hive.contains("NEW_ONE"),
                "中止时不得部分写入：NEW_ONE 也不应被创建"
            );
            assert_eq!(
                String::from_reg_value(&hive.get_raw("UNTOUCHED").unwrap()).unwrap(),
                "keep-me",
                "未参与差异的变量必须原样保留"
            );
        });
    }

    /// `--force` 模式下冲突被覆盖，写入成功（`Conflict` 按 `Modified` 处理）。
    ///
    /// 可证伪性：把 `match` 里 `Modified | Conflict` 的合并写回只处理 `Modified`
    /// （K3 的天然错误写法，因为 `Modified` 当前不可达），冲突变量的分支会落空
    /// ——`applied` 掉到 0 且值停在 `"changed-by-other-tool"`，两条断言同时失败。
    #[test]
    fn restore_force_overwrites_conflict() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("MY_VAR", "changed-by-other-tool", REG_SZ);

            let payload = payload_with(vec![stale_backup_var(
                "MY_VAR",
                "backed-up",
                "backed-up",
                EnvValueKind::String,
            )]);
            let path = write_env_backup_to(dir, &payload).unwrap();

            let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
                .expect("force 恢复必须成功");
            assert_eq!(outcome.applied, 1, "冲突变量在 force 下必须被覆盖写入");
            assert!(
                outcome.failures.is_empty(),
                "force 覆盖冲突不应产生失败: {:?}",
                outcome.failures
            );
            assert_eq!(
                String::from_reg_value(&hive.get_raw("MY_VAR").unwrap()).unwrap(),
                "backed-up"
            );
        });
    }

    /// S6：`--force` 不豁免保护名单 —— 保护名单变量出现在差异中，
    /// 但写入阶段被 core 拒绝并记入 failures，且**不中止其余变量的恢复**。
    ///
    /// 核对轮 E5 裁断：preview 不为保护名单新增枚举变体、不预排除；
    /// 拒绝发生在写入阶段（core 写函数内是保护名单的唯一真相源）。
    ///
    /// 可证伪性：把 `restore_in_stores` 的逐条失败改成 `?` 提前返回 → NORMAL_ONE
    /// 不会被创建（`hive.contains` 失败）且整个函数返回 `Err`（`expect` 失败）。
    #[test]
    fn restore_force_does_not_bypass_protected_names() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("windir", "C:\\Windows", REG_SZ);

            // windir：当前值 "C:\\Windows" 与备份 revision（由 "attacker-value" 算）不符
            // → Conflict；force 下按 Modified 处理 → 进 update_env_var_force_in_store
            // → 命中保护名单，返回 ErrorCode::Protected（见上方前置条件测试）。
            // NORMAL_ONE：备份有、注册表无 → Added，用来证明保护名单失败**不中止**其余恢复。
            let payload = payload_with(vec![
                stale_backup_var("windir", "C:\\evil", "attacker-value", EnvValueKind::String),
                backup_var("NORMAL_ONE", "ok", REG_SZ),
            ]);
            let path = write_env_backup_to(dir, &payload).unwrap();

            let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
                .expect("保护名单被拒不得使整个恢复失败");

            assert_eq!(outcome.applied, 1, "只有 NORMAL_ONE 应写入成功");
            assert_eq!(
                outcome.failures.len(),
                1,
                "windir 必须被记为失败，实际: {:?}",
                outcome.failures
            );
            assert!(
                failure_names(&outcome, "windir"),
                "失败原因必须点名 windir，实际: {:?}",
                outcome.failures
            );
            assert_eq!(
                String::from_reg_value(&hive.get_raw("windir").unwrap()).unwrap(),
                "C:\\Windows",
                "保护名单变量的值必须保持原样"
            );
            assert!(hive.contains("NORMAL_ONE"), "其余变量必须照常恢复");
        });
    }

    /// E5 的对应断言：保护名单变量**必须出现在 preview 差异里**（不预排除）。
    #[test]
    fn preview_includes_protected_names() {
        let hive = MemoryHive::new(true);
        hive.seed("windir", "C:\\Windows", REG_SZ);

        let payload = payload_with(vec![stale_backup_var(
            "windir",
            "C:\\evil",
            "attacker-value",
            EnvValueKind::String,
        )]);

        let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();
        assert_eq!(
            preview.conflicts, 1,
            "保护名单变量必须出现在差异中，不被预先排除"
        );
        assert!(preview.changes.iter().any(|c| c.name == "windir"));
    }

    /// 恢复新增与删除两类差异。
    ///
    /// 可证伪性：把 `Added` 分支改调 `update_env_var_force_in_store`（对不存在的
    /// 变量必失败）→ `applied` 掉到 1 且 `failures` 非空；把 `Removed` 分支写反
    /// （调 create）→ `WILL_BE_REMOVED` 仍在。
    #[test]
    fn restore_creates_and_removes() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("WILL_BE_REMOVED", "x", REG_SZ);

            let payload = payload_with(vec![backup_var("WILL_BE_CREATED", "created", REG_SZ)]);
            let path = write_env_backup_to(dir, &payload).unwrap();

            let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true).unwrap();
            assert_eq!(outcome.applied, 2, "一个新增 + 一个删除");
            assert!(
                outcome.failures.is_empty(),
                "正常恢复不应有失败: {:?}",
                outcome.failures
            );
            assert!(hive.contains("WILL_BE_CREATED"));
            let created = hive.get_raw("WILL_BE_CREATED").expect("新变量应存在");
            assert_eq!(String::from_reg_value(&created).unwrap(), "created");
            assert!(!hive.contains("WILL_BE_REMOVED"));
        });
    }

    /// 删除走 `delete_env_var_force_in_store`：**保护名单的删除同样被拒**。
    ///
    /// 与 `restore_force_does_not_bypass_protected_names`（覆盖写入分支）配对 ——
    /// 三个 `*_in_store` 的保护名单判定是**三处独立代码**，只测一处会漏掉删除侧。
    ///
    /// 可证伪性：把 `Removed` 分支改调 `store.delete_value` 裸删（S6 禁止的写法）
    /// → `windir` 被删掉，`contains` 断言失败。
    #[test]
    fn restore_removal_does_not_bypass_protected_names() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("windir", "C:\\Windows", REG_SZ);
            hive.seed("NORMAL_ONE", "ok", REG_SZ);

            // 备份里两个变量都没有 → 都应判 Removed
            let payload = payload_with(vec![]);
            let path = write_env_backup_to(dir, &payload).unwrap();

            let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
                .expect("保护名单被拒不得使整个恢复失败");

            assert_eq!(outcome.applied, 1, "只有 NORMAL_ONE 应被删除");
            assert_eq!(
                outcome.failures.len(),
                1,
                "windir 必须被记为失败，实际: {:?}",
                outcome.failures
            );
            assert!(
                failure_names(&outcome, "windir"),
                "失败原因必须点名 windir，实际: {:?}",
                outcome.failures
            );
            assert!(hive.contains("windir"), "保护名单变量不得被删除");
            assert!(!hive.contains("NORMAL_ONE"), "其余变量必须照常删除");
        });
    }

    /// 单 hive 失败不中止另一 hive，且已完成的 hive **不回滚**（best-effort）。
    ///
    /// 用 `fail_enum` 注入 user 侧枚举失败（`diff_one_hive` 会返回 `Err(Io)`）。
    /// 断言的是**顺序契约**：`preview_restore_in_stores` 先算 user 再算 system、
    /// 任一失败即整体 `Err`，因此 user 侧注入失败时 system 侧的差异**根本没被算出来**，
    /// 恢复必然零写入 —— 这正是「枚举失败必中止」在跨 hive 维度的表现
    /// （不产生半个 hive 已被改动的中间状态）。
    ///
    /// 反向那一半（user 差异能算出、system 失败）由
    /// `restore_preview_failure_in_second_hive_leaves_first_untouched` 覆盖；
    /// 「单变量失败不中止整体」由
    /// `restore_collects_per_variable_failure_without_aborting` 覆盖。
    #[test]
    fn restore_reports_hive_enumeration_failure() {
        with_temp_backup_dir(|dir| {
            let mut usr = MemoryHive::new(true);
            usr.fail_enum = true;
            usr.seed("USER_VAR", "u", REG_SZ);
            let sys = MemoryHive::new(true);

            let payload = payload_with(vec![backup_var("USER_NEW", "n", REG_SZ)]);
            let path = write_env_backup_to(dir, &payload).unwrap();

            let err = restore_in_stores(&sys, &usr, &path, true)
                .expect_err("枚举失败必须中止，不得产出半个 hive 的写入");
            assert_eq!(err.code, ErrorCode::Io, "枚举失败的错误码必须是 Io");
            assert!(
                !usr.contains("USER_NEW"),
                "中止时不得写入 —— user 侧的备份变量一个都不该被创建"
            );
        });
    }

    /// 差异计算在 user 侧失败时，**system 侧的写入一个都不能发生**。
    ///
    /// 这条与上一条互补：上一条用 `fail_enum`（根本没拿到 user 差异），
    /// 本条让 user 的差异**能算出来**（有 Added），再让 system 侧枚举失败 ——
    /// 断言的是「绝不因后半段失败而留下前半段已改动的状态」。
    #[test]
    fn restore_preview_failure_in_second_hive_leaves_first_untouched() {
        with_temp_backup_dir(|dir| {
            let usr = MemoryHive::new(true);
            let mut sys = MemoryHive::new(true);
            sys.fail_enum = true;

            // user 侧有一条真实的 Added 差异；system 侧枚举必然失败
            let payload = payload_with(vec![backup_var("USER_NEW", "n", REG_SZ)]);
            let path = write_env_backup_to(dir, &payload).unwrap();

            let err = restore_in_stores(&sys, &usr, &path, true)
                .expect_err("system 侧枚举失败必须让整次恢复中止");
            assert_eq!(err.code, ErrorCode::Io);
            assert!(
                !usr.contains("USER_NEW"),
                "system 侧失败时，user 侧已算出的差异也不得被写入（不留半个改动的注册表）"
            );
        });
    }

    /// 单个变量写入失败不中止其余变量 —— 与保护名单用例互补的一格。
    ///
    /// 这里用**同批两个变量、其中一个名称非法**（含 `=`）来触发失败：
    /// 名称非法是 `validate_env_name` 的判定，与保护名单是不同分支，
    /// 合起来才覆盖「逐条失败被收集而非整体中止」这条契约。
    ///
    /// 可证伪性：把逐条失败改回 `?` → 函数返回 `Err`，`expect` 失败。
    #[test]
    fn restore_collects_per_variable_failure_without_aborting() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);

            // 含 `=` 的名称对 `create_env_var_in_store` 非法（InvalidName）。
            // 备份侧能装下这种条目（备份是外部输入，不受写入校验约束）。
            let payload = payload_with(vec![
                backup_var("BAD=NAME", "x", REG_SZ),
                backup_var("GOOD_ONE", "ok", REG_SZ),
            ]);
            let path = write_env_backup_to(dir, &payload).unwrap();

            let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
                .expect("单变量失败不得使整次恢复失败");

            assert_eq!(outcome.applied, 1, "只有 GOOD_ONE 应写入成功");
            assert_eq!(
                outcome.failures.len(),
                1,
                "非法名必须被记为失败，实际: {:?}",
                outcome.failures
            );
            assert!(
                failure_names(&outcome, "BAD=NAME"),
                "失败原因必须点名 BAD=NAME，实际: {:?}",
                outcome.failures
            );
            assert!(hive.contains("GOOD_ONE"), "其余变量必须照常恢复");
            assert!(!hive.contains("BAD=NAME"), "非法名不得写入");
        });
    }

    /// **空备份的后果被显式固定**：两个 hive 皆空的备份会删光当前可写变量。
    ///
    /// 这是 `EnvBackupHives` 的 `system` / `user` 带 `#[serde(default)]`
    /// （Task 1 实现，本任务不得改动）的直接后果：形如
    /// `{"capturedAt":1,"hives":{}}` 的文件能反序列化成功，得到两个空列表，
    /// 差异计算于是把当前注册表里每个变量都判成 `Removed`。
    ///
    /// **裁定理由只写在 [`restore_in_stores`] 的 `# 空备份` 段，此处不复制** ——
    /// 同一条论证存两份必然漂移（复审已在同一处风险上抓到过逐字幸存的旧措辞）。
    /// 需要理由读那段；本测试只负责把**后果**钉住。
    ///
    /// 特别注意**不要**在这里写「默认模式会保护」之类的说法：
    /// [`restore_empty_backup_without_force_also_removes_all`] 断言
    /// `force == false` 时**同样删空** —— 空备份零冲突，K3 中止不会触发。
    ///
    /// 若将来有人加了防护，本测试会失败并强制其显式确认语义变更。
    ///
    /// 可证伪性：给恢复层加上「两 hive 皆空则拒绝」，本测试的 `expect` 立即失败。
    #[test]
    fn restore_empty_backup_removes_all_current_variables() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("KEEP_A", "a", REG_SZ);
            hive.seed("KEEP_B", "b", REG_SZ);

            let path = write_env_backup_to(dir, &payload_with(vec![])).unwrap();

            let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
                .expect("空备份是合法输入，恢复按差异语义执行");

            assert_eq!(outcome.applied, 2, "两个现存变量都应被判为删除");
            assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
            assert!(!hive.contains("KEEP_A"), "空备份 → 当前变量被删除");
            assert!(!hive.contains("KEEP_B"), "空备份 → 当前变量被删除");
        });
    }

    /// 空备份在**默认模式**下的行为同样被钉住：无冲突 → 不中止，照常删空。
    ///
    /// 与上一条配对：上一条走 `--force`，本条的 `force=false` 说明
    /// 「空备份删光变量」**不是 force 专属行为**，因此不能靠「不加 --force 就安全」
    /// 来规避 —— 也正因如此，文档必须如实写明这一后果。
    #[test]
    fn restore_empty_backup_without_force_also_removes_all() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("KEEP_A", "a", REG_SZ);

            let path = write_env_backup_to(dir, &payload_with(vec![])).unwrap();

            let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, false)
                .expect("空备份无冲突，默认模式不得中止");

            assert_eq!(outcome.applied, 1);
            assert!(!hive.contains("KEEP_A"), "默认模式下空备份同样会删空");
        });
    }

    /// 无操作路径（no-op restore）：备份与注册表完全一致时必须**零写入、零失败**，
    /// 且不触发广播门控。
    ///
    /// 存在的理由（复审 Minor）：`RestoreOutcome.skipped` 的恒 0 与
    /// [`restore_should_broadcast`] 的门控原先**只靠阅读保证**，没有任何测试覆盖
    /// `applied == 0` 这条路径。空备份测试只覆盖「全删」，覆盖不到「无事可做」。
    ///
    /// 构造方式：备份条目由 [`backup_var`] 生成（revision 与自身 name/type/value
    /// 自洽），注册表里是**同样的值** → `diff_one_hive` 走「revision 相同 → 不进差异」
    /// 分支（`core/src/backup.rs` 的 `Some((_, current_revision)) if ... == &var.revision`），
    /// 又因备份侧 `seen` 抑制了 `Removed`，于是 `changes.len() == 0`。
    ///
    /// **可证伪性**：
    /// - 把「revision 相同 → 不进差异」改错（例如恒 push `Modified`），`changes` 非空
    ///   → `applied` 不再为 0，第一条断言失败，广播门控也随之变 true；
    /// - 把 `restore_should_broadcast` 改成恒 `true`，最后一条断言失败。
    #[test]
    fn restore_noop_does_not_broadcast() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("SAME_A", "a", REG_SZ);
            hive.seed("SAME_B", "b", REG_EXPAND_SZ);

            // 两个条目都与注册表当前值/类型一致 → 零差异
            let payload = payload_with(vec![
                backup_var("SAME_A", "a", REG_SZ),
                backup_var("SAME_B", "b", REG_EXPAND_SZ),
            ]);
            let path = write_env_backup_to(dir, &payload).unwrap();

            let before = sorted_dir_names(dir);
            let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
                .expect("零差异的恢复必须成功（不是错误）");

            assert_eq!(outcome.applied, 0, "两端一致 → 不应有任何写入");
            assert_eq!(outcome.skipped, 0, "skipped 恒 0：没有「跳过」这条路径");
            assert!(
                outcome.failures.is_empty(),
                "零差异不应产生失败: {:?}",
                outcome.failures
            );
            // 前置条件：确认走的是「零差异」而不是「碰巧写失败」
            let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload)
                .expect("预览失败");
            assert_eq!(
                preview.changes.len(),
                0,
                "前置条件：值全同的备份必须产生零差异，否则本测试测的不是 no-op 路径"
            );
            assert!(
                !restore_should_broadcast(&outcome),
                "applied == 0 时不得广播 —— 什么都没变不该惊动已运行的进程"
            );
            // 顺带钉住 no-op 也不产生备份（与 restore_does_not_create_new_backup 互补）
            assert_eq!(
                sorted_dir_names(dir),
                before,
                "no-op 恢复同样不得产生新备份"
            );
        });
    }

    /// 广播门控的正向配对：`applied > 0` 时**必须**广播。
    ///
    /// 与 `restore_noop_does_not_broadcast` 合起来才构成完整契约
    /// 「有写入 → 广播，无写入 → 不广播」。只测一侧等于把恒 `true`/恒 `false`
    /// 这两种退化实现漏掉其一。
    #[test]
    fn restore_should_broadcast_when_something_applied() {
        let applied = RestoreOutcome {
            applied: 3,
            skipped: 0,
            failures: vec![],
        };
        assert!(
            restore_should_broadcast(&applied),
            "有实际写入时必须广播，否则运行中的进程会持有陈旧变量"
        );

        // 失败但确有写入（partial success）同样必须广播
        let partial = RestoreOutcome {
            applied: 1,
            skipped: 0,
            failures: vec!["[用户] windir: 受保护".into()],
        };
        assert!(
            restore_should_broadcast(&partial),
            "部分成功也算改动过注册表，必须广播"
        );
    }

    /// 恢复**不产生新备份**：整个恢复过程中备份目录里的文件数不变。
    ///
    /// 设计文档明文要求（「恢复自己不产生新备份」，否则每次恢复都新增文件、
    /// 与保留策略互相吞噬）。断言方式是直接的：把备份目录指向可写的临时目录
    /// （生产路径下 `backup_before_write()` 必然写进这里），恢复后目录内容必须
    /// **逐字节不变** —— 若实现里误接上写前备份，目录会多出一份 `env_backup_*.json`。
    ///
    /// 可证伪性：在 `restore_in_stores` 里加一次 `backup_env_vars()`，本测试失败。
    #[test]
    fn restore_does_not_create_new_backup() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("OLD_VAR", "old", REG_SZ);

            let payload = payload_with(vec![backup_var("NEW_VAR", "n", REG_SZ)]);
            let path = write_env_backup_to(dir, &payload).unwrap();

            let before: Vec<String> = sorted_dir_names(dir);
            assert_eq!(before.len(), 1, "前置条件：目录里只有我们写的那一份备份");

            restore_in_stores(&MemoryHive::new(true), &hive, &path, true).expect("恢复必须成功");

            let after = sorted_dir_names(dir);
            assert_eq!(
                after, before,
                "恢复不得产生新备份文件（否则与保留策略互相吞噬）"
            );
        });
    }

    /// 备份目录内的文件名，排序后返回（用于「目录内容不变」的逐字节比较）。
    fn sorted_dir_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("读取备份目录失败")
            .map(|e| {
                e.expect("目录项失败")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    /// `Removed` 条目的顺序必须**可复现**（Task 5 复审 Minor 的回归测试）。
    ///
    /// 多个 `Removed` 条目来自 `HashMap` 迭代，`RandomState` 让顺序随进程变化；
    /// 排序后必须恒定。断言方式：同一份输入构造**多份独立注册表快照**（每份新建
    /// `MemoryHive`，插入顺序不同），差异列表的名字序列必须逐项相同。
    ///
    /// **可证伪性（重要）**：光比对两次结果会因 `HashMap` 的哈希随机化在
    /// 单进程内的**种子复用**而漏检 —— 同一进程里 `RandomState` 的密钥是线程局部
    /// 缓存后复用的，两次遍历顺序可能恰好一致。因此这里刻意**改变插入顺序**
    /// 并把断言放在「跨 hive 的相对位置」上：未排序时 user 段的 `Removed`
    /// 会随插入顺序散开，排序后恒为升序 —— 删掉 `sort_changes` 调用即失败。
    #[test]
    fn preview_removed_entries_are_deterministically_sorted() {
        let payload = payload_with(vec![
            backup_var("A_ADDED", "a", REG_SZ),
            backup_var("Z_ADDED", "z", REG_SZ),
        ]);

        // 两份注册表：同样五个变量，**插入顺序相反**。未排序时
        // `Removed` 的相对顺序会跟着插入顺序（HashMap 迭代受它影响）漂移。
        let forward = MemoryHive::new(true);
        for name in [
            "B_REMOVED",
            "C_REMOVED",
            "D_REMOVED",
            "E_REMOVED",
            "F_REMOVED",
        ] {
            forward.seed(name, "x", REG_SZ);
        }
        let backward = MemoryHive::new(true);
        for name in [
            "F_REMOVED",
            "E_REMOVED",
            "D_REMOVED",
            "C_REMOVED",
            "B_REMOVED",
        ] {
            backward.seed(name, "x", REG_SZ);
        }

        let names = |hive: &MemoryHive| -> Vec<String> {
            preview_restore_in_stores(&MemoryHive::new(true), hive, &payload)
                .expect("预览失败")
                .changes
                .iter()
                .map(|c| format!("{:?}:{:?}", c.kind, c.name))
                .collect()
        };

        let first = names(&forward);
        let second = names(&backward);

        assert_eq!(
            first, second,
            "同一份差异集合必须给出同一顺序（Removed 来自 HashMap，未排序则不可复现）"
        );
        // 顺序本身也钉住：先 Added（按名升序）、再 Removed（按名升序）。
        assert_eq!(
            first,
            vec![
                "Added:\"A_ADDED\"",
                "Added:\"Z_ADDED\"",
                "Removed:\"B_REMOVED\"",
                "Removed:\"C_REMOVED\"",
                "Removed:\"D_REMOVED\"",
                "Removed:\"E_REMOVED\"",
                "Removed:\"F_REMOVED\"",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>(),
            "排序键是 (hive, kind, name)，kind 序为 added → modified → removed → conflict"
        );
    }

    /// user 段必须整体在 system 段之前，且排序后仍然成立（契约回归）。
    ///
    /// 可证伪性：把 [`hive_rank`] 的取值对调，本测试失败。
    #[test]
    fn preview_changes_keep_user_before_system_after_sort() {
        let sys = MemoryHive::new(true);
        let usr = MemoryHive::new(true);
        usr.seed("Z_USER", "u", REG_SZ); // user 侧 Removed
        sys.seed("A_SYS", "s", REG_SZ); // system 侧 Removed

        let payload = EnvBackupPayload {
            captured_at: 0,
            hives: EnvBackupHives {
                system: vec![],
                user: vec![],
            },
        };

        let preview = preview_restore_in_stores(&sys, &usr, &payload).expect("预览必须成功");

        assert_eq!(preview.removed, 2);
        assert_eq!(
            preview.changes[0].hive,
            EnvHive::User,
            "user 段必须整体在前（即便其变量名 A_SYS < Z_USER 也不能让 system 插队）"
        );
        assert_eq!(preview.changes[1].hive, EnvHive::System);
    }
}
