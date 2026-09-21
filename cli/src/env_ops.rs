//! 环境变量子命令实现：参数转换、core 调用与输出格式化。
//!
//! 本模块**不实现任何安全判定**（保留 / 保护 / 敏感 / 权限 / revision 校验），
//! 全部由 `path_editor_core` 负责，此处仅透传错误文本。

use crate::runtime::{exit_core_error, exit_err, warn_if_backup_failed};
use path_editor_core as core;
use path_editor_core::env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot};

/// 值的输入通道。三选一，互斥。
pub(crate) enum ValueSource {
    /// 位置参数或 `--value` 直接给出
    Argv(String),
    /// `--stdin`：从标准输入读到 EOF
    Stdin,
    /// `--value-file <F>`：读取文件全部内容
    File(String),
}

/// 统计显式给出的通道数（互斥校验用）。
pub(crate) fn channel_count(argv: Option<&str>, stdin: bool, file: Option<&str>) -> usize {
    usize::from(argv.is_some()) + usize::from(stdin) + usize::from(file.is_some())
}

/// 校验三通道互斥并选出唯一通道。
///
/// `required` 为 `true`（`set`/`add` 需要值）时零通道报错；为 `false` 时零通道
/// 由调用方决定语义。
pub(crate) fn resolve_value(
    argv: Option<String>,
    stdin: bool,
    file: Option<String>,
    required: bool,
) -> ValueSource {
    let count = channel_count(argv.as_deref(), stdin, file.as_deref());
    if count > 1 {
        exit_err("只能指定一种取值方式：位置参数 / --value / --stdin / --value-file");
    }
    if count == 0 {
        if required {
            exit_err("缺少值：请用位置参数、--value、--stdin 或 --value-file 提供");
        }
        return ValueSource::Argv(String::new());
    }
    if let Some(v) = argv {
        return ValueSource::Argv(v);
    }
    if stdin {
        return ValueSource::Stdin;
    }
    ValueSource::File(file.expect("count==1 且非 argv/stdin 时 file 必有值"))
}

/// 剥离末尾**一个**换行序列（`\n` / `\r\n` / `\r`）。
///
/// 管道 `echo value |` 会带一个换行；只剥一个使「值本身以换行结尾」仍可表达
/// （echo 两次或 `--value-file`）。空串与非换行结尾原样返回。
pub(crate) fn strip_trailing_newline(raw: &str) -> String {
    raw.strip_suffix("\r\n")
        .or_else(|| raw.strip_suffix('\n'))
        .or_else(|| raw.strip_suffix('\r'))
        .unwrap_or(raw)
        .to_string()
}

/// 按通道实际读取值。`Argv` 原样返回（argv 里的换行是用户输入的真实内容）。
pub(crate) fn read_value(src: &ValueSource) -> String {
    match src {
        ValueSource::Argv(v) => v.clone(),
        ValueSource::Stdin => {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .unwrap_or_else(|e| exit_err(&format!("读取标准输入失败: {e}")));
            strip_trailing_newline(&buf)
        }
        ValueSource::File(path) => {
            let raw = std::fs::read_to_string(path)
                .unwrap_or_else(|e| exit_err(&format!("读取值文件失败 ({path}): {e}")));
            strip_trailing_newline(&raw)
        }
    }
}

/// 写操作的并发控制模式。二选一，强制显式给出。
pub(crate) enum Concurrency {
    /// `--revision <R>`：CAS 校验，与 GUI 同强度
    Revision(String),
    /// `--force`：跳过 revision 校验直接覆盖（最后写入者胜，脚本 setx 风格）
    Force,
}

/// 统计并发选项数（互斥与缺失校验用）。
pub(crate) fn concurrency_count(revision: Option<&str>, force: bool) -> usize {
    usize::from(revision.is_some()) + usize::from(force)
}

/// 校验并发选项：必须且只能给一个。
///
/// 设计意图：CLI 是一次性进程，静默降级为「现读现写」会让用户在毫秒级竞态窗口下
/// 最后写入者胜，与仓库 `verify_and_save` 的安全文化相悖。强制显式选择让每次
/// 覆盖都是知情决策。
pub(crate) fn resolve_concurrency(revision: Option<String>, force: bool) -> Concurrency {
    match concurrency_count(revision.as_deref(), force) {
        1 => match revision {
            Some(r) => Concurrency::Revision(r),
            None => Concurrency::Force,
        },
        0 => exit_err("需要提供 --revision（并发校验）或 --force（跳过校验）"),
        _ => exit_err("--revision 与 --force 互斥，只能指定一个"),
    }
}

/// 注册表类型的人类可读标签。
pub(crate) fn kind_label(kind: EnvValueKind) -> &'static str {
    match kind {
        EnvValueKind::String => "string",
        EnvValueKind::ExpandString => "expand",
        EnvValueKind::Unsupported => "unsupported",
    }
}

/// PREVIEW 列内容。敏感变量永不显示值（core 已置 `preview=None`，此处再加一层显式标记）；
/// 非敏感但无 preview（空值 / `Unsupported`）显示 `-`。
pub(crate) fn render_preview(meta: &EnvVarMeta) -> String {
    if meta.sensitive {
        return "(敏感)".to_string();
    }
    meta.preview.clone().unwrap_or_else(|| "-".to_string())
}

/// NAME 列内容。不可编辑的变量加 `(只读)` 后缀。
pub(crate) fn render_name(meta: &EnvVarMeta) -> String {
    if meta.can_edit {
        meta.name.clone()
    } else {
        format!("{} (只读)", meta.name)
    }
}

/// 渲染变量表格。空列表返回占位文案。
pub(crate) fn render_table(metas: &[EnvVarMeta]) -> String {
    if metas.is_empty() {
        return "（无变量）".to_string();
    }
    let rows: Vec<(String, &str, String)> = metas
        .iter()
        .map(|m| (render_name(m), kind_label(m.kind), render_preview(m)))
        .collect();
    let name_w = rows
        .iter()
        .map(|r| r.0.chars().count())
        .chain(std::iter::once("NAME".len()))
        .max()
        .unwrap_or(4);
    let kind_w = rows
        .iter()
        .map(|r| r.1.len())
        .chain(std::iter::once("KIND".len()))
        .max()
        .unwrap_or(4);

    let mut out = format!(
        "{:<name_w$}  {:<kind_w$}  {}\n",
        "NAME",
        "KIND",
        "PREVIEW",
        name_w = name_w,
        kind_w = kind_w
    );
    for (name, kind, preview) in &rows {
        let pad = name_w.saturating_sub(name.chars().count());
        out.push_str(&format!(
            "{}{}  {:<kind_w$}  {}\n",
            name,
            " ".repeat(pad),
            kind,
            preview,
            kind_w = kind_w
        ));
    }
    out.trim_end().to_string()
}

