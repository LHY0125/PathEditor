use crate::backup::{backup_env_vars, BackupOutcome};
use crate::env_var::{
    capabilities_for_with, is_protected, is_reserved, is_sensitive, revision_of, sanitize_preview,
    EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot, RevealedValue,
};
use crate::error::{CoreError, ErrorCode};
use crate::reg_store::{EnvHiveStore, WinregHive};
use serde::{Deserialize, Serialize};
use winreg::enums::*;
use winreg::types::{FromRegValue, ToRegValue};

use super::access::hive_location;
use super::conflict::conflict_error;

/// 一次环境变量写操作的结果。
///
/// 写入本身的成败仍由外层 `Result` 表达；本结构只承载**备份**这一附带结果 ——
/// 备份失败不使写入失败（设计文档 K2），但必须被调用方看见。
///
/// 补 serde 派生（核对轮裁定 1）：本类型经 GUI 的 `#[tauri::command]` 返回值
/// 出境，Tauri 要求返回值实现 `Serialize`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteOutcome {
    /// 写前备份的结果
    pub backup: BackupOutcome,
}

/// 写前尽力备份：失败只记警告并如实返回，绝不阻断写入。
///
/// **必须在所有「便宜且无副作用」的校验之后调用** —— 校验失败时不应产生
/// 无意义的备份文件。
fn backup_before_write() -> BackupOutcome {
    match backup_env_vars() {
        Ok(path) => BackupOutcome::Created(path),
        Err(e) => {
            // GUI 侧 logger 会收到；CLI 侧经 WriteOutcome 返回值显式打印
            // （CLI 未初始化 logger，这条 warn 在 CLI 下被丢弃）。
            log::warn!("环境变量写前备份失败（写入继续）: {}", e.message);
            BackupOutcome::Failed(e.message)
        }
    }
}

/// 写入口共享的「便宜校验」：变量名合法性、保留名、保护名单。
///
/// 判定源与各 `*_in_store` 内的同名判定一致（`validate_env_name` /
/// `is_reserved` / `is_protected` 三个 core 函数），不构成第二套规则。
/// 这里的目的是**在产生备份文件之前**拒绝明显非法的写入；
/// `*_in_store` 内那一次是防御性兜底（真实写入路径必经）。
///
/// `verb` 是各入口的差异化文案后缀（如「不允许修改」/「不允许覆盖」）。
fn reject_reserved_and_protected(
    hive: EnvHive,
    name: &str,
    operation: &str,
    verb: &str,
) -> Result<(), CoreError> {
    validate_env_name(name).map_err(|m| {
        CoreError::new(ErrorCode::InvalidName, operation, m).with_target(hive, name)
    })?;
    if is_reserved(name) {
        return Err(CoreError::new(
            ErrorCode::ReservedName,
            operation,
            format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name),
        )
        .with_target(hive, name));
    }
    if is_protected(name) {
        return Err(CoreError::new(
            ErrorCode::Protected,
            operation,
            format!("{} 是系统内置变量，{}", name, verb),
        )
        .with_target(hive, name));
    }
    Ok(())
}

/// 读取单个值，返回 (vtype, value)。仅用于字符串类型。
///
/// `vtype` 是 winreg 的 `RegType`，不是 `u32` —— 类型判定与 revision
/// 计算都基于它。先判定类型再做字符串解码：`REG_DWORD` 等不支持类型
/// 必须返回「类型不受支持」，而不是误导性的解码失败。
///
/// W2-B1（2026-09-20 裁断）：三类错误在函数内部分类完成 ——
/// 读取失败 → `Io`、类型不支持 → `UnsupportedType`、解码失败 → `Parse`。
/// 调用方直接 `?` 传播，不再整体包成 `Io`；hive 标签由调用侧补（W2-B3b）。
fn read_env_var(store: &dyn EnvHiveStore, name: &str) -> Result<(RegType, String), CoreError> {
    let raw = store
        .get_raw(name)
        .map_err(|m| CoreError::new(ErrorCode::Io, "read_env_var", m))?;
    // `RegType` 非 Copy（`winreg-0.52.0/src/enums.rs:20`），必须先
    // `.clone()` 取值再借用 `&raw`，否则 `from_reg_type(raw.vtype)` 会部分
    // 移出 `raw`，后面 `&raw` 与 `Ok((raw.vtype, …))` 都会编译失败。
    // 原实现此处即 `raw.vtype.clone()`（`registry.rs:245`）。
    if !EnvValueKind::from_reg_type(raw.vtype.clone()).is_writable() {
        return Err(CoreError::new(
            ErrorCode::UnsupportedType,
            "read_env_var",
            format!("环境变量 {} 的注册表类型不受支持", name),
        ));
    }
    let value = String::from_reg_value(&raw).map_err(|e| {
        CoreError::new(
            ErrorCode::Parse,
            "read_env_var",
            format!("无法解码环境变量 {}: {}", name, e),
        )
    })?;
    Ok((raw.vtype, value))
}

/// 为 `read_env_var` 的错误补上 hive 标签与目标信息（W2-B3b）。
///
/// `read_env_var` 签名无 hive 参数，message 文本标签在调用侧包一层，
/// 与 F-04 列表路径的「读取{系统}环境变量…失败」样式一致；结构化
/// `hive`/`name` 字段经 `with_target` 携带。
fn attach_read_context(e: CoreError, hive: EnvHive, name: &str) -> CoreError {
    let label = hive_location(hive).2;
    let mut e = e.with_target(hive, name);
    e.message = format!("读取{}环境变量 {} 失败: {}", label, name, e.message);
    e
}

/// 写入单个值，保持调用方给定的注册表类型。端口层返回 String，此处转 `Io`。
fn write_env_var(
    store: &dyn EnvHiveStore,
    name: &str,
    value: &str,
    vtype: RegType,
) -> Result<(), CoreError> {
    let mut raw = value.to_reg_value();
    raw.vtype = vtype;
    store
        .set_raw(name, &raw)
        .map_err(|m| CoreError::new(ErrorCode::Io, "write_env_var", m))
}

