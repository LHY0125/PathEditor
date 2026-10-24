# Changelog

## 未发布

### 修复

- GUI「合并预览」的所有行叠在 y=0，渲染成一片重影；虚拟列表现在为每行写入行高与 `translateY`，并新增回归测试。
- 原生窗口标题硬编码为 `PathEditor v6.0.0`，与实际版本不一致；标题改为只显示产品名，版本由应用内标题栏按 `package.json` 渲染。
- 所有对话框补上 `role="dialog"`、`aria-modal` 与无障碍名称，打开时焦点移入、Tab 在弹窗内循环、关闭后归还触发元素；模态框打开期间全局快捷键暂停，避免 `Delete` 误删主界面选中项。
- 新增 `--app-warning-fg` / `--app-warning-text` 语义 token：警告底文字浅色 3.19:1 → 5.56:1、深色 1.76:1 → 10.05:1；V6 面板中 24 处 `--app-fg-faint` 正文改用 `--app-fg-muted`，达到 WCAG AA。

### 改进

- 状态栏错误判定改为结构化 `statusIsError`，不再匹配本地化文案；状态文案补 `role="status"` 与 `aria-live`。
- V6 面板文案全部走 i18n：统计 / 历史 / 搜索 / 同步 / 引用图 / 备份摘要 / 变量名校验；错误文案统一经 `core/error.ts` 提取，消除 `[object Object]`。
- 搜索命中可复制完整内容，并能在当前 PATH / 变量中定位（历史快照中的条目会禁用定位并说明原因）；历史时间线支持「加载更多」；同步中心接入原生文件选择器。
- 标签页、筛选胶囊、配置列表与分析对话框补齐 ARIA 语义（`role=tab` / `aria-selected` / `aria-pressed`）；同步中心改用设计系统类与 `Button` 原语。
- PATH 文本、环境变量单元格与等宽片段允许选中复制。
- `SearchHit` 新增 `content` 字段（完整索引内容，不含环境变量值），供前端定位与复制。
- 移除 `--app-fg-subtle` / `--app-fg-faint` 的全部文字用法（4.02:1 / 2.46:1），菜单快捷键、占位符、备份面板与配置面板统一改用 `--app-fg-muted`。
- PATH / 环境变量表增加空结果提示；配置详情用 `--app-disabled-fg` 表示禁用路径，不再误用危险红。
- 对比选择第三条时明确提示「最早选择已取消」；配置新建/重命名输入框补 `aria-label`。
- 同步导出/导入显示「导出中… / 导入中…」；备份恢复执行期间 Modal 不再响应遮罩点击与 Escape。

## 6.1.0 (2026-10-23)

### 重构与清理

- **目录整合**：源码统一到 `src/{core,cli,gui}`，前端位于 `src/gui/web`； Cargo workspace、Vite/Vitest/ESLint/Prettier/Playwright、CI 与文档路径同步更新。
- **移除 v5 过渡层**：迁移、归档、导出/导入、`disabled.json`、全局 loader/flag、 v5 CLI/IPC 与 golden 用例全部删除，DB 成为唯一持久化状态。
- **保存编排统一**：CLI 与 GUI 共用 `service::save_path_with_db`； CLI 打开 DB 失败时才走 `save_path_registry_only`。
- **错误模型统一**：`registry/path.rs`、`scanner.rs`、`validate_env_*` 与 `backup_registry` 全部返回 `CoreError`；删除 `From<String>` 隐式降级。
- **结构拆分**：`backend.ts` 拆为 `backend/{index,parse-core,parse-data}`； `backup.rs` 拆为 `config/env/path/restore`；CLI 的 clap 定义与命令实现分离； GUI 命令按领域拆分。
- **注释清理**：生产代码与测试中的里程碑、工单号、计划任务号全部移除，只保留解释「做什么 / 为什么」的注释。
- **测试隔离**：删除 `test_persist_lock`，pending 路径与备份目录改为显式注入，测试统一使用 `TempDir`，可并行运行。
- **派生表水位**：`daily_stats` / `var_references` 按事件/快照的计数与最大 id 判断是否需要重建，水位未变时跳过。

### 修复

- CLI 保存写入 DB 当前快照与 `changes`（此前只写注册表，跨设备同步看不到 CLI 修改）。
- GUI 每次读取 DB 当前快照，不再使用进程级缓存。
- 保存结果全失败时记为 `Failure`，不再误报 `Partial`。
- PATH 命令错误从纯文本改为结构化 `CoreError`，前端按 `code` 判定。

### 测试

- 新增 IPC 契约差集测试：`invoke_handler` 与 `backend/` 的命令集合双向一致。
- 新增派生表水位集成测试。
- 基线：Rust 320 passed / 2 ignored；前端 34 files / 399；E2E 54。

### 说明

- **v5 数据不再自动迁移**：从 v5 直接升级到 6.1.0 的用户不会读到 `disabled.json` / `profiles/*.json`；从 6.0.0 升级不受影响。
- PATH 相关命令的 rejection 从字符串变为对象；`parseCoreError` 同时兼容两种形状。

## 6.0.0 (2026-10-22)

### M4 配置同步、清理与发布

M3 只落 `snapshot`；M4 把配置（`profile` / `profile_revision`）真正接进导入管线，
并完成保留策略、v5 原件归档、遗留明文备份清理与 v5 配置导入。

#### 新增

