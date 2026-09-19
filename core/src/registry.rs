use crate::env_var::{
    capabilities_for_with, is_protected, is_reserved, is_sensitive, revision_of, sanitize_preview,
    EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot, RevealedValue,
};
use crate::path_entry::PathEntry;
use crate::reg_store::{EnvHiveStore, WinregHive};
use std::path::Path;
use winreg::enums::*;
use winreg::types::{FromRegValue, ToRegValue};
use winreg::{RegKey, RegValue};

pub(crate) const SYS_REG_PATH: &str =
    "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment";
pub(crate) const USER_REG_PATH: &str = "Environment";
const PATH_VALUE: &str = "Path";

/// 修订冲突统一错误消息。前端按 `[E_CONFLICT]` 前缀匹配，
/// 中文正文仅供人工阅读，修改时必须保留前缀原样。
pub(crate) const ERR_CONFLICT: &str = "[E_CONFLICT] 变量已被其他进程修改，请重新加载";

/// 冲突错误的完整文本。CLI / GUI 凭此前缀判定冲突，避免二次硬编码文案。
pub fn conflict_message() -> String {
    ERR_CONFLICT.to_string()
}

pub(crate) fn load_paths(
    root: winreg::HKEY,
    sub_path: &str,
    label: &str,
) -> Result<Vec<String>, String> {
    let key = RegKey::predef(root);
    let env_key = key
        .open_subkey_with_flags(sub_path, KEY_READ)
        .map_err(|e| format!("无法打开{}注册表项: {}", label, e))?;

    let value: String = env_key
        .get_value(PATH_VALUE)
        .map_err(|e| format!("无法读取{} PATH: {}", label, e))?;

    Ok(split_path(&value))
}

fn select_path_value_type(existing: Option<RegType>) -> RegType {
    match existing {
        Some(REG_SZ) => REG_SZ,
        _ => REG_EXPAND_SZ,
    }
}

fn path_value_type(env_key: &RegKey) -> RegType {
    let existing = env_key.get_raw_value(PATH_VALUE).ok().map(|raw| raw.vtype);
    select_path_value_type(existing)
}

fn make_path_value(value: &str, vtype: RegType) -> RegValue {
    // 复用 winreg 的 UTF-16LE 编码与结尾 NUL 逻辑，只覆盖值类型。
    let mut raw = value.to_reg_value();
    raw.vtype = vtype;
    raw
}

