//! CLI 入口：解析参数并分派到各命令实现。

use clap::Parser;

mod cli;
mod env_ops;
mod import_export;
mod maintenance_ops;
mod output;
mod path_ops;
mod profile_ops;
mod runtime;
mod scan_ops;
mod sync_ops;

use cli::{Cli, Command, EnvCmd, MaintenanceCmd, ProfileCmd, SyncCmd};
use import_export::{cmd_export, cmd_import};
use path_ops::{
    cmd_add, cmd_backup, cmd_clean, cmd_edit, cmd_list, cmd_move, cmd_remove, cmd_toggle,
};
use profile_ops::{
    profile_apply, profile_delete, profile_list, profile_load, profile_rename, profile_save,
};
use scan_ops::{cmd_check_admin, cmd_conflicts, cmd_scan};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::List { system, user, json } => cmd_list(system, user, json),
        Command::Add { path, system, user } => cmd_add(path, system, user),
        Command::Remove { index, system } => cmd_remove(index, system),
        Command::Edit {
            index,
            new_path,
            system,
        } => cmd_edit(index, new_path, system),
        Command::MoveUp {
            index,
            steps,
            system,
        } => cmd_move(index, steps, system, true),
        Command::MoveDown {
            index,
            steps,
            system,
        } => cmd_move(index, steps, system, false),
        Command::Clean {
            system,
            user,
            dry_run,
            json,
        } => cmd_clean(system, user, dry_run, json),
        Command::Enable {
            index,
            system,
            user,
        } => cmd_toggle(index, system, user, true),
        Command::Disable {
            index,
            system,
            user,
        } => cmd_toggle(index, system, user, false),
        Command::Import { file, target } => cmd_import(file, target),
        Command::Export { format, output } => cmd_export(format, output),
        Command::Backup => cmd_backup(),
        Command::Conflicts { json } => cmd_conflicts(json),
        Command::Scan { query, json } => cmd_scan(query, json),
        Command::CheckAdmin { json } => cmd_check_admin(json),
        Command::Env(cmd) => match cmd {
            EnvCmd::List { system, user, json } => env_ops::cmd_env_list(system, user, json),
            EnvCmd::Get { name, system } => env_ops::cmd_env_get(name, system),
            EnvCmd::Set {
                name,
                value,
                stdin,
                value_file,
                revision,
                force,
                dry_run,
                json,
                system,
            } => env_ops::cmd_env_set(
                name, value, stdin, value_file, revision, force, dry_run, json, system,
            ),
            EnvCmd::Add {
                name,
                value,
                stdin,
                value_file,
                kind,
                dry_run,
                json,
                system,
            } => env_ops::cmd_env_add(name, value, stdin, value_file, kind, dry_run, json, system),
            EnvCmd::Remove {
                name,
                revision,
                force,
                dry_run,
                json,
                system,
            } => env_ops::cmd_env_remove(name, revision, force, dry_run, json, system),
            EnvCmd::Backup { json } => env_ops::cmd_env_backup(json),
            EnvCmd::Backups { json } => env_ops::cmd_env_backups(json),
            EnvCmd::Restore {
                file,
                dry_run,
                force,
                json,
            } => env_ops::cmd_env_restore(file, dry_run, force, json),
        },
        Command::Profile(cmd) => match cmd {
            ProfileCmd::List { json } => profile_list(json),
            ProfileCmd::Save { name } => profile_save(name),
            ProfileCmd::Load { name } => profile_load(name),
            ProfileCmd::Apply { name } => profile_apply(name),
            ProfileCmd::Delete { name } => profile_delete(name),
            ProfileCmd::Rename { old, new } => profile_rename(old, new),
        },
        Command::Sync(cmd) => match cmd {
            SyncCmd::Export { file } => sync_ops::sync_export(&file),
            SyncCmd::Import { file } => sync_ops::sync_import(&file),
            SyncCmd::Status => sync_ops::sync_status(),
            SyncCmd::Conflicts { all } => sync_ops::sync_conflicts(all),
            SyncCmd::Resolve { id, take } => sync_ops::sync_resolve(id, &take),
        },
        Command::Maintenance(cmd) => match cmd {
            MaintenanceCmd::Prune { dry_run } => maintenance_ops::maintenance_prune(dry_run),
            MaintenanceCmd::CleanupLegacyBackups { dry_run, yes } => {
                maintenance_ops::maintenance_cleanup_legacy(dry_run, yes)
            }
        },
    }
}

#[cfg(test)]
mod help_tests {
    //! CLI help 文案的回归测试。
    //!
    //! 放在 `main.rs`（crate root）而非 `env_ops.rs`：bin-only crate 的 root 模块
    //! 对子模块是私有的，`struct Cli` 未 `pub`，子模块里写 `crate::Cli` 不可见。
    //! root 内可直接取用。
    use super::*;
    use clap::CommandFactory;

    /// `env add --help` 必须写明「纯新建语义」与「省略值会报错」。
    ///
    /// 可证伪性：删掉 `Add` 变体的文档注释或字段注释，对应片段消失，断言失败。
    /// 这是把「豁免理由」从源码注释提升为**用户可见契约**的回归测试。
    /// 刻意断言**具体短语**而非「解析成功」——后者无法证明文案存在。
    #[test]
    fn env_add_help_documents_pure_create_and_no_prompt() {
        let mut cmd = Cli::command();
        let env = cmd.find_subcommand_mut("env").expect("env 子命令必须存在");
        let add = env.find_subcommand_mut("add").expect("add 子命令必须存在");
        let help = add.render_long_help().to_string();

        assert!(
            help.contains("纯新建"),
            "add --help 必须写明纯新建语义（同名会被拒绝、不会覆盖）: {help}"
        );
        assert!(
            help.contains("不进入交互提示"),
            "add --help 必须写明省略值会报错、不交互: {help}"
        );
    }

    /// `env list --help` 不得声称「不含明文」，须限定为「敏感变量不含明文」。
    ///
    /// 可证伪性：把描述行改回旧文案，`敏感变量不含明文` 消失，断言失败。
    #[test]
    fn env_list_help_scopes_the_no_plaintext_claim() {
        let mut cmd = Cli::command();
        let env = cmd.find_subcommand_mut("env").expect("env 子命令必须存在");
        let list = env
            .find_subcommand_mut("list")
            .expect("list 子命令必须存在");
        let help = list.render_long_help().to_string();

        assert!(
            help.contains("敏感变量不含明文"),
            "list --help 必须把「不含明文」限定到敏感变量: {help}"
        );
    }
}