- **配置同步落库**（`core/src/db/sync/import.rs`、`core/src/db/profiles.rs`）： `profile` 的 `content` 按远端 uid 落库，`snapshotUid` 解析成本地快照主键。导入前按 `snapshot → profile → profile_revision → path_entry → env_value` **稳定排序**，包内顺序颠倒也能正确解析依赖。
- **同名自动改名**：与本地未删除配置同名时改为 `"{name} (来自 {设备前 8 位})"`，仍冲突则追加 ` 2`、` 3`…。改名只影响本地行，包内 uid 不变； `ImportReport.renamed_profiles` 计数并在 UI/CLI 明示。
- **墓碑软删除**：`content = null` **且** `field = "deleted_at"` 时软删除 `profiles` / `snapshots`，名称随之释放。
- **保留策略**（`core/src/db/maintenance.rs`）：删除「已导出超过 90 天」的 `changes` 与「已解决超过 30 天」的 `sync_conflicts`。GUI 每次启动执行一次，失败只告警；CLI 提供 `maintenance prune [--dry-run]`。
- **v5 原件归档**（`core/src/db/archive_v5.rs`）：迁移零失败时把 `disabled.json` 与 `profiles/*.json` 改名为 `.v5.bak`，目标已存在则跳过，**绝不覆盖**。
- **遗留明文备份清理**：`env_backup_YYYYMMDD_HHMMSS_mmm.json` 先转成 DPAPI 加密的 `.pebak` 并**回读校验**，成功后才删除明文；CLI 必须 `--yes`。
- **v5 配置导入**：`patheditor profile import <FILE>`，从 `profiles/*.json` 读回 DB。
- **维护命令**：`patheditor maintenance archive-v5 [--dry-run]`、 `maintenance cleanup-legacy-backups [--dry-run] [--yes]`。

#### 变更

- 同步导入的**实体级 LWW**：`snapshot` / `profile` 取该实体最新一条变更（不限 field），否则墓碑（`field = "deleted_at"`）与内容变更（`field = null`）互不可见，已删除的配置会被旧内容「复活」。
- 已知实体类型**全部可落库**，`skippedUnsupported` 对它们恒为 0。

#### 修复

- 墓碑不会被低 lamport 的旧内容覆盖（实体级 LWW 的直接结果）。

### M3 跨设备同步

在 M1/M2 的加密存储之上实现**文件式**跨设备同步：变更日志、加密同步包、
字段级 LWW 合并、冲突记录与同步中心 UI。**同步不写注册表**——导入只改数据库，
用户要生效必须另行应用配置。

#### 新增

- **schema v3**（`core/src/db/schema.rs`）：`changes`（变更日志，含 `device_id`/`seq`/`lamport`/`origin` 与三条索引）与 `sync_conflicts`（冲突记录）。 `changes` 是**真相**而非派生表，不能重建。
- **变更日志**（`core/src/db/sync/change_log.rs`）：`record_change_tx` 在业务事务内写入，`seq` 按设备取 `MAX+1`，`lamport` 与之同源推进并写回 `app_state`。 `pending_export` / `mark_exported` 支撑导出队列。
- **写路径接线（S-1）**：`save_profile_tx` / `delete_profile` / `rename_profile` / `insert_snapshot_tx` 各自在**自己的事务内**记变更。 **`EventSource::Migration` 不记**——v5 迁移与首次启动建基线不该进入同步队列。
- **同步包加密**（`core/src/db/sync/bundle.rs`）：`PESYNC1` 格式， Argon2id（m=65536, t=3, p=1）派生 + ChaCha20-Poly1305。 **AAD 覆盖 header**（magic / version / header_len / header JSON / salt / nonce），否则 header 可被篡改而不被发现。
- **导出**（`core/src/db/sync/export.rs`）：收集 → 加密 → 写文件 → **成功后**才 `mark_exported`。顺序反了会在写失败时永久丢失变更。
- **导入**（`core/src/db/sync/import.rs`）：单事务、按 `changes.uid` 幂等、实体类型白名单、与本地比较做 LWW、导入后把本地 lamport 提到至少导出方的值。
- **LWW 合并**（`core/src/db/sync/merge.rs`）：纯函数；`lamport` 高者胜，相等时同设备视为同源、不同设备且值相同按 `device_id` 字典序确定性取一个、值不同才记 `Conflict`。
- **冲突处理**（`core/src/db/sync/conflict.rs`）：列表 / 未解决计数 / 解决。解决时**追加一条 lamport 更高的变更**，否则第三台设备会按原 lamport 重新判一次。
- **退出码拆分**：`Sync`（合并冲突）→ 4；`SyncFormat`（包格式/口令/篡改）→ 1。拆开的原因是脚本要能区分「需要人工介入」与「重试无用」。
- **CLI**：`patheditor sync export|import|status|conflicts|resolve`。口令用 `rpassword` 隐藏输入、两次确认、不落盘、不进 shell 历史。
- **同步中心 UI**：密码输入不写入 store、冲突按实体类型分组、界面明示「导入不改注册表」。

#### 说明

- 同步包只搬运 `profiles` / `profile_revisions` / `snapshots` / `path_entries` / `env_values`；**`events` 不同步**（本机操作日志，不是用户数据）， `backups` 索引与派生表也不同步。
- ~~当前版本**实体落库只覆盖快照**（`snapshots` + `path_entries` + `env_values`）；配置（`profiles` / `profile_revisions`）的落库尚未接线，属 M3 已知未覆盖项。~~ **M4 已闭环**：配置与 revision 现在随包落库，见上文 M4「配置同步落库」。

### M2 检索与统计

在 M1 的加密存储之上补齐 Tier 2 能力：FTS5 全文检索、统计仪表盘、N 路快照对比、
`%VAR%` 引用图、冲突历史持久化，并清偿 M1 移交的 4 项欠账。

#### 新增