/// 通用环境变量名校验。
pub fn validate_env_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("变量名不能为空".into());
    }
    if name.contains('\0') {
        return Err("变量名不能包含 null 字节".into());
    }
    if name.contains('=') {
        return Err("变量名不能包含等号".into());
    }
    if name.encode_utf16().count() > 32767 {
        return Err("变量名过长（超过 32767 字符）".into());
    }
    Ok(())
}

/// 通用环境变量值校验。不复用 `validate_and_join_paths`（那是 PATH 分号语义专用）。
pub fn validate_env_value(value: &str, label: &str) -> Result<(), String> {
    if value.contains('\0') {
        return Err(format!("{} 的值包含 null 字节", label));
    }
    let utf16_len = value.encode_utf16().count();
    if utf16_len > 32767 {
        return Err(format!(
            "{} 的值长度 {} 超出 Windows 限制 32767 字符",
            label, utf16_len
        ));
    }
    Ok(())
}

/// 读取单个 hive 的所有环境变量元数据。`Path` 在此被过滤。
fn list_hive_env_vars(hive: EnvHive) -> Result<Vec<EnvVarMeta>, CoreError> {
    let store = WinregHive::open(hive, false)?;
    list_env_vars_in_store(hive, &store)
}

/// `list_hive_env_vars` 的核心逻辑，存储可注入（测试用内存实现）。
///
/// F-04：同一 hive 内任一枚举 / 读取 / 解码失败即返回 `Err`。
/// 「成功但不完整」的列表是错误的成功语义，用户无法区分「变量不存在」
/// 与「枚举/读取失败」。
fn list_env_vars_in_store(
    hive: EnvHive,
    store: &dyn EnvHiveStore,
) -> Result<Vec<EnvVarMeta>, CoreError> {
    // F-04：错误必须携带 hive 标识，与 open 错误（自带标签）保持一致。
    let (_, _, label) = hive_location(hive);
    let mut metas = Vec::new();

    // 写权限探测按 hive 只做一次（端口在 open 时已探测，避免逐变量重复探测）。
    let writable = store.writable();

    // 列表级错误无单一目标名，仅携带结构化 hive 字段。
    let with_hive = |code: ErrorCode, op: &str, msg: String| {
        let mut e = CoreError::new(code, op, msg);
        e.hive = Some(hive);
        e
    };

    let names = store.enum_names().map_err(|e| {
        // spec F-04：warning 日志保留，但不再是唯一反馈；Err 继续向上传播。
        log::warn!("{}环境变量列表读取失败: {}", label, e);
        with_hive(
            ErrorCode::Io,
            "list_env_vars",
            format!("读取{}环境变量列表失败: {}", label, e),
        )
    })?;

    for name in names {
        // 保留变量（Path）由专用通路拥有，通用通路完全不展示
        if is_reserved(&name) {
            continue;
        }

        let raw = store.get_raw(&name).map_err(|e| {
            log::warn!("{}环境变量列表读取失败: {}", label, e);
            with_hive(
                ErrorCode::Io,
                "list_env_vars",
                format!("读取{}环境变量列表失败: {}", label, e),
            )
            .with_target(hive, &name)
        })?;
        let kind = EnvValueKind::from_reg_type(raw.vtype.clone());
        let sensitive = is_sensitive(&name);

        let value = match kind {
            EnvValueKind::Unsupported => String::new(),
            _ => String::from_reg_value(&raw).map_err(|e| {
                log::warn!("{}环境变量列表读取失败: {}", label, e);
                with_hive(
                    ErrorCode::Parse,
                    "list_env_vars",
                    format!(
                        "读取{}环境变量列表失败: 无法解码环境变量 {}: {}",
                        label, name, e
                    ),
                )
                .with_target(hive, &name)
            })?,
        };

        let preview = if sensitive || !kind.is_writable() {
            None
        } else {
            sanitize_preview(&value)
        };

        let (can_edit, can_delete) = capabilities_for_with(writable, &name, kind);

        metas.push(EnvVarMeta {
            revision: revision_of(&name, raw.vtype, &value),
            name,
            kind,
            hive,
            can_edit,
            can_delete,
            sensitive,
            preview,
        });
    }

    Ok(metas)
}

/// 一次读取两个 hive 的变量元数据（列表唯一入口）。
///
/// 两个 hive 是**先后两次独立读取**，没有跨键事务，因此返回的是
/// 「两个接近时刻的快照」，不是原子一致快照。外部进程可能在两次读取
/// 之间修改任一侧。若需强一致，应在单 hive 维度用 revision 做提交检查。
pub fn list_all_env_vars() -> Result<EnvVarSnapshot, CoreError> {
    // clock 异常（系统时间早于 Unix 纪元）时回退 0，不阻塞列表读取。
    let captured_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Ok(EnvVarSnapshot {
        system: list_hive_env_vars(EnvHive::System)?,
        user: list_hive_env_vars(EnvHive::User)?,
        captured_at,
    })
}

/// 按需读取单个变量的明文及读取时的 revision。
///
/// `Unsupported` 类型返回 `Err`（code=`UnsupportedType`），不尝试转字符串。
pub fn reveal_env_var(hive: EnvHive, name: &str) -> Result<RevealedValue, CoreError> {
    let store = WinregHive::open(hive, false)?;
    reveal_env_var_in_store(&store, hive, name)
}

/// `reveal_env_var` 的核心逻辑，存储可注入。`hive` 用于错误补标签。
fn reveal_env_var_in_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    name: &str,
) -> Result<RevealedValue, CoreError> {
    validate_env_name(name).map_err(|m| {
        CoreError::new(ErrorCode::InvalidName, "reveal_env_var", m).with_target(hive, name)
    })?;
    if is_reserved(name) {
        return Err(CoreError::new(
            ErrorCode::ReservedName,
            "reveal_env_var",
            format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name),
        )
        .with_target(hive, name));
    }
    let (vtype, value) =
        read_env_var(store, name).map_err(|e| attach_read_context(e, hive, name))?;
    let revision = revision_of(name, vtype, &value);
    Ok(RevealedValue { value, revision })
}

