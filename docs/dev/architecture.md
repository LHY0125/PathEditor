# 架构与模块边界

> 本文件是 `CLAUDE.md` 的详细补充。日常开工只需读 `CLAUDE.md` 的速览与红线；
> 需要理解模块职责、完整目录或历史沿革时再读本文。

## 完整目录树

```text
PathEditor/
├── src/
│   ├── core/src/                # Rust 核心库（零 Tauri 依赖）
│   │   ├── domain/              # 契约与端口：零 OS、零 IO、零数据库
│   │   │   ├── error.rs         # CoreError / ErrorCode（含 exit_code）
│   │   │   ├── path_entry.rs    # PathEntry / PathSnapshot
│   │   │   ├── env_var.rs       # 通用环境变量契约 + 保留/保护/敏感判定
│   │   │   └── history.rs       # HistorySink 端口 + EventSource
│   │   ├── platform/            # 外部世界适配
│   │   │   ├── persist.rs       # 版本化文件信封、轮换、损坏隔离
│   │   │   ├── fs.rs            # 受限文件读取、导入导出
│   │   │   ├── system.rs        # 权限、变量展开、环境广播
│   │   │   ├── scanner.rs       # scan_paths 单次枚举、冲突与工具清单
│   │   │   ├── capabilities.rs  # 按 hive 探测 PathCapabilities
│   │   │   ├── registry/        # 注册表访问统一入口（path/access/env_edit/conflict/store）
│   │   │   └── backup/          # 注册表备份、env 备份（.pebak）
│   │   ├── db/                  # v6 SQLCipher 数据层
│   │   │   ├── mod.rs           # Db 句柄、事务、open_default / open_and_migrate
│   │   │   ├── schema.rs        # DDL v1/v2/v3；CURRENT_SCHEMA_VERSION = 3
│   │   │   ├── crypto.rs / dpapi.rs / dacl.rs   # 安全边界：密钥、DPAPI、DACL
│   │   │   ├── history.rs / history_sink.rs      # 快照、事件、HistorySink 实现
│   │   │   ├── profiles.rs      # 配置元数据/版本（DB 版）
│   │   │   ├── backup_index.rs  # 备份索引表（磁盘文件在 platform/backup/）
│   │   │   ├── search.rs / stats.rs / refs.rs / compare.rs / scan_history.rs
│   │   │   ├── maintenance.rs   # 保留策略 + 遗留明文清理
│   │   │   └── sync/            # bundle / change_log / merge / conflict / export / import
│   │   ├── app/                 # 编排：唯一同时依赖 platform 与 db 的层
│   │   │   ├── bootstrap.rs     # 启动引导序列
│   │   │   ├── disabled.rs      # 快照合并、pending 磁盘文件
│   │   │   └── service/         # 保存编排：save_path_with_db / save_path_registry_only
│   │   └── lib.rs               # 四层声明 + 契约再导出
│   ├── cli/src/                 # bin-only crate，无 lib target
│   │   ├── main.rs              # 入口 + 命令分派
│   │   ├── cli.rs               # clap 定义
│   │   ├── runtime.rs           # AppContext、错误退出码、快照提交
│   │   ├── output.rs            # JSON / 文本输出出口
│   │   └── commands/
│   │       ├── path.rs          # PATH 子命令
│   │       ├── profile.rs       # profile 子命令
│   │       ├── scan.rs          # 冲突、扫描、权限检查
│   │       ├── sync.rs          # sync 子命令（口令隐藏输入）
│   │       ├── maintenance.rs   # maintenance 子命令
│   │       ├── import_export.rs # 导入导出命令
│   │       └── env/             # env 子命令（cmd / render / value / tests）
│   └── gui/
│       ├── src-tauri/src/commands/ #[tauri::command] 薄包装，调用 core
│       └── web/                    # React + Vite
```

层间依赖只允许往下走：

    app → db → platform → domain
     └──────────↗

`platform` 内部 `registry` ↔ `backup` 互相引用（写前备份 / 备份后回读），
是唯一的层内强连通分量，因此两者必须同层。

## 模块边界速记

| 层                                    | 职责                                           | 禁止                                                         |
| ------------------------------------- | ---------------------------------------------- | ------------------------------------------------------------ |
| `src/core/src/`                       | 业务规则、注册表访问、DB 访问、加解密          | 依赖 Tauri / React                                           |
| `src/gui/src-tauri/src/commands/`     | `#[tauri::command]` 薄包装、参数转换、错误呈现 | 复制业务规则                                                 |
| `src/cli/src/`                        | Clap 解析、命令分派、错误文本                  | 复制安全判定（保留名/保护名单/类型/权限/revision 全在 core） |
| `src/gui/web/src/core/`               | 纯函数、零框架依赖                             | 引入 React / Tauri                                           |
| `src/gui/web/src/services/backend.ts` | **唯一 IPC 入口** + 运行时形状校验             | 组件/Store 直调 `invoke`                                     |

## 数据流（保存 PATH）

```text
UI 保存 → app store → backend.savePathWithSidecar
  → core::service::save_path_with_db（DB 可用）/ save_path_registry_only（降级）
      ① 写前备份（path_backup_*.txt）
      ② 乐观并发校验（原始值）
      ③ 顺序写 system → user
      ④ SendMessageTimeout(WM_SETTINGCHANGE)
      ⑤ DB：提交 current 快照 + 事件；降级：只写注册表，禁用状态不持久化
```

完整编码约束以 `CLAUDE.md` 的「关键约束」为准；本文件只补目录与职责。