fn save_paths(
    root: winreg::HKEY,
    sub_path: &str,
    label: &str,
    paths: &[String],
) -> Result<(), String> {
    let value = validate_and_join_paths(paths, label)?;

    let key = RegKey::predef(root);
    // 需要同时读取原值类型并写回，因此请求 READ | WRITE。
    let env_key = key
        .open_subkey_with_flags(sub_path, KEY_READ | KEY_WRITE)
        .map_err(|e| format!("无法写入{}注册表（需要管理员权限）: {}", label, e))?;

    let vtype = path_value_type(&env_key);
    let raw = make_path_value(&value, vtype);

    env_key
        .set_raw_value(PATH_VALUE, &raw)
        .map_err(|e| format!("无法写入{} PATH: {}", label, e))?;

    log::info!("已保存{} PATH，{} 个条目", label, paths.len());
    Ok(())
}
/// 从 HKLM 注册表读取系统 PATH
///
/// # Returns
/// - `Ok(Vec<String>)` — 系统 PATH 路径列表
/// - `Err(String)` — 注册表读取失败
pub fn load_system_paths() -> Result<Vec<String>, String> {
    load_paths(HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统")
}

/// 从 HKCU 注册表读取用户 PATH
///
/// # Returns
/// - `Ok(Vec<String>)` — 用户 PATH 路径列表
/// - `Err(String)` — 注册表读取失败
pub fn load_user_paths() -> Result<Vec<String>, String> {
    load_paths(HKEY_CURRENT_USER, USER_REG_PATH, "用户")
}

/// 保存系统 PATH 到注册表，含 32767 字符上限检查
///
/// # Returns
/// - `Ok(())` — 保存成功
/// - `Err(String)` — 写入失败或超过字符上限
pub fn save_system_paths(paths: Vec<String>) -> Result<(), String> {
    save_paths(HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统", &paths)
}

/// 保存用户 PATH 到注册表
///
/// # Returns
/// - `Ok(())` — 保存成功
/// - `Err(String)` — 写入失败
pub fn save_user_paths(paths: Vec<String>) -> Result<(), String> {
    save_paths(HKEY_CURRENT_USER, USER_REG_PATH, "用户", &paths)
}

/// 探测当前用户是否有权写入 HKCU 的 PATH 注册表项。
pub fn can_write_user() -> bool {
    let key = RegKey::predef(HKEY_CURRENT_USER);
    key.open_subkey_with_flags(USER_REG_PATH, KEY_WRITE).is_ok()
}

/// 将分号分隔的 PATH 字符串拆分为数组。
/// TS 端 split_path 仅保留为测试夹具；正式 PATH 解析以此处为准。
fn split_path(raw: &str) -> Vec<String> {
    raw.split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn join_path(paths: &[String]) -> String {
    paths
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(";")
}

/// 验证路径列表并拼接为分号分隔字符串
/// - 检查 null 字节
/// - 检查 UTF-16 总长度不超过 32767
fn validate_and_join_paths(paths: &[String], label: &str) -> Result<String, String> {
    if let Some(bad) = paths.iter().find(|p| p.contains('\0')) {
        return Err(format!("{} PATH 包含非法字符（null 字节）: {}", label, bad));
    }
    let value = join_path(paths);
    const MAX_PATH_LEN: usize = 32767;
    let utf16_len = value.encode_utf16().count();
    if utf16_len > MAX_PATH_LEN {
        return Err(format!(
            "{} PATH 总长度 {} 超出 Windows 限制 {} 字符，请移除部分路径后再保存",
            label, utf16_len, MAX_PATH_LEN
        ));
    }
    Ok(value)
}

/// 清理 PathEntry 列表：移除空路径、不存在的目录和重复路径（保留首次出现）。
///
/// 环境变量路径会先展开；无法展开的路径视为 unknown 并保留，避免误删。
pub fn clean_path_entries(entries: Vec<PathEntry>) -> (Vec<PathEntry>, Vec<PathEntry>) {
    use std::collections::HashSet;

    let mut seen: HashSet<String> = HashSet::new();
    let mut kept = Vec::new();
    let mut removed = Vec::new();

    for entry in entries {
        let trimmed = entry.path.trim();
        let key = trimmed.to_lowercase();
        if key.is_empty() || seen.contains(&key) {
            removed.push(entry);
            continue;
        }
        seen.insert(key);

        let expanded = crate::system::expand_env_vars(trimmed);
        let exists = expanded.contains('%') || Path::new(&expanded).is_dir();
        if exists {
            kept.push(PathEntry {
                path: trimmed.to_string(),
                enabled: entry.enabled,
            });
        } else {
            removed.push(entry);
        }
    }

    (kept, removed)
}

/// 清理路径字符串列表；保留旧 CLI 接口，语义与 `clean_path_entries` 一致。
pub fn clean_paths(paths: Vec<String>) -> (Vec<String>, Vec<String>) {
    let entries = paths
        .into_iter()
        .map(|path| PathEntry {
            path,
            enabled: true,
        })
        .collect();
    let (kept, removed) = clean_path_entries(entries);
    (
        kept.into_iter().map(|entry| entry.path).collect(),
        removed.into_iter().map(|entry| entry.path).collect(),
    )
}

// ── 通用环境变量读写 ──

/// 返回指定 hive 的 (根键, 子路径, 显示标签)。
pub(crate) fn hive_location(hive: EnvHive) -> (winreg::HKEY, &'static str, &'static str) {
    match hive {
        EnvHive::System => (HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统"),
        EnvHive::User => (HKEY_CURRENT_USER, USER_REG_PATH, "用户"),
    }
}

/// 读取单个值，返回 (vtype, value)。仅用于字符串类型。
///
/// `vtype` 是 winreg 的 `RegType`，不是 `u32` —— 类型判定与 revision
/// 计算都基于它。先判定类型再做字符串解码：`REG_DWORD` 等不支持类型
/// 必须返回「类型不受支持」，而不是误导性的解码失败。
fn read_env_var(store: &dyn EnvHiveStore, name: &str) -> Result<(RegType, String), String> {
    let raw = store.get_raw(name)?;
    // `RegType` 非 Copy（`winreg-0.52.0/src/enums.rs:20`），必须先
    // `.clone()` 取值再借用 `&raw`，否则 `from_reg_type(raw.vtype)` 会部分
    // 移出 `raw`，后面 `&raw` 与 `Ok((raw.vtype, …))` 都会编译失败。
    // 原实现此处即 `raw.vtype.clone()`（`registry.rs:245`）。
    if !EnvValueKind::from_reg_type(raw.vtype.clone()).is_writable() {
        return Err(format!("环境变量 {} 的注册表类型不受支持", name));
    }
    let value =
        String::from_reg_value(&raw).map_err(|e| format!("无法解码环境变量 {}: {}", name, e))?;
    Ok((raw.vtype, value))
}

/// 写入单个值，保持调用方给定的注册表类型。错误由端口层格式化。
fn write_env_var(
    store: &dyn EnvHiveStore,
    name: &str,
    value: &str,
    vtype: RegType,
) -> Result<(), String> {
    let mut raw = value.to_reg_value();
    raw.vtype = vtype;
    store.set_raw(name, &raw)
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
fn list_hive_env_vars(hive: EnvHive) -> Result<Vec<EnvVarMeta>, String> {
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
) -> Result<Vec<EnvVarMeta>, String> {
    // F-04：错误必须携带 hive 标识，与 open 错误（自带标签）保持一致。
    let (_, _, label) = hive_location(hive);
    let mut metas = Vec::new();

    // 写权限探测按 hive 只做一次（端口在 open 时已探测，避免逐变量重复探测）。
    let writable = store.writable();

    let names = store.enum_names().map_err(|e| {
        // spec F-04：warning 日志保留，但不再是唯一反馈；Err 继续向上传播。
        log::warn!("{}环境变量列表读取失败: {}", label, e);
        format!("读取{}环境变量列表失败: {}", label, e)
    })?;

    for name in names {
        // 保留变量（Path）由专用通路拥有，通用通路完全不展示
        if is_reserved(&name) {
            continue;
        }

        let raw = store.get_raw(&name).map_err(|e| {
            log::warn!("{}环境变量列表读取失败: {}", label, e);
            format!("读取{}环境变量列表失败: {}", label, e)
        })?;
        let kind = EnvValueKind::from_reg_type(raw.vtype.clone());
        let sensitive = is_sensitive(&name);

        let value = match kind {
            EnvValueKind::Unsupported => String::new(),
            _ => String::from_reg_value(&raw).map_err(|e| {
                log::warn!("{}环境变量列表读取失败: {}", label, e);
                format!(
                    "读取{}环境变量列表失败: 无法解码环境变量 {}: {}",
                    label, name, e
                )
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
pub fn list_all_env_vars() -> Result<EnvVarSnapshot, String> {
    Ok(EnvVarSnapshot {
        system: list_hive_env_vars(EnvHive::System)?,
        user: list_hive_env_vars(EnvHive::User)?,
    })
}

/// 按需读取单个变量的明文及读取时的 revision。
///
/// `Unsupported` 类型返回 `Err`，不尝试转字符串。
pub fn reveal_env_var(hive: EnvHive, name: &str) -> Result<RevealedValue, String> {
    let store = WinregHive::open(hive, false)?;
    reveal_env_var_in_store(&store, name)
}

/// `reveal_env_var` 的核心逻辑，存储可注入。
fn reveal_env_var_in_store(store: &dyn EnvHiveStore, name: &str) -> Result<RevealedValue, String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!(
            "{} 由专用 PATH 通路管理，请使用 PATH 视图编辑",
            name
        ));
    }
    let (vtype, value) = read_env_var(store, name)?;
    let revision = revision_of(name, vtype, &value);
    Ok(RevealedValue { value, revision })
}

/// 写入已有变量。类型从注册表读取，不由前端决定。
pub fn update_env_var(
    hive: EnvHive,
    name: &str,
    value: &str,
    expected_revision: &str,
) -> Result<(), String> {
    let store = WinregHive::open(hive, true)?;
    update_env_var_in_store(&store, name, value, expected_revision)?;
    crate::system::broadcast_env_change();
    Ok(())
}

/// `update_env_var` 的核心逻辑，存储可注入。
///
/// 在同一调用内完成「读 → 算 revision → 比对 → 校验 → 写」。
/// 读与写是两次独立存储调用，仍有竞态窗口；revision 校验缩小影响，
/// 不能完全消除 TOCTOU。
fn update_env_var_in_store(
    store: &dyn EnvHiveStore,
    name: &str,
    value: &str,
    expected_revision: &str,
) -> Result<(), String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!(
            "{} 由专用 PATH 通路管理，请使用 PATH 视图编辑",
            name
        ));
    }
    if is_protected(name) {
        return Err(format!("{} 是系统内置变量，不允许修改", name));
    }

    let (vtype, current) = read_env_var(store, name)?;

    // 并发校验：revision 不匹配时拒绝写入，交给前端提示重新加载
    let current_revision = revision_of(name, vtype.clone(), &current);
    if current_revision != expected_revision {
        return Err(ERR_CONFLICT.into());
    }

    // 类型与值校验
    let kind = EnvValueKind::from_reg_type(vtype.clone());
    if !kind.is_writable() {
        return Err(format!(
            "{} 的注册表类型不受支持，无法修改（仅可查看）",
            name
        ));
    }
    validate_env_value(value, name)?;

    // 写入，类型原样保留
    write_env_var(store, name, value, vtype)
}

/// 新建变量。`kind` 仅在此决定。
pub fn create_env_var(
    hive: EnvHive,
    name: &str,
    value: &str,
    kind: EnvValueKind,
) -> Result<(), String> {
    let store = WinregHive::open(hive, true)?;
    create_env_var_in_store(&store, name, value, kind)?;
    crate::system::broadcast_env_change();
    Ok(())
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
    name: &str,
    value: &str,
    kind: EnvValueKind,
) -> Result<(), String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!(
            "{} 由专用 PATH 通路管理，无法通过通用通路创建",
            name
        ));
    }
    if is_protected(name) {
        return Err(format!("{} 是系统内置变量，不允许覆盖", name));
    }
    if !kind.is_writable() {
        return Err("新建变量只支持 String 或 ExpandString 类型".into());
    }
    validate_env_value(value, name)?;

    // 写入前检查：忽略大小写地确认该名不存在（非原子，见函数文档）
    let existing = store
        .enum_names()?
        .iter()
        .any(|n| n.eq_ignore_ascii_case(name));
    if existing {
        return Err(format!("变量 {} 已存在，请使用编辑功能", name));
    }

    let vtype = match kind {
        EnvValueKind::String => REG_SZ,
        _ => REG_EXPAND_SZ,
    };
    write_env_var(store, name, value, vtype)
}

/// 删除变量。在同一调用内完成 revision 比对与删除。
pub fn delete_env_var(hive: EnvHive, name: &str, expected_revision: &str) -> Result<(), String> {
    let store = WinregHive::open(hive, true)?;
    delete_env_var_in_store(&store, name, expected_revision)?;
    crate::system::broadcast_env_change();
    Ok(())
}

/// `delete_env_var` 的核心逻辑，存储可注入。
///
/// 读与删除是两次独立调用，revision 校验缩小竞态影响，不能完全消除 TOCTOU。
fn delete_env_var_in_store(
    store: &dyn EnvHiveStore,
    name: &str,
    expected_revision: &str,
) -> Result<(), String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!(
            "{} 由专用 PATH 通路管理，请使用 PATH 视图编辑",
            name
        ));
    }
    if is_protected(name) {
        return Err(format!("{} 是系统内置变量，不允许删除", name));
    }

    let (vtype, current) = read_env_var(store, name)?;
    let current_revision = revision_of(name, vtype.clone(), &current);
    if current_revision != expected_revision {
        return Err(ERR_CONFLICT.into());
    }

    if EnvValueKind::from_reg_type(vtype) == EnvValueKind::Unsupported {
        // 防御性、当前不可达（评审裁断 O-3）：`read_env_var` 已对 Unsupported
        // 类型提前返回 Err，上面的 `?` 会先短路。保留以显式表达删除的前置条件，
        // 行为与现状一致。
        return Err(format!("{} 的注册表类型不受支持，无法删除", name));
    }

    store.delete_value(name)
}