/// 按 hive 过滤构造 JSON 输出对象。
///
/// 该字段直接来自 core 的 `EnvVarSnapshot` 契约（camelCase、无 `value`），
/// 不做任何字段重组，避免出现第二套契约。
pub(crate) fn snapshot_json(
    snapshot: &EnvVarSnapshot,
    system: bool,
    user: bool,
) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    if system || !user {
        map.insert(
            "system".to_string(),
            serde_json::to_value(&snapshot.system).unwrap_or(serde_json::Value::Null),
        );
    }
    if user || !system {
        map.insert(
            "user".to_string(),
            serde_json::to_value(&snapshot.user).unwrap_or(serde_json::Value::Null),
        );
    }
    serde_json::Value::Object(map)
}

/// hive 选择：默认 user，`--system` 切换。
///
/// 写操作绝不跨 hive 兜底 —— 用户没指定 `--system` 时改的是 user hive，
/// 同名变量存在于系统 hive 也不会被意外写入。`--system` 与 `--user` 同时给出
/// 由 clap 的 `conflicts_with` 拒绝（见 main.rs）。
pub(crate) fn select_hive(system: bool, user: bool) -> EnvHive {
    let _ = user;
    if system {
        EnvHive::System
    } else {
        EnvHive::User
    }
}

/// hive 的中文标签（用于提示文案）。
pub(crate) fn hive_label(hive: EnvHive) -> &'static str {
    match hive {
        EnvHive::System => "系统",
        EnvHive::User => "用户",
    }
}

/// `env get` 的 stdout 负载：裸值 + 换行。
///
/// 无前缀、无引号、无标签 —— 使 `patheditor env get JAVA_HOME` 可直接被
/// 命令替换或管道消费（对齐 `git config --get`）。
pub(crate) fn format_get_output(value: &str) -> String {
    format!("{value}\n")
}

/// `env list` —— 列出变量元数据（不含明文）。
pub(crate) fn cmd_env_list(system: bool, user: bool, json_out: bool) {
    let snapshot = core::registry::list_all_env_vars().unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        let value = snapshot_json(&snapshot, system, user);
        println!("{}", serde_json::to_string_pretty(&value).unwrap());
        return;
    }
    let show_sys = system || !user;
    let show_usr = user || !system;
    if show_sys {
        println!("═══ 系统环境变量（{} 个）═══", snapshot.system.len());
        println!("{}", render_table(&snapshot.system));
    }
    if show_usr {
        println!("═══ 用户环境变量（{} 个）═══", snapshot.user.len());
        println!("{}", render_table(&snapshot.user));
    }
}

/// `env get` —— 读取单个变量的明文。这是 CLI 侧唯一的明文出口。
pub(crate) fn cmd_env_get(name: String, system: bool) {
    let hive = select_hive(system, false);
    match core::registry::reveal_env_var(hive, &name) {
        Ok(revealed) => print!("{}", format_get_output(&revealed.value)),
        Err(e) => {
            let msg = e.message.clone();
            // 仅只读路径提供「变量存在于另一 hive」的提示，帮助用户加 --system。
            // 写操作不做此兜底 —— 见设计文档「hive 选择」。
            if let Some(other) = other_hive_hint(&name, hive) {
                exit_err(&format!("{msg}\n{other}"));
            }
            exit_core_error(&e)
        }
    }
}

/// 当前 hive 未命中时，探测另一 hive 以生成更准确的错误提示。
///
/// 返回 `None` 表示另一 hive 也没有该变量（沉默，避免误导）。
fn other_hive_hint(name: &str, current: EnvHive) -> Option<String> {
    let other = match current {
        EnvHive::System => EnvHive::User,
        EnvHive::User => EnvHive::System,
    };
    let snapshot = core::registry::list_all_env_vars().ok()?;
    let metas = match other {
        EnvHive::System => &snapshot.system,
        EnvHive::User => &snapshot.user,
    };
    let found = metas.iter().any(|m| m.name.eq_ignore_ascii_case(name));
    if found {
        Some(format!(
            "提示：{name} 存在于{} hive，请加 --system",
            hive_label(other)
        ))
    } else {
        None
    }
}

/// 解析 `--kind` 取值。非法值由 clap 的 `value_parser` 提前拒绝。
pub(crate) fn parse_kind(raw: &str) -> EnvValueKind {
    if raw.eq_ignore_ascii_case("expand") {
        EnvValueKind::ExpandString
    } else {
        EnvValueKind::String
    }
}

/// `env set` —— 修改已有变量的值。类型跟随注册表现状，不可更改。
///
/// `--revision` 走 CAS（冲突退出码 3）；`--force` 走真正的覆盖写
/// （最后写入者胜，**不会**产生退出码 3）。
pub(crate) fn cmd_env_set(
    name: String,
    value: Option<String>,
    stdin: bool,
    value_file: Option<String>,
    revision: Option<String>,
    force: bool,
    system: bool,
) {
    let hive = select_hive(system, false);
    let mode = resolve_concurrency(revision, force);
    let src = resolve_value(value, stdin, value_file, true);
    let new_value = read_value(&src);
    match mode {
        Concurrency::Revision(r) => {
            let outcome = core::registry::update_env_var(hive, &name, &new_value, &r)
                .unwrap_or_else(|e| exit_core_error(&e));
            warn_if_backup_failed(&outcome.backup);
        }
        Concurrency::Force => {
            let outcome = core::registry::update_env_var_force(hive, &name, &new_value)
                .unwrap_or_else(|e| exit_core_error(&e));
            warn_if_backup_failed(&outcome.backup);
        }
    }
    println!("已更新{}变量: {name}", hive_label(hive));
}

/// `env add` —— 新建变量。`--kind` 决定注册表类型。
pub(crate) fn cmd_env_add(
    name: String,
    value: Option<String>,
    stdin: bool,
    value_file: Option<String>,
    kind: String,
    system: bool,
) {
    let hive = select_hive(system, false);
    let src = resolve_value(value, stdin, value_file, true);
    let new_value = read_value(&src);
    let kind = parse_kind(&kind);
    // 新建无并发语义：core 会拒绝重名（检查与写入是两步，存在竞态窗口，见 IPC 文档）
    let outcome = core::registry::create_env_var(hive, &name, &new_value, kind)
        .unwrap_or_else(|e| exit_core_error(&e));
    warn_if_backup_failed(&outcome.backup);
    // 广播由 core 负责，此处不重复
    println!("已新建{}变量: {name}", hive_label(hive));
}

/// `env remove` —— 删除变量。
///
/// `--revision` 走 CAS；`--force` 直接删除（最后写入者胜，不产生退出码 3）。
pub(crate) fn cmd_env_remove(name: String, revision: Option<String>, force: bool, system: bool) {
    let hive = select_hive(system, false);
    let mode = resolve_concurrency(revision, force);
    match mode {
        Concurrency::Revision(r) => {
            let outcome = core::registry::delete_env_var(hive, &name, &r)
                .unwrap_or_else(|e| exit_core_error(&e));
            warn_if_backup_failed(&outcome.backup);
        }
        Concurrency::Force => {
            let outcome = core::registry::delete_env_var_force(hive, &name)
                .unwrap_or_else(|e| exit_core_error(&e));
            warn_if_backup_failed(&outcome.backup);
        }
    }
    println!("已删除{}变量: {name}", hive_label(hive));
}