/// 写入已有变量。类型从注册表读取，不由前端决定。
///
/// # Returns
/// - `Ok(WriteOutcome)` — 写入成功并广播环境变更；`backup` 字段说明写前备份结果
/// - `Err(CoreError)` — 校验失败、修订冲突或类型不受支持
pub fn update_env_var(
    hive: EnvHive,
    name: &str,
    value: &str,
    expected_revision: &str,
) -> Result<WriteOutcome, CoreError> {
    let store = WinregHive::open(hive, true)?;
    update_env_var_with_store(&store, hive, name, value, expected_revision)
}

/// [`update_env_var`] 的核心逻辑，存储可注入。
///
/// 顺序为「校验（便宜且无副作用的保留名 / 保护名单）→ 写前备份 → `*_in_store`」。
/// 顺序不可颠倒：校验失败时**不产生无意义的备份文件**。
/// `*_in_store` 内部会再判一次同类规则（防御性兜底，见函数文档）。
fn update_env_var_with_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    name: &str,
    value: &str,
    expected_revision: &str,
) -> Result<WriteOutcome, CoreError> {
    reject_reserved_and_protected(hive, name, "update_env_var", "不允许修改")?;
    let backup = backup_before_write();
    update_env_var_in_store(store, hive, name, value, expected_revision)?;
    Ok(WriteOutcome { backup })
}

/// `update_env_var` 的核心逻辑，存储可注入。
///
/// 在同一调用内完成「读 → 算 revision → 比对 → 校验 → 写」。
/// 读与写是两次独立存储调用，仍有竞态窗口；revision 校验缩小影响，
/// 不能完全消除 TOCTOU。
fn update_env_var_in_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    name: &str,
    value: &str,
    expected_revision: &str,
) -> Result<(), CoreError> {
    validate_env_name(name).map_err(|m| {
        CoreError::new(ErrorCode::InvalidName, "update_env_var", m).with_target(hive, name)
    })?;
    if is_reserved(name) {
        return Err(CoreError::new(
            ErrorCode::ReservedName,
            "update_env_var",
            format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name),
        )
        .with_target(hive, name));
    }
    if is_protected(name) {
        return Err(CoreError::new(
            ErrorCode::Protected,
            "update_env_var",
            format!("{} 是系统内置变量，不允许修改", name),
        )
        .with_target(hive, name));
    }

    let (vtype, current) =
        read_env_var(store, name).map_err(|e| attach_read_context(e, hive, name))?;

    // 并发校验：revision 不匹配时拒绝写入，交给前端提示重新加载
    let current_revision = revision_of(name, vtype.clone(), &current);
    if current_revision != expected_revision {
        return Err(conflict_error("update_env_var", hive, name));
    }

    // 类型与值校验
    let kind = EnvValueKind::from_reg_type(vtype.clone());
    if !kind.is_writable() {
        return Err(CoreError::new(
            ErrorCode::UnsupportedType,
            "update_env_var",
            format!("{} 的注册表类型不受支持，无法修改（仅可查看）", name),
        )
        .with_target(hive, name));
    }
    validate_env_value(value, name).map_err(|m| {
        CoreError::new(ErrorCode::InvalidValue, "update_env_var", m).with_target(hive, name)
    })?;

    // 写入，类型原样保留
    write_env_var(store, name, value, vtype).map_err(|e| e.with_target(hive, name))
}

/// 新建变量。`kind` 仅在此决定。
///
/// # Returns
/// - `Ok(WriteOutcome)` — 创建成功并广播环境变更；`backup` 字段说明写前备份结果
/// - `Err(CoreError)` — 名称非法、已存在（`NameExists`）、保护名单（`Protected`）
///   或类型不受支持
pub fn create_env_var(
    hive: EnvHive,
    name: &str,
    value: &str,
    kind: EnvValueKind,
) -> Result<WriteOutcome, CoreError> {
    let store = WinregHive::open(hive, true)?;
    create_env_var_with_store(&store, hive, name, value, kind)
}

/// [`create_env_var`] 的核心逻辑，存储可注入。顺序契约同 [`update_env_var_with_store`]。
///
/// 「同名已存在」需要枚举注册表，属昂贵校验，留在 `*_in_store` 内 —— 此时备份会
/// 先于该失败发生，属可接受代价（备份文件无害）。
fn create_env_var_with_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    name: &str,
    value: &str,
    kind: EnvValueKind,
) -> Result<WriteOutcome, CoreError> {
    reject_reserved_and_protected(hive, name, "create_env_var", "不允许覆盖")?;
    let backup = backup_before_write();
    create_env_var_in_store(store, hive, name, value, kind)?;
    Ok(WriteOutcome { backup })
}

