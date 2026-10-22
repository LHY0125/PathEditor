//! CLI 的 clap 命令定义。
//!
//! 参数语义与冲突规则只写在这里；命令实现见 `path_ops` / `env_ops` 等模块。

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "patheditor", version = env!("CARGO_PKG_VERSION"))]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// 列出 PATH 路径
    List {
        #[arg(short, long)]
        system: bool,
        #[arg(short, long)]
        user: bool,
        #[arg(long)]
        json: bool,
    },
    /// 添加一条路径
    Add {
        path: String,
        #[arg(short, long)]
        system: bool,
        #[arg(short, long)]
        user: bool,
    },
    /// 删除指定位置的路径
    Remove {
        index: usize,
        #[arg(short, long)]
        system: bool,
    },
    /// 编辑指定位置的路径
    Edit {
        index: usize,
        new_path: String,
        #[arg(short, long)]
        system: bool,
    },
    /// 上移路径（--steps 指定移动格数，默认 1）
    MoveUp {
        index: usize,
        #[arg(long, default_value = "1")]
        steps: usize,
        #[arg(short, long)]
        system: bool,
    },
    /// 下移路径（--steps 指定移动格数，默认 1）
    MoveDown {
        index: usize,
        #[arg(long, default_value = "1")]
        steps: usize,
        #[arg(short, long)]
        system: bool,
    },
    /// 清理无效和重复路径
    Clean {
        #[arg(short, long)]
        system: bool,
        #[arg(short, long)]
        user: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// 启用指定位置的路径
    Enable {
        index: usize,
        #[arg(short, long)]
        system: bool,
        #[arg(short, long)]
        user: bool,
    },
    /// 禁用指定位置的路径
    Disable {
        index: usize,
        #[arg(short, long)]
        system: bool,
        #[arg(short, long)]
        user: bool,
    },
    /// 从文件导入 PATH（JSON/CSV/TXT）
    Import {
        file: String,
        #[arg(long, default_value = "both")]
        target: String,
    },
    /// 导出 PATH 为文件
    Export {
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// 创建注册表备份
    Backup,
    /// 检测可执行文件冲突
    Conflicts {
        #[arg(long)]
        json: bool,
    },
    /// 列出 PATH 目录中的可执行文件
    Scan {
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// 检查管理员权限
    CheckAdmin {
        #[arg(long)]
        json: bool,
    },
    /// 管理通用环境变量（Path 除外，请使用 PATH 专用命令）
    #[command(subcommand)]
    Env(EnvCmd),
    /// 管理配置文件
    #[command(subcommand)]
    Profile(ProfileCmd),
    /// 同步（文件式变更包，跨设备搬运配置）
    #[command(subcommand)]
    Sync(SyncCmd),
    /// 维护任务（保留策略清理、遗留明文备份清理）
    #[command(subcommand)]
    Maintenance(MaintenanceCmd),
}

/// 维护子命令。
#[derive(Subcommand)]
pub(crate) enum MaintenanceCmd {
    /// 清理过期的变更与冲突（已导出 90 天 / 已解决 30 天）
    Prune {
        /// 只统计，不删除
        #[arg(long)]
        dry_run: bool,
    },
    /// 把遗留 env_backup_*.json 转为 .pebak 后删除（必须先 --yes）
    CleanupLegacyBackups {
        /// 只显示计划，不改动任何文件
        #[arg(long)]
        dry_run: bool,
        /// 确认执行（不加就只打印提示并以 1 退出）
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum SyncCmd {
    /// 把待导出的变更写成加密同步包
    Export {
        /// 输出文件路径（.pesync）
        file: String,
    },
    /// 解密并应用一个同步包（只改 DB，不改注册表）
    Import {
        /// 同步包路径
        file: String,
    },
    /// 显示待导出变更数与未解决冲突数
    Status,
    /// 列出冲突
    Conflicts {
        /// 含已解决的冲突
        #[arg(long)]
        all: bool,
    },
    /// 解决一条冲突
    Resolve {
        /// 冲突 id（来自 `sync conflicts`）
        id: i64,
        /// 取哪一边：local 或 remote
        #[arg(long)]
        take: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum ProfileCmd {
    /// 列出所有配置
    List {
        #[arg(long)]
        json: bool,
    },
    /// 保存当前 PATH 为配置
    Save { name: String },
    /// 加载配置（预览）
    Load { name: String },
    /// 应用配置（写入注册表）
    Apply { name: String },
    /// 删除配置
    Delete { name: String },
    /// 重命名配置
    Rename {
        #[arg(long)]
        old: String,
        #[arg(long)]
        new: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum EnvCmd {
    /// 列出环境变量元数据（敏感变量不含明文，非敏感变量给出可能截断的 preview）
    List {
        #[arg(short, long)]
        system: bool,
        #[arg(short, long, conflicts_with = "system")]
        user: bool,
        #[arg(long)]
        json: bool,
    },
    /// 读取单个变量的明文
    Get {
        name: String,
        #[arg(short, long)]
        system: bool,
    },
    /// 修改已有变量的值（类型不变）
    Set {
        name: String,
        /// 值（敏感值建议改用 --stdin 或 --value-file，避免进 shell 历史）
        #[arg(long)]
        value: Option<String>,
        /// 从标准输入读取值（读到 EOF）
        #[arg(long, conflicts_with_all = ["value", "value_file"])]
        stdin: bool,
        /// 从文件读取值
        #[arg(long, conflicts_with_all = ["value", "stdin"])]
        value_file: Option<String>,
        /// 并发校验摘要（来自 `env list --json` 的 revision 字段）
        #[arg(long, conflicts_with = "force")]
        revision: Option<String>,
        /// 跳过并发校验直接覆盖
        #[arg(long)]
        force: bool,
        /// 只显示将要发生的变更，不写注册表
        #[arg(long)]
        dry_run: bool,
        /// 以 JSON 输出变更摘要（需与 --dry-run 同用）
        #[arg(long, requires = "dry_run")]
        json: bool,
        #[arg(short, long)]
        system: bool,
    },
    /// 新建变量
    ///
    /// **纯新建语义**：同名变量会被拒绝（`NameExists`），不会覆盖 —— 因此无需
    /// revision 守卫（新建没有「旧值」可被并发覆盖）。注意检查与写入是两步，
    /// 存在毫秒级竞态窗口。
    ///
    /// 值必须经三通道之一提供；都不给会**立即报错**（不进入交互提示）。
    Add {
        name: String,
        /// 值。省略且不给 --stdin/--value-file 时报错（不进入交互提示）；
        /// 敏感值建议改用 --stdin 或 --value-file
        value: Option<String>,
        /// 从标准输入读取值（读到 EOF）
        #[arg(long, conflicts_with_all = ["value", "value_file"])]
        stdin: bool,
        /// 从文件读取值
        #[arg(long, conflicts_with_all = ["value", "stdin"])]
        value_file: Option<String>,
        /// 注册表类型：string (REG_SZ) 或 expand (REG_EXPAND_SZ)
        #[arg(long, default_value = "string", value_parser = ["string", "expand"])]
        kind: String,
        /// 只显示将要发生的变更，不写注册表
        #[arg(long)]
        dry_run: bool,
        /// 以 JSON 输出变更摘要（需与 --dry-run 同用）
        #[arg(long, requires = "dry_run")]
        json: bool,
        #[arg(short, long)]
        system: bool,
    },
    /// 删除变量
    Remove {
        name: String,
        #[arg(long, conflicts_with = "force")]
        revision: Option<String>,
        /// 跳过并发校验直接删除
        #[arg(long)]
        force: bool,
        /// 只显示将要发生的变更，不写注册表
        #[arg(long)]
        dry_run: bool,
        /// 以 JSON 输出变更摘要（需与 --dry-run 同用）
        #[arg(long, requires = "dry_run")]
        json: bool,
        #[arg(short, long)]
        system: bool,
    },
    /// 立即创建一份环境变量备份
    Backup {
        #[arg(long)]
        json: bool,
    },
    /// 列出已有的环境变量备份（按时间倒序）
    Backups {
        #[arg(long)]
        json: bool,
    },
    /// 从备份文件恢复环境变量
    Restore {
        /// 备份文件路径
        file: String,
        /// 只显示将要发生的变更，不写注册表
        #[arg(long)]
        dry_run: bool,
        /// 跳过 revision 校验直接覆盖（不豁免保护名单与类型判定）
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
}
