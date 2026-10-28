//! CLI 的 clap 命令定义。
//!
//! 参数语义与冲突规则只写在这里；命令实现见 `commands/path.rs` / `commands/env/`。
//!
//! **帮助文案分两层**：`-h` 只给速查（命令名 + 一行说明），`--help` 给详解
//! （长说明 + 示例 + 退出码）。机制上，变体文档注释的**第一段**进 `about`、
//! **全部内容**进 `long_about`；示例统一放 `after_long_help`，只在 `--help` 出现。
//!
//! 改文案前先看 `main.rs` 的 `help_tests`：那里断言了 `env add` 的「纯新建」
//! 「不进入交互提示」与 `env list` 的「敏感变量不含明文」。删掉对应注释会让测试失败。

use clap::{Parser, Subcommand};

/// 顶层 `--help` 的详解正文（`-h` 不显示）。
const TOP_LONG_ABOUT: &str = "\
Windows PATH / 环境变量管理器

PATH 命令默认操作用户 hive，加 --system 才动系统 hive（写系统 hive 需管理员）。
环境变量走 env 子命令，Path 例外 —— 它只能经 PATH 专用命令编辑。";

/// 顶层 `--help` 末尾的快速上手与指引（`-h` 不显示）。
const TOP_AFTER_LONG_HELP: &str = "\
快速上手:
  patheditor list --user               看用户 PATH
  patheditor add \"C:\\tools\" --user     加一条（默认就是用户 hive）
  patheditor clean --user --dry-run    先预览清理，不写注册表
  patheditor env list --json           看环境变量（含 revision 字段）

退出码: 0 成功 / 1 一般错误 / 2 参数解析失败 / 3 revision 冲突 / 4 同步合并冲突 / 5 密钥

完整语义见仓库文档 docs/dev/interfaces.md。";