/// 强制写入已有变量（**最后写入者胜**）。不做 revision 比对。
///
/// 仅供 CLI `--force` 使用；GUI 一律走 [`update_env_var`] 的 CAS 语义。
/// force 只豁免并发校验，**不豁免**保留名 / 保护名单 / 类型 / 权限判定。
/// 这不是原子操作：读类型与写入仍是两次独立调用。
pub fn update_env_var_force(hive: EnvHive, name: &str, value: &str) -> Result<(), String> {
    let store = WinregHive::open(hive, true)?;
    update_env_var_force_in_store(&store, name, value)?;
    crate::system::broadcast_env_change();
    Ok(())
}

/// `update_env_var_force` 的核心逻辑，存储可注入。
fn update_env_var_force_in_store(
    store: &dyn EnvHiveStore,
    name: &str,
    value: &str,
) -> Result<(), String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!(
            "{} 由专用 PATH 通路管理，请使用 PATH 视图编辑",
            name
        ));
    }
    if is_protected(name) {
        return Err(format!("{} 是系统内置变量，不允许修改", name));
    }

    // 读现有类型以便原样保留；**不比对** revision
    let (vtype, _current) = read_env_var(store, name)?;
    let kind = EnvValueKind::from_reg_type(vtype.clone());
    if !kind.is_writable() {
        return Err(format!(
            "{} 的注册表类型不受支持，无法修改（仅可查看）",
            name
        ));
    }
    validate_env_value(value, name)?;

    write_env_var(store, name, value, vtype)
}

