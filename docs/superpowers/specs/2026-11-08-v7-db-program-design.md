# V7 数据库能力扩展 — 设计文档

- **日期**：2026-11-08
- **作者**：审核窗口
- **状态**：待核对（批次 A 已开工）
- **范围**：`src/core/src/db`、`src/cli`、`src/gui`、`docs/dev`
- **目标版本**：v7.1 – v7.5（按批次发布，不绑定单一版本号）

> **2026-11-10 用户指令覆盖**：同步只接受 v2 包，旧包与 `profile.field = null`
> 兼容路径不再保留；§12 中「旧包 `field = null` 视为 content」作废。
> 旧库迁移与迁移前备份/还原命令也已移除，旧 schema 库直接拒绝。

## 一、背景与原则

V7 已完成「数据库去加密」，数据层（SQLite + FTS5 + 快照/事件/同步）本身是可用的。
现状盘点发现三类机会：

1. **已建未接线**：表与逻辑写好了，产品没有任何入口（`scan_runs`、`backups` 索引、`snapshots.note`）；
2. **明确未实现**：`CompareResult` 的环境变量三个字段恒空；CLI 写后派生表陈旧；
3. **数据生命周期缺失**：`prune` 只删 `changes`/`sync_conflicts`，`snapshots`/`events` 无限增长。

设计原则：

- **先接线，再建表**：能在现有列上完成的，不升 schema。
- **每项独立可发布**：一项功能不依赖另一项的提交顺序，除显式登记的依赖。
- **fail-open 不变**：DB 不可用时读返回空、写返回结构化错误；不新增静默降级。
- **敏感面不扩大**：新功能不新增明文值出口；值只在既有 `reveal` 路径出现。
- **同步协议变更单独发版**：bundle 版本升级（12/13）不与普通功能同批。

## 二、现状基线

| 表/能力                                                | 状态                                                                    |
| ------------------------------------------------------ | ----------------------------------------------------------------------- |
| `snapshots` / `path_entries` / `env_values` / `events` | 已接线（历史、恢复、FTS）                                               |
| `profiles` / `profile_revisions`                       | 已接线（保存/应用/同步）                                                |
| `backups`                                              | **只写不读**：`index_backups` 仅启动时调用，`list_backups` 无生产调用者 |
| `scan_runs`                                            | **零生产调用**：`record_run`/`list_runs`/`diff_runs` 仅测试使用         |
| `snapshots.note`                                       | **完全未用**：无写入、无读取、无 UI                                     |
| `CompareResult.env_*`                                  | 恒为空，源码注释写明「尚未实现」                                        |
| `prune`                                                | 只处理 `changes` / `sync_conflicts`                                     |
| `reveal_env_var` / 历史 reveal                         | 不写事件，无审计                                                        |
| `migration_imports`                                    | 仅 schema 与测试（v5 迁移遗留，暂不处理）                               |

## 三、横切决策

| 编号 | 决策                                                                                                                             | 理由                                                  |
| ---- | -------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| D1   | schema **v5 只承载两项**：快照固定标记（#4）与规则引擎（#18）                                                                    | 其余 16 项都可在现有列/索引上完成，避免无谓迁移       |
| D2   | 新事件动作 `revealEnv`、`drift` 必须**同一提交**改 Rust 枚举、`action_str`、TS `EventAction` 联合与 `EVENT_ACTIONS` 白名单、i18n | 任一端漏改会让 `parseEventItem` 直接抛「action 非法」 |
| D3   | 保留策略（#6）必须先于历史增强类功能（#1/#4/#5/#15）落地                                                                         | 否则历史越多、库越大，保留策略的删除面也越复杂        |
| D4   | 同步协议（#12/#13）升级 `BUNDLE_VERSION`，新版本可读旧包；旧版本读新包明确报 `SyncFormat`                                        | 跨设备必须显式拒绝，不能静默丢字段                    |
| D5   | 新增读查询必须带 `LIMIT` 上限；新增过滤列在 v5 一并建索引                                                                        | 与现有 `list_*` 的 1–200 上限纪律一致                 |
| D6   | UI 优先复用现有面板：历史→HistoryPanel，备份→备份面板，扫描→分析对话框，同步→SyncCenter                                          | 不新增顶层 tab，降低信息架构成本                      |

