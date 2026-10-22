# 接口：Tauri IPC 与 CLI

> 本文件是 `CLAUDE.md` 的详细补充。日常开工只需读 `CLAUDE.md` 的接口速览；
> 需要具体命令名、参数或退出码时再读本文。

## Tauri IPC 接口

| Command                                                                                                                                        | 参数 / 返回值                                        | 说明                                                                                         |
| ---------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `load_system_paths` / `load_user_paths`                                                                                                        | `() -> Result<Vec<String>, String>`                  | 读取注册表原始路径                                                                           |
| `save_path_with_sidecar`                                                                                                                       | `(system?, user?) -> Result<ApplyOutcome, String>`   | **唯一保存编排**：写前备份 + 乐观并发校验 + 顺序写 system→user + 广播 + sidecar 事务         |
| `clean_path_entries`                                                                                                                           | `Vec<PathEntry> -> (kept, removed)`                  | 统一清理语义，展开变量后检查目录                                                             |
| `get_path_capabilities`                                                                                                                        | `() -> PathCapabilities`                             | 按 hive 返回读写能力                                                                         |
| `check_admin` / `validate_path` / `expand_env_vars` / `broadcast_env_change`                                                                   | 系统工具接口                                         | 权限、目录验证、变量展开、环境通知                                                           |
| `load_path_snapshot`                                                                                                                           | `() -> Result<PathSnapshot, String>`                 | 合并注册表、禁用状态和有序快照                                                               |
| `save_path_snapshot`                                                                                                                           | `(system?, user?) -> Result<(), String>`             | 保存完整有序快照，`None` 保留对应 hive                                                       |
| `load_disabled_state` / `save_disabled_state`                                                                                                  | 旧版字符串接口                                       | 仅用于兼容                                                                                   |
| `import_file` / `export_path_entries` / `read_text_file`                                                                                       | 文件路径 / `PathEntry` + 格式                        | Rust 统一导入导出与受限文本读取                                                              |
| `scan_paths`                                                                                                                                   | `(paths, query) -> ScanResult`                       | 一次枚举生成冲突和工具清单                                                                   |
| `backup_registry`                                                                                                                              | `() -> Result<String, String>`                       | 备份当前注册表 PATH 值                                                                       |
| `profile_list` / `profile_save` / `profile_paths` / `profile_rename` / `profile_delete_by_name` / `profile_mark_applied` / `profile_export_v5` | 配置 CRUD（DB 版）                                   | 配置保存 `PathEntry[]`；v5 文件版 IPC 已下线                                                 |
| `list_all_env_vars`                                                                                                                            | `() -> Result<EnvVarSnapshot, CoreError>`            | 一次读取两个 hive 的全部环境变量元数据（不含敏感明文）                                       |
| `reveal_env_var`                                                                                                                               | `(hive, name) -> Result<RevealedValue, CoreError>`   | 按需读取单个变量明文及读取时的 revision；`Unsupported` 类型返回 `CoreError`                  |
| `update_env_var`                                                                                                                               | `(hive, name, value, expectedRevision)`              | 写入已有变量；类型从注册表读取，revision 不匹配则拒绝（`CoreError.code = conflict`）         |
| `create_env_var`                                                                                                                               | `(hive, name, value, kind)`                          | 新建变量；写入前检查名称是否存在（检查与写入是两步操作，存在竞态窗口；重复创建由 Rust 拒绝） |
| `delete_env_var`                                                                                                                               | `(hive, name, expectedRevision)`                     | 删除变量；revision 不匹配则拒绝                                                              |
| `backup_env_vars`                                                                                                                              | `() -> Result<String, CoreError>`                    | 立即创建一份 env 备份，返回文件路径（不保证绝对：受 PATHEDITOR_BACKUP_DIR 影响）             |
| `list_env_backups`                                                                                                                             | `() -> Result<Vec<EnvBackupInfo>, CoreError>`        | 列出备份文件，按时间倒序；只枚举与 stat，**不解析内容**（`variableCount` 恒为 0）            |
| `preview_env_backup`                                                                                                                           | `(file) -> Result<RestorePreview, CoreError>`        | 读取备份并计算差异（纯读）。先做路径来源校验；对不可解析文件会经 persist 层重命名隔离        |
| `restore_env_backup`                                                                                                                           | `(file, force) -> Result<RestoreOutcome, CoreError>` | 执行恢复。未提权时打开 HKLM 必然失败，恒返回 `permissionDenied`；GUI 只提示需管理员          |

