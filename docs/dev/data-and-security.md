# 数据、保存、事务与安全

> 本文件是 `CLAUDE.md` 的详细补充。日常开工只需读 `CLAUDE.md` 的红线；
> 涉及保存编排、备份、并发或安全边界时再读本文。

## 数据、保存与事务

- `disabled.json` 是 v5 状态文件（`systemSnapshot` / `userSnapshot` / 旧版禁用字符串）。v6 迁移成功后**停写**并归档为 `.v5.bak`；**降级模式下仍是兜底读源**。正常启动以注册表为启用路径真相来源，用 DB 的当前快照恢复禁用项和顺序。
- **保存 PATH 的编排在 `core::service`**（GUI 经 `backend.savePathWithSidecar`、CLI 的所有 PATH 写入口统一调用）。DB 可用时走 `save_path_with_db`：先写注册表，再提交 `kind=current` 快照与事件；DB 不可用时走 `save_path_with_sidecar`：写 v5 文件 sidecar。两种情况都在注册表成功但快照写入失败时，在 **core 侧**落 pending 磁盘文件（`disabled::save_pending_path_snapshot`），下次 PATH 命令开头经 `retry_pending_path_state`（文件模式）或 `retry_pending_path_state_db`（DB 模式）补写。**GUI store 不携带 `_pendingSys/_pendingUser`**；保存成功的状态栏会**非阻断提示**快照未落盘。两个 hive 分别报告成功或失败（`ApplyOutcome` 按 hive 分字段）。
- 保存前使用原始值做外部修改检测；不得用旧草稿覆盖其他进程刚写入的内容。
- 清理、禁用和启用操作必须保留 `PathEntry`，不能只传路径字符串。
- 通用环境变量写入（CLI 3 命令、GUI 3 命令）前自动产生一份备份，**默认写 DPAPI 加密的 `~/.patheditor/backups/env_backup_<时间戳>.pebak`**；只有拿不到密钥时才回落明文 `.json` 并打印警告。备份失败**不阻断写入**，只记警告并如实返回（`WriteOutcome.backup`）。默认保留最近 20 份（`backup::ENV_BACKUP_KEEP`，可被 `~/.patheditor/config.ini` 的 `env_backup_keep` 覆盖），轮换只删除本功能生成的 `env_backup_*.pebak` / `.json`。恢复默认在 revision 冲突时中止（退出码 3）且零写入；`--force` 跳过 revision 校验但**不豁免**保护名单与类型判定。
- PATH 备份（`path_backup_<时间戳>.txt`，由 `backup_registry` 写入，含两个 hive 的完整 PATH 明文）同样写后自动轮换：默认保留最近 20 份（`backup::PATH_BACKUP_KEEP`，可被 `~/.patheditor/config.ini` 的 `path_backup_keep` 覆盖，回落语义同 `env_backup_keep`），只删除本功能生成的 `path_backup_*.txt`，且**仅作用于默认备份目录**——用户通过 `custom_dir` 指定的目录不参与轮换（可能混有用户自己的文件）。
- 恢复的差异计数有两个**已知的规格级口径问题**，不是已修复项：`RestorePreview.modified` 与 `RestoreOutcome.skipped` **恒为 0**（前者因为 `revision_of` 是 `(name, type, value)` 的纯函数，「值变了」与「备份已过期」是同一条件，后者一律判 `Conflict`；后者因为没有路径会「跳过」）。因此 `--dry-run` 与 GUI 确认弹窗的「修改 N」恒显示 0，而 `--force` 下被冲突覆盖的变量**确实被改写却计入 `conflicts`** —— 两者都会**低报** force 模式的实际改动量。`EnvBackupInfo.variable_count` 同样恒为 0（列表不解析内容）。**未提权时恢复恒失败**：普通用户无法以写权限打开 HKLM，`restore_env_backup_from` 必然返回 `permissionDenied`，且 GUI **不提供**「仅恢复 user hive」这类降级路径。
- `env restore --dry-run` 对**损坏备份**并非严格纯读：`read_env_backup` 对不可解析文件会经 `persist::parse_error_quarantined` 把文件重命名为 `<file>.corrupt-<ts>`（注册表未触碰）。
- 备份文件的暴露面：`.pebak` 是 DPAPI 加密的，但**备份目录 ACL 未收紧**；遗留/回落 `.json` 仍是**明文**（spec §S2.1 登记的未覆盖项）。
- **v5 文件停写与归档**：DB 可用时 `disabled.json` / `profiles/*.json` 不再写入（降级模式仍写 `disabled.json`）；迁移成功后启动序列末尾把原件改名为 `.v5.bak`（目标已存在则跳过，绝不覆盖）。遗留 `env_backup_*.json` 用 `maintenance cleanup-legacy-backups --yes` 先转 `.pebak` 并回读校验，再删除明文。
- **同步与保留策略**：见 `docs/dev/sqlite-storage.md`；`maintenance prune` 只删已导出且到期的 `changes` 与已解决且到期的 `sync_conflicts`，**待导出队列（`exported_at IS NULL`）永不删**。