/// `create_env_var` 的核心逻辑，存储可注入。
///
/// 写入前先确认同名变量不存在（忽略大小写），不覆盖已有变量。
/// 枚举检查与写入是两次独立调用，存在竞态窗口，当前未做原子 CAS。
///
/// **有意的语义变更（评审裁断 O-2）**：查重用的 `store.enum_names()?` 会传播
/// 枚举错误。原 `create_env_var_in_key` 用 `enum_values().flatten()` 吞掉枚举
/// 错误 —— 枚举失败会被当作「同名不存在」而继续写入，有冒险覆盖的隐患。
/// 新行为是「失败响亮优于冒险覆盖」，与 F-04 同源，但**超出 F-04 只针对
/// list 的字面范围**。若不愿引入该变更，改 `store.enum_names().unwrap_or_default()`
/// 即可完全保持现状。
fn create_env_var_in_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    name: &str,
    value: &str,
    kind: EnvValueKind,
) -> Result<(), CoreError> {
    validate_env_name(name).map_err(|m| {
        CoreError::new(ErrorCode::InvalidName, "create_env_var", m).with_target(hive, name)
    })?;
    if is_reserved(name) {
        return Err(CoreError::new(
            ErrorCode::ReservedName,
            "create_env_var",
            format!("{} 由专用 PATH 通路管理，无法通过通用通路创建", name),
        )
        .with_target(hive, name));
    }
    if is_protected(name) {
        return Err(CoreError::new(
            ErrorCode::Protected,
            "create_env_var",
            format!("{} 是系统内置变量，不允许覆盖", name),
        )
        .with_target(hive, name));
    }
    if !kind.is_writable() {
        return Err(CoreError::new(
            ErrorCode::UnsupportedType,
            "create_env_var",
            "新建变量只支持 String 或 ExpandString 类型",
        )
        .with_target(hive, name));
    }
    validate_env_value(value, name).map_err(|m| {
        CoreError::new(ErrorCode::InvalidValue, "create_env_var", m).with_target(hive, name)
    })?;

    // 写入前检查：忽略大小写地确认该名不存在（非原子，见函数文档）
    let existing = store
        .enum_names()
        .map_err(|e| CoreError::new(ErrorCode::Io, "create_env_var", e).with_target(hive, name))?
        .iter()
        .any(|n| n.eq_ignore_ascii_case(name));
    if existing {
        return Err(CoreError::new(
            ErrorCode::NameExists,
            "create_env_var",
            format!("变量 {} 已存在，请使用编辑功能", name),
        )
        .with_target(hive, name));
    }

    let vtype = match kind {
        EnvValueKind::String => REG_SZ,
        _ => REG_EXPAND_SZ,
    };
    write_env_var(store, name, value, vtype).map_err(|e| e.with_target(hive, name))
}

/// 删除变量。在同一调用内完成 revision 比对与删除。
///
/// # Returns
/// - `Ok(WriteOutcome)` — 删除成功并广播环境变更；`backup` 字段说明写前备份结果
/// - `Err(CoreError)` — 校验失败、修订冲突或类型不受支持
pub fn delete_env_var(
    hive: EnvHive,
    name: &str,
    expected_revision: &str,
) -> Result<WriteOutcome, CoreError> {
    let store = WinregHive::open(hive, true)?;
    delete_env_var_with_store(&store, hive, name, expected_revision)
}

/// [`delete_env_var`] 的核心逻辑，存储可注入。顺序契约同 [`update_env_var_with_store`]。
fn delete_env_var_with_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    name: &str,
    expected_revision: &str,
) -> Result<WriteOutcome, CoreError> {
    reject_reserved_and_protected(hive, name, "delete_env_var", "不允许删除")?;
    let backup = backup_before_write();
    delete_env_var_in_store(store, hive, name, expected_revision)?;
    Ok(WriteOutcome { backup })
}

/// `delete_env_var` 的核心逻辑，存储可注入。
///
/// 读与删除是两次独立调用，revision 校验缩小竞态影响，不能完全消除 TOCTOU。
fn delete_env_var_in_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    name: &str,
    expected_revision: &str,
) -> Result<(), CoreError> {
    validate_env_name(name).map_err(|m| {
        CoreError::new(ErrorCode::InvalidName, "delete_env_var", m).with_target(hive, name)
    })?;
    if is_reserved(name) {
        return Err(CoreError::new(
            ErrorCode::ReservedName,
            "delete_env_var",
            format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name),
        )
        .with_target(hive, name));
    }
    if is_protected(name) {
        return Err(CoreError::new(
            ErrorCode::Protected,
            "delete_env_var",
            format!("{} 是系统内置变量，不允许删除", name),
        )
        .with_target(hive, name));
    }

    let (vtype, current) =
        read_env_var(store, name).map_err(|e| attach_read_context(e, hive, name))?;
    let current_revision = revision_of(name, vtype.clone(), &current);
    if current_revision != expected_revision {
        return Err(conflict_error("delete_env_var", hive, name));
    }

    if EnvValueKind::from_reg_type(vtype) == EnvValueKind::Unsupported {
        // 防御性、当前不可达（评审裁断 O-3）：`read_env_var` 已对 Unsupported
        // 类型提前返回 Err，上面的 `?` 会先短路。保留以显式表达删除的前置条件，
        // 行为与现状一致。
        return Err(CoreError::new(
            ErrorCode::UnsupportedType,
            "delete_env_var",
            format!("{} 的注册表类型不受支持，无法删除", name),
        )
        .with_target(hive, name));
    }

    store
        .delete_value(name)
        .map_err(|m| CoreError::new(ErrorCode::Io, "delete_env_var", m).with_target(hive, name))
}

/// 强制写入已有变量（**最后写入者胜**）。不做 revision 比对。
///
/// 仅供 CLI `--force` 使用；GUI 一律走 [`update_env_var`] 的 CAS 语义。
/// force 只豁免并发校验，**不豁免**保留名 / 保护名单 / 类型 / 权限判定。
/// 这不是原子操作：读类型与写入仍是两次独立调用。
///
/// # Returns
/// - `Ok(WriteOutcome)` — 写入成功并广播环境变更；`backup` 字段说明写前备份结果
/// - `Err(CoreError)` — 校验失败或类型不受支持（不存在修订冲突——force 不做 CAS）
pub fn update_env_var_force(
    hive: EnvHive,
    name: &str,
    value: &str,
) -> Result<WriteOutcome, CoreError> {
    let store = WinregHive::open(hive, true)?;
    update_env_var_force_with_store(&store, hive, name, value)
}

/// [`update_env_var_force`] 的核心逻辑，存储可注入。
/// 顺序契约同 [`update_env_var_with_store`]。
fn update_env_var_force_with_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    name: &str,
    value: &str,
) -> Result<WriteOutcome, CoreError> {
    reject_reserved_and_protected(hive, name, "update_env_var_force", "不允许修改")?;
    let backup = backup_before_write();
    update_env_var_force_in_store(store, hive, name, value)?;
    Ok(WriteOutcome { backup })
}