> 服务层 IPC 只剩 `save_path_with_sidecar`（前端经 `backend.savePathWithSidecar` 调用）；CLI 的所有 PATH 写入口也统一经它。`apply_path_snapshot` / `retry_pending_path_state` 仍是 core 函数，由 GUI 历史恢复与启动序列直接调用，不再暴露为 IPC；`apply_profile`（读 v5 文件）已删除，配置应用统一走 DB + `save_path_with_sidecar`。
>
> 已退役的 IPC 命令：`save_system_paths` / `save_user_paths`（由 `save_path_with_sidecar` 取代）、`scan_conflicts` / `scan_tools`（由 `scan_paths` 取代）、`get_appdata_dir`（前端零调用）。对应的 core 函数（`scanner::scan_conflicts` / `scan_tools`、`backup::get_appdata_dir`）**仍保留**，被内部使用。

### v6 新增的 DB 命令

全部经 `backend.ts` 运行时形状校验：

- 历史：`history_list_events` / `history_get_snapshot` / `history_get_current` / `history_reveal_env_value` / `history_restore_snapshot`
- 配置：`profile_list` / `profile_save` / `profile_paths` / `profile_rename` / `profile_delete_by_name` / `profile_mark_applied` / `profile_mark_applied_by_name` / `profile_export_v5`
- 备份：`backup_list`
- 检索 / 统计：`search` / `search_rebuild` / `stats_dashboard` / `stats_rebuild` / `compare_snapshots`
- 引用图 / 冲突历史：`refs_dependents` / `refs_dependencies` / `refs_cycles` / `refs_rebuild` / `scan_history_list` / `scan_history_diff` / `scan_history_record`
- 同步：`sync_status` / `sync_export` / `sync_import` / `sync_list_conflicts` / `sync_resolve_conflict`

v5 文件版 `list_profiles` / `save_profile` / `load_profile` / `delete_profile` / `rename_profile`
**已下线**（`gui/src/commands/profiles.rs` 模块保留但不在 `generate_handler!` 中）；`core::profiles` 仍是 v5 兼容通路。

## CLI 命令

```text
patheditor list          [--system|--user] [--json]
patheditor add           <PATH> [--system|--user]
patheditor remove        <INDEX> [--system]
patheditor edit          <INDEX> <NEW> [--system]
patheditor move-up       <INDEX> [--steps N] [--system]
patheditor move-down     <INDEX> [--steps N] [--system]
patheditor clean         [--system|--user] [--dry-run] [--json]
patheditor enable        <INDEX> [--system|--user]
patheditor disable       <INDEX> [--system|--user]
patheditor import        <FILE> [--target system|user|both]
patheditor export        [--format json|csv|txt] [--output <FILE>]
patheditor backup
patheditor conflicts     [--json]
patheditor scan          [--query <NAME>] [--json]
patheditor check-admin   [--json]
patheditor profile       {list [--json]|save <NAME>|load <NAME>|apply <NAME>|delete <NAME>|rename --old <OLD> --new <NEW>|export-v5 <NAME>|import <FILE>}
patheditor env list      [--system|--user] [--json]
patheditor env get       <NAME> [--system]
patheditor env set       <NAME> [--value <V>|--stdin|--value-file <F>] (--revision <R>|--force) [--dry-run] [--json]
patheditor env add       <NAME> [<VALUE>] [--kind string|expand] [--system] [--dry-run] [--json]
patheditor env remove    <NAME> (--revision <R>|--force) [--dry-run] [--json]
patheditor env backup    [--json]
patheditor env backups   [--json]
patheditor env restore   <FILE> [--dry-run] [--force] [--json]
patheditor sync          {export <FILE>|import <FILE>|status|conflicts [--all]|resolve <ID> --take local|remote}
patheditor maintenance   {prune [--dry-run]|archive-v5 [--dry-run]|cleanup-legacy-backups [--dry-run] [--yes]}
```