/// `env backup` —— 立即创建一份环境变量备份。
///
/// 与写前自动备份共用同一 core 实现；可在「改之前想手动留个还原点」时使用。
/// 备份文件含明文（设计文档 D2），落盘位置由 `PATHEDITOR_BACKUP_DIR` 或
/// `~/.patheditor/backups/` 决定。
///
/// # Returns
/// 无返回值：失败经 `exit_core_error` 终止进程（退出码由 `CoreError::exit_code()` 决定）。
pub(crate) fn cmd_env_backup(json_out: bool) {
    match core::backup::backup_env_vars() {
        Ok(path) => {
            if json_out {
                println!("{}", serde_json::json!({ "path": path.to_string_lossy() }));
            } else {
                println!("备份已保存到: {}", path.display());
            }
        }
        Err(e) => exit_core_error(&e),
    }
}

/// `env backups` —— 列出已有的环境变量备份（按时间倒序）。
///
/// 只枚举目录与 stat，**不解析内容**：单个损坏的备份不会让列表失败，
/// 因此表格里不显示变量数（`EnvBackupInfo::variable_count` 恒为 0）。
///
/// # Returns
/// 无返回值：失败经 `exit_core_error` 终止进程。
pub(crate) fn cmd_env_backups(json_out: bool) {
    let list = core::backup::list_env_backups().unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        println!("{}", serde_json::to_string_pretty(&list).unwrap());
        return;
    }
    if list.is_empty() {
        println!(
            "暂无环境变量备份（目录: {}）",
            core::backup::env_backup_dir().display()
        );
        return;
    }
    println!("{:<40} {:>12}  路径", "文件", "大小(字节)");
    for info in &list {
        println!("{:<40} {:>12}  {}", info.file, info.size_bytes, info.path);
    }
}