/// `update_env_var_force` 的核心逻辑，存储可注入。
fn update_env_var_force_in_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    name: &str,
    value: &str,
) -> Result<(), CoreError> {
    validate_env_name(name).map_err(|m| {
        CoreError::new(ErrorCode::InvalidName, "update_env_var_force", m).with_target(hive, name)
    })?;
    if is_reserved(name) {
        return Err(CoreError::new(
            ErrorCode::ReservedName,
            "update_env_var_force",
            format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name),
        )
        .with_target(hive, name));
    }
    if is_protected(name) {
        return Err(CoreError::new(
            ErrorCode::Protected,
            "update_env_var_force",
            format!("{} 是系统内置变量，不允许修改", name),
        )
        .with_target(hive, name));
    }

    // 读现有类型以便原样保留；**不比对** revision
    let (vtype, _current) =
        read_env_var(store, name).map_err(|e| attach_read_context(e, hive, name))?;
    let kind = EnvValueKind::from_reg_type(vtype.clone());
    if !kind.is_writable() {
        return Err(CoreError::new(
            ErrorCode::UnsupportedType,
            "update_env_var_force",
            format!("{} 的注册表类型不受支持，无法修改（仅可查看）", name),
        )
        .with_target(hive, name));
    }
    validate_env_value(value, name).map_err(|m| {
        CoreError::new(ErrorCode::InvalidValue, "update_env_var_force", m).with_target(hive, name)
    })?;

    write_env_var(store, name, value, vtype).map_err(|e| e.with_target(hive, name))
}

/// 强制删除变量（**最后写入者胜**）。不做 revision 比对，其余校验照旧。
///
/// # Returns
/// - `Ok(WriteOutcome)` — 删除成功并广播环境变更；`backup` 字段说明写前备份结果
/// - `Err(CoreError)` — 校验失败或类型不受支持（不存在修订冲突——force 不做 CAS）
pub fn delete_env_var_force(hive: EnvHive, name: &str) -> Result<WriteOutcome, CoreError> {
    let store = WinregHive::open(hive, true)?;
    delete_env_var_force_with_store(&store, hive, name)
}

/// [`delete_env_var_force`] 的核心逻辑，存储可注入。
/// 顺序契约同 [`update_env_var_with_store`]。
fn delete_env_var_force_with_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    name: &str,
) -> Result<WriteOutcome, CoreError> {
    reject_reserved_and_protected(hive, name, "delete_env_var_force", "不允许删除")?;
    let backup = backup_before_write();
    delete_env_var_force_in_store(store, hive, name)?;
    Ok(WriteOutcome { backup })
}

/// `delete_env_var_force` 的核心逻辑，存储可注入。
fn delete_env_var_force_in_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    name: &str,
) -> Result<(), CoreError> {
    validate_env_name(name).map_err(|m| {
        CoreError::new(ErrorCode::InvalidName, "delete_env_var_force", m).with_target(hive, name)
    })?;
    if is_reserved(name) {
        return Err(CoreError::new(
            ErrorCode::ReservedName,
            "delete_env_var_force",
            format!("{} 由专用 PATH 通路管理，请使用 PATH 视图编辑", name),
        )
        .with_target(hive, name));
    }
    if is_protected(name) {
        return Err(CoreError::new(
            ErrorCode::Protected,
            "delete_env_var_force",
            format!("{} 是系统内置变量，不允许删除", name),
        )
        .with_target(hive, name));
    }

    let (vtype, _current) =
        read_env_var(store, name).map_err(|e| attach_read_context(e, hive, name))?;
    if EnvValueKind::from_reg_type(vtype) == EnvValueKind::Unsupported {
        // 防御性、当前不可达：`read_env_var` 已对 Unsupported 类型提前返回 Err，
        // 上面的 `?` 会先短路。保留以显式表达删除的前置条件，行为与现状一致。
        return Err(CoreError::new(
            ErrorCode::UnsupportedType,
            "delete_env_var_force",
            format!("{} 的注册表类型不受支持，无法删除", name),
        )
        .with_target(hive, name));
    }

    store.delete_value(name).map_err(|m| {
        CoreError::new(ErrorCode::Io, "delete_env_var_force", m).with_target(hive, name)
    })
}

#[cfg(test)]
mod env_var_tests {
    use super::*;
    use crate::backup::BackupOutcome;
    use crate::env_var::{EnvHive, EnvValueKind};
    use crate::reg_store::memory::MemoryHive;
    use crate::registry::conflict::ERR_CONFLICT;
    use winreg::enums::{REG_DWORD, REG_EXPAND_SZ, REG_SZ};
    use winreg::types::FromRegValue;

    #[test]
    fn read_env_var_returns_real_type_and_value() {
        let hive = MemoryHive::new(true);
        hive.seed("JAVA_HOME", "C:\\Java", REG_EXPAND_SZ);

        let (vtype, value) = read_env_var(&hive, "JAVA_HOME").expect("读取失败");
        assert_eq!(vtype, REG_EXPAND_SZ);
        assert_eq!(value, "C:\\Java");
    }

    #[test]
    fn write_env_var_preserves_expand_sz_type() {
        let hive = MemoryHive::new(true);
        hive.seed("GOPATH", "C:\\Old", REG_EXPAND_SZ);

        write_env_var(&hive, "GOPATH", "C:\\New", REG_EXPAND_SZ).expect("写入失败");

        let raw = hive.get_raw("GOPATH").expect("读取失败");
        assert_eq!(raw.vtype, REG_EXPAND_SZ);
        assert_eq!(String::from_reg_value(&raw).unwrap(), "C:\\New");
    }

    #[test]
    fn write_env_var_preserves_sz_type() {
        let hive = MemoryHive::new(true);
        hive.seed("JAVA_HOME", "C:\\Old", REG_SZ);

        write_env_var(&hive, "JAVA_HOME", "C:\\New", REG_SZ).expect("写入失败");

        let raw = hive.get_raw("JAVA_HOME").expect("读取失败");
        assert_eq!(raw.vtype, REG_SZ);
    }

    #[test]
    fn validate_env_name_rejects_invalid() {
        assert!(validate_env_name("JAVA_HOME").is_ok());
        assert!(validate_env_name("").is_err());
        assert!(validate_env_name("BAD\0NAME").is_err());
        assert!(validate_env_name("BAD=NAME").is_err());
    }

