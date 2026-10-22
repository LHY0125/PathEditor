# 架构与模块边界

> 本文件是 `CLAUDE.md` 的详细补充。日常开工只需读 `CLAUDE.md` 的速览与红线；
> 需要理解模块职责、完整目录或历史沿革时再读本文。

## 完整目录树

```text
PathEditor/
├── core/src/                    # Rust 核心库（零 Tauri 依赖）
│   ├── registry/                # 注册表访问统一入口（path/access/env_var/conflict 子模块）
│   ├── db/                      # v6 SQLCipher 数据层
│   │   ├── mod.rs               # Db 句柄、事务、open_default / open_and_migrate
│   │   ├── schema.rs            # DDL v1/v2/v3；CURRENT_SCHEMA_VERSION = 3
│   │   ├── crypto.rs / dpapi.rs / dacl.rs   # 密钥、DPAPI、DACL
│   │   ├── history.rs / history_sink.rs      # 快照、事件、HistorySink 接缝
│   │   ├── profiles.rs          # 配置元数据/版本（DB 版）
│   │   ├── backups.rs           # .pebak 写入/读取、备份索引
│   │   ├── search.rs / stats.rs / refs.rs / compare.rs / scan_history.rs
│   │   ├── migrate_v5.rs        # v5 文件一次性导入（幂等）
│   │   ├── v5_export.rs / v5_import.rs      # v5 JSON 导出/导入
│   │   ├── maintenance.rs       # 保留策略 + 遗留明文清理
│   │   ├── archive_v5.rs        # .v5.bak 归档
│   │   └── sync/                # bundle / change_log / merge / conflict / export / import
│   ├── service.rs               # 保存编排：save_path_with_sidecar / apply_path_snapshot
│   ├── capabilities.rs          # 按 hive 探测 PathCapabilities
│   ├── system.rs                # 权限、路径验证、变量展开、环境广播
│   ├── disabled.rs              # 完整快照、禁用状态、旧格式兼容、pending 磁盘文件
│   ├── env_var.rs               # 通用环境变量契约与保留/保护/敏感判定
│   ├── path_entry.rs            # PathEntry / PathSnapshot 契约
│   ├── error.rs                 # 结构化 CoreError / ErrorCode（含 exit_code）
│   ├── reg_store.rs             # 注册表值类型读写（EnvHiveStore 端口）
│   ├── persist.rs               # 持久化文件解析与损坏隔离
│   ├── scanner.rs               # scan_paths 单次枚举、冲突和工具清单
│   ├── profiles.rs              # v5 文件版配置（兼容通路；DB 版在 db/profiles.rs）
│   ├── fs.rs                    # 受限文件读取、导入导出
│   └── backup.rs                # 注册表备份、env 备份（.pebak）
├── gui/src/commands/            # #[tauri::command] 薄包装，调用 core
│   ├── registry.rs / system.rs / backup.rs / fs.rs / disabled.rs / scanner.rs
│   ├── env_var.rs / service.rs / profiles.rs（v5 兼容模块，未注册）
│   ├── history.rs               # 历史 / DB 配置 / 备份索引
│   ├── m2.rs                    # 检索 / 统计 / 对比 / 引用图 / 冲突历史
│   └── sync.rs                  # 同步状态 / 导出 / 导入 / 冲突
├── cli/src/                     # Clap 定义 + 命令分派（bin-only crate，无 lib target）
│   ├── runtime.rs               # 注册表安全保存、错误退出、快照提交
│   ├── env_ops.rs               # env 子命令：值通道/并发选项/表格与 JSON 渲染
│   ├── import_export.rs         # 导入导出命令
│   ├── profile_ops.rs           # profile 子命令
│   ├── sync_ops.rs              # sync 子命令（口令隐藏输入）
│   ├── maintenance_ops.rs       # maintenance 子命令
│   └── scan_ops.rs              # 冲突、扫描、权限检查
├── src/                         # React 前端
│   ├── core/                    # 纯逻辑，零 React/Tauri 依赖（env-var、path-entry、undo-redo…）
│   ├── services/backend.ts      # 唯一 IPC 适配入口 + 运行时形状校验
│   ├── services/path-session.ts # 保存计划前置（权限预检 + 组合长度软提醒 + 文案映射）
│   ├── store/                   # Zustand：app / env / history / search / sync / theme
│   ├── components/              # layout / path-list / env-list / toolbar / dialogs / history / search / stats / sync / ui
│   ├── hooks/                   # useAppActions、useKeyboard、usePathValidation
│   └── i18n/                    # zh-CN / en
├── tests/unit/                  # Vitest 测试
├── tests/fixtures/              # 跨 Rust/TS 契约夹具
├── e2e/tests/                   # Playwright 测试（mock IPC）
├── probes/v6-sqlcipher/         # v6 SQLCipher 探针（不在 workspace；M0 证据）
├── .github/workflows/           # ci.yml（MSVC 质量门）/ release.yml（tag 发布）/ 探针
├── docs/superpowers/specs/      # 特性设计文档（YYYY-MM-DD-*-design.md）
├── docs/superpowers/plans/      # 实施计划（M1–M4）
├── docs/dev/                    # 本目录：按需读取的开发细节
└── docs/审核和开发/YYYY.MM.DD/  # 审核与开发记录
```

## 模块边界速记

| 层                        | 职责                                           | 禁止                                                         |
| ------------------------- | ---------------------------------------------- | ------------------------------------------------------------ |
| `core/src/`               | 业务规则、注册表访问、DB 访问、加解密          | 依赖 Tauri / React                                           |
| `gui/src/commands/`       | `#[tauri::command]` 薄包装、参数转换、错误呈现 | 复制业务规则                                                 |
| `cli/src/`                | Clap 解析、命令分派、错误文本                  | 复制安全判定（保留名/保护名单/类型/权限/revision 全在 core） |
| `src/core/`               | 纯函数、零框架依赖                             | 引入 React / Tauri                                           |
| `src/services/backend.ts` | **唯一 IPC 入口** + 运行时形状校验             | 组件/Store 直调 `invoke`                                     |

## 数据流（保存 PATH）

```text
UI 保存 → app store → backend.savePathWithSidecar
  → core::service::save_path_with_sidecar
      ① 写前备份（path_backup_*.txt）
      ② 乐观并发校验（原始值）
      ③ 顺序写 system → user
      ④ SendMessageTimeout(WM_SETTINGCHANGE)
      ⑤ 提交 sidecar 快照（失败则落 pending，下次启动/命令补写）
```

完整编码约束以 `CLAUDE.md` 的「关键约束」为准；本文件只补目录与职责。
