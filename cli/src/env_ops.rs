//! 环境变量子命令实现：参数转换、core 调用与输出格式化。
//!
//! 本模块**不实现任何安全判定**（保留 / 保护 / 敏感 / 权限 / revision 校验），
//! 全部由 `path_editor_core` 负责，此处仅透传错误文本。

use crate::runtime::{exit_conflict, exit_err, is_conflict};
use path_editor_core as core;
use path_editor_core::env_var::{EnvHive, EnvValueKind, EnvVarMeta, EnvVarSnapshot};
use serde_json::json;

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
    /// `--force`：跳过校验直接覆盖（脚本 setx 风格）
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
pub(crate) fn apply_concurrency(result: Result<(), String>) {
    match result {
        Ok(()) => {}
        Err(msg) if is_conflict(&msg) => exit_conflict(&msg),
        Err(msg) => exit_err(&msg),
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
        let core_msg = core::registry::conflict_message();
        assert!(is_conflict(&core_msg), "core 冲突消息必须被判为冲突");
        assert!(!is_conflict("变量不存在"));
    }

    #[test]
    fn force_mode_skips_revision_check() {
        // Force 模式不携带 revision；写入时不传 expected_revision 的路径由 cmd 层负责。
        // 此处断言模式本身：Force 不产生 revision 字符串。
        let mode = resolve_concurrency(None, true);
        assert!(matches!(mode, Concurrency::Force));
    }
}