- **schema v2**（`core/src/db/schema.rs`）：`scan_runs` / `var_references` / `daily_stats` / `search_fts`（FTS5 虚表）。`var_references` 与 `daily_stats` / `search_fts` 是**派生表**，可随时重建（spec §4.2 第 5 条）；`scan_runs` 是真实记录，无法重建。 **顺带修复 M1 的一处升级缺陷**：`PRAGMA user_version` 原写在 `if current < 1` 内部， v1 库升级到 v2 会执行 DDL 但不更新版本号，导致下次启动重复建表报错。
- **FTS5 检索**（`core/src/db/search.rs`）：`rebuild_index` / `index_snapshot_tx` / `search`。只索引 PATH 串、变量名、配置 name/description/tags，**绝不索引 env 的值**。查询串在 core 侧做 FTS5 phrase 转义（裸 `"` 不再触发 `syntax error`）。
- **统计聚合**（`core/src/db/stats.rs`）：`rebuild_daily` / `dashboard`。 `dashboard` 返回历史序列 + 当前状态 + 最常改变量 + 配置应用次数。
- **引用图**（`core/src/db/refs.rs`）：`parse_refs` / `rebuild_refs` / `dependents` / `dependencies`（递归 CTE）/ `find_cycles`。环做了旋转归一化去重。
- **快照对比**（`core/src/db/compare.rs`）：`compare` 返回**双 hive** `{ system, user }` + 恒为空的 `env_*` 三字段。
- **冲突历史**（`core/src/db/scan_history.rs`）：`record_run` / `list_runs` / `diff_runs`，支持「首次出现」与「最近出现」两种语义。
- **v5 配置导出**（`core/src/db/v5_export.rs` + `profile export-v5`）：降级到 v5 文件的退路。
- **`profiles::load_profile_from(path)`**：从指定路径读配置，供集成测试与 v5 导入使用。解析逻辑复用既有的 `read_profile_file`，**没有第二份实现**。
- **`history_reveal_env_value`**：显式 reveal 历史快照里的单个变量明文。掩码是默认；明文只存在组件局部 state，不写入 store。
- **IPC**：`search` / `stats_dashboard` / `compare_snapshots` / `refs_*` / `scan_history_*` 共 12 条命令 + `backend.ts` 的 7 个 `parseXxx` 形状校验。
- **前端**：新增「搜索」「统计」标签页；历史面板支持多选对比；引用图对话框展示反向/前向/环三个方向。

#### 变更

- **对比结果分 hive**：`CompareResult` 从「只比 system」改为 `{ system, user }`。只比一侧会让「只有用户 PATH 变了」显示成「没有差异」——不报错、给出错误结论。
- **对比增加启用状态维度**：`PathDiff.toggled` 记录「同一条路径从启用变禁用」。 `PathEntry { path, enabled }` 是跨层契约，只比路径串会漏掉这类真实变化。

#### 测试约定

- 新增 12 个 DB 测试文件，仍遵守**一文件一测试**（SQLCipher 栈溢出约束）。

#### 已知限制

- **环境变量的 N 路对比未实现**：`env_*` 三字段恒为空数组，界面上显式显示「将在 M3 提供」。掩码值在对比结果里如何呈现需要单独设计。
- **CLI 不重建派生表**：用 CLI 改过 PATH 后，GUI 的引用图与统计可能仍是旧值，重启 GUI 后对齐。修复需要 CLI 也接 DB 的 current 快照，属范围扩张。
- **`index_snapshot_tx` 尚未接入写入路径**：启动时按需全量重建，增量函数已实现并有测试，但 `save_path_with_sidecar` 的写入链路尚未调用它。

### M1 存储与历史

在 core 内建立 **SQLCipher 加密数据库**，把 PATH 快照与操作事件持久化，并把配置方案与备份索引迁入 DB。**注册表仍是唯一真相源**，DB 是 sidecar（spec D16 fail-open）。

> ⚠️ **本里程碑不含 FTS5 检索、统计仪表盘、N 路对比、引用图与跨设备同步** —— 那些属于 M2/M3。

#### 新增

- **`HistorySink` 服务接缝**（`core/src/history.rs`）：`save_path_with_sidecar` / `apply_path_snapshot` / `retry_pending_path_state` / `apply_profile` 增加 `history: &dyn HistorySink` 参数。测试与无 DB 场景注入 `NoopHistory`，生产注入 `DbHistory`。**不要求 `Send + Sync`**（M1 为同步单线程调用；跨线程留到 M3）。
- **数据库句柄**（`core/src/db/mod.rs`）：`Db::open_encrypted` / `with_transaction`（`BEGIN IMMEDIATE`）/ `schema_version` / `migrate_to_current` / `open_default`。启动只读 `user_version`，只有建库/迁移事务内写。
- **schema v1**（`core/src/db/schema.rs`）：`snapshots` / `app_state` / `path_entries` / `env_values` / `events` / `profiles` / `profile_revisions` / `backups` / `migration_imports`，与 spec §4.1 逐表对应。
- **DPAPI 密钥生命周期**（`core/src/db/crypto.rs`）：`<home>/keys/db.key.dpapi`，内容为 `version + db_key(32) + hash_salt(16)` 的 DPAPI 密文。首次创建用命名互斥量 `Local\PathEditor.KeyInit` + `create_new(true)` 原子创建，**不使用「先 exists() 再写」**。
- **快照与事件读写**（`core/src/db/history.rs`）：`insert_snapshot` / `insert_snapshot_tx` / `list_events`（独立 `count(*)` 提供 `total`）/ `get_snapshot`（`reveal=false` 掩码）/ `current_snapshot` / `set_current_snapshot`（`version` CAS）。
- **配置方案入 DB**（`core/src/db/profiles.rs`）：标签/描述/应用次数/最后应用时间；软删除后允许同名重建（部分唯一索引）。
- **`.pebak` 加密备份**（`core/src/db/backups.rs`）：55 字节明文头（magic + version + 时间 + 变量数 + 明文长度 + `payload_hash`）+ DPAPI 加密正文。**列表与索引只读头，不解密**。遗留 `.json` 仍可一次性读取（带明文警告）。
- **v5 迁移**（`core/src/db/migrate_v5.rs`）：`disabled.json` / `profiles/*.json` / `env_backup_*.json` / `path_backup_*.txt` 一次性导入；`migration_imports(source_path, content_hash)` 保证幂等；**原件只读、不改名、不删除**。
- **IPC 命令**：`history_list_events` / `history_get_snapshot` / `history_get_current` / `profile_list` / `profile_delete` / `profile_mark_applied` / `backup_list`；DB 不可用时返回 `code = database`。
- **前端**：`backend.ts` 新增对应的 `parseXxx` 形状校验；`history-store.ts` + `HistoryPanel` + `SnapshotDetail`（敏感值默认掩码）；新增「历史记录」标签页。