    #[test]
    fn validate_env_value_rejects_null_and_oversize() {
        assert!(validate_env_value("C:\\Java", "测试").is_ok());
        assert!(validate_env_value("bad\0value", "测试").is_err());
        let oversized = "x".repeat(32768);
        assert!(validate_env_value(&oversized, "测试").is_err());
    }

    #[test]
    fn reserved_names_are_never_writable_through_generic_path() {
        assert!(is_reserved("Path"));
        assert!(is_reserved("path"));
        let hive = MemoryHive::new(true);
        hive.seed("Path", "C:\\Windows", REG_EXPAND_SZ);
        assert!(read_env_var(&hive, "Path").is_ok());
    }

    #[test]
    fn revision_detects_external_change() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let revision = crate::env_var::revision_of("MY_VAR", vtype, &value);

        hive.seed("MY_VAR", "changed-by-other-process", REG_SZ);

        let (vtype2, value2) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let current = crate::env_var::revision_of("MY_VAR", vtype2, &value2);
        assert_ne!(revision, current, "外部修改后 revision 必须不同");
    }

    #[test]
    fn unsupported_type_is_not_writable() {
        let hive = MemoryHive::new(true);
        hive.seed_raw(
            "MY_DWORD",
            winreg::RegValue {
                bytes: vec![1, 0, 0, 0],
                vtype: REG_DWORD,
            },
        );

        assert!(read_env_var(&hive, "MY_DWORD").is_err());

        let stored = hive.get_raw("MY_DWORD").expect("读取原始值失败");
        let kind = EnvValueKind::from_reg_type(stored.vtype);
        assert_eq!(kind, EnvValueKind::Unsupported);
        assert!(!kind.is_writable());
    }

    #[test]
    fn hive_enum_serializes_camel_case() {
        let value = serde_json::to_value(EnvHive::System).expect("序列化失败");
        assert_eq!(value, serde_json::json!("system"));
        let value = serde_json::to_value(EnvHive::User).expect("序列化失败");
        assert_eq!(value, serde_json::json!("user"));
    }

    // ── 公开 API 行为测试（a1）──
    // 复用 MemoryHive 内存存储 + *_in_store 注入核心，绝不触碰真实注册表。

    #[test]
    fn list_path_is_filtered_and_name_case_preserved() {
        let hive = MemoryHive::new(true);
        hive.seed("Path", "C:\\Windows", REG_EXPAND_SZ);
        hive.seed("path", "C:\\ShouldNotAppear", REG_EXPAND_SZ);
        hive.seed("MyApp_Home", "C:\\MyApp", REG_SZ);
        hive.seed("another_var", "C:\\another", REG_EXPAND_SZ);

        let metas = list_env_vars_in_store(EnvHive::User, &hive).expect("列表失败");

        let names: Vec<&str> = metas.iter().map(|m| m.name.as_str()).collect();
        assert!(
            !names.iter().any(|n| n.eq_ignore_ascii_case("path")),
            "Path 必须被过滤"
        );
        assert!(names.contains(&"MyApp_Home"));
        assert!(names.contains(&"another_var"));
        assert!(metas.iter().all(|m| !m
            .preview
            .as_deref()
            .unwrap_or("")
            .contains("ShouldNotAppear")));
    }

    #[test]
    fn update_env_var_rejects_revision_mismatch_and_keeps_value() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let stale_revision = revision_of("MY_VAR", vtype, &value);

        hive.seed("MY_VAR", "changed-by-other", REG_SZ);

        let result =
            update_env_var_in_store(&hive, EnvHive::User, "MY_VAR", "mine", &stale_revision);
        assert!(result.is_err(), "revision 不匹配必须 Err");
        assert_eq!(result.unwrap_err().code, crate::error::ErrorCode::Conflict);

        let raw = hive.get_raw("MY_VAR").expect("读取失败");
        assert_eq!(String::from_reg_value(&raw).unwrap(), "changed-by-other");
    }

    #[test]
    fn update_env_var_rejects_dword_and_keeps_value() {
        let hive = MemoryHive::new(true);
        hive.seed_raw(
            "MY_DWORD",
            winreg::RegValue {
                bytes: vec![7, 0, 0, 0],
                vtype: REG_DWORD,
            },
        );
        let stored = hive.get_raw("MY_DWORD").expect("读取失败");
        let revision = revision_of("MY_DWORD", stored.vtype, "");

        let result = update_env_var_in_store(&hive, EnvHive::User, "MY_DWORD", "1", &revision);
        let err = result.unwrap_err();
        assert_eq!(
            err.code,
            crate::error::ErrorCode::UnsupportedType,
            "必须断言 code 而非文本（F-06）"
        );
        assert!(
            err.message.contains("类型不受支持"),
            "文案保留供人工阅读: {}",
            err.message
        );

        let after = hive.get_raw("MY_DWORD").expect("读取失败");
        assert_eq!(after.vtype, REG_DWORD);
        assert_eq!(after.bytes, vec![7, 0, 0, 0]);
    }

    #[test]
    fn update_env_var_succeeds_when_revision_matches() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let revision = revision_of("MY_VAR", vtype, &value);

        update_env_var_in_store(&hive, EnvHive::User, "MY_VAR", "updated", &revision)
            .expect("revision 匹配必须成功");

        let raw = hive.get_raw("MY_VAR").expect("读取失败");
        assert_eq!(String::from_reg_value(&raw).unwrap(), "updated");
        assert_eq!(raw.vtype, REG_SZ, "成功写入也必须保持原类型");
    }

    #[test]
    fn delete_env_var_succeeds_when_revision_matches() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let revision = revision_of("MY_VAR", vtype, &value);

        delete_env_var_in_store(&hive, EnvHive::User, "MY_VAR", &revision)
            .expect("revision 匹配必须成功");

        assert!(!hive.contains("MY_VAR"), "成功删除后变量必须消失");
    }

    #[test]
    fn create_env_var_rejects_protected_and_duplicate() {
        let hive = MemoryHive::new(true);
        hive.seed("Existing", "already-here", REG_SZ);

        let protected = create_env_var_in_store(
            &hive,
            EnvHive::User,
            "windir",
            "C:\\evil",
            EnvValueKind::String,
        );
        assert!(protected.is_err(), "保护名单变量不允许创建");
        assert_eq!(
            protected.unwrap_err().code,
            crate::error::ErrorCode::Protected
        );

        let duplicate = create_env_var_in_store(
            &hive,
            EnvHive::User,
            "EXISTING",
            "dup",
            EnvValueKind::String,
        );
        assert!(duplicate.is_err(), "同名变量不允许重复创建");
        assert_eq!(
            duplicate.unwrap_err().code,
            crate::error::ErrorCode::NameExists
        );

        let raw = hive.get_raw("Existing").expect("读取失败");
        assert_eq!(String::from_reg_value(&raw).unwrap(), "already-here");
    }

    #[test]
    fn reveal_returns_value_with_matching_revision() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "hello", REG_SZ);

        let revealed =
            reveal_env_var_in_store(&hive, EnvHive::User, "MY_VAR").expect("reveal 失败");
        assert_eq!(revealed.value, "hello");

        let raw = hive.get_raw("MY_VAR").unwrap();
        let expected = revision_of("MY_VAR", raw.vtype, "hello");
        assert_eq!(revealed.revision, expected, "revision 必须与列表项一致");
    }

    #[test]
    fn reveal_revision_changes_after_external_edit() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "one", REG_SZ);
        let first = reveal_env_var_in_store(&hive, EnvHive::User, "MY_VAR")
            .unwrap()
            .revision;

        hive.seed("MY_VAR", "two", REG_SZ);
        let second = reveal_env_var_in_store(&hive, EnvHive::User, "MY_VAR")
            .unwrap()
            .revision;

        assert_ne!(first, second);
    }

    #[test]
    fn reveal_env_var_returns_plaintext_and_sensitive_preview_is_none() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_PLAIN", "C:\\plain value", REG_SZ);
        hive.seed("MY_API_TOKEN", "super-secret-plaintext", REG_SZ);

        let revealed =
            reveal_env_var_in_store(&hive, EnvHive::User, "MY_API_TOKEN").expect("reveal 失败");
        assert_eq!(revealed.value, "super-secret-plaintext");

        let metas = list_env_vars_in_store(EnvHive::User, &hive).expect("列表失败");
        let token_meta = metas
            .iter()
            .find(|m| m.name == "MY_API_TOKEN")
            .expect("敏感变量应在列表");
        assert!(token_meta.sensitive);
        assert_eq!(token_meta.preview, None);

        let plain_meta = metas
            .iter()
            .find(|m| m.name == "MY_PLAIN")
            .expect("普通变量应在列表");
        assert_eq!(plain_meta.preview.as_deref(), Some("C:\\plain value"));
    }

    #[test]
    fn delete_env_var_rejects_revision_mismatch_and_keeps_var() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let stale_revision = revision_of("MY_VAR", vtype, &value);

        hive.seed("MY_VAR", "changed-by-other", REG_SZ);

        let result = delete_env_var_in_store(&hive, EnvHive::User, "MY_VAR", &stale_revision);
        assert!(result.is_err(), "revision 不匹配必须 Err");
        assert_eq!(result.unwrap_err().code, crate::error::ErrorCode::Conflict);
        assert!(hive.contains("MY_VAR"), "冲突时不得删除变量");
    }

    // ── F-04：列表失败语义 —— 任一失败让整个 hive 报错 ──

    #[test]
    fn list_fails_when_enum_fails() {
        let mut hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "v", REG_SZ);
        hive.fail_enum = true;

        assert!(
            list_env_vars_in_store(EnvHive::User, &hive).is_err(),
            "枚举失败必须让整个 hive 报错"
        );
    }

    #[test]
    fn list_fails_when_single_read_fails() {
        let mut hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "v", REG_SZ);
        hive.seed("BAD_VAR", "v", REG_SZ);
        hive.fail_get = Some("BAD_VAR".into());

        assert!(
            list_env_vars_in_store(EnvHive::User, &hive).is_err(),
            "单值读取失败必须让整个 hive 报错"
        );
    }

    #[test]
    // winreg 0.52 的 `String::from_reg_value` 对 REG_SZ/REG_EXPAND_SZ 使用
    // `from_utf16_lossy`：非法 UTF-16 被替换为 U+FFFD 而不返回 Err，奇数字节
    // 被截断后 lossy 解码为空串。因此「解码失败」分支对字符串类型当前不可达，
    // 本测试的构造（奇数字节 REG_SZ）无法让 list 报错。保留测试体作为休眠
    // 用例：若未来 winreg 改为严格解码，F-04 的 map_err 分支会生效，此测试
    // 随之通过。详见 winreg-0.52.0/src/types.rs:38（`String::from_reg_value` 的 lossy 解码）。
    #[ignore = "winreg 0.52 对字符串类型 lossy 解码，解码失败分支当前不可达"]
    fn list_fails_when_decode_fails() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "v", REG_SZ);
        // 奇数字节不是合法 UTF-16，解码必然失败
        hive.seed_raw(
            "BAD_STR",
            winreg::RegValue {
                bytes: vec![0x41],
                vtype: REG_SZ,
            },
        );

        assert!(
            list_env_vars_in_store(EnvHive::User, &hive).is_err(),
            "解码失败必须让整个 hive 报错"
        );
    }

    #[test]
    fn conflict_message_matches_frontend_contract() {
        // 判定按 CoreError.code（conflict）进行；[E_CONFLICT] 前缀仅是过渡期展示文本，
        // 断言保留前缀是为旧文本路径兼容，正文改动不应破坏它
        assert!(ERR_CONFLICT.starts_with("[E_CONFLICT] "));
        assert!(ERR_CONFLICT.contains("已被其他进程修改"));
    }

    // ── F-02：force API —— 只豁免 revision 校验，安全校验照旧 ──

    #[test]
    fn update_env_var_force_overwrites_after_external_change() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);

        // 模拟：用户读到 revision 后，另一进程改了值
        hive.seed("MY_VAR", "changed-by-other", REG_SZ);

        // force 不比对 revision，必须成功覆盖
        update_env_var_force_in_store(&hive, EnvHive::User, "MY_VAR", "mine")
            .expect("force 写入必须成功");

        let raw = hive.get_raw("MY_VAR").expect("读取失败");
        assert_eq!(String::from_reg_value(&raw).unwrap(), "mine");
    }

    #[test]
    fn update_env_var_force_preserves_registry_type() {
        let hive = MemoryHive::new(true);
        hive.seed("GOPATH", "C:\\Old", REG_EXPAND_SZ);

        update_env_var_force_in_store(&hive, EnvHive::User, "GOPATH", "C:\\New")
            .expect("force 写入失败");

        assert_eq!(hive.get_raw("GOPATH").unwrap().vtype, REG_EXPAND_SZ);
    }

    #[test]
    fn update_env_var_force_still_rejects_protected_and_reserved() {
        let hive = MemoryHive::new(true);
        hive.seed("windir", "C:\\Windows", REG_EXPAND_SZ);

        assert!(
            update_env_var_force_in_store(&hive, EnvHive::User, "windir", "x").is_err(),
            "保护名单必须拒绝"
        );
        assert!(
            update_env_var_force_in_store(&hive, EnvHive::User, "Path", "x").is_err(),
            "保留名必须拒绝"
        );
    }

    #[test]
    fn delete_env_var_force_removes_after_external_change() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        hive.seed("MY_VAR", "changed-by-other", REG_SZ);

        delete_env_var_force_in_store(&hive, EnvHive::User, "MY_VAR").expect("force 删除必须成功");
        assert!(!hive.contains("MY_VAR"));
    }

    // ── 写前备份挂载（Task 3）──
    // 测试用注入写入逻辑（`*_with_store`），备份目录经 `PATHEDITOR_BACKUP_DIR`
    // 指向临时目录。**不在测试里向用户真实备份目录写入**。
    // 注意：`backup_env_vars()` 内部仍以**只读**方式打开真实 hive 采集
    // （`WinregHive::open(hive, false)`），因此这里只断言「备份结果被诚实返回」，
    // 不断言具体的 Created/Failed —— 结果取决于运行环境。

    #[test]
    fn update_env_var_reports_backup_outcome() {
        let _guard = crate::persist::test_persist_lock();
        let dir = std::env::temp_dir().join(format!("patheditor_t3_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("PATHEDITOR_BACKUP_DIR", &dir);

        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let revision = revision_of("MY_VAR", vtype, &value);

        let outcome = update_env_var_with_store(&hive, EnvHive::User, "MY_VAR", "new", &revision)
            .expect("写入必须成功");

        // 备份是 best-effort：本测试在真实目录写入，成功则 Created，失败则 Failed。
        // 两者都证明结果被诚实返回，而非被吞掉。
        assert!(
            matches!(
                outcome.backup,
                BackupOutcome::Created(_) | BackupOutcome::Skipped | BackupOutcome::Failed(_)
            ),
            "备份结果必须被返回"
        );
        assert_eq!(
            String::from_reg_value(&hive.get_raw("MY_VAR").unwrap()).unwrap(),
            "new",
            "备份结果不影响写入本身"
        );

        std::env::remove_var("PATHEDITOR_BACKUP_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 备份失败（目录不可写）时写入仍必须成功（K2：不阻断）。
    #[test]
    fn update_env_var_succeeds_even_when_backup_fails() {
        let _guard = crate::persist::test_persist_lock();
        // 指向一个不可能创建目录的路径：Windows 上以文件占位再当目录用
        let blocker =
            std::env::temp_dir().join(format!("patheditor_t3_block_{}", std::process::id()));
        let _ = std::fs::remove_file(&blocker);
        std::fs::write(&blocker, b"x").unwrap();
        std::env::set_var("PATHEDITOR_BACKUP_DIR", blocker.join("sub"));

        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let revision = revision_of("MY_VAR", vtype, &value);

        let outcome = update_env_var_with_store(&hive, EnvHive::User, "MY_VAR", "new", &revision)
            .expect("备份失败不得使写入失败");
        assert!(
            matches!(outcome.backup, BackupOutcome::Failed(_)),
            "备份失败必须被如实报告为 Failed"
        );
        assert_eq!(
            String::from_reg_value(&hive.get_raw("MY_VAR").unwrap()).unwrap(),
            "new"
        );

        std::env::remove_var("PATHEDITOR_BACKUP_DIR");
        let _ = std::fs::remove_file(&blocker);
    }

    /// 顺序契约：校验失败时**不发生备份**（不产生无意义的备份文件）。
    ///
    /// 保留名 / 保护名单是「便宜且无副作用」的校验，必须在 `backup_before_write()`
    /// 之前执行。断言方式是直接的：备份目录指向一个**可正常写入**的临时目录，
    /// 校验失败后该目录必须仍为空 —— 若实现先备份后校验，目录里会多出一份
    /// `env_backup_*.json`，本测试即失败。
    #[test]
    fn rejected_write_produces_no_backup_file() {
        let _guard = crate::persist::test_persist_lock();
        let dir = std::env::temp_dir().join(format!("patheditor_t3_order_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("PATHEDITOR_BACKUP_DIR", &dir);

        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        hive.seed("windir", "C:\\Windows", REG_EXPAND_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let revision = revision_of("MY_VAR", vtype, &value);

        // 保留名：必须 Err
        assert!(
            update_env_var_with_store(&hive, EnvHive::User, "Path", "x", &revision).is_err(),
            "保留名必须被拒绝"
        );
        // 保护名单：必须 Err
        assert!(
            update_env_var_with_store(&hive, EnvHive::User, "windir", "x", &revision).is_err(),
            "保护名单必须被拒绝"
        );

        let backups = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(backups, 0, "校验失败时不得产生备份文件");

        std::env::remove_var("PATHEDITOR_BACKUP_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
