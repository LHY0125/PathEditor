# V7 数据库能力扩展 Implementation Plan

> 对应设计：`docs/superpowers/specs/2026-11-08-v7-db-program-design.md`。
> 每批独立可发布；批次内按列出的顺序实施。

**Goal:** 把 18 项数据库相关能力按「先接线、再治理、后协议、最后大件」的顺序落地，每批都能单独过质量门并交付。

**Architecture:** 优先复用既有表与 core API；只有 #4 与 #18 需要 schema v5。事件动作扩展（#9/#14）必须前后端同一提交。同步协议（#12/#13）单独升级 bundle 版本。

## 一、批次总表

| 批次 | 内容                   | 前置 | 特点                          |
| ---- | ---------------------- | ---- | ----------------------------- |
| A    | 3 → 9 → 8 → 2 → 1      | 无   | 零 schema，先让既有表产生价值 |
| B    | 5 → 16 → 4（v5）→ 6    | A    | 对比与数据治理；v5 首次迁移   |
| C    | 15 → 14 → 7            | B    | 检索、漂移、健康              |
| D    | 10 → 17 → 11 → 12 → 13 | C    | 同步与配置；协议升级          |
| E    | 18（v5）               | D    | 规则引擎，单独 spec           |

依赖关系：`#6 → #4`；`#11 → #5`；`#12 ↔ #13`（同一次 bundle 升级）；其余无硬依赖。

## 二、Batch A（接线，零 schema）

### A1 `#3` CLI 写后派生表刷新 — **已完成**

- [x] `runtime::refresh_derived_if_stale()`：`open_existing` 静默打开 → `migrate_to_current` → `derived::rebuild_if_stale`
- [x] 水位路径改用 `data_home()`，随 `PATHEDITOR_DATA_DIR` 隔离测试
- [x] `main` 收尾调用（广播 flush 之后）
- [x] 回归测试 `derived_refresh_rebuilds_only_when_watermark_changes`
- 验收：`cargo test -p patheditor-cli --bins` 73 passed

### A2 `#9` 敏感值访问审计 — **已完成**

- [x] `EventAction::RevealEnv` + `action_str/from_str`（落库字符串 `revealEnv`）
- [x] `db::history_sink::record_reveal`：只写 hive/变量名/结果，`changed_fields` 恒 `[]`
- [x] 三条路径接入：CLI `env get`、GUI 实时 `reveal_env_var`、GUI 历史 `history_reveal_env_value`；全部 best-effort
- [x] TS `EventAction` 联合与 `EVENT_ACTIONS` 白名单、中英文 i18n
- [x] 回归测试：core `reveal_audit_records_action_without_value`、CLI `env_get_writes_reveal_audit_event`、前端 `接受 revealEnv 动作`
- 验收：Rust 340 passed / 2 ignored；前端 40 files / 427 passed，覆盖率行 81.4%

### A3 `#8` 审计导出 — **已完成**

- [x] core `db::audit_export`：`export_events`（复用 `EVENT_WHERE`）/ `export_changes`，CSV 与 JSON，单次上限 1 万行
- [x] CLI `maintenance export --events|--changes --format csv|json --out FILE --since/--until`
- [x] IPC `history_export_events` + 前端 `backend.historyExportEvents`；历史面板「导出 CSV」按钮与 `revealEnv` 筛选项
- [x] 回归测试：core `export_events_csv_and_json_respect_filter`、`export_changes_csv_and_json_include_pending_rows`
- 验收：Rust 342 passed / 2 ignored；前端 40 files / 427 passed，覆盖率行 81.1%

### A4 `#2` 备份索引对账 — **已完成**

- [x] core `backup_index::reconcile_backups` / `purge_missing`（只标记 `deleted_at`，不删磁盘文件）
- [x] CLI `maintenance reconcile-backups [--json] [--purge-missing]`；冒烟通过
- [x] IPC `backup_reconcile` / `backup_purge_missing`；备份面板增加「检查索引 / 标记缺失为已删除」
- [x] 回归测试 `reconcile_reports_missing_and_orphans`（缺失/孤儿/幂等/不删文件）
- 验收：Rust 343 passed / 2 ignored；前端 427 passed

### A5 `#1` 扫描历史与差异 — **已完成**

- [x] CLI `conflicts` / `scan` 在 DB 可用时调用 `record_run`（best-effort，失败只告警）；新增 `scan-history` / `scan-diff`
- [x] GUI `scan_paths` 记录本次扫描（冲突名 + 工具目录）；新增 IPC `scan_history_list` / `scan_history_diff`
- [x] 分析对话框新增「扫描历史」页签：选两次扫描查看 appeared / disappeared / persistent
- [x] 前端契约：`parseScanRuns` / `parseScanRunDiff` + 形状测试
- [x] CLI 冒烟：`scan` → `conflicts` → `scan-history` → `scan-diff`（临时 `PATHEDITOR_DATA_DIR`）
- 验收：Rust 343 passed / 2 ignored；前端 40 files / 432 passed，覆盖率行 80.6%

## 三、Batch B（对比与治理）

### B1 `#5` 环境变量对比 — **已完成**

- `CompareResult` 填 `envAdded/envRemoved/envModified` + `envDetails`（hive/name/change，均不含值）；`modified` 按 `revision` 判定，敏感值掩码下同样有效。
- 前端契约与解析；对比对话框新增环境变量段（无变化时显式提示）。
- 测试：core `compare_reports_env_changes_without_values`（断言 JSON 不含值）；前端 parser + 对话框用例。

### B2 `#16` 统计扩展 — **已完成**