#### 变更

- **`ErrorCode` 新增** `Database` / `Crypto` / `Sync` / `Migration`（serde camelCase）。前端 `ErrorCode` 联合类型与 `parseCoreError` 白名单**同步扩充** —— 否则降级模式的判定会被静默兜底成 `internal`。
- **CLI 退出码**：`Crypto` 映射为 **5**（spec §8）；`Conflict` 仍为 3；4 预留给 M3 的同步冲突。
- **`cipher_memory_security` 默认 OFF**：M0 实测 ON 在受限进程里 `VirtualLock` 失败，且 ON/OFF 性能无显著差异（902 µs vs 931 µs）。

#### 测试约定

- **一文件一测试**：同一进程内跑第二个 SQLCipher 测试会 `STATUS_STACK_OVERFLOW`（GNU/MSVC 均已复现，`--test-threads=1` 无效）。DB 测试文件各含**唯一**一个 `#[test]`。

#### 已知限制

- `profile_save` 的 IPC 命令未实现（需要同时传 PATH 条目 + 元数据，形状较重）—— 配置写入在 M1 仍走 v5 文件通路 + 迁移。
- 历史面板只做「列表 + 预览」；恢复按钮、筛选、分页与 reveal 入口留到 M2。
- 事件与快照的关联**字段已就位、链路已通，但服务层不填值**：`EventView` 含 `snapshotUid`， `events.snapshot_id` 也会在记录时解析写入；但 4 个写命令当前传入的事件都不带快照 uid （`save_path_with_sidecar` 不知道 DB 存在），所以该字段实际恒为 `null`，历史面板的详情按钮因此为禁用状态。M2 接线。

## 5.1.7 (2026-10-17)

本版重做 **GUI 视觉与信息架构**：工具栏从「两行 18 个同权重按钮」收口为**单行分层工具栏 + ⋯ 菜单**，浅色主题改为中性现代、深色主题改为玻璃质感，并建立单一的设计 token 层。**Rust / CLI / IPC 契约零改动**，仅前端。

### 新增

- **设计 token 层**（`src/styles/globals.css`）：18 个语义变量（背景/表面/边框/文字/强调/语义色/浮层）+ 5 个几何变量（圆角 8px、行高 28px、间距 6px），两套主题共用同一骨架，仅材质分化。
- **按钮原语**：`buttonVariants` 提供 `primary` / `secondary` / `ghost` / `danger` 四个变体；`btnClass` / `btnStyle` 保留为过渡别名。
- **无依赖下拉菜单** `Menu`：受控开合、点击外部关闭、`Escape` 关闭并交还焦点、方向键 / `Home` / `End` 导航（跳过禁用项、两端循环）、`role="menu"` / `role="menuitem"`、`aria-haspopup` / `aria-expanded` / `aria-controls`。
- **内联 SVG 图标集**（零依赖）。当前生产代码仅使用 `MoreIcon`，其余五个为后续波次预留并已标注。
- **800px 视觉回归测试**：在两种尺寸 × 两种主题 × 中英双语下检测工具栏是否被裁切，并附**可证伪守卫**（注入超宽内容必须失败）。
- **菜单遮挡回归测试**：用 `elementFromPoint` 命中测试检测菜单项是否被祖先裁切（`isVisible()` / `click()` 都不检测这一点）。

### 变更

- **工具栏单行分层**：`搜索 │ 新建 编辑 删除 │ 上移 下移 │ 撤销 重做 ...推... 导入 导出 │ 保存 │ ⋯`。保存按钮恒有强调色。
- **低频操作收入 ⋯ 菜单**：浏览…、清理无效路径、冲突分析、配置方案（维护组）；深色/浅色模式、切换语言、帮助（应用组）；取消并关闭（底部，分隔线隔开）。
- **浅色主题**：中性现代（`#fafafa` 背景、细边框、靛蓝 `#6366f1`）。
- **深色主题**：玻璃质感（深色渐变背景、半透明表面、提亮强调色 `#7c6cff` + 辉光、菜单 `backdrop-filter: blur(14px)`）。
- **表格行**交替底色改由**逻辑索引**驱动（`data-odd`），不再使用 `:nth-child`——虚拟滚动下后者会在滚动时闪烁。
- **英文工具栏文案简写**：`Move Up` / `Move Down` → `Up` / `Down`，使 800px 最窄窗口下不换行。
- **搜索框在窄窗口弹性收缩**（`flex: 0 1 160px; min-width: 96px`），空间充足时仍为 160px。

### 修复

