use crate::env_var::{
    capabilities_for, is_protected, is_reserved, is_sensitive, revision_of, sanitize_preview,
    EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot,
};
use crate::path_entry::PathEntry;
use std::path::Path;
use winreg::enums::*;
use winreg::types::{FromRegValue, ToRegValue};
use winreg::{RegKey, RegValue};

pub(crate) const SYS_REG_PATH: &str =
    "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment";
pub(crate) const USER_REG_PATH: &str = "Environment";
const PATH_VALUE: &str = "Path";

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

fn env_key(root: winreg::HKEY, sub_path: &str, label: &str, write: bool) -> Result<RegKey, String> {
    let flags = if write {
        KEY_READ | KEY_WRITE
    } else {
        KEY_READ
    };
    let key = RegKey::predef(root);
    key.open_subkey_with_flags(sub_path, flags)
        .map_err(|e| format!("无法打开{}环境变量注册表项: {}", label, e))
}

fn hive_location(hive: EnvHive) -> (winreg::HKEY, &'static str, &'static str) {
    match hive {
        EnvHive::System => (HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统"),
        EnvHive::User => (HKEY_CURRENT_USER, USER_REG_PATH, "用户"),
    }
}

/// 读取单个值，返回 (vtype, value)。仅用于字符串类型。
///
/// `vtype` 是 winreg 的 `RegType`（即 `RegValue::vtype` 的真实类型），
/// 不是 `u32` —— 类型判定与 revision 计算都基于它。
fn read_env_var(key: &RegKey, name: &str) -> Result<(RegType, String), String> {
    let raw = key
        .get_raw_value(name)
        .map_err(|e| format!("无法读取环境变量 {}: {}", name, e))?;
    let value =
        String::from_reg_value(&raw).map_err(|e| format!("无法解码环境变量 {}: {}", name, e))?;
    Ok((raw.vtype, value))
}

/// 写入单个值，保持调用方给定的注册表类型。
fn write_env_var(key: &RegKey, name: &str, value: &str, vtype: RegType) -> Result<(), String> {
    let mut raw = value.to_reg_value();
    raw.vtype = vtype;
    key.set_raw_value(name, &raw)
        .map_err(|e| format!("无法写入环境变量 {}: {}", name, e))
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
    let (root, sub_path, label) = hive_location(hive);
    let key = env_key(root, sub_path, label, false)?;
    let mut metas = Vec::new();

    for name in key.enum_values().flatten().map(|(n, _)| n) {
        // 保留变量（Path）由专用通路拥有，通用通路完全不展示
        if is_reserved(&name) {
            continue;
        }

        let raw = match key.get_raw_value(&name) {
            Ok(raw) => raw,
            Err(e) => {
                log::warn!("跳过无法读取的环境变量 {}: {}", name, e);
                continue;
            }
        };
        let kind = EnvValueKind::from_reg_type(raw.vtype.clone());
        let sensitive = is_sensitive(&name);

        let value = match kind {
            EnvValueKind::Unsupported => String::new(),
            _ => match String::from_reg_value(&raw) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("跳过无法解码的环境变量 {}: {}", name, e);
                    continue;
                }
            },
        };

        let preview = if sensitive || !kind.is_writable() {
            None
        } else {
            sanitize_preview(&value)
        };

        let (can_edit, can_delete) = capabilities_for(hive, &name, kind);

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
/// 单次调用内读两个 hive，保证快照一致 —— 若分两次调用，两次读取之间
/// 注册表可能变化，合并视图会出现 hive 来自不同时刻的不一致。
pub fn list_all_env_vars() -> Result<EnvVarSnapshot, String> {
    Ok(EnvVarSnapshot {
        system: list_hive_env_vars(EnvHive::System)?,
        user: list_hive_env_vars(EnvHive::User)?,
    })
}

/// 按需读取单个变量的明文（命中敏感规则的变量的唯一取值入口）。
///
/// `Unsupported` 类型返回 `Err`，不尝试转字符串。
pub fn reveal_env_var(hive: EnvHive, name: &str) -> Result<String, String> {
    validate_env_name(name)?;
    if is_reserved(name) {
        return Err(format!(
            "{} 由专用 PATH 通路管理，请使用 PATH 视图编辑",
            name
        ));
    }
    let (root, sub_path, label) = hive_location(hive);
    let key = env_key(root, sub_path, label, false)?;
    let (vtype, value) = read_env_var(&key, name)?;
    match EnvValueKind::from_reg_type(vtype) {
        EnvValueKind::String | EnvValueKind::ExpandString => Ok(value),
        EnvValueKind::Unsupported => Err(format!("{} 的注册表类型不受支持，无法读取其值", name)),
    }
}

/// 写入已有变量。类型从注册表读取，不由前端决定。
///
/// 在同一调用内完成「读 → 算 revision → 比对 → 校验 → 写」，消除 TOCTOU。
pub fn update_env_var(
    hive: EnvHive,
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

    let (root, sub_path, label) = hive_location(hive);
    // 打开后不释放句柄，直到写入完成
    let key = env_key(root, sub_path, label, true)?;

    let (vtype, current) = read_env_var(&key, name)?;

    // 步骤 3：并发校验
    let current_revision = revision_of(name, vtype.clone(), &current);
    if current_revision != expected_revision {
        return Err("变量已被其他进程修改，请重新加载".into());
    }

    // 步骤 4：类型与值校验
    let kind = EnvValueKind::from_reg_type(vtype.clone());
    if !kind.is_writable() {
        return Err(format!(
            "{} 的注册表类型不受支持，无法修改（仅可查看）",
            name
        ));
    }
    validate_env_value(value, name)?;

    // 步骤 5：写入，类型原样保留
    write_env_var(&key, name, value, vtype)?;
    crate::system::broadcast_env_change();
    Ok(())
}

/// 新建变量。`kind` 仅在此决定。
///
/// 在 Rust 内原子检查名称不存在 —— 不覆盖已有变量。
pub fn create_env_var(
    hive: EnvHive,
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

    let (root, sub_path, label) = hive_location(hive);
    let key = env_key(root, sub_path, label, true)?;

    // 原子检查：忽略大小写地确认该名不存在
    let existing = key
        .enum_values()
        .flatten()
        .any(|(n, _)| n.eq_ignore_ascii_case(name));
    if existing {
        return Err(format!("变量 {} 已存在，请使用编辑功能", name));
    }

    let vtype = match kind {
        EnvValueKind::String => REG_SZ,
        _ => REG_EXPAND_SZ,
    };
    write_env_var(&key, name, value, vtype)?;
    crate::system::broadcast_env_change();
    Ok(())
}

/// 删除变量。在同一调用内完成 revision 比对与删除。
pub fn delete_env_var(hive: EnvHive, name: &str, expected_revision: &str) -> Result<(), String> {
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

    let (root, sub_path, label) = hive_location(hive);
    let key = env_key(root, sub_path, label, true)?;

    let (vtype, current) = read_env_var(&key, name)?;
    let current_revision = revision_of(name, vtype.clone(), &current);
    if current_revision != expected_revision {
        return Err("变量已被其他进程修改，请重新加载".into());
    }

    if EnvValueKind::from_reg_type(vtype) == EnvValueKind::Unsupported {
        return Err(format!("{} 的注册表类型不受支持，无法删除", name));
    }

    key.delete_value(name)
        .map_err(|e| format!("无法删除环境变量 {}: {}", name, e))?;
    crate::system::broadcast_env_change();
    Ok(())
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
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_EXPAND_SZ, REG_SZ};
    use winreg::types::FromRegValue;

    /// RAII 隔离测试键：Drop 时递归删除，绝不触碰真实环境变量键。
    struct TempRegistryKey {
        root: winreg::HKEY,
        path: String,
    }

    impl TempRegistryKey {
        fn new(label: &str) -> Self {
            let parent = "Software\\PathEditor\\EnvVarTests";
            let unique = format!(
                "{}\\{}-{}-{}",
                parent,
                label,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("系统时间早于 UNIX_EPOCH")
                    .as_nanos()
            );
            let root = RegKey::predef(HKEY_CURRENT_USER);
            root.create_subkey(parent).expect("创建隔离测试父键失败");
            root.create_subkey(&unique).expect("创建隔离测试键失败");
            TempRegistryKey {
                root: HKEY_CURRENT_USER,
                path: unique,
            }
        }

        fn key(&self) -> RegKey {
            let root = RegKey::predef(self.root);
            root.open_subkey_with_flags(&self.path, KEY_READ | KEY_WRITE)
                .expect("打开隔离测试键失败")
        }
    }

    impl Drop for TempRegistryKey {
        fn drop(&mut self) {
            let root = RegKey::predef(self.root);
            let _ = root.delete_subkey_all(&self.path);
        }
    }

    fn seed(key: &RegKey, name: &str, value: &str, vtype: RegType) {
        let mut raw = value.to_reg_value();
        raw.vtype = vtype;
        key.set_raw_value(name, &raw).expect("写入种子值失败");
    }

    #[test]
    fn read_env_var_returns_real_type_and_value() {
        let temp = TempRegistryKey::new("read");
        let key = temp.key();
        seed(&key, "JAVA_HOME", "C:\\Java", REG_EXPAND_SZ);

        let (vtype, value) = read_env_var(&key, "JAVA_HOME").expect("读取失败");
        assert_eq!(vtype, REG_EXPAND_SZ);
        assert_eq!(value, "C:\\Java");
    }

    #[test]
    fn write_env_var_preserves_expand_sz_type() {
        let temp = TempRegistryKey::new("preserve-expand");
        let key = temp.key();
        seed(&key, "GOPATH", "C:\\Old", REG_EXPAND_SZ);

        write_env_var(&key, "GOPATH", "C:\\New", REG_EXPAND_SZ).expect("写入失败");

        let raw = key.get_raw_value("GOPATH").expect("读取失败");
        assert_eq!(raw.vtype, REG_EXPAND_SZ);
        assert_eq!(String::from_reg_value(&raw).unwrap(), "C:\\New");
    }

    #[test]
    fn write_env_var_preserves_sz_type() {
        let temp = TempRegistryKey::new("preserve-sz");
        let key = temp.key();
        seed(&key, "JAVA_HOME", "C:\\Old", REG_SZ);

        write_env_var(&key, "JAVA_HOME", "C:\\New", REG_SZ).expect("写入失败");

        let raw = key.get_raw_value("JAVA_HOME").expect("读取失败");
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
        // Path 走专用通路，通用通路的写入口必须拒绝
        assert!(is_reserved("Path"));
        assert!(is_reserved("path"));
        let temp = TempRegistryKey::new("reserved");
        let key = temp.key();
        seed(&key, "Path", "C:\\Windows", REG_EXPAND_SZ);
        // 值仍在，但通用通路不展示它
        assert!(read_env_var(&key, "Path").is_ok());
    }

    #[test]
    fn revision_detects_external_change() {
        let temp = TempRegistryKey::new("revision");
        let key = temp.key();
        seed(&key, "MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&key, "MY_VAR").expect("读取失败");
        let revision = crate::env_var::revision_of("MY_VAR", vtype, &value);

        // 模拟外部进程修改
        seed(&key, "MY_VAR", "changed-by-other-process", REG_SZ);

        let (vtype2, value2) = read_env_var(&key, "MY_VAR").expect("读取失败");
        let current = crate::env_var::revision_of("MY_VAR", vtype2, &value2);
        assert_ne!(revision, current, "外部修改后 revision 必须不同");
    }

    #[test]
    fn unsupported_type_is_not_writable() {
        let temp = TempRegistryKey::new("dword");
        let key = temp.key();
        // REG_DWORD 需要 4 字节小端数据
        let raw = RegValue {
            bytes: vec![1, 0, 0, 0],
            vtype: REG_DWORD,
        };
        key.set_raw_value("MY_DWORD", &raw)
            .expect("写入 DWORD 失败");

        // 不经字符串解码：直接取原始类型验证映射与可写性。
        // read_env_var 的职责是只读字符串类型，用它读 DWORD 会得到
        // ERROR_UNSUPPORTED_TYPE，那是测试用错函数而非产品缺陷。
        let stored = key.get_raw_value("MY_DWORD").expect("读取原始值失败");
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
}