/// 强制删除变量（**最后写入者胜**）。不做 revision 比对，其余校验照旧。
pub fn delete_env_var_force(hive: EnvHive, name: &str) -> Result<(), String> {
    let store = WinregHive::open(hive, true)?;
    delete_env_var_force_in_store(&store, name)?;
    crate::system::broadcast_env_change();
    Ok(())
}

/// `delete_env_var_force` 的核心逻辑，存储可注入。
fn delete_env_var_force_in_store(store: &dyn EnvHiveStore, name: &str) -> Result<(), String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!(
            "{} 由专用 PATH 通路管理，请使用 PATH 视图编辑",
            name
        ));
    }
    if is_protected(name) {
        return Err(format!("{} 是系统内置变量，不允许删除", name));
    }

    let (vtype, _current) = read_env_var(store, name)?;
    if EnvValueKind::from_reg_type(vtype) == EnvValueKind::Unsupported {
        // 防御性、当前不可达：`read_env_var` 已对 Unsupported 类型提前返回 Err，
        // 上面的 `?` 会先短路。保留以显式表达删除的前置条件，行为与现状一致。
        return Err(format!("{} 的注册表类型不受支持，无法删除", name));
    }

    store.delete_value(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_empty() {
        assert_eq!(split_path(""), Vec::<String>::new());
    }

    #[test]
    fn split_single() {
        assert_eq!(split_path("C:\\Windows"), vec!["C:\\Windows"]);
    }

    #[test]
    fn split_multiple() {
        assert_eq!(
            split_path("C:\\Windows;D:\\Projects"),
            vec!["C:\\Windows", "D:\\Projects"]
        );
    }

    #[test]
    fn split_trims_and_filters_empty() {
        assert_eq!(split_path(" C:\\ ; ; D:\\ "), vec!["C:\\", "D:\\"]);
    }

    #[test]
    fn join_and_split_roundtrip() {
        let paths = vec!["C:\\Windows".to_string(), "D:\\Projects".to_string()];
        let joined = join_path(&paths);
        let split = split_path(&joined);
        assert_eq!(split, paths);
    }

    #[test]
    fn join_trims_entries() {
        let paths = vec![" C:\\Windows ".to_string(), " D:\\ ".to_string()];
        assert_eq!(join_path(&paths), "C:\\Windows;D:\\");
    }

    #[test]
    fn validate_rejects_null_bytes() {
        let paths = vec!["C:\\safe".into(), "C:\0invalid".into()];
        let result = validate_and_join_paths(&paths, "测试");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("null 字节"));
    }

    #[test]
    fn validate_accepts_cjk_paths() {
        let paths = vec!["C:\\用户\\工具".into()];
        let result = validate_and_join_paths(&paths, "测试");
        assert!(result.is_ok());
    }

    #[test]
    fn validate_rejects_oversized_paths() {
        // 构造总长超过 32767 UTF-16 字符的路径
        let long_path = "C:\\".to_string() + &"a".repeat(32767);
        let paths = vec![long_path];
        let result = validate_and_join_paths(&paths, "测试");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("超出 Windows 限制"));
    }
}