- `Dashboard` 新增 `scanRuns` / `conflictRate` / `topConflicts`（最近 200 次扫描聚合）。
- 统计页新增「最常出现的冲突」卡片与扫描占比。
- 测试：core `dashboard_reports_scan_conflict_stats`；前端 parser 兼容旧契约（缺字段按 0 处理）。

### B3 `#4` 快照备注与固定（**v5 part 1**） — **已完成**

- v5 迁移：`ALTER TABLE snapshots ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0` + 索引；`note` 直接复用。
- Core/IPC/CLI/HistoryPanel；备注上限 500 字符（按字符计，两端空白裁剪，全空白清空）。
- 实现：`set_snapshot_note` / `set_snapshot_pinned` / `list_pinned`；IPC `history_set_note|set_pinned|list_pinned`；CLI `history-note|history-pin|history-pinned`；SnapshotDetail 增加备注输入与固定按钮。
- 测试：迁移 v2→v5（默认未固定）、备注/固定 roundtrip 与 NotFound/超长、前端契约缺省兼容。

### B4 `#6` 保留策略扩展 — **已完成**

- 配置项 `snapshot_keep_days/max`、`event_keep_days`；删除保护：current/pinned/profile_revision/backups 引用。
- `maintenance vacuum`：`wal_checkpoint(TRUNCATE)` + 可选 `VACUUM`。
- 实现：`SNAPSHOT_PROTECTED` 保护谓词（pinned/current/profile_revisions/backups）；到期 + 超限两条规则；删除前把 `events.snapshot_id` 解链（审计事件保留）；`prune_plan` 用 UNION 去重；`vacuum()` + `maintenance vacuum`。
- 默认值：快照 365 天 / 500 条，事件 730 天（`config.ini` 同名键可覆盖）。
- 验收：保护项不删、级联干净、`integrity_check` 通过（`db_retention` 集成测试覆盖）。

## 四、Batch C（检索/漂移/健康）

### C1 `#15` 搜索增强 — **未实施（按需再做）**

- 决策：FTS 已可用，备注搜索属锦上添花；等真实检索需求出现再排期。
- `SearchQuery` 结构体；`snapshotNote` 索引类型；保存搜索用 `localStorage`。

### C2 `#14` 漂移检测 — **已完成**

- `EventAction::Drift` + `drift` 字符串映射；TS 联合/白名单/i18n/历史筛选项同步。
- `bootstrap::record_drift`：逐项比较顺序、启用状态与路径（Windows 大小写不敏感），只记事件、不建快照。
- 去重：实时状态哈希存 `events.before_revision`，同一外部状态重复启动只记一次。
- 事件 `result=partial`、`target=` 命中 hive、`snapshot_uid` 指向漂移前的 current 快照。
- 测试：`db_drift` 覆盖无差异/大小写/新增/重复/另一 hive/无基线。

### C3 `#7` 健康面板与迁移前备份 — **已完成**

- `db::health::health`：integrity_check、schema、9 张表行数、主库/WAL/SHM 体积。
- 迁移前用 SQLite Online Backup API 生成 `<db>.pre-v{n}.bak`，保留最近 3 份；备份失败中止迁移。
- `maintenance.json` 记录 `lastPruneAt`（`prune_now` / CLI 实删后写入），避免升 schema。
- 接口：CLI `maintenance health [--json]`、IPC `db_health`、统计页健康卡片。
- 测试：`db_migration_backup`（每版备份、轮换到 3 份、全新库不备份）、`db_health`（行数/完整性/元数据 roundtrip）。

## 五、Batch D（同步与配置）

### D1 `#10` 设备视图 → D2 `#17` 冲突引导 → D3 `#11` 配置对比

- 顺序按「读 → 操作 → 比较」；#11 复用 #5 的 env 段。

### D4 `#12` 字段级 LWW + D5 `#13` 依赖补齐（同一次 bundle 升级）

- 先做 #13 的 self-contained baseline（纯增量补齐），再切 #12 的字段维度；两者共用 `BUNDLE_VERSION + 1`。
- 旧包读取测试必须保留；新包在旧端报 `SyncFormat`。

## 六、Batch E（规则引擎）

- 单独 spec + plan；首版只支持「路径排序」「路径剔除」两类动作；v5 part 2（`rules`/`rule_hits`）。

## 七、每批质量门

```powershell
npm run verify        # 含一致性检查、前端覆盖率、Rust 全量
npm run test:e2e      # 涉及 UI 的批次必须跑
```

- 新增 IPC：更新 `ipc-contract.test.ts` 的命令集合不会自动覆盖新命令——新增命令后跑该测试确认无死命令。
- 新增事件动作：Rust/TS 双端 + i18n + fixture，缺一即红。
- 新增 schema：`CURRENT_SCHEMA_VERSION` 递增、补迁移测试、迁移前备份（#7 之后自动）。
- 新基线数字只改 `docs/dev/testing.md`，其余文档引用。

## 八、风险与回滚

| 风险                           | 回滚                                                                  |
| ------------------------------ | --------------------------------------------------------------------- |
| v5 迁移失败                    | 迁移在事务内；失败不写 `user_version`；#7 的 `.pre-v5.bak` 可手工替换 |
| 保留策略误删                   | 默认保守阈值；`maintenance prune --dry-run` 先看计划                  |
| bundle 升级不兼容              | 新版本保留旧包读取；发布说明标注「两端需同时升级」                    |
| 事件动作扩展导致历史页解析失败 | 双端同提交；CI 的 `ipc-contract` 与前端单测兜底                       |

## 九、未决事项

- #6 的默认保留阈值是否按「天数」还是「条数」优先，需在 B4 前用真实库体积估算。
- #12/#13 的协议版本号是否合并为一次 `BUNDLE_VERSION = 3`，实施前再确认。
- #18 是否允许用户导入/导出规则 JSON，单独 spec 决定。