- **⋯ 菜单被工具栏裁切**：`.toolbar { overflow: hidden }` 会裁掉其子元素，菜单原先作为工具栏后代只显示第 1 项。改用 `createPortal` 渲染到 `document.body` + `position: fixed` 按触发按钮定位（下方空间不足时翻到上方，窗口 resize / 滚动时重算）。`overflow: hidden` 保留。
- **语言切换对区域变体失效**：原先读 `localStorage['i18nextLng']` 与字面量 `'zh-CN'` 比较，`zh-TW` 等变体下切换无效。改按 `i18n.resolvedLanguage ?? i18n.language` 判 `startsWith('zh')`。
- **禁用按钮被 hover 样式覆盖**：Tailwind 的 `disabled:opacity-40` 位于 `@layer utilities`，而 `.btn-*` 规则未分层、优先级更高。新增显式 `:disabled` 规则（含 `box-shadow: none`，避免深色下辉光残留）。
- **撤销/重做按钮未迁移**：仍用旧的 `btnClass`（`py-1`，50px 高），把工具栏撑到 67px 并换行。迁移至 `Button` 后回到 28px。
- **表格行纵向节奏丢失**：`.path-text` 只有水平 padding，内容在 28px 行内偏移。补纵向 padding。
- **淡色文字对比度不足**：菜单分组标题、快捷键提示、禁用项曾用 `--app-fg-faint`（2.46:1 / 3.27:1），低于 WCAG AA。改用 `--app-fg-muted`，并为快捷键提示新增 `--app-fg-subtle`（4.02:1 / 5.99:1）以保留与标签的层级差异。
- **`index.html` 标题长期停留在 `v5.1.2`**：它是静态文件，读不到 `package.json`，也不在版本清单内。改为 Vite `transformIndexHtml` 注入 `%APP_VERSION%`，**从此不再是手工版本源**。

### 说明

- **`tests/setup.ts` 注册 jest-dom**：`@testing-library/jest-dom` 早已在 devDependencies 却从未接线，匹配器在运行时为空。同时全局修复了 RTL 自动 cleanup 缺失（未开 `vitest.globals`）与测试语言锁定为 `zh-CN`。
- **未保存状态指示改由状态栏承担**：原设计在保存按钮内加实心圆点，但按钮内部只有强调底色与前景文字两色，任何标记都会与其中之一同色而不可辨。信号未丢失——状态栏本就有 `● 未保存`。
- **过渡别名保留至 Wave 4.1**：`--app-list-bg` / `--app-list-alt` / `--app-select-row` 仍被 `src/components/dialogs/` 的 7 个文件引用（14 处）。本波**未重做对话框**，其视觉与主界面暂不一致，将在 Wave 4.1 一并对齐并清理别名。
- **`body` 字号由 14px 调整为 13px**，会影响对话框内继承该值的文字。
- 已知未覆盖：窗口缩放到 150%/200% 时的布局未验证；工具栏的 800px 约束由 E2E 断言保障。

## 5.1.6 (2026-10-17)

本版完成 **F-08 收口**：把散在 GUI（TypeScript）与 CLI 里的两份「保存 PATH + sidecar」编排收敛为 `core::service::save_path_with_sidecar` 单一实现（补齐写前备份与乐观并发），并清理前端 3 个运行时零引用的死码模块与 3 个孤儿 IPC 命令。同时修复了一个**既有的保存死锁**（并发校验对顺序敏感）。

### 变更

- **CLI 每次成功写 PATH 都会产生一份备份**（`path_backup_*.txt`，默认目录、保留最近 20 份）。此前只有显式执行 `patheditor backup` 才产生。与 `env` 侧行为对称。**本次收口不是等价迁移**：`add` / `remove` / `edit` / `move-up` / `move-down` / `clean` / `enable` / `disable` / `import` / `profile apply` 现在都会写前备份。
- **保存 PATH 的编排统一到 core**：GUI 与 CLI 都经 `save_path_with_sidecar`（写前备份 + 乐观并发 + 顺序写 system→user + 广播 + sidecar/pending 事务）。前端 `path-session.ts` 退化为「权限预检 + 组合长度软提醒 + 文案映射」。
- GUI 的 pending 状态由 **store 内存**改为 **core 磁盘文件**（`disabled::save_pending_path_snapshot`）：跨重启存活的语义不变，但 GUI 不再以 `isModified` 表达「快照未落盘」；改为状态栏**非阻断提示**。
- **PathEditor 存盘会覆盖外部对 PATH 顺序的改动**。既然快照是 PATH 顺序的真相源，保存时**不再**因「外部重排了顺序」而拒绝——这是下面那项死锁修复的直接含义。外部**增删**路径仍会照常检测并拒绝。

### 修复

- **PATH 保存的「顺序敏感死锁」**：乐观并发校验此前**按顺序比较**期望值与当前值，而期望值来自快照合并（**顺序取自快照**）、当前值是**注册表顺序**——二者在**任何外部重排**（例如 Scoop 重写 PATH）后即不相等，导致保存**永久被拒**（存不了 → 快照永不更新 → 一直拒绝），且「重新加载后重试」也无效（重新加载走同一合并，产出同一顺序）。现改为**内容比较（多重集）**：忽略顺序、保留重复计数。**真实成员变化仍被拒绝**。
- **`patheditor clean` 后侧车快照陈旧**：此前 `clean` 只写注册表、不提交 `disabled.json` 快照，导致「注册表已清理，但有序快照仍是旧的」，下次 `load_path_snapshot` 合并时可能把已清理条目带回。收口到单一编排后由构造修复（clean 与其它入口一样提交 sidecar）。

### 说明

- **CLI 侧不再做乐观并发校验**：CLI 是单次进程，每次命令重新加载快照，「期望值」恒等于「当前值」，该层校验从未因真实并发触发（只可能因顺序差异误报，后者已由多重集修复消除）。现由类型显式去除该参数；并发校验由 GUI 侧承担（期望值来自界面加载时的已保存快照）。若未来 CLI 变为批处理模式，应重新引入。
- **删除**：前端 3 个死码模块 `src/core/{path-manager,import-export,validation}.ts`（及各自测试）——运行时零引用，且已有 Rust 对应实现；3 个孤儿 IPC 命令 `scan_conflicts` / `scan_tools` / `get_appdata_dir`（前端零调用）。core 侧的 `scan_conflicts` / `scan_tools` / `get_appdata_dir` 函数**保留**（被内部使用）。
- 移除项目级 `.cargo/config.toml`：其中的 `link-arg=-lmcfgthread` 已迁至全局 cargo 配置，项目级副本不再需要。

