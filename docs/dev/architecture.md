# 架构与模块边界

> 本文件是 `CLAUDE.md` 的详细补充。日常开工只需读 `CLAUDE.md` 的速览与红线；
> 需要理解模块职责、完整目录或历史沿革时再读本文。

## 完整目录树

```text
PathEditor/
├── src/
│   ├── core/src/                # Rust 核心库（零 Tauri 依赖）
│   │   ├── registry/            # 注册表访问统一入口
│   │   │   ├── path.rs / access.rs / env_var.rs / conflict.rs
│   │   │   └── store.rs         # 注册表值类型读写（EnvHiveStore 端口）
│   │   ├── db/                  # v6 SQLCipher 数据层
│   │   │   ├── mod.rs           # Db 句柄、事务、open_default / open_and_migrate
│   │   │   ├── schema.rs        # DDL v1/v2/v3；CURRENT_SCHEMA_VERSION = 3
│   │   │   ├── crypto.rs / dpapi.rs / dacl.rs   # 密钥、DPAPI、DACL
│   │   │   ├── history.rs / history_sink.rs      # 快照、事件、HistorySink 接缝
│   │   │   ├── profiles.rs      # 配置元数据/版本（DB 版）
│   │   │   ├── backups.rs       # .pebak 写入/读取、备份索引
│   │   │   ├── search.rs / stats.rs / refs.rs / compare.rs / scan_history.rs
│   │   │   ├── maintenance.rs   # 保留策略 + 遗留明文清理
│   │   │   └── sync/            # bundle / change_log / merge / conflict / export / import
│   │   ├── service.rs           # 保存编排：save_path_with_db / save_path_registry_only
│   │   ├── capabilities.rs      # 按 hive 探测 PathCapabilities
│   │   ├── system.rs            # 权限、路径验证、变量展开、环境广播
│   │   ├── disabled.rs          # 快照合并、pending 磁盘文件
│   │   ├── env_var.rs           # 通用环境变量契约与保留/保护/敏感判定
│   │   ├── path_entry.rs        # PathEntry / PathSnapshot 契约
│   │   ├── error.rs             # 结构化 CoreError / ErrorCode（含 exit_code）
│   │   ├── persist.rs           # 持久化文件解析与损坏隔离
│   │   ├── scanner.rs           # scan_paths 单次枚举、冲突和工具清单
│   │   ├── fs.rs                # 受限文件读取、导入导出
│   │   └── backup.rs            # 注册表备份、env 备份（.pebak）
│   ├── cli/src/                 # Clap 定义 + 命令分派（bin-only crate，无 lib target）
│   │   ├── cli.rs               # clap 定义
│   │   ├── path_ops.rs          # PATH 子命令实现
│   │   ├── runtime.rs           # 注册表安全保存、错误退出、快照提交
│   │   ├── env_ops/             # env 子命令（value/render/cmd/tests）
│   │   ├── import_export.rs     # 导入导出命令
│   │   ├── profile_ops.rs       # profile 子命令
│   │   ├── sync_ops.rs          # sync 子命令（口令隐藏输入）
│   │   ├── maintenance_ops.rs   # maintenance 子命令
│   │   ├── scan_ops.rs          # 冲突、扫描、权限检查
│   │   └── output.rs            # JSON 输出出口
│   └── gui/
│       ├── src-tauri/src/commands/ # #[tauri::command] 薄包装，调用 core
│       │   ├── db_state.rs         # 共享 DbState 与取用辅助
│       │   ├── registry.rs / system.rs / backup.rs / fs.rs / disabled.rs / scanner.rs
│       │   ├── env_var.rs / service.rs
│       │   ├── history.rs          # 历史快照
│       │   ├── profiles.rs         # 配置方案
│       │   ├── analytics.rs        # 检索 / 统计 / 对比 / 引用图 / 冲突历史
│       │   └── sync.rs             # 同步状态 / 导出 / 导入 / 冲突
│       └── web/                    # React + Vite
│           ├── src/                # 纯逻辑与 UI（core/services/store/components/hooks/i18n）
│           ├── tests/              # Vitest 测试 + 跨 Rust/TS 契约夹具
│           └── e2e/                # Playwright 测试（mock IPC）
├── probes/v6-sqlcipher/         # v6 SQLCipher 探针（不在 workspace）
├── .github/workflows/           # ci.yml（MSVC 质量门）/ release.yml（tag 发布）/ 探针
├── docs/superpowers/specs/      # 特性设计文档（YYYY-MM-DD-*-design.md）
├── docs/superpowers/plans/      # 实施计划（M1–M4）
├── docs/dev/                    # 本目录：按需读取的开发细节
└── docs/审核和开发/YYYY.MM.DD/  # 审核与开发记录
```

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