#[cfg(test)]
mod issue26_tests {
    use super::{join_path, make_path_value, save_paths, select_path_value_type, PATH_VALUE};
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, REG_DWORD, REG_EXPAND_SZ, REG_SZ};
    use winreg::types::FromRegValue;
    use winreg::RegKey;

    struct TempRegistryKey {
        root: winreg::HKEY,
        path: String,
    }

    impl Drop for TempRegistryKey {
        fn drop(&mut self) {
            let root = RegKey::predef(self.root);
            let _ = root.delete_subkey_all(&self.path);
        }
    }

    #[test]
    fn select_path_value_type_keeps_expand_sz() {
        assert_eq!(select_path_value_type(Some(REG_EXPAND_SZ)), REG_EXPAND_SZ);
    }

    #[test]
    fn select_path_value_type_keeps_sz() {
        assert_eq!(select_path_value_type(Some(REG_SZ)), REG_SZ);
    }

    #[test]
    fn select_path_value_type_defaults_for_missing_value() {
        assert_eq!(select_path_value_type(None), REG_EXPAND_SZ);
    }

    #[test]
    fn select_path_value_type_defaults_for_non_string_value() {
        assert_eq!(select_path_value_type(Some(REG_DWORD)), REG_EXPAND_SZ);
    }

    #[test]
    fn make_path_value_preserves_type_and_text() {
        let raw = make_path_value("%SystemRoot%\\system32", REG_EXPAND_SZ);
        assert_eq!(raw.vtype, REG_EXPAND_SZ);
        assert_eq!(
            String::from_reg_value(&raw).expect("解码注册表值失败"),
            "%SystemRoot%\\system32"
        );
    }

    #[test]
    fn make_path_value_uses_utf16_nul_terminator() {
        let raw = make_path_value("C:\\Windows", REG_SZ);
        assert_eq!(raw.bytes.len() % 2, 0);
        assert_eq!(&raw.bytes[raw.bytes.len() - 2..], &[0, 0]);
    }

    #[test]
    #[ignore = "需要真实注册表写权限；只使用隔离测试键，不接触 PATH"]
    fn save_paths_keeps_expand_sz_in_isolated_key() {
        let parent = "Software\\PathEditor\\Tests";
        let unique = format!(
            "{}\\issue26-{}-{}",
            parent,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("系统时间早于 UNIX_EPOCH")
                .as_nanos()
        );

        let root = RegKey::predef(HKEY_CURRENT_USER);
        root.create_subkey(parent).expect("创建隔离测试父键失败");
        let (key, _) = root.create_subkey(&unique).expect("创建隔离测试键失败");
        key.set_raw_value(
            PATH_VALUE,
            &make_path_value("C:\\Windows;%SystemRoot%\\system32", REG_EXPAND_SZ),
        )
        .expect("写入初始 REG_EXPAND_SZ 失败");
        drop(key);

        let _guard = TempRegistryKey {
            root: HKEY_CURRENT_USER,
            path: unique.clone(),
        };

        let paths = vec![
            "C:\\Windows".to_string(),
            "%SystemRoot%\\system32".to_string(),
        ];
        save_paths(HKEY_CURRENT_USER, &unique, "测试", &paths).expect("保存测试 PATH 失败");

        let key = root
            .open_subkey_with_flags(&unique, KEY_READ)
            .expect("重新打开隔离测试键失败");
        let raw = key.get_raw_value(PATH_VALUE).expect("读取测试值失败");
        assert_eq!(raw.vtype, REG_EXPAND_SZ);
        assert_eq!(
            String::from_reg_value(&raw).expect("解码测试值失败"),
            join_path(&paths)
        );
    }
}

