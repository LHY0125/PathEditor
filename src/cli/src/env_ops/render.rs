//! 环境变量列表与 dry-run 预览的渲染。

use crate::output::print_json;
use path_editor_core as core;
use path_editor_core::env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot};

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

/// 构造 dry-run 的变更摘要（统一 JSON 契约）。
///
/// 字段固定为 `operation / hive / name / oldValue / newValue / kind / writable / sensitive`；
/// 无值一侧一律 `null`（`add` 无 oldValue，`remove` 无 newValue）。**判定不在本函数**——
/// `writable` / `kind` / 敏感标记由调用方从 core 结果传入，本函数只做形状组装。
///
/// **安全（安全审查修正）**：`old` / `new` 必须已在 core 侧经敏感打码与控制字符净化 ——
/// 本函数不做判定也不做净化，只透传（CLI 零判定）。调用方对 `add` 的新值需经
/// [`mask_new_value`] 处理后再传入。`sensitive` 用于让消费者区分「敏感打码」与「空值」。
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_change_preview(
    op: &str,
    hive: EnvHive,
    name: &str,
    old: Option<String>,
    new: Option<String>,
    kind: Option<String>,
    writable: bool,
    sensitive: bool,
) -> serde_json::Value {
    serde_json::json!({
        "operation": op,
        "hive": match hive { EnvHive::System => "system", EnvHive::User => "user" },
        "name": name,
        "oldValue": old,
        "newValue": new,
        "kind": kind,
        "writable": writable,
        "sensitive": sensitive,
    })
}

/// 对 dry-run 预览里的**新值**做敏感打码（`add` 的 `newValue` 用）。
///
/// `add` 的新值来自命令行，不在 `preview_env_change` 的覆盖范围（该函数只处理旧值）。
/// 命中 `is_sensitive` 时返回 `None`（不预览明文），否则经 `sanitize_preview` 净化控制字符。
/// **判定复用 core 的 `is_sensitive` / `sanitize_preview`，不在此重写规则**。
pub(crate) fn mask_new_value(name: &str, value: String) -> Option<String> {
    if core::env_var::is_sensitive(name) {
        None
    } else {
        core::env_var::sanitize_preview(&value)
    }
}

/// 把变更摘要渲染成人类可读的一行（文本契约）。
///
/// 三种操作各自成句；一律以「（未写入任何内容）」收尾，避免用户误以为已生效。
/// 敏感变量（`sensitive = true`）的值以 `(敏感)` 占位，与 `env list` 的既有打码一致。
pub(crate) fn render_change_preview_text(v: &serde_json::Value) -> String {
    let op = v["operation"].as_str().unwrap_or("");
    let hive = if v["hive"] == "system" {
        "系统"
    } else {
        "用户"
    };
    let name = v["name"].as_str().unwrap_or("");
    let sensitive = v["sensitive"].as_bool().unwrap_or(false);
    // 敏感变量的值不显示；非敏感且无值（空/Unsupported）显示 `-`。
    let placeholder = if sensitive { "(敏感)" } else { "-" };
    let old = v["oldValue"].as_str().unwrap_or(placeholder);
    let new = v["newValue"].as_str().unwrap_or(placeholder);
    let kind = v["kind"].as_str().unwrap_or("");
    let body = match op {
        "set" => format!("将更新{hive}变量 {name}：{old} → {new}"),
        "add" => format!("将新建{hive}变量 {name}：{new}（类型 {kind}）"),
        "remove" => format!("将删除{hive}变量 {name}（当前值：{old}）"),
        _ => format!("将变更{hive}变量 {name}"),
    };
    format!("{body}（未写入任何内容）")
}

/// dry-run 的统一出口：`--json` 打 JSON，否则打文本。**不写注册表、不备份、不广播。**
pub(crate) fn emit_preview(preview: &serde_json::Value, json_out: bool) {
    if json_out {
        print_json(preview);
    } else {
        println!("{}", render_change_preview_text(preview));
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

/// 当前 hive 未命中时，探测另一 hive 以生成更准确的错误提示。
///
/// 返回 `None` 表示另一 hive 也没有该变量（沉默，避免误导）。
pub(crate) fn other_hive_hint(name: &str, current: EnvHive) -> Option<String> {
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