## 5.1.5 (2026-10-16)

本版收口 `env` 子命令在真实使用中登记的 6 条可用性 issue（#33 / #34 / #35 / #36 / #37 / #38）：修正一处错误泄漏，补齐三处文案/语义不一致，给三个破坏性写命令加上变更预览，并给 PATH 备份补上保留策略。

### 新增

- `env set` / `env add` / `env remove` 支持 `--dry-run`：只预览将要发生的变更，**不写注册表、不产生备份、不广播**。预览不要求 `--revision` / `--force`；变量不存在时照常报 `notFound`（退出码 1），与真正执行时的判定一致。
- `--dry-run --json` 输出脚本友好的统一结构 `{operation, hive, name, oldValue, newValue, kind, writable, sensitive}`（#36）。
- PATH 备份加入保留策略：`path_backup_*.txt` 写后自动轮换，默认保留最近 20 份，可被 `~/.patheditor/config.ini` 的 `path_backup_keep` 覆盖（#37）。

### 变更

- `env add --help` 补「纯新建语义」（同名会被拒绝 `NameExists`，不会覆盖）与「省略 VALUE 会立即报错，不进入交互提示」两处说明（#35 / #38）。
- `env list --help` 不再笼统声称「不含明文」，改为「敏感变量不含明文，非敏感变量给出可能截断的 preview」（#33）。

### 修复

- **`env get` 泄漏 Win32 原生错误串（#34）**：读取不存在的变量时，此前返回 `读取用户环境变量 X 失败: 无法读取环境变量 X: 系统找不到指定的文件。 (os error 2)`（变量名重复、暴露原生串）。现改为 `用户环境变量 X 不存在`，错误码 `NotFound`，退出码 1。根因修复在 `reg_store`：新增 `get_raw_kind` 按 `io::ErrorKind` 分类错误，使 `NotFound` 的结构信息能到达上层——此前 `get_raw` 把 `io::Error` 降级为 `String`，结构信息在中途丢失，上层无从判定。
- 其它错误（类型不支持 / IO / 解码失败）的 message 与错误码**逐字节不变**。
- **`--dry-run` 预览的敏感值保护（安全审查修正）**：预览输出会进终端、shell 历史、CI 日志，不应成为第二个明文出口。现 `preview_env_change` 对命中 `is_sensitive` 的变量**不回传明文**（`oldValue` 置 `null`、文本显示 `(敏感)`），`env add` 的新值同样经 `mask_new_value` 打码。
- **展示路径的控制字符净化（安全审查修正）**：`sanitize_preview` 的剔除范围从 `\r`/`\n`/`\0` 扩展到**全部 C0 控制字符与 DEL**，覆盖 `\x1b`（ESC）——防止注册表值中的 ANSI 转义序列在 `env list` / `--dry-run` 输出时注入终端（清屏、改标题等）。

### 说明

- `--json` 必须与 `--dry-run` 同用：单独使用由 clap 拒绝，退出码 **2**（参数用法错误），而非静默忽略。
- `--dry-run` 是纯读操作：不写注册表、不产生备份文件、不广播环境变更。预览展示的是「预览时刻」的状态（TOCTOU），**不保证**执行时不变。
- `path_backup_*.txt` 含两个 hive 的完整 PATH **明文**（可能含用户名、内网地址），保留策略是主要缓解措施。
- **#38 经核实不成立**（省略 VALUE 本就立即报错、不阻塞），本版只补 help 文案并关闭该 issue，无代码改动。`#32`（批量往返）需跨命令 session 状态，属架构变动，不在本版。

## 5.1.4 (2026-09-23)

本版包含三批内容：**一致性与架构收口**（Wave 0/1/2，2026-09-18 复审的 11 项问题）、**GUI 关窗死锁修复**、以及**环境变量备份与恢复**新特性。

### 新增

- 环境变量备份：CLI 与 GUI 的每一次环境变量写入前自动备份到 `~/.patheditor/backups/env_backup_<时间戳>.json`，含两个 hive 的全部可写变量与注册表类型（`REG_SZ` / `REG_EXPAND_SZ`）。
- 环境变量恢复：新增 `patheditor env restore <FILE>` 与 GUI 备份恢复界面，支持差异预览（新增 / 删除 / 冲突）与冲突保护；`--dry-run` 只打印差异、不写注册表。
- 新增 `patheditor env backup`（立即备份）与 `patheditor env backups`（列出备份，按时间倒序）。
- 新增 GUI 备份与恢复对话框（`envBackupPanel`），含删除项逐名提示、手工兜底命令提示与冲突二次确认。
- 保留份数可通过 `~/.patheditor/config.ini` 的 `env_backup_keep` 覆盖（默认 20 份）。
- 结构化错误契约 `CoreError` / `ErrorCode`：环境变量通路的错误不再是自由文本，前端按 `code` 判定，CLI 退出码由 `CoreError::exit_code()` 驱动。`[E_CONFLICT]` 消息前缀降级为过渡期展示文本，不再作为判定机制。
- 持久化文件（`disabled.json` / profiles）增加 `schemaVersion`、写入前保留上一份 `.bak` 轮换，以及损坏文件的隔离与恢复提示 —— 解析失败不再只返回通用 JSON 错误。
- 环境变量写入口新增 `reveal_env_var` 携带读取时 revision，编辑弹窗据此绑定「本值读取到的版本」。

### 变更