## 四、逐项设计

### 1. 扫描历史与两次扫描差异

- **目标**：每次扫描留档；选择任意两次查看冲突的新增/消失/持续，以及每个冲突的首次/最近出现。
- **数据**：`scan_runs(uid, created_at, conflicts_json, tools_json)` 已足够，无需迁移。
- **Core**：已实现 `record_run/list_runs/diff_runs`，不改签名。
- **接线**：CLI `scan` 改为「有 DB 时记录」；新增 `scan history [--limit] [--json]` 与 `scan diff <a> <b> [--json]`；GUI `scan_paths` 命令增加 `DbState`，扫描后 best-effort 记录；新增 IPC `scan_history_list` / `scan_history_diff`。
- **UI**：分析对话框新增「扫描历史」页签：运行列表（时间/冲突数/工具数）、两次选择、差异三段。
- **测试**：core 已有；CLI 进程级用 `PATHEDITOR_DATA_DIR` 临时库验证记录与 diff；GUI mock IPC 断言形状。
- **规模**：M。**依赖**：无（#6 落地后更稳）。

### 2. 备份索引对账

- **目标**：回答「索引里的备份文件还在吗」「目录里的备份为什么看不到」。
- **数据**：`backups(file_path, content_hash, size_bytes, deleted_at)`。
- **Core**：新增 `reconcile_backups(db, dir) -> BackupReconcile { missing: Vec<BackupView>, orphans: Vec<PathBuf> }`；`purge_missing(db)` 只把缺失行标记 `deleted_at`，不删磁盘文件。
- **接口**：IPC `backup_reconcile` / `backup_purge_missing`；CLI `maintenance backups reconcile [--json] [--purge-missing]`。
- **UI**：备份面板顶部提示「N 条索引缺失 / M 个文件未入索引」，链接到清理动作。
- **测试**：TempDir 构造「索引有文件无」「文件有索引无」「两边一致」三种；断言不触碰真实备份目录。
- **规模**：S。**依赖**：无。

### 3. CLI 写后派生表刷新

- **目标**：CLI 改完 PATH/环境变量后，`daily_stats` 与 `var_references` 立即对齐，不必等下一次 GUI 启动。
- **数据**：水位文件 `derived.watermark`（随 `data_home()`，测试经 `PATHEDITOR_DATA_DIR` 隔离）。
- **Core**：`derived::rebuild_if_stale` 已存在。
- **接线**：CLI `main` 收尾调用 `runtime::refresh_derived_if_stale()`；DB 不存在/迁移失败/水位未变时静默空操作；失败只写 stderr。
- **测试**：`runtime::derived_refresh_rebuilds_only_when_watermark_changes`（已随批次 A 落地）。
- **规模**：S。**依赖**：无。**状态**：已实现。

### 4. 快照备注与固定基线

- **目标**：给快照写备注（「装驱动前」「崩溃前」），并固定为基线不被保留策略清理。
- **数据**：`snapshots.note` 已存在；**v5 新增** `pinned INTEGER NOT NULL DEFAULT 0` + `idx_snapshots_pinned`。
- **Core**：`set_snapshot_note(db, uid, note)`、`set_pinned(db, uid, pinned)`、`list_pinned(db)`。
- **接口**：IPC `history_set_note` / `history_set_pinned` / `history_list_pinned`；CLI `history note <uid> <text>`、`history pin <uid> [--off]`（新增 `history` 子命令组）。
- **UI**：HistoryPanel 行内编辑备注、图钉按钮；列表支持「只看固定」。
- **测试**：备注长度上限、空备注清空、固定项不被 #6 清理。
- **规模**：M。**依赖**：#6 的保护名单读取 `pinned`。

### 5. 环境变量快照对比

