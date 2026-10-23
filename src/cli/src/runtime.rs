use path_editor_core as core;
use path_editor_core::service::{self, HiveOutcome, PathHiveWrite, SidecarOutcome};

pub(crate) fn exit_err(msg: &str) -> ! {
    eprintln!("错误: {msg}");
    std::process::exit(1);
}

/// 按结构化错误决定退出码与输出。
///
/// 退出码由 `CoreError::exit_code()` 统一映射（冲突 3，其余 1），
/// 不再匹配 `[E_CONFLICT]` 文本前缀 —— 判定只看 `code`。
/// PATH 命令的注册表错误恒为 1；sidecar/pending 等持久化错误沿用同一映射。
pub(crate) fn exit_core_error(err: &core::CoreError) -> ! {
    eprintln!("错误: {}", err.message);
    std::process::exit(err.exit_code());
}

/// 备份失败时在 stderr 提示，**不改变退出码**。
///
/// CLI 未初始化 logger，core 的 `log::warn!` 在此被丢弃 —— 必须经返回值显式打印。
pub(crate) fn warn_if_backup_failed(outcome: &core::backup::BackupOutcome) {
    if let core::backup::BackupOutcome::Failed(reason) = outcome {
        eprintln!("警告: 环境变量写前备份失败（写入已完成）: {reason}");
    }
}

pub(crate) fn ensure_single_target(system: bool, user: bool) -> &'static str {
    if system && user {
        exit_err("不能同时指定 --system 和 --user");
    }
    if system {
        "system"
    } else {
        "user"
    }
}

/// 尝试打开已存在的默认数据库；不存在或打不开时返回 `None`。
///
/// CLI 的 PATH 命令不应因为 DB 不可用而失败，也不应在首次运行时创建空库
/// 或密钥文件——DB 是 GUI 建立的 sidecar，CLI 只使用它。
fn open_db_optional() -> Option<core::db::Db> {
    let path = core::db::crypto::config_dir().join("patheditor.db");
    if !path.exists() {
        return None;
    }
    match core::db::open_default() {
        Ok(mut db) => match db.migrate_to_current() {
            Ok(()) => Some(db),
            Err(e) => {
                eprintln!("警告: 数据库迁移失败（{}），降级为文件快照", e.message);
                None
            }
        },
        Err(e) => {
            eprintln!("警告: 数据库不可用（{}），降级为文件快照", e.message);
            None
        }
    }
}

/// 读取完整 PATH 快照：DB 可用时读 `current_snapshot`，否则只读注册表。
pub(crate) fn load_path_snapshot_any() -> Result<core::PathSnapshot, core::CoreError> {
    let db = open_db_optional();
    core::service::load_path_snapshot_any(db.as_ref())
}

/// 把「完整有序条目」交给 core 的唯一保存编排，并按结果决定退出行为。
///
/// DB 存在时走 `save_path_with_db`（写注册表 + 当前快照 + 事件）；
/// DB 不存在或打不开时降级为 `save_path_registry_only`（只写注册表）。
///
/// CLI 是单次进程，不传 `expected`：每次命令重新读取快照，
/// `expected == current` 恒成立，校验在 CLI 侧没有真实并发窗口。
///
/// 退出行为：
/// - 任一 hive `Failed` → `exit_err`（退出码 **1**；**不**用 3，3 专属 `env --revision`）；
/// - sidecar 为 `Pending` / `Failed` → 打印「注册表已写入」提示并 `exit_err`；
/// - 全部成功 → 静默返回（广播已由 core 内部完成）。
pub(crate) fn save_path_write(
    system: Option<Vec<core::PathEntry>>,
    user: Option<Vec<core::PathEntry>>,
) {
    let to_write = |entries: Vec<core::PathEntry>| PathHiveWrite {
        entries,
        expected: None,
    };
    let system = system.map(to_write);
    let user = user.map(to_write);
    let outcome = match open_db_optional() {
        Some(mut db) => {
            service::save_path_with_db(&mut db, core::history::EventSource::Cli, system, user)
        }
        None => service::save_path_registry_only(
            system,
            user,
            core::history::EventSource::Cli,
            &core::history::NoopHistory,
        ),
    }
    .unwrap_or_else(|e| exit_core_error(&e));

    // 逐 hive 报告失败（best-effort 多 hive：一个失败不掩盖另一个的成功）。
    for (label, hive_outcome) in [("系统", &outcome.system), ("用户", &outcome.user)] {
        if let HiveOutcome::Failed(e) = hive_outcome {
            exit_err(&format!("{} PATH 写入失败: {}", label, e.message));
        }
    }

    match outcome.sidecar {
        SidecarOutcome::Saved => {}
        SidecarOutcome::Pending(e) => exit_err(&sidecar_failure_message(&e.message, true)),
        SidecarOutcome::Failed(e) => exit_err(&format!(
            "{}\n（待补写状态记录失败，请手工核对）",
            sidecar_failure_message(&e.message, false)
        )),
    }
}