- 环境变量写入口（CLI `env set/add/remove`、GUI 编辑 / 新建 / 删除）返回值携带备份结果（`WriteOutcome.backup`）；备份失败不阻断写入，仅在 stderr 或状态栏提示。
- 恢复复用既有的 `create_env_var_in_store` / `update_env_var_force_in_store` / `delete_env_var_force_in_store` 写函数，保护名单、类型可写性、hive 权限判定仍只在 core 一处；CLI 与 GUI 均不做二次判定。
- 差异列表排序键固定为 `(hive, kind, name)`（user 在前、system 在后），使 `--dry-run --json` 的输出可复现。
- core 新增共享应用服务层（`apply_path_snapshot` / `apply_profile` / `save_path_with_sidecar` / `retry_pending_path_state`），统一 GUI 与 CLI 的 PATH 事务编排；GUI 侧接线延后，`path-session.ts` 仍走旧编排。
- 注册表访问改为 `EnvHiveStore` 端口：生产用 `WinregHive`，测试用内存替身 `MemoryHive`，`cargo test --workspace` 不再写真实 HKCU，可在任意环境执行。
- `core/src/registry.rs`（1101 行）拆分为 `registry/` 目录模块（纯搬家，零行为变化）。
- 新增 C→Rust 行为等价 golden 基线，覆盖 PATH 分割、写回类型、备份格式与广播时机。

### 修复

- **GUI 关窗死锁（v5.1.3 已知问题）**：关窗确认从阻塞式 `window.confirm` 改为 Tauri 异步对话框，并补齐 `core:window:allow-destroy` 权限（v5.1.3 中缺失，导致无草稿关窗挂起）。同步把其余阻塞式 `confirm` 全部换为异步对话框。
- **CLI 安装包装错二进制（v5.1.3 已知问题）**：GUI 与 CLI 产物同名（`patheditor.exe`）且共用 Cargo target 目录，NTFS 大小写不敏感使两者实为同一条目录项，CLI 链接产物覆盖了 GUI 本体——`scoop install lhy/patheditor-cli` 因此装到的是 GUI。现已将 GUI 二进制改名为 `PathEditor.exe`，并让 CI 用独立 target 目录（`--target-dir target/cli`）构建 CLI。
- `--force` 语义与文档不符：此前实为「重读一次 revision 再 CAS」，读与写之间的窗口仍会以退出码 3 失败。现改为真正的 force API（最后写入者胜，不做 revision 比对），仍保留名称校验、保护名单、类型可写性与 hive 写权限判定。
- 编辑弹窗可能用陈旧值覆盖外部更新：现在记录读值时的 revision，快照刷新时若已变化则提示并重载，提交前断言一致。
- CLI 注册表与 `disabled.json` 双写失败后状态丢失：sidecar 写失败会落 pending 待补写状态，PATH 命令启动时自动补写。
- 环境变量列表失败不再静默：此前枚举 / 读取 / 解码错误被 `Iterator::flatten()` 与 `continue` 吞掉，只留 warning；现在同一 hive 内任一步失败即返回错误，调用方明确知道结果不可用。

### 说明

- `~/.patheditor/backups/` 下的 `env_backup_*.json` 含敏感值**明文**（可能包括 API key、token），请勿同步到云端或提交到版本库。备份目录的文件权限**未做收紧**（登记为未覆盖项）。
- 备份默认保留最近 20 份，旧的自动轮换删除；只删除本工具生成的 `env_backup_*.json`，绝不触碰 `.txt` PATH 备份、`.bak` 与 `.corrupt-*` 文件。
- **差异计数的两个已知口径**：`RestorePreview.modified` 与 `RestoreOutcome.skipped` 恒为 0。备份 revision 与注册表不一致时一律判为「冲突」而非「修改」（`revision_of` 是 `(name, type, value)` 的纯函数，两者是同一条件）。因此 `--dry-run` 与 GUI 确认弹窗的「修改 N」恒显示 0，而 `--force` 下被冲突覆盖的变量确实被改写却计入「冲突」——dry-run 会**低报** force 模式的实际改动量。
- **未提权时恢复恒失败**：普通用户无法以写权限打开系统 hive，`restore_env_backup_from` 必然返回 `permissionDenied`（退出码 1），GUI 会提示需要管理员权限，且**不提供**「仅恢复用户 hive」这类降级路径。
- 恢复**逐条失败不改变退出码**（仍为 0），脚本无法从退出码检出部分失败；单变量失败以 stderr 警告呈现。
- `env restore --dry-run` 对**损坏备份**并非严格纯读：`read_env_backup` 对不可解析文件会经 persist 层将其重命名为 `<file>.corrupt-<ts>`（注册表未触碰）。
- `EnvBackupInfo.variableCount` 恒为 0 —— 列表只枚举目录与 stat，**不解析内容**，单个损坏备份不会让列表整体失败。
- GUI 关窗修复只在本机 GNU 构建上验证（WM_CLOSE 退出 + 异步确认对话框）；死锁仅复现于 CI/MSVC 构建，**CI 侧最终验证待本次发布构建后确认**。
- 4 个服务层 IPC 命令（`apply_path_snapshot` 等）已在 GUI 注册，但前端尚未接线，`path-session.ts` 仍走旧编排。

## 5.1.3 (2026-09-18)

### 新增

- CLI 新增 `env` 子命令组（`list` / `get` / `set` / `add` / `remove`），脚本与自动化场景获得与 GUI 等价的通用环境变量管理能力。
- 值输入三通道：位置参数 / `--stdin` / `--value-file`，互斥。后两者让敏感值可绕开 shell 历史与进程列表。
- `env list --json` 输出直接序列化 core 契约（camelCase，不含 `value` 字段），供脚本消费与获取 revision。
- `--force` 模式：跳过并发校验直接覆盖（脚本 `setx` 风格）。

### 变更

- CLI 写操作强制显式选择并发模式：`--revision <R>`（CAS 校验）或 `--force`（跳过校验），两者都不给报错、都给也报错。
- CLI 新增退出码 **3** 表示 revision 冲突，使脚本可凭退出码区分「重试后可恢复」与致命错误，无需 grep 中文文案。PATH 命令保持退出码 1 不变（向后兼容）。
- `env add` 的 `--kind` 默认 `string`（`REG_SZ`），可选 `expand`（`REG_EXPAND_SZ`）。