- **目标**：补齐 `CompareResult` 的 `env_added/env_removed/env_modified`，让「变量变了」不再显示「无差异」。
- **数据**：`env_values(snapshot_id, hive, name, kind, value, sensitive)`。
- **设计**：只返回**变量名**，不返回值（避免新增明文出口）；比较键为 `(hive, 小写 name)`，`modified` 以 `revision` 或 `value` 差异判定；`sensitive` 与 `restorable` 变化计入 modified。
- **Core**：`compare` 填充三字段；新增 `env_details: Vec<EnvNameChange>`（hive/name/kind/change 类型）作为附加字段（additive，不破坏旧 JSON）。
- **interface**：沿用 `compare_snapshots`；CLI `compare` 输出增加 env 段。
- **UI**：对比对话框新增「环境变量」段，敏感项只显示名称。
- **测试**：新增/删除/改值/改类型/敏感项五种；断言值不出现在 JSON。
- **规模**：S–M。**依赖**：无。

### 6. 保留策略扩展与压缩

- **目标**：控制 `snapshots`/`events` 增长，避免历史功能越多库越大。
- **数据**：不新增表；配置写 `~/.patheditor/config.ini`：`snapshot_keep_days`（默认 90）、`snapshot_keep_max`（默认 200）、`event_keep_days`（默认 365）。
- **删除规则**：永不删 `current_snapshot`、`pinned`、被 `profile_revisions.snapshot_id` 或 `backups.snapshot_id` 引用的快照；`path_entries`/`env_values` 由外键级联删除。`events` 按 `created_at` 删除，但保留最近一次 `save`/`apply`。
- **接口**：`maintenance prune` 扩展返回字段；新增 `maintenance vacuum`（`wal_checkpoint(TRUNCATE)` + 可选 `VACUUM`）。
- **测试**：构造 300 快照/多事件，断言删除数量、保护项不删、级联干净、prune 后 `integrity_check`。
- **规模**：M。**依赖**：#4（pinned）。

### 7. 数据库健康面板与迁移前备份

- **目标**：出问题时有自检入口；schema 升级前自动留一份可回退副本。
- **数据**：`PRAGMA integrity_check(1)`、各表 `COUNT(*)`、文件与 `-wal` 大小、schema 版本、最近 prune 时间（写 `app_state` 新列需 v5；或写独立元数据文件，优先文件避免迁移）。
- **Core**：`health(db, db_path) -> DbHealth`；迁移前备份：`ensure_schema` 前若 `user_version < 当前`，复制 `patheditor.db.pre-v{n}.bak`（同目录，保留 3 份）。
- **接口**：IPC `db_health`；CLI `maintenance health [--json]`。
- **UI**：统计页顶部卡片；异常时红色提示并给出「打开目录」。
- **规模**：M。**依赖**：无。

### 8. 审计导出

- **目标**：把 `events`/`changes` 导出为 CSV/JSON，便于留档与排查。
- **Core**：`export_events(db, filter, format) -> String`、`export_changes(db, exported_only: bool, format)`；复用 `EventFilter`。
- **接口**：CLI `maintenance export --events|--changes --format csv|json --out <file>`；GUI 历史页「导出当前筛选」。
- **安全**：导出不包含 `env_values.value`；`changes.old_value/new_value` 原样导出（它们本就是同步载荷，不含敏感变量值时为空）。
- **测试**：CSV 转义、时间范围过滤、空结果表头仍存在。
- **规模**：S。**依赖**：无。

### 9. 敏感值访问审计

- **目标**：每次显式查看明文都留下事件（谁、何时、哪个变量），可追溯。
- **数据**：`events.action` 新增 `revealEnv`（字符串），`target` 为变量名，`hive` 照旧，`result` 恒 `success`/`failure`。
- **接线**：三条路径——GUI 实时 reveal、GUI 历史 reveal、CLI `env get`。统一走 `DbHistory` 记录；DB 不可用时 best-effort 跳过（不阻断 reveal）。
- **契约**：Rust `EventAction` + `action_str/from_str`、TS `EventAction` 联合 + `EVENT_ACTIONS`、i18n `history.action.revealEnv`。
- **测试**：三路径各一条；断言不写入值本身。
- **规模**：S–M。**依赖**：无。