/// 加载当前快照 → 对目标 hive 施加 `f` → 经 [`save_path_write`] 保存。
pub(crate) fn load_and_save(
    system: bool,
    f: impl FnOnce(Vec<core::PathEntry>) -> Vec<core::PathEntry>,
) {
    let target = ensure_single_target(system, false);
    let snapshot = load_path_snapshot_any().unwrap_or_else(|e| exit_core_error(&e));

    if target == "system" {
        let entries = f(snapshot.system);
        save_path_write(Some(entries), None);
    } else {
        let entries = f(snapshot.user);
        save_path_write(None, Some(entries));
    }
}

/// 加载、检查索引、操作、保存的通用模式。索引基于完整快照（含禁用项）。
pub(crate) fn load_operate_save(
    system: bool,
    index: usize,
    operate: impl FnOnce(Vec<core::PathEntry>, usize) -> (Vec<core::PathEntry>, String),
) {
    let target = ensure_single_target(system, false);
    let snapshot = load_path_snapshot_any().unwrap_or_else(|e| exit_core_error(&e));
    let entries = if target == "system" {
        snapshot.system
    } else {
        snapshot.user
    };
    if index >= entries.len() {
        exit_err(&format!("索引 {index} 超出范围 (共 {} 条)", entries.len()));
    }

    let (new_entries, message) = operate(entries, index);
    if target == "system" {
        save_path_write(Some(new_entries), None);
    } else {
        save_path_write(None, Some(new_entries));
    }

    println!("{message}");
}

#[allow(dead_code)] // 兼容后续 CLI 调试/导出使用；生产路径统一保存完整快照。
pub(crate) fn disabled_paths(entries: &[core::PathEntry]) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| !entry.enabled)
        .map(|entry| entry.path.clone())
        .collect()
}