#[derive(Parser)]
#[command(
    name = "patheditor",
    version = env!("CARGO_PKG_VERSION"),
    about = "Windows PATH / 环境变量管理器",
    long_about = TOP_LONG_ABOUT,
    after_long_help = TOP_AFTER_LONG_HELP
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// 列出 PATH 路径
    List {
        /// 只看系统 PATH（HKLM）。与 --user 同给则两者都看
        #[arg(short, long)]
        system: bool,
        /// 只看用户 PATH（HKCU）。与 --system 同给则两者都看
        #[arg(short, long)]
        user: bool,
        /// 以 JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 添加一条路径
    Add {
        /// 要添加的路径
        path: String,
        /// 写入系统 PATH（HKLM，需管理员）。与 --user 互斥
        #[arg(short, long)]
        system: bool,
        /// 写入用户 PATH（HKCU，默认）。与 --system 互斥
        #[arg(short, long)]
        user: bool,
    },
    /// 删除指定位置的路径
    Remove {
        /// 路径序号（来自 `list` 的输出）
        index: usize,
        /// 操作系统 PATH（默认用户 PATH）
        #[arg(short, long)]
        system: bool,
    },
    /// 编辑指定位置的路径
    Edit {
        /// 路径序号（来自 `list` 的输出）
        index: usize,
        /// 替换成的新路径
        new_path: String,
        /// 操作系统 PATH（默认用户 PATH）
        #[arg(short, long)]
        system: bool,
    },
    /// 上移路径（--steps 指定移动格数，默认 1）
    MoveUp {
        /// 路径序号（来自 `list` 的输出）
        index: usize,
        /// 上移格数
        #[arg(long, default_value = "1")]
        steps: usize,
        /// 操作系统 PATH（默认用户 PATH）
        #[arg(short, long)]
        system: bool,
    },
    /// 下移路径（--steps 指定移动格数，默认 1）
    MoveDown {
        /// 路径序号（来自 `list` 的输出）
        index: usize,
        /// 下移格数
        #[arg(long, default_value = "1")]
        steps: usize,
        /// 操作系统 PATH（默认用户 PATH）
        #[arg(short, long)]
        system: bool,
    },
    /// 清理无效和重复路径
    Clean {
        /// 只清理系统 PATH。与 --user 同给则两者都清
        #[arg(short, long)]
        system: bool,
        /// 只清理用户 PATH。与 --system 同给则两者都清
        #[arg(short, long)]
        user: bool,
        /// 只预览将删除的条目，不写注册表
        #[arg(long)]
        dry_run: bool,
        /// 以 JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 启用指定位置的路径
    Enable {
        /// 路径序号（来自 `list` 的输出）
        index: usize,
        /// 操作系统 PATH（默认用户 PATH）。与 --user 互斥
        #[arg(short, long)]
        system: bool,
        /// 操作用户 PATH（默认）。与 --system 互斥
        #[arg(short, long)]
        user: bool,
    },
    /// 禁用指定位置的路径
    Disable {
        /// 路径序号（来自 `list` 的输出）
        index: usize,
        /// 操作系统 PATH（默认用户 PATH）。与 --user 互斥
        #[arg(short, long)]
        system: bool,
        /// 操作用户 PATH（默认）。与 --system 互斥
        #[arg(short, long)]
        user: bool,
    },
    /// 从文件导入 PATH（JSON/CSV/TXT）
    Import {
        /// 源文件路径
        file: String,
        /// 导入到哪个 hive：system / user / both
        #[arg(long, default_value = "both")]
        target: String,
    },
    /// 导出 PATH 为文件
    Export {
        /// 导出格式：json / csv / txt
        #[arg(long, default_value = "json")]
        format: String,
        /// 输出文件路径；省略则写到 stdout
        #[arg(short, long)]
        output: Option<String>,
    },
    /// 创建注册表备份
    Backup,
    /// 检测可执行文件冲突
    Conflicts {
        /// 以 JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 列出 PATH 目录中的可执行文件
    Scan {
        /// 按名称过滤（子串匹配）
        #[arg(long)]
        query: Option<String>,
        /// 以 JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 检查管理员权限
    CheckAdmin {
        /// 以 JSON 输出
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
#[command(after_long_help = "\
示例:
  patheditor maintenance prune --dry-run        先看会清理多少条
  patheditor maintenance prune                  按保留策略清理
  patheditor maintenance cleanup-legacy-backups --dry-run
  patheditor maintenance cleanup-legacy-backups --yes

保留策略: 已导出的 changes 90 天、已解决的 sync_conflicts 30 天。
待导出队列永不删除。cleanup-legacy-backups 未给 --yes 时只打印计划并以 1 退出。")]
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
#[command(after_long_help = "\
示例:
  patheditor sync status                        看待导出变更与未解决冲突
  patheditor sync export out.pesync             导出加密同步包（口令隐藏输入）
  patheditor sync import out.pesync             在另一台机器上应用
  patheditor sync conflicts --all               列出冲突（含已解决）
  patheditor sync resolve 3 --take local        取本地一侧

同步只改 DB，不写注册表 —— 要让改动生效须另走 apply_path_snapshot 或前端保存。
退出码: 合并冲突 4 / 包格式或口令或篡改 1 / 密钥或 DPAPI 5。")]
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
#[command(after_long_help = "\
示例:
  patheditor profile list                       列出所有配置
  patheditor profile save work                  把当前 PATH 存为 work
  patheditor profile load work                  预览（不写注册表）
  patheditor profile apply work                 应用（写注册表）
  patheditor profile rename --old work --new office

load 只预览，apply 才写注册表。配置保存在 DB 中，DB 不可用时不可用。")]
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
#[command(after_long_help = "\
默认操作用户 hive（HKCU），加 --system 才操作系统 hive（HKLM，写需管理员）。
Path 不在通用通路内 —— 它只能经 PATH 专用命令编辑。

set / remove 必须显式给出 --revision 或 --force（互斥，缺一报错）：
  --revision <R>  并发安全。R 取自 `env list --json` 的 revision 字段，
                  不匹配时以退出码 3 退出。
  --force         跳过校验直接覆盖（最后写入者胜），仍受保护名单/类型/权限约束，
                  不会产生退出码 3。

get 是 CLI 侧唯一的明文出口：stdout 只打印裸值 + 换行（管道友好）。
敏感值建议用 --stdin 或 --value-file 传入，避免明文进入 shell 历史与进程列表。

示例:
  # 1) 并发安全的修改：先取 revision，再带上它
  patheditor env list --json
  patheditor env set JAVA_HOME --value \"C:\\jdk-25\" --revision <R>

  # 2) 敏感值走 stdin，不进 shell 历史
  echo -n \"secret\" | patheditor env set API_KEY --stdin --force

  # 3) 先预览再执行（--json 必须与 --dry-run 同用）
  patheditor env set FOO --value bar --force --dry-run --json

  # 4) 新建（纯新建语义：同名会被拒绝，不会覆盖）
  patheditor env add MY_TOOL --value \"C:\\tools\" --kind expand

  # 5) 读明文（管道友好）
  patheditor env get JAVA_HOME

退出码: 0 成功 / 1 一般错误 / 2 参数解析失败 / 3 revision 冲突 / 5 密钥或 DPAPI。")]
pub(crate) enum EnvCmd {
    /// 列出环境变量元数据（敏感变量不含明文，非敏感变量给出可能截断的 preview）
    List {
        /// 只看系统 hive（HKLM）。与 --user 同给则两者都看
        #[arg(short, long)]
        system: bool,
        /// 只看用户 hive（HKCU）。与 --system 互斥
        #[arg(short, long, conflicts_with = "system")]
        user: bool,
        /// 以 JSON 输出（含 revision 字段，供 set/remove 使用）
        #[arg(long)]
        json: bool,
    },
    /// 读取单个变量的明文
    Get {
        /// 变量名
        name: String,
        /// 从系统 hive 读取（默认用户 hive）
        #[arg(short, long)]
        system: bool,
    },
    /// 修改已有变量的值（类型不变）
    Set {
        /// 变量名（不能是 Path，Path 走专用命令）
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
        /// 操作系统 hive（HKLM，写需管理员）。默认用户 hive（HKCU）
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
        /// 操作系统 hive（HKLM，写需管理员）。默认用户 hive（HKCU）
        #[arg(short, long)]
        system: bool,
    },
    /// 删除变量
    Remove {
        /// 变量名（不能是 Path，Path 走专用命令）
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
        /// 操作系统 hive（HKLM，写需管理员）。默认用户 hive（HKCU）
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
