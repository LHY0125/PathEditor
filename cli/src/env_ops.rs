//! 环境变量子命令实现：参数转换、core 调用与输出格式化。
//!
//! 本模块**不实现任何安全判定**（保留 / 保护 / 敏感 / 权限 / revision 校验），
//! 全部由 `path_editor_core` 负责，此处仅透传错误文本。

use crate::runtime::{exit_conflict, exit_err};
use path_editor_core as core;
use path_editor_core::env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot};
use path_editor_core::error::CoreError;

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

/// 统一处理写操作结果：冲突走退出码 3，其他错误走退出码 1。
///
/// F-06（Wave 2 Task 2）：冲突判定改为结构化 `code == ErrorCode::Conflict`，
/// 不再匹配 `[E_CONFLICT]` 文本前缀（Task 3 之外提前落地的部分）。
pub(crate) fn apply_concurrency(result: Result<(), CoreError>) {
    match result {
        Ok(()) => {}
        Err(e) if e.code == core::error::ErrorCode::Conflict => {
            exit_conflict(&e.message);
        }
        Err(e) => exit_err(&e.message),
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
    let snapshot = core::registry::list_all_env_vars().unwrap_or_else(|e| exit_err(&e.message));
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
            let msg = e.message;
            // 仅只读路径提供「变量存在于另一 hive」的提示，帮助用户加 --system。
            // 写操作不做此兜底 —— 见设计文档「hive 选择」。
            if let Some(other) = other_hive_hint(&name, hive) {
                exit_err(&format!("{msg}\n{other}"));
            }
            exit_err(&msg)
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
            apply_concurrency(core::registry::update_env_var(hive, &name, &new_value, &r));
        }
        Concurrency::Force => {
            core::registry::update_env_var_force(hive, &name, &new_value)
                .unwrap_or_else(|e| exit_err(&e.message));
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
    core::registry::create_env_var(hive, &name, &new_value, kind)
        .unwrap_or_else(|e| exit_err(&e.message));
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
            apply_concurrency(core::registry::delete_env_var(hive, &name, &r));
        }
        Concurrency::Force => {
            core::registry::delete_env_var_force(hive, &name)
                .unwrap_or_else(|e| exit_err(&e.message));
        }
    }
    println!("已删除{}变量: {name}", hive_label(hive));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::is_conflict;

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
        let core_msg = core::registry::conflict_message();
        assert!(is_conflict(&core_msg), "core 冲突消息必须被判为冲突");
        assert!(!is_conflict("变量不存在"));
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
}
