//! 值输入通道与并发选项的解析。

use crate::runtime::exit_err;
use path_editor_core::env_var::EnvValueKind;

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

/// 解析 `--kind` 取值。非法值由 clap 的 `value_parser` 提前拒绝。
pub(crate) fn parse_kind(raw: &str) -> EnvValueKind {
    if raw.eq_ignore_ascii_case("expand") {
        EnvValueKind::ExpandString
    } else {
        EnvValueKind::String
    }
}