`sync export/import` 的口令用 `rpassword` 隐藏输入，不落盘、不进 shell 历史；同步只改 DB，不写注册表。退出码：合并冲突 → 4，包格式/口令/篡改 → 1，密钥/DPAPI → 5。`maintenance cleanup-legacy-backups` 未给 `--yes` 时只打印计划并以 1 退出；`prune` 是保留策略，不需要 `--yes`。

`remove`、`edit`、`move-up`、`move-down` 默认操作用户 PATH，传入 `--system` 才操作系统 PATH。CLI 的 `list`、`import/export`、`profile`、`enable/disable` 都使用完整快照，避免丢失 `enabled=false` 条目和顺序。

`env` 子命令默认操作用户 hive，加 `--system` 操作系统 hive；`Path` 不在通用通路内。`set`/`remove` 必须显式给出 `--revision` 或 `--force`（互斥，缺一报错）；`--force` 跳过 revision 校验直接覆盖（最后写入者胜，仍受保护名单/类型/权限约束），不会因并发冲突产生退出码 3；退出码 3 仅在 `--revision` 不匹配时出现，其余错误为 1。`get` 是 CLI 侧唯一明文出口，stdout 只打印裸值 + 换行（管道友好）；`env add` 的 `--kind` 默认 `string`，可选 `string`（`REG_SZ`）/ `expand`（`REG_EXPAND_SZ`）；敏感值建议用 `--stdin` 或 `--value-file` 传入，避免明文进入 shell 历史与进程列表。`set`/`add`/`remove` 支持 `--dry-run` 只预览变更、不写注册表、不产生备份、不广播；预览**不要求** `--revision`/`--force`，且不含保护名单/类型判定（`writable` 如实标记，真正的拒绝留给执行阶段）。预览**不新开明文出口**：命中 `is_sensitive` 的变量不回传明文（`oldValue` 为 `null`、文本显示 `(敏感)`），非敏感值经 `sanitize_preview` 剔除控制字符（含 ESC，防 ANSI 注入）。`--json` 需与 `--dry-run` 同用（clap `requires`，单独用退出码 2），输出统一结构 `{operation, hive, name, oldValue, newValue, kind, writable, sensitive}`。

`env restore` 默认模式在备份中有变量被外部改动（revision 不匹配）时整批中止且**注册表零改动**（退出码 3）；`--force` 跳过 revision 校验直接覆盖，但**不豁免**保护名单与类型判定，也不会因并发冲突产生退出码 3。`--dry-run` 只打印差异、不写注册表。**逐条失败不改变退出码**（仍是 0）——恢复是 best-effort，单变量失败只进 `failures` 并以 stderr 警告呈现，脚本无法从退出码检出部分失败。

### 退出码总表

| 码  | 含义                                                              |
| --- | ----------------------------------------------------------------- |
| 0   | 成功                                                              |
| 1   | 一般错误；同步包格式/口令/篡改（`SyncFormat`）也归入此码          |
| 2   | clap 参数解析失败                                                 |
| 3   | revision 冲突（仅 `env set` / `env remove` 使用 `--revision` 时） |
| 4   | 同步合并冲突（`ErrorCode::Sync`，需人工解决）                     |
| 5   | 密钥 / DPAPI（`ErrorCode::Crypto`）                               |