/// 构造 sidecar 快照保存失败时的错误文案（纯函数，便于测试）。
///
/// 两个分支都必须包含「注册表已写入」，让用户/脚本明确知道注册表侧已经生效、
/// 只有侧车快照未落盘，避免误判为整个操作失败。
pub(crate) fn sidecar_failure_message(err: &str, pending_ok: bool) -> String {
    if pending_ok {
        format!(
            "注册表已写入，但快照保存失败: {err}\n已记录待补写状态，下次运行 PATH 命令会自动补写"
        )
    } else {
        format!(
            "注册表已写入，但快照保存失败: {err}\n且待补写状态记录失败，注册表与快照可能不一致，请手工核对"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_prefix_matches_core_constant() {
        // 契约：`[E_CONFLICT]` 前缀仍是前端展示文本与
        // core 消息的稳定标识，测试改为直接断言 core 常量本身；CLI 退出码
        // 已由 `CoreError.exit_code()` 驱动，不再依赖文本前缀。
        use path_editor_core as core;
        let msg = core::registry::conflict_message();
        assert!(
            msg.starts_with("[E_CONFLICT]"),
            "core 冲突消息必须以 [E_CONFLICT] 开头，实际: {msg}"
        );
    }

    #[test]
    fn core_conflict_maps_to_exit_code_3() {
        // 契约：冲突退出码 3 由 CoreError.exit_code() 决定
        let e = core::CoreError::new(
            core::ErrorCode::Conflict,
            "update_env_var",
            core::registry::conflict_message(),
        );
        assert_eq!(e.exit_code(), 3);
    }

    #[test]
    fn core_other_errors_map_to_exit_code_1() {
        let e = core::CoreError::new(core::ErrorCode::Protected, "op", "保护");
        assert_eq!(e.exit_code(), 1);
    }

    #[test]
    fn sidecar_failure_message_pending_ok_mentions_registry_written_and_pending() {
        let msg = sidecar_failure_message("磁盘已满", true);
        assert!(
            msg.contains("注册表已写入"),
            "必须包含「注册表已写入」: {msg}"
        );
        assert!(msg.contains("待补写"), "必须包含「待补写」: {msg}");
        assert!(msg.contains("磁盘已满"), "必须透传原始错误: {msg}");
    }

    #[test]
    fn sidecar_failure_message_pending_failed_mentions_registry_written_and_manual_check() {
        let msg = sidecar_failure_message("磁盘已满", false);
        assert!(
            msg.contains("注册表已写入"),
            "必须包含「注册表已写入」: {msg}"
        );
        assert!(msg.contains("手工核对"), "必须提示手工核对: {msg}");
        assert!(msg.contains("磁盘已满"), "必须透传原始错误: {msg}");
    }

    // ── `warn_if_backup_failed`：CLI 侧唯一的「写前备份失败」可见渠道──
    //
    // 为什么必须用**子进程**而不是进程内调用：被测函数直接 `eprintln!` 到真实
    // stderr，进程内拿不到那串字节（libtest 默认还会把 `eprintln!` 收进内存捕获
    // 缓冲，连 fd 都到不了）。而断言「警告确实出现在 stderr」正是本用例的全部价值
    // ——删掉 `eprintln!` 的函数体会让所有进程内写法一起变绿，等于没测。
    //
    // 做法：**重新执行测试二进制自身**（`current_exe()`），用环境变量让子进程只做
    // 一次探针调用后正常返回；父进程读被重定向的 stderr 断言。子进程**不接触注册表**
    // ——探针函数只接收一个内存里的 `BackupOutcome`（项目硬约束：不写真实注册表），
    // 因此这条路既拿到了真实 stderr，又比启动 CLI 二进制更安全。
    //
    // 已知未覆盖项（如实标注）：本用例证明「`BackupOutcome::Failed` → 输出该警告」
    // 这个映射，以及删掉实现会让它失败；**不证明** 5 个 CLI 写路径都调用了本函数
    // （「调用点已接线」由代码审阅确认，见 `env_ops.rs` 的 5 处调用）。

    /// 子进程探针模式的环境变量名。
    const WARN_PROBE_ENV: &str = "PATHEDITOR_WARN_PROBE";

    /// 本用例的完整测试名，供子进程用 `--exact` 精确复现。
    ///
    /// 写死字符串是刻意的：测试若被改名，父进程的 `--exact` 会匹配不到任何用例、
    /// 子进程 stderr 不含警告，断言立刻失败（失败是响亮的，不会静默空转）。
    const WARN_PROBE_TEST: &str = "runtime::tests::warn_if_backup_failed_writes_warning_to_stderr";

    /// 子进程分支：按 `WARN_PROBE_ENV` 指定的结果调用一次被测函数。
    ///
    /// 返回 `true` 表示当前进程是子进程（已执行探针，调用方应立即返回）。
    /// 未知取值一律 panic —— 避免「环境变量被外部设成意外值」把用例静默变成空跑。
    fn run_warn_probe_if_child() -> bool {
        let Ok(mode) = std::env::var(WARN_PROBE_ENV) else {
            return false;
        };
        let outcome = match mode.as_str() {
            "failed" => core::backup::BackupOutcome::Failed("磁盘已满".into()),
            "created" => core::backup::BackupOutcome::Created(std::path::PathBuf::from("x.json")),
            "skipped" => core::backup::BackupOutcome::Skipped,
            other => panic!("未知探针模式: {other}"),
        };
        warn_if_backup_failed(&outcome);
        true
    }

    /// **`warn_if_backup_failed` 的可证伪性测试：真实 stderr 字节级断言。**
    ///
    /// 覆盖的三条语义：
    /// 1. `Failed` → stderr 出现「警告: 环境变量写前备份失败」，且透传原因；
    /// 2. 该警告只在 `Failed` 出现（`Created` / `Skipped` 一条都不许有）；
    /// 3. 警告走 stderr 而非 stdout（stdout 不得出现该文案）。
    ///
    /// 变异可证伪性（任一改动都会让某条断言失败）：
    /// - 删掉 `warn_if_backup_failed` 里整个 `if let` 分支 → 情形 1 失败；
    /// - 去掉 `if let ... == Failed` 的判断、无条件打印 → 情形 2 失败；
    /// - 把 `eprintln!` 换成 `println!` → 情形 1（stderr 无）与情形 3（stdout 有）同时失败。
    #[test]
    fn warn_if_backup_failed_writes_warning_to_stderr() {
        if run_warn_probe_if_child() {
            return;
        }

        let exe = std::env::current_exe().expect("取当前测试可执行文件路径失败");
        // 复现同一个测试二进制、只跑本用例，并把 `--nocapture` 打开：
        // 否则 libtest 会把子进程里的 `eprintln!` 收进内存缓冲，fd 上什么都看不到。
        let run = |mode: &str| {
            let out = std::process::Command::new(&exe)
                .args(["--exact", WARN_PROBE_TEST, "--nocapture"])
                .env(WARN_PROBE_ENV, mode)
                .output()
                .expect("启动探针子进程失败");
            assert!(
                out.status.success(),
                "探针子进程必须成功退出（模式 {mode}）: {:?} / {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr)
            );
            (
                String::from_utf8_lossy(&out.stdout).into_owned(),
                String::from_utf8_lossy(&out.stderr).into_owned(),
            )
        };

        // 情形 1：Failed → 必须输出警告、透传原因、点明「写入已完成」
        let (stdout, stderr) = run("failed");
        assert!(
            stderr.contains("警告: 环境变量写前备份失败"),
            "Failed 必须输出写前备份失败警告（这是 CLI 用户唯一能得知该失败的地方）: {stderr}"
        );
        assert!(
            stderr.contains("写入已完成"),
            "警告必须点明写入已完成（备份失败不阻断写入）: {stderr}"
        );
        assert!(
            stderr.contains("磁盘已满"),
            "警告必须透传备份失败原因: {stderr}"
        );

        // 情形 3：警告走 stderr，绝不污染 stdout
        assert!(
            !stdout.contains("警告: 环境变量写前备份失败"),
            "警告必须走 stderr 而非 stdout: {stdout}"
        );

        // 情形 2：Created / Skipped 一条警告都不许有
        for mode in ["created", "skipped"] {
            let (stdout, stderr) = run(mode);
            assert!(
                !stderr.contains("警告"),
                "{mode} 不得输出备份失败警告: {stderr}"
            );
            assert!(
                !stdout.contains("警告"),
                "{mode} stdout 亦不得有警告: {stdout}"
            );
        }
    }
}
