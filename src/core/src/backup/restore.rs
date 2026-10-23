//! 环境变量备份的预览与恢复。

use super::env::*;
use crate::env_var::{is_reserved, revision_of, EnvHive, EnvValueKind};
use crate::error::{CoreError, ErrorCode};
use crate::registry::store::{EnvHiveStore, WinregHive};
use crate::registry::{
    create_env_var_in_store, delete_env_var_force_in_store, hive_location,
    update_env_var_force_in_store,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use winreg::types::FromRegValue;

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
    /// 。
    ///
    /// 保留本变体是为了维持对外声明的类型形状，以及恢复执行侧对
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
pub(crate) fn hive_rank(hive: EnvHive) -> u8 {
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
pub(crate) fn kind_rank(kind: RestoreChangeKind) -> u8 {
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
/// `changes`，于是同一输入产出不同 JSON。
///
/// **排序键：`(hive, kind, name)`**，其中 `name` 一律**小写化后**比较
/// （`to_ascii_lowercase`）：`diff_one_hive` 已按大小写不敏感语义把变量名配成
/// 「同一个」，用原始大小写作键会让 `Windir` 与 `windir` 这类同义名落在不同区间。
/// 小写键相同时再比原始名 —— 同 hive 同 kind 下小写键相同的两个条目属于同一变量，
/// 本不会同时出现；这一层只是让比较成为严格全序，结果不依赖比较器的调用顺序。
///
/// **排序不改变任何计数**：`added` / `modified` / `removed` / `conflicts` 都在排序后
/// 按遍历计数，与顺序无关（已完成排序后再计数，见 [`preview_restore_in_stores`]）。
pub(crate) fn sort_changes(changes: &mut [RestoreChange]) {
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
/// 不同即说明备份之后被改动过，记 `Conflict`。
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
pub(crate) fn diff_one_hive(
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
            e.code,
            "preview_restore",
            format!("读取{}环境变量列表失败: {}", label, e.message),
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

/// 读取任意格式的 env 备份：`.pebak` 解密，`.json` 明文兼容。
///
/// 这是 `backup.rs` 内部的**统一读入口**——生产路径（预览、恢复）都走它，
/// 保证「新备份能读、旧备份也能读」。委托给 [`crate::db::backups::read_env_backup_any`]，
/// 因为 DPAPI 密钥与 `.pebak` 头部解析都在那一层。
///
/// **`.pebak` 但密钥不可用**时返回 `Crypto` 错误，
/// 不会静默回落明文读取（那会给出错误的「文件不存在」类提示）。
pub(crate) fn read_env_backup_any(path: &Path) -> Result<EnvBackupPayload, CoreError> {
    let is_pebak = path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("pebak"))
        .unwrap_or(false);
    if !is_pebak {
        // 遗留明文路径：给一次明确警告
        log::warn!(
            "读取遗留明文备份 {}：v6 起的备份是 DPAPI 加密的 .pebak",
            path.display()
        );
        return read_env_backup(path);
    }
    let home = crate::db::crypto::config_dir();
    let material = crate::db::crypto::load_or_create_key(&home)?;
    crate::db::backups::read_env_backup_any(path, &material)
}

/// 读取任意格式的 env 备份（**公开入口**）：`.pebak` 解密，`.json` 明文兼容。
///
/// 生产路径（`preview_restore_file` / 恢复）与 CLI 测试夹具都走它，
/// 保证「新备份能读、旧备份也能读」。
///
/// **`.pebak` 但密钥不可用**时返回 `Crypto` 错误，
/// 不会静默回落明文读取（那会给出错误的「文件不存在」类提示）。
pub fn read_env_backup_from(path: &Path) -> Result<EnvBackupPayload, CoreError> {
    read_env_backup_any(path)
}

/// 备份文件的格式。用于让调用方**明确告知用户**当前读的是明文还是密文。
///
/// 整改的背景：core 里这些提示原本只写 `log::warn!`，而 CLI 没有
/// 初始化日志后端（`grep set_logger cli/` 零命中），所有 `log::warn!` 在 CLI 下
/// 都是空操作。README 承诺的「遗留 `.json` 会提示明文警告」因此不成立。
///
/// 库不该直接 `eprintln!`（会污染 GUI 的 stdout/stderr），正确做法是把
/// 「发生了什么」作为返回值交给调用方，由 GUI 记日志、CLI 打印到 stderr。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupFormat {
    /// DPAPI 加密的 `.pebak`
    Encrypted,
    /// 遗留明文 `.json`
    LegacyPlaintext,
}

/// 读取备份 + 告知其格式。
///
/// 生产入口（CLI `env restore` / GUI 预览）应使用本函数而非
/// [`read_env_backup_from`]，以便把明文警告真正呈现给用户。
pub fn read_env_backup_with_format(
    path: &Path,
) -> Result<(EnvBackupPayload, BackupFormat), CoreError> {
    let is_pebak = path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("pebak"))
        .unwrap_or(false);
    let payload = read_env_backup_from(path)?;
    Ok((
        payload,
        if is_pebak {
            BackupFormat::Encrypted
        } else {
            BackupFormat::LegacyPlaintext
        },
    ))
}

/// **仅供测试**：把载荷写成 `.pebak` 到给定目录（用本机默认密钥）。
///
/// 生产写入走 [`write_env_backup_to`]（自动取密钥并轮换）。本函数存在的理由：
/// CLI 的夹具需要「改过的载荷 → 可被 CLI 读回」这一步，而测试进程
/// 里的 CLI 子进程与主进程用的是同一把 `~/.patheditor` 密钥。
///
/// **不要在生产代码里调用**：它跳过轮换，且失败只返回 `None`。
#[doc(hidden)]
pub fn write_env_backup_for_test(dir: &Path, payload: &EnvBackupPayload) -> Option<PathBuf> {
    let key = crate::db::crypto::load_or_create_key(&crate::db::crypto::config_dir()).ok()?;
    let path = crate::db::backups::write_env_backup_pebak(dir, payload, &key).ok()?;
    // 固定文件名，避免被列表排序断言干扰
    let fixed = dir.join("env_backup_t7_fixture.pebak");
    std::fs::rename(&path, &fixed).ok()?;
    Some(fixed)
}

/// 从文件读取备份后计算差异（GUI 确认弹窗用）。
///
/// # Returns
/// - `Ok(RestorePreview)` — 差异摘要
/// - `Err(CoreError)` — 读取/解析备份失败（见 [`read_env_backup`]），
///   或打开注册表键/枚举失败（见 [`preview_restore`]）
pub fn preview_restore_file(path: &Path) -> Result<RestorePreview, CoreError> {
    let payload = read_env_backup_any(path)?;
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
    /// 其余三类都必然尝试写入。保留该字段是因为它的形状由 CLI `--json`
    /// 与 GUI 命令的消费端共同约定，删掉会破坏跨层契约。
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
/// # 写入通路（**不得改动**）
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
/// # 冲突语义
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
/// `EnvBackupHives` 的 `system` / `user` 带 `#[serde(default)]`（产物，
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
/// （备份侧没有任何条目，`diff_one_hive` 无从产出 `Conflict`），而下面的中止
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
/// 且 CLI/GUI 的展示层必须把「删除」数量显著呈现（验收标准 12/13）。
/// **若将来要加防护，这两个测试会先红 —— 那是有意的，它强制语义变更被显式确认。**
///
/// **本函数不产生新备份**：否则每次恢复都新增一份文件，与保留策略互相吞噬
/// （设计文档明文要求）。恢复前的手工兜底提示由 CLI/GUI 层打印，也不自动执行。
///
/// # Returns
/// - `Ok(RestoreOutcome)` — 恢复结果（可能含逐条失败）
/// - `Err(CoreError)` — 读取/解析备份失败，差异计算失败（code=`Io`），
///   或**默认模式下检测到冲突**（code=`Conflict`，注册表零改动）
pub(crate) fn restore_in_stores(
    sys: &dyn EnvHiveStore,
    usr: &dyn EnvHiveStore,
    path: &Path,
    force: bool,
) -> Result<RestoreOutcome, CoreError> {
    let payload = read_env_backup_any(path)?;
    let preview = preview_restore_in_stores(sys, usr, &payload)?;

    // 默认模式下任何冲突都中止，且不做任何写入。
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
pub(crate) fn inconsistent_change(name: &str) -> CoreError {
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
pub(crate) fn restore_should_broadcast(outcome: &RestoreOutcome) -> bool {
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