### 说明

- CLI 侧零安全判定逻辑：保留名、保护名单、`Unsupported` 类型、hive 写权限、revision 校验全部由 `core` 判定，CLI 仅透传错误文本。
- `env get` 是 CLI 侧唯一明文出口，stdout 只打印裸值 + 换行（管道友好，对齐 `git config --get`）。
- `Path` 不在通用通路内，仍只能经 PATH 专用命令编辑。
- 真实注册表闭环测试通过（`add` / `set --force` / 退出码 3 冲突 / `remove` CAS 四项），快照对比零污染。记录见 `docs/审核和开发/2026.09.18/PathEditor-CLI环境变量闭环测试记录.md`。
- 已知边界：Windows 注册表无 CAS，读-比-写是两次独立调用，revision 校验缩小竞态窗口但不能完全消除 TOCTOU。

## 5.1.2 (2026-09-15)

### 修复

- 修复保存 PATH 时注册表值类型被降级为 `REG_SZ` 的问题；现在会保持原有 `REG_EXPAND_SZ` / `REG_SZ` 类型，新值默认使用 `REG_EXPAND_SZ`（Issue #26）。

### 迁移说明

- 旧版本已经把 PATH 写成 `REG_SZ` 的机器不会自动改回 `REG_EXPAND_SZ`；下一次保存只会保持当前类型。
- 如需恢复 `%VAR%` 展开，请先备份注册表，再使用系统注册表编辑器或后续提供的显式修复命令将 PATH 改回 `REG_EXPAND_SZ`。
- 修复方案和人工验收步骤见：`docs/审核和开发/2026.09.15/PathEditor-Issue-26-PATH值类型降级修复方案.md`。

## 5.1.1 (2026-09-14)

### 修复

- 修复非管理员模式下用户 PATH 被错误锁定，系统 PATH 与用户 PATH 改为独立权限判断
- 修复路径验证批次在列表重渲染后任务丢失、条目长期停留在 pending 的问题
- 修复禁用路径重启后丢失，`disabled.json` 会保留完整有序快照并与注册表合并
- 修复禁用状态写入失败后仍被视为已保存、无法再次补写的问题
- 修复导入配置时无权限注册表 hive 仍可被操作的问题，并补充明确提示
- 修复超过 20 条路径后停止验证和环境变量展开的问题
- 修复 GUI 清理路径时不检查目录是否存在的问题
- 修复 `PathCapabilities` 序列化字段大小写不一致导致的真实 Tauri 运行时读取错误
- 修复 TS/Rust 导入导出语义漂移，CSV/JSON 的 `enabled` 状态处理保持一致
- 修复 CLI 禁用操作只写 sidecar、与 GUI 快照语义不一致的问题

### 优化

- 长列表引入虚拟滚动，优化 `PathTable` 与 `MergePreview` 的渲染性能
- 注册表保存增加原始路径并发比对，降低覆盖外部修改的风险
- 统一 Windows API 调用到 `windows-sys`，减少手写 FFI 声明
- 增加 PathCapabilities、PathEntry、PathSnapshot、禁用状态和 IPC 运行时形状校验
- 拆分 CLI、AnalyzeDialog、ProfileDialog、app-store 等过大模块，降低维护成本
- 补充路径验证、禁用状态、导入权限、快捷键和 E2E mock 回归测试

### 工程化

- 增加 Husky、lint-staged、commitlint、Prettier、ESLint 和覆盖率门禁
- 增加 Dependabot、CODEOWNERS、Issue/PR 模板
- 精简 GitHub Actions，改为 tag 触发的自动构建和 GitHub Release
- 发布产物同时包含 NSIS 安装包和 CLI 可执行文件

## 5.0.0 (2026-05-29)

### Added

- Cargo workspace 三层架构 (core + gui + cli)
- CLI 命令行工具，17 条命令，支持 JSON 输出
- PATH 可执行文件冲突检测 (`scan_conflicts`)
- PATH 目录工具清单 (`scan_tools`)
- 配置文件管理：保存/加载/应用/重命名/删除
- 系统+用户合并预览视图
- CLI 原子性保护：写入前重新读取注册表对比
- `--steps N` 参数支持多格移动 (CLI 特有)

### Changed

- Rust + Tauri 2.x + React 19 + TypeScript strict 全重写
- 撤销/重做系统扩展至 10 种操作类型
- 禁用状态即时持久化，不依赖保存按钮
- 深色模式 / 浅色模式 CSS 变量驱动
- 中英双语界面 (i18next)
- 备份文件存储路径统一到 `~/.patheditor/`
- 版本号集中管理: Rust 端 `Cargo.toml` workspace, 前端 `package.json`

### Fixed

- 非管理员自动进入只读模式
- 保存失败精确提示哪个注册表 hive 出错 (Promise.allSettled)
- CLI `--system`/`--user` 互斥校验
- 修改操作后广播 `WM_SETTINGCHANGE`
- 深色模式下行选中颜色对比度不足
- 窗口内容溢出无法滚动

## 4.2.0

### Fixed

- Release workflow 兼容已存在的 release

## 4.1.0

### Added

- 路径验证 (红色无效、橙色重复)
- 环境变量路径悬浮展开预览
- 全局键盘快捷键
- 修改状态指示 + 未保存退出确认

## 4.0.0

### Added

- Tauri 2.x + React + TypeScript 首次发布
- Windows 系统/用户 PATH 的增删改查
- 拖拽排序、多选批量删除
- 实时搜索过滤
- 导入导出 JSON/CSV/TXT
- 撤销/重做支持
- 保存前自动备份注册表