路径验证和环境变量展开使用有界并发队列，不按前 20 条截断；扫描通过 `scan_paths` 共享枚举结果，最多使用 8 个扫描线程。

## 错误处理与安全

- `backend.ts` 对能力对象、快照、禁用状态和 `PathEntry` 做运行时形状校验，不能只依赖 TypeScript 断言。
- 旧命令仍返回自由文本 `Result<T, String>`；v6 的 DB / 同步命令返回结构化 `CoreError`，前端按 `code` 判定（`conflict` / `sync` / `syncFormat` / `crypto` …），**不要匹配 message 文本**。
- 导入文件读取限制在用户目录、临时目录或当前工作目录，并继续校验扩展名。
- Tauri CSP 不允许设置为 `null`；不要放松 `gui/tauri.conf.json` 的安全配置。
- 所有 `unsafe` 块必须有 `// SAFETY:` 注释。PATH 写入前检查 null 字节和 32767 字符上限。
- 所有 `pub fn` 必须有 `///` 文档注释；`pub(crate)` 同样补注释（CONTRIBUTING.md 硬性要求）。
- 环境变量输入在 core 侧统一校验；CLI/GUI 不做二次判定，但 `backend.ts` 仍要拒绝来路不明的 `EnvVarMeta`（含 `value` 字段者一律拒绝，白名单构造字段）。
- `Path` 列入 `RESERVED_NAMES`，`list_all_env_vars` 过滤掉它，Rust 写入口拒绝——**PATH 只能经专用通路编辑**，否则会绕过 `disabled.json` 与快照事务。`EnvVarMeta` 契约上不含 `value` 字段；命中敏感规则的变量明文只能经 `reveal_env_var` 获取。
- GUI 与 CLI 共用同一套 revision 契约（`expected_revision`），前端与 CLI 都**不得**自行实现并发校验。
- E2E 使用 mock IPC，**不得写真实注册表**。真实 Tauri/注册表闭环测试必须显式授权，并记录备份、操作前后快照、重启结果和回滚结果（样本见 `docs/审核和开发/2026.09.18/`）。
- **同步包安全**：Argon2id + ChaCha20-Poly1305，AAD 覆盖 header/salt/nonce；口令不落盘、不进 shell 历史/`ps`、错误文案不含口令；导入失败整体回滚。
- **同步导入的外部输入**：`profile.name` 过 `validate_name`，非法名称 → `SyncFormat`；`profile.content.snapshotUid` 悬空 → `SyncFormat` 整包回滚；未知 `entityType` → `SyncFormat`。
- **维护命令**：`cleanup-legacy-backups` 必须 `--yes`；转换或回读失败保留原文件；`archive-v5` 只改名、不覆盖、不删除。
- `env get` 读取不存在的变量时，返回 `ErrorCode::NotFound`、message 为 `{用户|系统}环境变量 <NAME> 不存在`（不泄漏 `os error` 原生串、变量名只出现一次），退出码 1。判定只认 `code`，**不得**匹配 message 文本——NotFound 的结构信息由 `reg_store::get_raw_kind` 分类上传。
- CLI 退出码总表见 `docs/dev/interfaces.md`。