#[cfg(test)]
mod env_var_tests {
    use super::*;
    use crate::env_var::{EnvHive, EnvValueKind};
    use crate::reg_store::memory::MemoryHive;
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

        let result = update_env_var_in_store(&hive, "MY_VAR", "mine", &stale_revision);
        assert!(result.is_err(), "revision 不匹配必须 Err");
        assert_eq!(result.unwrap_err(), ERR_CONFLICT);

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

        let result = update_env_var_in_store(&hive, "MY_DWORD", "1", &revision);
        assert!(result.is_err(), "REG_DWORD 不可通过通用通路修改");
        assert!(
            result.unwrap_err().contains("类型不受支持"),
            "应报「类型不受支持」而非解码错误"
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

        update_env_var_in_store(&hive, "MY_VAR", "updated", &revision)
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

        delete_env_var_in_store(&hive, "MY_VAR", &revision).expect("revision 匹配必须成功");

        assert!(!hive.contains("MY_VAR"), "成功删除后变量必须消失");
    }

    #[test]
    fn create_env_var_rejects_protected_and_duplicate() {
        let hive = MemoryHive::new(true);
        hive.seed("Existing", "already-here", REG_SZ);

        let protected = create_env_var_in_store(&hive, "windir", "C:\\evil", EnvValueKind::String);
        assert!(protected.is_err(), "保护名单变量不允许创建");
        assert!(protected.unwrap_err().contains("系统内置"));

        let duplicate = create_env_var_in_store(&hive, "EXISTING", "dup", EnvValueKind::String);
        assert!(duplicate.is_err(), "同名变量不允许重复创建");
        assert!(duplicate.unwrap_err().contains("已存在"));

        let raw = hive.get_raw("Existing").expect("读取失败");
        assert_eq!(String::from_reg_value(&raw).unwrap(), "already-here");
    }

