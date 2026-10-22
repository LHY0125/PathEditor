//! 环境变量子命令实现。

use super::render::*;
use super::value::*;
use crate::output::print_json;
use crate::runtime::{exit_core_error, exit_err, warn_if_backup_failed};
use path_editor_core as core;

/// `env list` —— 列出变量元数据（不含明文）。
pub(crate) fn cmd_env_list(system: bool, user: bool, json_out: bool) {
    let snapshot = core::registry::list_all_env_vars().unwrap_or_else(|e| exit_core_error(&e));
    if json_out {
        let value = snapshot_json(&snapshot, system, user);
        print_json(&value);
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

/// `env set` —— 修改已有变量的值。类型跟随注册表现状，不可更改。
///
/// `--revision` 走 CAS（冲突退出码 3）；`--force` 走真正的覆盖写
/// （最后写入者胜，**不会**产生退出码 3）。
///
/// `--dry-run` 只预览、不写注册表，且**不要求** `--revision`/`--force`
/// 。
#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_env_set(
    name: String,
    value: Option<String>,
    stdin: bool,
    value_file: Option<String>,
    revision: Option<String>,
    force: bool,
    dry_run: bool,
    json_out: bool,
    system: bool,
) {
    let hive = select_hive(system, false);
    let src = resolve_value(value, stdin, value_file, true);
    let new_value = read_value(&src);
    if dry_run {
        // 预览是只读的：经 core 的 preview_env_change 取当前值/类型/写权限。
        // 变量不存在时 core 返回 NotFound，照常报错 —— 预览与执行判定一致（验收标准 8）。
        let preview =
            core::registry::preview_env_change(hive, &name).unwrap_or_else(|e| exit_core_error(&e));
        // 安全：旧值已由 core 打码/净化；新值来自命令行，须同样经 mask_new_value 处理。
        let json = build_change_preview(
            "set",
            hive,
            &name,
            preview.old_value,
            mask_new_value(&name, new_value),
            Some(preview.kind),
            preview.writable,
            preview.sensitive,
        );
        emit_preview(&json, json_out);
        return;
    }
    // 预览分支之后才解析并发模式：dry-run 不要求 --revision/--force（验收标准 7）。
    let mode = resolve_concurrency(revision, force);
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
///
/// `--dry-run` 只预览、不写注册表；`kind` 来自 `--kind`（非注册表读取），
/// `writable` 经 core 判定。
#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_env_add(
    name: String,
    value: Option<String>,
    stdin: bool,
    value_file: Option<String>,
    kind: String,
    dry_run: bool,
    json_out: bool,
    system: bool,
) {
    let hive = select_hive(system, false);
    let src = resolve_value(value, stdin, value_file, true);
    let new_value = read_value(&src);
    if dry_run {
        // add 的 kind 来自 --kind（默认 string），非注册表读取；writable 经 core 判定。
        // 同名已存在时**照常展示**（不在预览层拒绝），执行时才由 core 拒 NameExists（验收标准 9）。
        // 安全：新值来自命令行，经 mask_new_value 打码/净化（敏感名不预览明文）。
        let sensitive = core::env_var::is_sensitive(&name);
        let json = build_change_preview(
            "add",
            hive,
            &name,
            None,
            mask_new_value(&name, new_value),
            Some(kind_label(parse_kind(&kind)).to_string()),
            core::registry::env_hive_writable(hive),
            sensitive,
        );
        emit_preview(&json, json_out);
        return;
    }
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
///
/// `--dry-run` 只预览、不写注册表，且**不要求** `--revision`/`--force`。
pub(crate) fn cmd_env_remove(
    name: String,
    revision: Option<String>,
    force: bool,
    dry_run: bool,
    json_out: bool,
    system: bool,
) {
    let hive = select_hive(system, false);
    if dry_run {
        // 预览是只读的；变量不存在时 core 返回 NotFound，照常报错（验收标准 8）。
        let preview =
            core::registry::preview_env_change(hive, &name).unwrap_or_else(|e| exit_core_error(&e));
        let json = build_change_preview(
            "remove",
            hive,
            &name,
            preview.old_value,
            None,
            None,
            preview.writable,
            preview.sensitive,
        );
        emit_preview(&json, json_out);
        return;
    }
    // 预览分支之后才解析并发模式：dry-run 不要求 --revision/--force。
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
/// 备份文件含明文，落盘位置由 `PATHEDITOR_BACKUP_DIR` 或
/// `~/.patheditor/backups/` 决定。
///
/// # Returns
/// 无返回值：失败经 `exit_core_error` 终止进程（退出码由 `CoreError::exit_code()` 决定）。
pub(crate) fn cmd_env_backup(json_out: bool) {
    match core::backup::backup_env_vars() {
        Ok(path) => {
            // production 路径优先写 .pebak；回落明文时 core 只 log::warn!，
            // 而 CLI 没有日志后端——必须在这里明说。
            if path
                .extension()
                .map(|e| !e.eq_ignore_ascii_case("pebak"))
                .unwrap_or(true)
            {
                eprintln!(
                    "警告: 本次写的是**明文** .json 备份（无法获取 DPAPI 密钥）。\
                     该文件含环境变量明文，建议修复密钥问题后重新备份。"
                );
            }
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
        print_json(&list);
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
        // 统一读入口，同时支持 `.pebak`（解密）与遗留 `.json`
        let (payload, format) = core::backup::read_env_backup_with_format(&path)
            .unwrap_or_else(|e| exit_core_error(&e));
        // CLI 没有日志后端，core 的 `log::warn!` 到这里会被丢弃。
        // 明文警告必须走 stderr 才真的可见（README 承诺的就是这一条）。
        if format == core::backup::BackupFormat::LegacyPlaintext {
            eprintln!(
                "警告: {} 是遗留明文备份（v6 起的备份为 DPAPI 加密的 .pebak）。\
                 恢复后建议重新执行 `patheditor env backup` 生成加密副本。",
                path.display()
            );
        }
        let preview =
            core::backup::preview_restore(&payload).unwrap_or_else(|e| exit_core_error(&e));
        if json_out {
            print_json(&preview);
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

    // 手工兜底出口：恢复自己不产生备份（否则每次恢复都新增文件，
    // 与保留策略互相吞噬），所以「恢复错了」没有自动回退。先把两条命令告诉用户。
    // **只打印、绝不自动执行**——自动备份会与保留策略互相吞噬。
    // 走 stderr：不污染 stdout 的输出契约。
    eprintln!("提示: 恢复不会自动备份当前状态。如需留还原点，请先执行:");
    eprintln!("  patheditor backup       # 备份当前 PATH");
    eprintln!("  patheditor env backup   # 备份当前全部环境变量");

    // 明文警告必须在**写入之前**说——恢复完再提示就晚了。
    // 这里只看扩展名（不解析内容），代价为零。
    if path
        .extension()
        .map(|e| !e.eq_ignore_ascii_case("pebak"))
        .unwrap_or(true)
    {
        eprintln!(
            "警告: {} 是遗留明文备份（v6 起的备份为 DPAPI 加密的 .pebak）。\
             恢复后建议重新执行 `patheditor env backup` 生成加密副本。",
            path.display()
        );
    }

    let outcome =
        core::backup::restore_env_backup_from(&path, force).unwrap_or_else(|e| exit_core_error(&e));

    if json_out {
        print_json(&outcome);
    } else {
        println!("恢复完成: 成功 {} 个", outcome.applied);
    }
    // 逐条失败是 best-effort 语义的一部分（不改变退出码），必须可见。
    for failure in &outcome.failures {
        eprintln!("警告: {failure}");
    }
}