/// `env restore` —— 从备份文件恢复环境变量。
///
/// 路径校验（扩展名 / 位置 / 大小上限）与保护名单、类型、权限判定**全部在 core 侧**，
/// 本函数只做参数转换与错误透传。
///
/// 默认模式在检测到 revision 冲突时中止且**不做任何写入**（`code == Conflict` →
/// 退出码 3）；`--force` 跳过 revision 校验直接覆盖，但**不豁免**保护名单与类型判定，
/// 也不会因并发冲突产生退出码 3。
///
/// `--dry-run` 走 `read_env_backup` + `preview_restore`（**不带 force**：差异计算与
/// force 无关），只打印不写入。
///
/// 注意：`preview_restore` 的 `modified` 恒为 0（见 `RestoreChangeKind::Modified`），
/// 故 dry-run 的「修改 N」恒显示 0；`--force` 下被冲突覆盖的变量计入 `conflicts`，
/// 所以 dry-run 会**低报** force 模式的实际改动量。CLI 不得为此做换算（那会复制
/// core 的判定逻辑），如实现原样输出计数。
///
/// # Returns
/// 无返回值：失败经 `exit_core_error` 终止进程。
pub(crate) fn cmd_env_restore(file: String, dry_run: bool, force: bool, json_out: bool) {
    let path = core::backup::validate_backup_path(&file).unwrap_or_else(|e| exit_core_error(&e));

    if dry_run {
        let payload = core::backup::read_env_backup(&path).unwrap_or_else(|e| exit_core_error(&e));
        let preview =
            core::backup::preview_restore(&payload).unwrap_or_else(|e| exit_core_error(&e));
        if json_out {
            println!("{}", serde_json::to_string_pretty(&preview).unwrap());
            return;
        }
        println!(
            "将新增 {} 个、修改 {} 个、删除 {} 个；冲突 {} 个（未写入任何内容）",
            preview.added, preview.modified, preview.removed, preview.conflicts
        );
        for change in &preview.changes {
            println!("  [{:?}] {} ({:?})", change.hive, change.name, change.kind);
        }
        return;
    }

    // 手工兜底出口（核对轮 C7）：恢复自己不产生备份（否则每次恢复都新增文件，
    // 与保留策略互相吞噬），所以「恢复错了」没有自动回退。先把两条命令告诉用户。
    // **只打印、绝不自动执行**——自动备份会与保留策略互相吞噬。
    // 走 stderr：不污染 stdout 的输出契约。
    eprintln!("提示: 恢复不会自动备份当前状态。如需留还原点，请先执行:");
    eprintln!("  patheditor backup       # 备份当前 PATH");
    eprintln!("  patheditor env backup   # 备份当前全部环境变量");

    let outcome =
        core::backup::restore_env_backup_from(&path, force).unwrap_or_else(|e| exit_core_error(&e));

    if json_out {
        println!("{}", serde_json::to_string_pretty(&outcome).unwrap());
    } else {
        println!("恢复完成: 成功 {} 个", outcome.applied);
    }
    // 逐条失败是 best-effort 语义的一部分（不改变退出码），必须可见。
    for failure in &outcome.failures {
        eprintln!("警告: {failure}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 值通道互斥 ──

    #[test]
    fn value_source_argv_alone_is_accepted() {
        let src = resolve_value(Some("C:\\Java".into()), false, None, true);
        assert!(matches!(src, ValueSource::Argv(v) if v == "C:\\Java"));
    }

    #[test]
    fn value_source_stdin_alone_is_accepted() {
        let src = resolve_value(None, true, None, true);
        assert!(matches!(src, ValueSource::Stdin));
    }

    #[test]
    fn value_source_file_alone_is_accepted() {
        let src = resolve_value(None, false, Some("secret.txt".into()), true);
        assert!(matches!(src, ValueSource::File(f) if f == "secret.txt"));
    }

    #[test]
    fn value_source_rejects_multiple_channels() {
        // 互斥校验必须在读取任何输入之前失败；用 catch_unwind 捕获 process::exit 不现实，
        // 因此这里只断言「判定函数」的纯逻辑：通道计数 > 1 时为错误。
        assert_eq!(channel_count(Some("v"), true, None), 2);
        assert_eq!(channel_count(Some("v"), false, Some("f")), 2);
        assert_eq!(channel_count(None, true, Some("f")), 2);
        assert_eq!(channel_count(Some("v"), true, Some("f")), 3);
        assert_eq!(channel_count(Some("v"), false, None), 1);
        assert_eq!(channel_count(None, false, None), 0);
    }

    #[test]
    fn value_source_missing_when_required() {
        assert_eq!(channel_count(None, false, None), 0);
        // required=true 且 0 通道 → 调用方报错；required=false 且 0 通道 → 空值合法
    }

    // ── 末尾换行剥离 ──

    #[test]
    fn strips_single_lf() {
        assert_eq!(strip_trailing_newline("C:\\Java\n"), "C:\\Java");
    }

    #[test]
    fn strips_single_crlf() {
        assert_eq!(strip_trailing_newline("C:\\Java\r\n"), "C:\\Java");
    }

    #[test]
    fn strips_only_one_trailing_sequence() {
        // 只剥一个 —— 值本身以换行结尾时靠第二个换行表达
        assert_eq!(strip_trailing_newline("a\n\n"), "a\n");
        assert_eq!(strip_trailing_newline("a\r\n\r\n"), "a\r\n");
    }

    #[test]
    fn leaves_value_without_newline_untouched() {
        assert_eq!(strip_trailing_newline("C:\\Java"), "C:\\Java");
        assert_eq!(strip_trailing_newline(""), "");
    }

    #[test]
    fn strips_lone_cr() {
        // 单独的 \r（旧式 Mac / 误操作）也按换行处理
        assert_eq!(strip_trailing_newline("a\r"), "a");
    }

    // ── 并发模式互斥 ──

    #[test]
    fn concurrency_accepts_revision_alone() {
        let mode = resolve_concurrency(Some("a1b2c3".into()), false);
        assert!(matches!(mode, Concurrency::Revision(r) if r == "a1b2c3"));
    }

    #[test]
    fn concurrency_accepts_force_alone() {
        let mode = resolve_concurrency(None, true);
        assert!(matches!(mode, Concurrency::Force));
    }

    #[test]
    fn concurrency_channel_count_rule() {
        // 互斥与缺失的判定是纯函数，单独断言避免依赖 process::exit
        assert_eq!(concurrency_count(Some("r"), false), 1);
        assert_eq!(concurrency_count(None, true), 1);
        assert_eq!(concurrency_count(Some("r"), true), 2); // 互斥冲突
        assert_eq!(concurrency_count(None, false), 0); // 缺失
    }

    // ── 冲突结果映射到退出码 3 ──

    #[test]
    fn conflict_result_maps_to_exit_3() {
        // F-06：退出码由 CoreError.exit_code() 决定，冲突码必须映射到 3
        let e = core::CoreError::new(core::ErrorCode::Conflict, "update_env_var", "冲突");
        assert_eq!(e.exit_code(), 3);
        assert_eq!(
            core::CoreError::new(core::ErrorCode::NotFound, "op", "不存在").exit_code(),
            1
        );
    }

    #[test]
    fn force_mode_never_carries_a_revision() {
        // Force 分支不产生 revision 字符串，也不调用 current_revision ——
        // 由 cmd 层直接调用 core 的 update_env_var_force。
        let mode = resolve_concurrency(None, true);
        assert!(matches!(mode, Concurrency::Force));
    }

    // ── 表格渲染 ──

    fn meta(
        name: &str,
        kind: EnvValueKind,
        preview: Option<&str>,
        can_edit: bool,
        sensitive: bool,
    ) -> EnvVarMeta {
        EnvVarMeta {
            name: name.into(),
            kind,
            hive: EnvHive::User,
            can_edit,
            can_delete: can_edit,
            sensitive,
            preview: preview.map(|p| p.to_string()),
            revision: "0000000000000000".into(),
        }
    }

    #[test]
    fn kind_label_maps_all_variants() {
        assert_eq!(kind_label(EnvValueKind::String), "string");
        assert_eq!(kind_label(EnvValueKind::ExpandString), "expand");
        assert_eq!(kind_label(EnvValueKind::Unsupported), "unsupported");
    }

    #[test]
    fn sensitive_variable_hides_preview() {
        let m = meta("MY_TOKEN", EnvValueKind::String, None, true, true);
        assert_eq!(render_preview(&m), "(敏感)");
    }

    #[test]
    fn non_sensitive_variable_shows_preview() {
        let m = meta(
            "JAVA_HOME",
            EnvValueKind::String,
            Some("C:\\Java"),
            true,
            false,
        );
        assert_eq!(render_preview(&m), "C:\\Java");
    }

    #[test]
    fn empty_preview_renders_dash() {
        // preview=None 且非敏感（空值 / Unsupported）→ 占位
        let m = meta("EMPTY_VAR", EnvValueKind::String, None, true, false);
        assert_eq!(render_preview(&m), "-");
    }

    #[test]
    fn unwritable_variable_shows_readonly_marker() {
        let m = meta(
            "windir",
            EnvValueKind::String,
            Some("C:\\Windows"),
            false,
            false,
        );
        assert_eq!(render_name(&m), "windir (只读)");
    }

    #[test]
    fn writable_variable_has_no_marker() {
        let m = meta(
            "JAVA_HOME",
            EnvValueKind::String,
            Some("C:\\Java"),
            true,
            false,
        );
        assert_eq!(render_name(&m), "JAVA_HOME");
    }

    #[test]
    fn table_contains_header_and_rows() {
        let metas = vec![
            meta(
                "JAVA_HOME",
                EnvValueKind::String,
                Some("C:\\Java"),
                true,
                false,
            ),
            meta("MY_TOKEN", EnvValueKind::String, None, true, true),
        ];
        let table = render_table(&metas);
        assert!(table.contains("NAME"));
        assert!(table.contains("KIND"));
        assert!(table.contains("PREVIEW"));
        assert!(table.contains("JAVA_HOME"));
        assert!(table.contains("string"));
        assert!(table.contains("C:\\Java"));
        assert!(table.contains("(敏感)"));
    }

    #[test]
    fn empty_table_reports_none() {
        assert_eq!(render_table(&[]), "（无变量）");
    }

    // ── JSON 输出 ──

    fn sample_snapshot() -> EnvVarSnapshot {
        EnvVarSnapshot {
            system: vec![meta(
                "windir",
                EnvValueKind::String,
                Some("C:\\Windows"),
                false,
                false,
            )],
            user: vec![meta(
                "JAVA_HOME",
                EnvValueKind::String,
                Some("C:\\Java"),
                true,
                false,
            )],
            captured_at: 0,
        }
    }

    #[test]
    fn json_both_hives_included_by_default() {
        let value = snapshot_json(&sample_snapshot(), false, false);
        assert!(value.get("system").is_some());
        assert!(value.get("user").is_some());
    }

    #[test]
    fn json_system_only_filters_user() {
        let value = snapshot_json(&sample_snapshot(), true, false);
        assert!(value.get("system").is_some());
        assert!(value.get("user").is_none(), "单 hive 模式不得输出另一 hive");
    }

    #[test]
    fn json_user_only_filters_system() {
        let value = snapshot_json(&sample_snapshot(), false, true);
        assert!(value.get("user").is_some());
        assert!(value.get("system").is_none());
    }

    #[test]
    fn json_uses_camel_case_contract() {
        let value = snapshot_json(&sample_snapshot(), false, true);
        let first = &value["user"][0];
        assert!(first.get("canEdit").is_some(), "契约字段必须是 camelCase");
        assert!(first.get("canDelete").is_some());
        assert!(first.get("name").is_some());
        assert!(first.get("revision").is_some());
        assert!(
            first.get("value").is_none(),
            "契约上不得出现 value 字段（明文只经 env get 输出）"
        );
        assert!(first.get("can_edit").is_none(), "不得混入 snake_case");
    }

    #[test]
    fn json_empty_hive_is_empty_array() {
        let empty = EnvVarSnapshot::default();
        let value = snapshot_json(&empty, false, false);
        assert_eq!(value["system"].as_array().map(Vec::len), Some(0));
        assert_eq!(value["user"].as_array().map(Vec::len), Some(0));
    }

    // ── hive 选择 ──

    #[test]
    fn hive_defaults_to_user() {
        assert_eq!(select_hive(false, false), EnvHive::User);
    }

    #[test]
    fn hive_system_flag_wins() {
        assert_eq!(select_hive(true, false), EnvHive::System);
    }

    #[test]
    fn hive_user_flag_selects_user() {
        assert_eq!(select_hive(false, true), EnvHive::User);
    }

    #[test]
    fn hive_label_is_chinese() {
        assert_eq!(hive_label(EnvHive::System), "系统");
        assert_eq!(hive_label(EnvHive::User), "用户");
    }

    // ── get 的输出必须是裸值 ──

    #[test]
    fn get_output_is_bare_value() {
        // stdout 契约：值 + 换行，无前缀、无引号、无标签
        assert_eq!(format_get_output("C:\\Java"), "C:\\Java\n");
        assert_eq!(format_get_output(""), "\n");
        assert_eq!(format_get_output("a b"), "a b\n");
    }

    #[test]
    fn get_output_preserves_inner_newlines() {
        // 值内部的换行不处理，仅补一个结尾换行
        assert_eq!(format_get_output("a\nb"), "a\nb\n");
    }

    // ── kind 解析 ──

    #[test]
    fn parse_kind_maps_both_writable_kinds() {
        assert_eq!(parse_kind("string"), EnvValueKind::String);
        assert_eq!(parse_kind("expand"), EnvValueKind::ExpandString);
    }

    #[test]
    fn parse_kind_is_case_insensitive() {
        assert_eq!(parse_kind("STRING"), EnvValueKind::String);
        assert_eq!(parse_kind("Expand"), EnvValueKind::ExpandString);
    }

    // ── env backup / env backups / env restore：命令行参数解析 ──

    /// 恢复命令的参数解析：`--dry-run` 与 `--force` 可共存（先看差异再覆盖）。
    ///
    /// 本用例不止断言「解析成功」—— 那只说明 clap 没报错，无法区分
    /// 「两个标志都被接受」与「其中一个被静默忽略 / 未接线到结构体字段」。
    /// 因此逐字段断言，且断言两个布尔值**都为 `true`**：把任一标志改成别名、
    /// 或把字段默认值写死，对应断言就会失败。
    #[test]
    fn restore_flags_are_independent() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "patheditor",
            "env",
            "restore",
            "env_backup_x.json",
            "--dry-run",
            "--force",
        ])
        .expect("--dry-run 与 --force 必须可共存");
        let crate::Command::Env(crate::EnvCmd::Restore {
            file,
            dry_run,
            force,
            json,
        }) = cli.command
        else {
            panic!("应解析为 env restore");
        };
        assert!(dry_run, "--dry-run 必须解析为 true，而不是被静默忽略");
        assert!(force, "--force 必须解析为 true，而不是被静默忽略");
        assert!(!json, "未给 --json 时应为 false");
        assert_eq!(file, "env_backup_x.json", "位置参数必须落到 file");
    }

    /// 恢复命令的默认值是「所有开关关闭」，即默认走真恢复而不是 dry-run。
    ///
    /// 这条断言与安全相关：若 `dry_run` 默认成 `true`，用户敲
    /// `patheditor env restore <file>` 会以为已恢复、实际什么都没写；
    /// 若 `force` 默认成 `true`，则 revision 保护会被静默跳过。
    #[test]
    fn restore_defaults_are_all_off() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from(["patheditor", "env", "restore", "env_backup_x.json"])
            .expect("不带任何标志的形式必须可解析");
        let crate::Command::Env(crate::EnvCmd::Restore {
            dry_run,
            force,
            json,
            ..
        }) = cli.command
        else {
            panic!("应解析为 env restore");
        };
        assert!(!dry_run, "默认必须是真恢复，而不是 dry-run");
        assert!(!force, "默认不得跳过 revision 校验");
        assert!(!json);
    }

    /// `env backup` / `env backups` 的标志解析各自落到正确字段。
    #[test]
    fn backup_and_backups_flags_parse() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from(["patheditor", "env", "backup", "--json"])
            .expect("env backup --json 必须可解析");
        let crate::Command::Env(crate::EnvCmd::Backup { json }) = cli.command else {
            panic!("应解析为 env backup");
        };
        assert!(json, "--json 必须解析为 true");

        let cli = crate::Cli::try_parse_from(["patheditor", "env", "backups"])
            .expect("env backups 必须可解析");
        let crate::Command::Env(crate::EnvCmd::Backups { json }) = cli.command else {
            panic!("应解析为 env backups");
        };
        assert!(!json, "未给 --json 时应为 false");
    }

    /// 备份列表在空目录下返回空数组而非报错（JSON 输出形状稳定）。
    ///
    /// B3（核对轮第二轮）：**不取 `core::persist::test_persist_lock`** —— 该函数是
    /// `pub(crate)`（`core/src/lib.rs` 的 `pub(crate) mod persist;`），从 cli crate
    /// 引用会报 `error[E0603]: module persist is private`。而且也不需要：`--bins`
    /// 测试跑在**独立进程**里，与 core 的测试不共享进程级环境变量，那把进程内的锁
    /// 跨进程无意义。core 的 `test_persist_lock` 只用于 core crate 内部的同进程测试。
    ///
    /// 说明：本用例断言的是 core 的契约（空目录不报错、`[]` 形状），
    /// **不是**本任务改动的回归测试 —— 删掉 `cmd_env_backups` 它依然会通过。
    /// 本任务对 CLI 行为的覆盖在下面的进程级用例里。
    #[test]
    fn backups_list_json_shape_is_stable() {
        let dir = std::env::temp_dir().join(format!("patheditor_t7_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("PATHEDITOR_BACKUP_DIR", &dir);

        let list = path_editor_core::backup::list_env_backups().expect("空目录不得报错");
        let json = serde_json::to_string(&list).unwrap();
        assert_eq!(json, "[]");

        std::env::remove_var("PATHEDITOR_BACKUP_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── env backup / env backups / env restore：进程级行为 ──
    //
    // 为什么用子进程而不是直接调用 `cmd_env_*`：这三个函数在错误路径上调用
    // `std::process::exit`，并直接打印 stdout / stderr，进程内既拿不到退出码也
    // 拿不到输出。「测试看似通过但测不到东西」是本波反复出现的失败模式，因此这里
    // 断言的是**退出码 + stdout + stderr + 文件系统副作用**四者。
    //
    // 安全边界：只走只读路径，以及「只写临时目录」的备份路径。
    // `env backup` 只读注册表、只写 `PATHEDITOR_BACKUP_DIR`（本文件把它指向临时
    // 目录），`env backups` 与 `env restore --dry-run` 完全不写；
    // **没有任何用例会写真实注册表** —— `env restore`（非 dry-run）的用例在未提权
    // 环境下必定在第一次写入之前以权限错误失败，这一点由用例自身断言。

    /// 已构建的 CLI 二进制路径。
    ///
    /// **不用 `env!("CARGO_BIN_EXE_patheditor")`**：该变量只对 `tests/` 下的集成测试
    /// target 注入，对 `--bins`（bin target 自身的 `#[cfg(test)]`）是 `None`
    /// （实测 `option_env!` 为 `None`、运行时 `std::env::var` 为 `NotPresent`）。
    ///
    /// 改为按需构建二进制并缓存路径。嵌套 `cargo build` 在首次跑测试时已完成编译，
    /// 此处是秒级的空跑；加锁保证并行用例只构建一次。
    ///
    /// **target 目录从 `cargo metadata` 读，不写死 `target/`**：后者只是
    /// 「`CARGO_TARGET_DIR` 未设置且无 `--target`」时的约定。若外部环境设了
    /// `CARGO_TARGET_DIR`，嵌套构建会把产物写到别处，而写死的仓库内路径可能被
    /// **上一次构建的陈旧二进制**满足 —— 测试就「为错误的原因通过」了。
    /// 让「问 cargo 拿目录」与「让 cargo 往那里构建」共用同一个答案，二者才必然一致。
    ///
    /// 刻意**不**用 mtime 断言「产物晚于本次构建」：cargo 对已是最新的产物不重写
    /// （实测第二次空跑 mtime 不变），那种断言会在任何 no-op 重建上假失败。
    /// 排除陈旧产物要靠路径正确，不是靠时间戳。
    ///
    /// 已知未覆盖项：`Command::output()` 无超时，若并发 `cargo` 持锁会阻塞而非失败
    /// （未被观测到；加超时需自建 spawn+poll 循环，评估为不划算，记为待办）。
    fn cli_bin() -> std::path::PathBuf {
        static BUILD: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
        BUILD
            .get_or_init(|| {
                let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
                let root = manifest_dir
                    .parent()
                    .expect("cli crate 必有父目录（workspace 根）");

                // 1) 问 cargo 真实的 target 目录（它自己会考虑 CARGO_TARGET_DIR 与配置）。
                let meta = std::process::Command::new("cargo")
                    .args(["metadata", "--format-version", "1", "--no-deps"])
                    .current_dir(root)
                    .output()
                    .expect("启动 cargo metadata 失败");
                assert!(
                    meta.status.success(),
                    "cargo metadata 失败: {}",
                    String::from_utf8_lossy(&meta.stderr)
                );
                let meta_json: serde_json::Value =
                    serde_json::from_slice(&meta.stdout).expect("cargo metadata 输出不是合法 JSON");
                let target_dir = std::path::PathBuf::from(
                    meta_json["target_directory"]
                        .as_str()
                        .expect("cargo metadata 缺少 target_directory"),
                );

                // 2) 用**同一个** target 目录构建（不显式传 --target-dir，让它走默认解析，
                //    与 metadata 的答案保持一致）。
                let out = std::process::Command::new("cargo")
                    .args(["build", "-p", "patheditor-cli"])
                    .current_dir(root)
                    .output()
                    .expect("启动 cargo build 失败");
                assert!(
                    out.status.success(),
                    "构建 CLI 二进制失败: {}",
                    String::from_utf8_lossy(&out.stderr)
                );

                let bin = target_dir.join("debug").join("patheditor.exe");
                assert!(bin.is_file(), "构建后仍找不到二进制: {}", bin.display());
                bin
            })
            .clone()
    }

    /// 本次用例独占的临时目录（进程号 + 标签，避免并行用例互相干扰）。
    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("patheditor_t7_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建临时目录失败");
        dir
    }

    /// 在 `dir` 下真备份一份（只读注册表 + 只写临时目录），返回备份文件路径。
    ///
    /// 作为多个进程级用例的前置条件使用；前置失败直接 panic（后续断言无意义）。
    fn make_backup(dir: &std::path::Path) -> std::path::PathBuf {
        let out = std::process::Command::new(cli_bin())
            .args(["env", "backup"])
            .env("PATHEDITOR_BACKUP_DIR", dir)
            .output()
            .expect("启动 env backup 失败");
        assert!(
            out.status.success(),
            "前置备份必须成功（只写临时目录）: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        std::fs::read_dir(dir)
            .expect("枚举备份目录失败")
            .next()
            .expect("备份目录应恰好有一个文件")
            .expect("读取目录项失败")
            .path()
    }

    /// `env backups`（无 `--json`）的人类可读表格：含表头、文件名、真实字节数与路径。
    ///
    /// 覆盖非 JSON 分支 —— 该分支此前无任何断言（变异测试把表格里的 `sizeBytes`
    /// 整个删掉时用例仍然全绿）。
    #[test]
    fn env_backups_table_lists_file_size_and_path() {
        let dir = temp_dir("backups_table");
        let file = make_backup(&dir);
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let size = std::fs::metadata(&file).unwrap().len();

        let out = std::process::Command::new(cli_bin())
            .args(["env", "backups"])
            .env("PATHEDITOR_BACKUP_DIR", &dir)
            .output()
            .expect("启动 env backups 失败");

        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "列备份必须成功: {stdout} / {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(stdout.contains("文件"), "表格必须有表头: {stdout}");
        assert!(
            stdout.contains("大小(字节)"),
            "表头必须标注字节数: {stdout}"
        );
        assert!(stdout.contains(&name), "表格必须列出文件名: {stdout}");
        assert!(
            stdout.contains(&size.to_string()),
            "表格必须列出真实字节数 {size}: {stdout}"
        );
        assert!(
            stdout.contains(&file.to_string_lossy().to_string()),
            "表格必须列出完整路径: {stdout}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `env backups` 在空目录下成功、输出 `[]`，且不创建任何文件。
    #[test]
    fn env_backups_reports_empty_dir_without_touching_it() {
        let dir = temp_dir("backups_empty");
        let out = std::process::Command::new(cli_bin())
            .args(["env", "backups", "--json"])
            .env("PATHEDITOR_BACKUP_DIR", &dir)
            .output()
            .expect("启动 env backups 失败");

        assert!(
            out.status.success(),
            "空目录必须成功退出: {:?} / {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "[]");
        assert!(
            std::fs::read_dir(&dir).unwrap().next().is_none(),
            "env backups 是纯读命令，不得在备份目录里创建任何文件"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `env backups` **不解析备份内容**：目录里存在一个损坏文件时仍然成功，
    /// 且把它一并列出。
    ///
    /// 这是「不解析内容」（S5）的可证伪断言：若实现改为逐个读取/解析备份文件，
    /// 损坏文件会让命令失败或让该条目消失 —— 两种改动都会让本用例失败。
    #[test]
    fn env_backups_lists_entries_without_parsing_contents() {
        let dir = temp_dir("backups_corrupt");
        let _ = make_backup(&dir); // 一份正常备份
                                   // 一份故意损坏的备份：命名合规、内容不是 JSON
        let corrupt = dir.join("env_backup_00000000_000000_000.json");
        std::fs::write(&corrupt, b"{{{ not json at all").unwrap();

        let out = std::process::Command::new(cli_bin())
            .args(["env", "backups", "--json"])
            .env("PATHEDITOR_BACKUP_DIR", &dir)
            .output()
            .expect("启动 env backups 失败");

        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "损坏文件不得让列表失败（列表不解析内容）: {stdout} / {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let list: serde_json::Value = serde_json::from_str(&stdout).expect("必须是合法 JSON");
        let files: Vec<&str> = list
            .as_array()
            .expect("--json 必须输出数组")
            .iter()
            .map(|item| item["file"].as_str().expect("每项必须含 file"))
            .collect();
        assert_eq!(files.len(), 2, "两个文件都应被列出: {files:?}");
        assert!(
            files.contains(&"env_backup_00000000_000000_000.json"),
            "损坏文件也必须出现在列表里: {files:?}"
        );
        // 列表不解析内容，所以变量数恒为 0（契约字段 variableCount）；
        // 大小来自 stat，必须与磁盘上的真实字节数一致（含那个损坏文件）。
        for item in list.as_array().unwrap() {
            assert_eq!(
                item["variableCount"].as_u64(),
                Some(0),
                "variableCount 恒为 0（列表不解析内容）: {item}"
            );
            let file = item["file"].as_str().unwrap();
            let expected = std::fs::metadata(dir.join(file)).unwrap().len();
            assert_eq!(
                item["sizeBytes"].as_u64(),
                Some(expected),
                "sizeBytes 必须来自 stat，与实际文件大小一致: {item}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `env backup --json` 真写出一份备份文件，`path` 字段指向真实存在的文件。
    #[test]
    fn env_backup_writes_a_file_and_reports_its_path() {
        let dir = temp_dir("backup_write");
        let out = std::process::Command::new(cli_bin())
            .args(["env", "backup", "--json"])
            .env("PATHEDITOR_BACKUP_DIR", &dir)
            .output()
            .expect("启动 env backup 失败");

        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "备份应成功: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let value: serde_json::Value =
            serde_json::from_str(&stdout).expect("--json 必须输出合法 JSON");
        let reported = value["path"].as_str().expect("JSON 必须含 path 字段");
        assert!(
            std::path::Path::new(reported).is_file(),
            "报告的路径必须真实存在: {reported}"
        );

        let names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 1, "目录里应恰好一份备份: {names:?}");
        assert!(
            names[0].starts_with("env_backup_") && names[0].ends_with(".json"),
            "备份文件命名必须符合 env_backup_*.json: {names:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `env restore --dry-run` 成功预览、四个计数全为 0（刚采集的备份与注册表一致），
    /// 打印「未写入任何内容」，且**不**打印手工兜底指引、**不**写任何文件。
    ///
    /// 「四个计数全为 0」由刚生成的备份保证：备份与注册表逐条同 name / type / value，
    /// revision 必然相等，故既无 `Added` / `Removed` 也无 `Conflict`。
    /// 这同时钉住了 `modified` 恒为 0 的现状（见 `RestoreChangeKind::Modified` 文档）。
    ///
    /// 计数器**字段映射**的证伪靠 [`env_restore_dry_run_reports_each_counter_field`]：
    /// 全 0 的计数无法区分「四个字段接对了」与「四个字段互相串了」。
    #[test]
    fn env_restore_dry_run_previews_without_writing() {
        let dir = temp_dir("restore_dry");
        let file = make_backup(&dir);

        let out = std::process::Command::new(cli_bin())
            .arg("env")
            .arg("restore")
            .arg(&file)
            .arg("--dry-run")
            .env("PATHEDITOR_BACKUP_DIR", &dir)
            .output()
            .expect("启动 env restore 失败");

        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success(),
            "dry-run 必须成功: {stdout} / {stderr}"
        );
        assert!(
            stdout.contains("将新增 0 个、修改 0 个、删除 0 个；冲突 0 个"),
            "dry-run 计数不符（备份即当前状态应为全 0）: {stdout}"
        );
        assert!(stdout.contains("未写入任何内容"), "实际 stdout: {stdout}");
        assert!(
            !stderr.contains("patheditor env backup"),
            "dry-run 不得打印手工兜底指引（它什么都没改）: {stderr}"
        );
        let names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 1, "dry-run 不得新增文件: {names:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 把真实备份改造成「新增 3 / 修改 0 / 删除 1 / 冲突 2」的夹具，返回其路径。
    ///
    /// **四个计数两两不等是本夹具的存在理由**：若计数有重复（例如删除与冲突都是 1），
    /// 把格式串里两个字段对调仍然是「通过」的 —— 断言就成了自证的。
    /// 本夹具的第一次版本正是 `删除 1 / 冲突 1`，变异测试（对调 `removed` 与
    /// `conflicts`）**没有**让它失败，因此重做为两两不等：
    /// 新增 3、修改 0、删除 1、冲突 2 互不相同，任两个字段对调都会被断言抓到。
    ///
    /// 三个改动都只落在**临时目录里的副本**上，不碰注册表：
    /// - 塞入 3 个注册表中不存在的名字 → `Added`；
    /// - 从载荷里删掉 `user[0]` → 注册表有、备份无 → `Removed`（恰 1 条）；
    /// - 把 `user[1]`、`user[2]` 的 **revision 换成不可能匹配的固定值** → `Conflict`（恰 2 条）。
    ///
    /// 注意冲突的造法：`diff_one_hive` 比较的是**载荷里存的 revision**
    /// 与注册表当前 revision，而不是重算 `revision_of(name, type, value)`。
    /// 因此只改 `value` 字段是无效的 —— 存的 revision 仍是旧值、与注册表一致，
    /// 该条目会被判为「无变化」而**静默消失**（本用例首次运行即踩到这个坑）。
    /// 必须把 `revision` 本身改成不匹配的固定值。
    ///
    /// 需要至少 4 个用户变量（`user[1]` / `user[2]` 造冲突，`user[3..]` 保持不变）；
    /// 不足时返回 `None`，由调用方显式失败而不是静默跳过。
    fn craft_preview_fixture(dir: &std::path::Path) -> Option<std::path::PathBuf> {
        let source = make_backup(dir);
        let raw = std::fs::read_to_string(&source).expect("读取备份失败");
        let mut value: serde_json::Value = serde_json::from_str(&raw).expect("备份必须是合法 JSON");

        let user = value["hives"]["user"].as_array()?.clone();
        if user.len() < 4 {
            return None;
        }
        // user[0] 刻意不入 patched → 注册表有、备份无 → Removed（恰 1 条）
        let mut patched: Vec<serde_json::Value> = user[3..].to_vec();
        // user[1]、user[2]：revision 换成不可能匹配的值 → Conflict（恰 2 条）
        for index in [1, 2] {
            let mut conflicted = user[index].clone();
            conflicted["revision"] = serde_json::json!("ffffffffffffffff");
            patched.push(conflicted);
        }
        // 3 个注册表中不存在的名字 → Added（恰 3 条）
        for suffix in ["A", "B", "C"] {
            patched.push(serde_json::json!({
                "name": format!("PATHEDITOR_T7_ABSENT_VAR_9F3{suffix}"),
                "kind": "string",
                "value": "absent",
                "revision": "0000000000000000",
            }));
        }

        value["hives"]["user"] = serde_json::Value::Array(patched);
        let fixture = dir.join("env_backup_t7_fixture.json");
        std::fs::write(
            &fixture,
            serde_json::to_string_pretty(&value).expect("序列化夹具失败"),
        )
        .expect("写入夹具失败");
        // 夹具文件名不带时间戳，避免被后续 `env backups` 的排序断言干扰
        let _ = std::fs::remove_file(&source);
        Some(fixture)
    }

    /// 造出四类差异计数两两不等的夹具，逐个断言**计数器字段的映射**。
    ///
    /// 这条是本任务里唯一能证伪「格式串里字段串位」的用例：期望值
    /// 新增 3 / 修改 0 / 删除 1 / 冲突 2 **两两不等**，任两个字段对调都会被断言抓到。
    /// （初版用「删除 1 / 冲突 1」时对调 `removed` 与 `conflicts` 变异**测不出来**，
    /// 已按变异结果重做夹具 —— 见 [`craft_preview_fixture`]。）
    /// 同时断言 `--json` 输出的同名字段，钉住 camelCase 契约。
    #[test]
    fn env_restore_dry_run_reports_each_counter_field() {
        let dir = temp_dir("restore_counters");
        let Some(fixture) = craft_preview_fixture(&dir) else {
            panic!("本机用户变量不足 4 个，无法构造夹具；不要让它静默跳过");
        };

        let out = std::process::Command::new(cli_bin())
            .arg("env")
            .arg("restore")
            .arg(&fixture)
            .arg("--dry-run")
            .env("PATHEDITOR_BACKUP_DIR", &dir)
            .output()
            .expect("启动 env restore 失败");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "dry-run 必须成功: {stdout} / {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            stdout.contains("将新增 3 个、修改 0 个、删除 1 个；冲突 2 个"),
            "计数器映射不符（期望 新增3/修改0/删除1/冲突2）: {stdout}"
        );

        // 同一夹具走 --json：字段名与取值都必须一致
        let out = std::process::Command::new(cli_bin())
            .arg("env")
            .arg("restore")
            .arg(&fixture)
            .arg("--dry-run")
            .arg("--json")
            .env("PATHEDITOR_BACKUP_DIR", &dir)
            .output()
            .expect("启动 env restore --json 失败");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let preview: serde_json::Value =
            serde_json::from_str(&stdout).expect("--json 必须输出合法 JSON");
        assert_eq!(preview["added"].as_u64(), Some(3), "added: {stdout}");
        assert_eq!(preview["modified"].as_u64(), Some(0), "modified: {stdout}");
        assert_eq!(preview["removed"].as_u64(), Some(1), "removed: {stdout}");
        assert_eq!(
            preview["conflicts"].as_u64(),
            Some(2),
            "conflicts: {stdout}"
        );
        assert_eq!(
            preview["changes"].as_array().map(Vec::len),
            Some(6),
            "changes 条数必须与计数之和（3+0+1+2）一致: {stdout}"
        );
        // 逐条差异必须带 hive / name / kind（camelCase 契约）
        let first = &preview["changes"][0];
        assert!(first["hive"].is_string(), "差异项必须有 hive: {stdout}");
        assert!(first["name"].is_string(), "差异项必须有 name: {stdout}");
        assert!(first["kind"].is_string(), "差异项必须有 kind: {stdout}");

        // E3 裁断：dry-run 的预览**不受 --force 影响**（差异计算与 force 无关）。
        // 同一夹具再加 --force，输出必须逐字节相同 —— 若实现把 force 传进预览，
        // 或为 force 走了另一条分支，这里就会失败。
        let out = std::process::Command::new(cli_bin())
            .arg("env")
            .arg("restore")
            .arg(&fixture)
            .arg("--dry-run")
            .arg("--force")
            .arg("--json")
            .env("PATHEDITOR_BACKUP_DIR", &dir)
            .output()
            .expect("启动 env restore --dry-run --force 失败");
        assert!(
            out.status.success(),
            "--dry-run --force 必须可共存: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let with_force: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&out.stdout))
                .expect("--dry-run --force --json 必须输出合法 JSON");
        assert_eq!(
            with_force, preview,
            "dry-run 的预览必须与 force 无关（E3），加 --force 后输出应完全相同"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `env restore`（非 dry-run）在**任何写入之前**把手工兜底指引打印到 stderr，
    /// stdout 保持干净；未提权时以退出码 1 失败（已登记的已知限制）。
    ///
    /// 区分两个分支，两个分支都断言到底：
    /// - 未提权（本机常规情形）：HKLM 无法以写权限打开 → `PermissionDenied` →
    ///   退出码 1，且此时尚未写入任何内容（core 先开 hive、后写入）；
    /// - 已提权：备份即当前状态 → 零变更、零失败。
    #[test]
    fn env_restore_prints_manual_fallback_guidance_to_stderr() {
        let dir = temp_dir("restore_guidance");
        let file = make_backup(&dir);

        let out = std::process::Command::new(cli_bin())
            .arg("env")
            .arg("restore")
            .arg(&file)
            .env("PATHEDITOR_BACKUP_DIR", &dir)
            .output()
            .expect("启动 env restore 失败");

        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);

        // C7：兜底指引必须打印，且走 stderr —— 绝不能污染 stdout 的输出契约。
        // 这两条断言与退出码无关，两个分支都成立。
        assert!(
            stderr.contains("patheditor backup"),
            "必须提示 patheditor backup: {stderr}"
        );
        assert!(
            stderr.contains("patheditor env backup"),
            "必须提示 patheditor env backup: {stderr}"
        );

        match out.status.code() {
            // 未提权：HKLM 写权限错误，退出码 1（spec 允许）。
            // 这条路径**没有成功输出**，所以 stdout 必须干净 —— 指引没有漏进 stdout。
            Some(1) => {
                assert!(
                    stderr.contains("错误:"),
                    "退出码 1 时必须打印错误文本: {stderr}"
                );
                assert!(
                    stdout.is_empty(),
                    "失败路径 stdout 必须保持干净（指引走 stderr）: {stdout}"
                );
            }
            // 已提权：备份即当前状态，零变更零失败。
            // 注意：成功路径**必然**往 stdout 打「恢复完成」那一行，所以
            // 「stdout 必须干净」的断言绝不能提到 match 之前 —— 那会让本分支
            // 构造上不可达（提权环境含 GitHub windows-latest runner）。
            Some(0) => {
                assert!(
                    stdout.contains("恢复完成: 成功 0 个"),
                    "零差异恢复应报 0: {stdout}"
                );
                assert!(!stderr.contains("警告:"), "不应有逐条失败: {stderr}");
            }
            other => panic!(
                "退出码应为 0（已提权）或 1（未提权，HKLM 无写权限），实际: {other:?} / {stderr}"
            ),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 备份文件不存在时 `env restore` 以退出码 1 失败（路径校验在 core 侧）。
    #[test]
    fn env_restore_rejects_missing_file_with_exit_code_1() {
        let dir = temp_dir("restore_missing");
        let out = std::process::Command::new(cli_bin())
            .args(["env", "restore", "env_backup_missing.json"])
            .env("PATHEDITOR_BACKUP_DIR", &dir)
            .output()
            .expect("启动 env restore 失败");

        assert_eq!(
            out.status.code(),
            Some(1),
            "文件不存在必须是退出码 1（不是 2，也不是 3）"
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("错误:"), "必须打印错误文本: {stderr}");
        assert!(
            out.stdout.is_empty(),
            "失败路径 stdout 必须干净: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