    #[test]
    fn reveal_returns_value_with_matching_revision() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "hello", REG_SZ);

        let revealed = reveal_env_var_in_store(&hive, "MY_VAR").expect("reveal 失败");
        assert_eq!(revealed.value, "hello");

        let raw = hive.get_raw("MY_VAR").unwrap();
        let expected = revision_of("MY_VAR", raw.vtype, "hello");
        assert_eq!(revealed.revision, expected, "revision 必须与列表项一致");
    }

    #[test]
    fn reveal_revision_changes_after_external_edit() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "one", REG_SZ);
        let first = reveal_env_var_in_store(&hive, "MY_VAR").unwrap().revision;

        hive.seed("MY_VAR", "two", REG_SZ);
        let second = reveal_env_var_in_store(&hive, "MY_VAR").unwrap().revision;

        assert_ne!(first, second);
    }

    #[test]
    fn reveal_env_var_returns_plaintext_and_sensitive_preview_is_none() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_PLAIN", "C:\\plain value", REG_SZ);
        hive.seed("MY_API_TOKEN", "super-secret-plaintext", REG_SZ);

        let revealed = reveal_env_var_in_store(&hive, "MY_API_TOKEN").expect("reveal 失败");
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

        let result = delete_env_var_in_store(&hive, "MY_VAR", &stale_revision);
        assert!(result.is_err(), "revision 不匹配必须 Err");
        assert_eq!(result.unwrap_err(), ERR_CONFLICT);
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
        // 前端按 [E_CONFLICT] 前缀匹配；正文改动不应破坏契约
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
        update_env_var_force_in_store(&hive, "MY_VAR", "mine").expect("force 写入必须成功");

        let raw = hive.get_raw("MY_VAR").expect("读取失败");
        assert_eq!(String::from_reg_value(&raw).unwrap(), "mine");
    }

    #[test]
    fn update_env_var_force_preserves_registry_type() {
        let hive = MemoryHive::new(true);
        hive.seed("GOPATH", "C:\\Old", REG_EXPAND_SZ);

        update_env_var_force_in_store(&hive, "GOPATH", "C:\\New").expect("force 写入失败");

        assert_eq!(hive.get_raw("GOPATH").unwrap().vtype, REG_EXPAND_SZ);
    }

    #[test]
    fn update_env_var_force_still_rejects_protected_and_reserved() {
        let hive = MemoryHive::new(true);
        hive.seed("windir", "C:\\Windows", REG_EXPAND_SZ);

        assert!(
            update_env_var_force_in_store(&hive, "windir", "x").is_err(),
            "保护名单必须拒绝"
        );
        assert!(
            update_env_var_force_in_store(&hive, "Path", "x").is_err(),
            "保留名必须拒绝"
        );
    }

    #[test]
    fn delete_env_var_force_removes_after_external_change() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        hive.seed("MY_VAR", "changed-by-other", REG_SZ);

        delete_env_var_force_in_store(&hive, "MY_VAR").expect("force 删除必须成功");
        assert!(!hive.contains("MY_VAR"));
    }
}