### 10. 设备视图

- **目标**：按 `changes.device_id` 聚合设备，显示最近活动、待导出数、未解决冲突数。
- **数据**：`changes(device_id, created_at, exported_at)`、`sync_conflicts(device_id?)`。
- **Core**：`list_devices(db) -> Vec<DeviceSummary>`，本机设备来自 `app_state.device_id` 并标记 `local`。
- **接口**：IPC `sync_devices`；CLI `sync devices [--json]`。
- **UI**：SyncCenter 增加设备列表。
- **规模**：M。**依赖**：无。

### 11. 配置方案对比

- **目标**：选择两个配置方案，查看 PATH/变量差异，避免盲选。
- **数据**：`profiles` + `profile_revisions.snapshot_id`；复用 `compare_paths` 与 #5 的 env 对比。
- **Core**：`compare_profiles(db, a_uid, b_uid) -> ProfileDiff { name_changed, tags_added/removed, system, user, env_added/removed/modified }`。
- **接口**：IPC `profile_diff`；CLI `profile diff <a> <b> [--json]`。
- **UI**：配置对话框「对比」入口，复用对比对话框渲染。
- **规模**：M。**依赖**：#5（env 段）。

### 12. 配置合并改为字段级 LWW

- **目标**：两端同时改同一配置的不同字段时都保留，只有同字段冲突才记冲突。
- **现状**：`profile` 与 `snapshot` 按整实体比较（`latest_for_entity_tx`），后写覆盖整条。
- **设计**：`profile` 拆为 `field` 维度：`name` / `description` / `tags` / `content`（content 内含 snapshot_uid）；`snapshot` 保持整实体（内容不可字段化）。旧包 `field = null` 视为 `content`，按内容比较；新包写明确 field。冲突判定与 `resolve_conflict` 同步扩展。
- **协议**：`BUNDLE_VERSION + 1`；旧包可读（按内容回退），新包在旧客户端报 `SyncFormat`。
- **测试**：两端改不同字段 → 无冲突且都保留；同字段 → 冲突；旧包回退路径；双向导入幂等。
- **规模**：L。**依赖**：#13 同期设计。

### 13. 同步包依赖自动补齐

- **目标**：新设备导入任意一包即可，不再要求「从第一份按顺序应用」。
- **设计**：导出增加 `baseline` 段（导出时刻的当前快照 + 全部未导出变更的依赖实体），导入时若缺前置实体，用 baseline 补齐后再应用变更；包仍是自包含文件，不做网络拉取。
- **协议**：`BUNDLE_VERSION + 1`（与 #12 同一次升级）；旧包无 baseline，仍按原顺序规则导入。
- **测试**：只导入第 3 包 → 成功且实体完整；baseline 与增量重复 → 幂等；篡改 baseline → 整体回滚。
- **规模**：L。**依赖**：#12 共用协议升级。

### 14. 注册表漂移检测

- **目标**：启动时发现「注册表被本工具之外改过」，落一条事件并可一键建快照。
- **数据**：`app_state.current_snapshot` vs 实时注册表；`events.action = drift`。
- **设计**：bootstrap 在建立基线前比较；差异非空时写入 `events`（`source=gui|cli`，`result=partial`），并在 `Snapshots` 建 `kind=drift` 快照（可选，默认只记事件）。CLI `list`/`save` 前也可检测，但首版只做 GUI 启动。
- **契约**：新增 action `drift`（同 D2）。
- **测试**：外部修改 PATH 后启动 → 一条 drift 事件；无修改 → 零事件。
- **规模**：M。**依赖**：#9 的事件契约改动方式。

### 15. 搜索增强

- **目标**：按来源/时间过滤、搜索快照备注、保存常用搜索。
- **数据**：`search_fts` 增加 `snapshotNote` 类型（重建时索引 `snapshots.note`）；搜索过滤参数 `kind`/`from_ms`/`to_ms` 在 SQL 层 join `snapshots` 时间；保存搜索用 `localStorage`（不建表）。
- **Core**：`search(db, query, limit)` 改为 `search(db, SearchQuery { text, kind, from_ms, to_ms, limit })`。
- **UI**：搜索面板增加过滤行与「保存本次搜索」。
- **规模**：M。**依赖**：无。

### 16. 统计扩展

- **目标**：把已有 `daily_stats.env_items`/`conflicts` 暴露为趋势与冲突率，并加「最常冲突的变量」。
- **数据**：`daily_stats`、`events`。
- **Core**：`Dashboard` 增加 `env_items` 序列（`DailyPoint` 已含？补字段）、`conflict_rate`、`top_conflicts`。
- **UI**：统计页新增两张小卡片。
- **规模**：S–M。**依赖**：无。

### 17. 冲突解决引导

- **目标**：并排展示本地/远端实体内容，支持批量采纳/保留。
- **数据**：`sync_conflicts(local_value, remote_value, remote_content)` 已有；批量操作需事务内逐条 `resolve_conflict`。
- **Core**：`conflict_detail(db, id)`（本地实体从当前快照/配置读取，远端用 `remote_content`）；`resolve_many(db, ids, resolution)` 单事务。
- **UI**：冲突列表支持多选、并排 diff、批量按钮。
- **规模**：M。**依赖**：无（#10 之后体验更完整）。

### 18. 规则引擎

- **目标**：用数据库保存「整理规则」与命中历史（如「Python 路径置顶」）。
- **数据（v5）**：`rules(id, uid, name, enabled, priority, condition_json, action_json, created_at, updated_at)` + `rule_hits(rule_id, event_uid, hit_at)` + 索引。
- **Core**：规则求值（纯函数）+ 在保存编排前应用；命中写 `rule_hits`。
- **接口**：CLI `rules list/add/edit/remove/test`；GUI 规则编辑器。
- **测试**：条件匹配/不匹配、优先级、禁用、命中记录。
- **规模**：L。**依赖**：#4（同一 v5 迁移），建议单独 spec。

## 五、批次与依赖

```mermaid
flowchart LR
    A["批次 A：接线<br/>3 → 9 → 8 → 2 → 1"] --> B["批次 B：对比与治理<br/>5 → 16 → 4(v5) → 6"]
    B --> C["批次 C：检索/漂移/健康<br/>15 → 14 → 7"]
    C --> D["批次 D：同步<br/>10 → 17 → 11 → 12 → 13"]
    D --> E["批次 E：规则引擎<br/>18 (v5)"]
```

- 批次 A 全部零 schema；A 内的顺序按「先小后大」：3（已完成）→ 9 → 8 → 2 → 1。
- 批次 B 的 v5 迁移与 #4 一起落；#6 依赖 pinned。
- 批次 D 的 12/13 共用一次 bundle 版本升级，必须同批发布。
- 批次 E 单独 spec，不复用本文件的实现细节。

## 六、非目标

- 不做服务器端同步、账号体系与远程 Web 面板。
- 不引入 ORM 或新数据库引擎。
- 不把环境变量值写入 FTS 或任何导出文件。
- 不为 `migration_imports` 补功能（v5 迁移遗留，保留表）。

## 七、风险

| 编号 | 风险                                           | 缓解                                                           |
| ---- | ---------------------------------------------- | -------------------------------------------------------------- |
| R1   | 新事件动作漏改 TS 白名单导致历史页整体解析失败 | D2：同一提交双端改 + `ipc-contract` 类测试                     |
| R2   | 保留策略误删被引用快照                         | 删除前用引用子查询保护；测试覆盖 profile/backup/pinned/current |
| R3   | 同步协议升级让旧客户端读新包                   | 旧端明确 `SyncFormat`；新端保留旧包读取测试                    |
| R4   | 扫描/备份对账引入大目录 IO 卡顿                | 只读目录一层；上限 200 条；失败降级                            |
| R5   | 规则引擎范围失控                               | 单独 spec；首版只支持「路径排序/剔除」两类动作                 |
