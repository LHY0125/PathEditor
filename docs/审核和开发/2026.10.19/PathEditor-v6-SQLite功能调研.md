# PathEditor v6.0.0 SQLite 存储功能调研

- **日期**：2026-10-19
- **作者**：审核窗口
- **状态**：调研（**未进入 spec、未改任何代码**）；已按核对轮 G1–G5 修订（2026-10-19）
- **一句话结论**：SQLite 值得作为 v6.0.0 的存储底座，但它解锁的核心不是「更大的数据量」，而是**把当前散落在四种文件格式里的「快照」变成一等公民**，从而支撑历史、审计、检索、对比、统计与未来的跨设备同步。

---

## 一、现状核实（file:line 可查）

| 数据                  | 位置                                                                       | 形态                                   | 规模         | 关键事实                                                           |
| --------------------- | -------------------------------------------------------------------------- | -------------------------------------- | ------------ | ------------------------------------------------------------------ |
| PATH 真相源           | Windows 注册表 HKLM/HKCU                                                   | 系统契约                               | 几十条       | **不可替换**；core 只做读写编排                                    |
| 当前快照 + 禁用状态   | `~/.patheditor/disabled.json`（`core/src/disabled.rs:14-18`）              | 单个 JSON，`schemaVersion` 信封        | KB 级        | 含 `systemSnapshot` / `userSnapshot` 完整有序快照                  |
| pending 快照          | `~/.patheditor/pending_path_snapshot.json`（`disabled.rs:27-31`）          | 单个 JSON                              | KB 级        | 注册表已写、快照失败时的补偿文件                                   |
| 命名快照（配置方案）  | `~/.patheditor/profiles/<name>.json`（`core/src/profiles.rs:13-17,66-68`） | **一配置一文件**                       | KB 级/个     | 元数据只有 `name/created/modified`（`core/src/profiles.rs:75-79`） |
| 定时快照（env 备份）  | `~/.patheditor/backups/env_backup_*.json`（`core/src/backup.rs:336-357`）  | 每份全量 JSON，含**明文值 + revision** | 10–100 KB/份 | 默认保留 20 份，按文件名轮换                                       |
| 定时快照（path 备份） | `~/.patheditor/backups/path_backup_*.txt`（`backup.rs:1250-1267`）         | 每份全量 TXT                           | KB 级/份     | 默认保留 20 份，直接 `fs::write`                                   |
| 保留策略              | `~/.patheditor/config.ini`                                                 | INI                                    | 2–3 个键     | `env_backup_keep` / `path_backup_keep` / `custom_dir`              |

**已经存在的「穷人版事务」**（值得保留其设计思想）：

- `Versioned<T>` 信封 + `schemaVersion`（`core/src/persist.rs:37-50`）
- 写前把主文件轮换为 `.bak`（`persist.rs:65-77`）
- `atomic_write` = 写临时文件 + rename（`core/src/fs.rs:18-24`）
- 解析失败隔离为 `<file>.corrupt-<ts>`（`persist.rs:102-129`）

**当前的能力缺口（已核实）**：

- 没有跨文件事务：`disabled.json`、`profiles/*.json`、备份文件各自独立写。
- 没有跨进程锁：GUI 与 CLI 可同时运行，core 无 `fs2`/`fd-lock` 依赖。
- 没有 fsync：`atomic_write` 只 rename，不 `sync_all`。
- 撤销/重做只在内存：`UndoRedoManager` 由 `app-store.ts:77` 构造，进程退出即丢。
- 配置列表逐个解析文件；扫描结果（`core/src/scanner.rs:92-129`）不落盘。
- 零 SQLite 依赖（`core/Cargo.toml` 仅 serde/winreg/dirs/chrono/windows-sys）。

> **核心观察**：`disabled.json`（当前快照）、`profiles/*.json`（命名快照）、`env_backup_*.json`（定时快照）、`path_backup_*.txt`（定时快照）**存的是同一类东西——某一时刻的 PATH/env 状态**，却用了四种格式、四套生命周期，彼此不可查询、不可关联、大量重复。这是 SQLite 最直接的价值来源。

---

## 二、判断口径：三种「做不到」

讨论「SQLite 能实现什么」时，必须区分：

1. **结构上做不到**：文件存储缺少该能力（跨文件事务、索引查询、外键、行级并发、FTS）。
2. **文件能做但成本/风险高**：需要自己造索引、扫描全部文件、维护旁路元数据。
3. **SQLite 也不解决**：不要期待数据库能带来这些（见 §三 C）。

---

## 三、SQLite 能解锁的功能清单

### A. 结构上文件存储做不到（或只能靠「自己造一个数据库」）

#### A1. 跨多实体的 ACID 事务

- **场景**：应用配置方案时，需要同时完成「写注册表 → 更新当前快照 → 记录历史事件 → `apply_count += 1` → 写审计日志」。
- **现状**：这是 5 个独立文件/步骤；进程在中间崩溃会留下不一致状态。现有的 pending 机制只覆盖「注册表 → disabled.json」这一个缺口。
- **SQLite**：`BEGIN … COMMIT` 一次原子提交所有 DB 侧改动；注册表仍在外，注册表→DB 的 gap 仍要保留 pending 机制（见 §七）。
- **边界**：SQLite **不能**让注册表写入与 DB 提交成为一个事务——这是操作系统契约，任何本地数据库都做不到。

#### A2. 行级并发控制（GUI + CLI 多进程）

- **场景**：GUI 正在保存 PATH，CLI 同时执行 `patheditor add`；两者都读改写 `disabled.json`。
- **现状**：无文件锁，后写覆盖先写（lost update）。
- **SQLite**：WAL 提供事务隔离与提交原子性，`busy_timeout` 处理短暂争用；但它**不自动消除业务层的 lost update**——两个进程可以各自读到旧状态、各自写回，后提交者覆盖前者。写路径必须在事务内重新读取，并用 `version` / `generation` 列做 compare-and-swap（`BEGIN IMMEDIATE` + `UPDATE … WHERE version = ?`，影响 0 行即返回 `conflict`）。**不要宣称「SQLite 天然解决多进程并发」**（G3）。
- **边界**：WAL 在 **SMB/网络家目录**上不可靠（锁语义依赖本地文件系统），需要降级策略（见 §七）。

#### A3. 任意条件的索引查询

- **场景**：
  - 「过去 30 天 `JAVA_HOME` 改过几次？每次的前后值是什么？」
  - 「哪些历史快照包含过 `C:\Foo\bin`？」
  - 「列出所有由 CLI 发起、结果为 partial 的写入。」
- **现状**：只能按文件名列举，想查内容必须逐个解析全部文件。
- **SQLite**：`(target, created_at)`、`(path)`、`(name, hive)` 等索引 + `LIKE`/FTS；查询与数据量解耦。

#### A4. 引用完整性与级联删除

- **场景**：删除一个配置方案时，它的版本历史、标签、关联事件怎么办？
- **现状**：无外键，容易留下孤儿文件；清理逻辑要手写。
- **SQLite**：`FOREIGN KEY … ON DELETE CASCADE` + `PRAGMA foreign_keys=ON`，库层保证不产生孤儿。

#### A5. 原子 schema 迁移

- **场景**：v6.1 给历史表加一列、拆分标签表；需要迁移已有上万行。
- **现状**：每类文件各自 `Versioned`，跨文件迁移要自己写日志与回滚。
- **SQLite**：`PRAGMA user_version` + 事务内迁移，失败自动回滚；迁移脚本可测试。

#### A6. 行级变更集与墓碑（同步/合并的基础）

- **场景**：两台机器各改各的 PATH/配置，之后要合并；或「导出上次同步以来的变更」。
- **现状**：整文件快照，无稳定行 ID、无删除标记、无法做行级合并。
- **SQLite**：`changes` 表（UUID、`deleted_at`、`updated_at`）→ 可按时间导出变更集；这是未来跨设备同步的前置条件。

---

### B. 文件能做但成本高，SQLite 明显更优

#### B1. 持久化历史与「时间旅行」（含跨会话撤销/重做）— **旗舰功能**

- **场景**：
  - 打开「历史」面板，按时间线查看每次保存/应用/导入/恢复；
  - 点击任意历史点，预览当时的系统/用户 PATH；
  - 一键恢复某个历史状态；
  - 关闭应用后重新打开，仍能撤销上一次性会话的操作。
- **现状**：撤销栈只在内存（最多 50 步，`src/core/undo-redo.ts`）；备份是 20 份全量文件，无时间线、无预览、无法跨会话。
- **SQLite**：`snapshots` + `path_entries` + `events`；按时间查询、按内容对比、按需恢复。
- **价值**：这是用户能直接感知的「高级特性」，也是迁移 SQLite 最充分的理由。

#### B2. 快照统一与内容去重

- **场景**：同一份 PATH 状态被 `disabled.json`、`profiles/*.json`、`path_backup_*.txt` 各存一遍；10 次保存产生 10 份近乎相同的全量数据。
- **现状**：无去重、无引用计数；备份轮换只按数量删。
- **SQLite**：`content_hash` + 规范化 `path_entries`/`env_values`；相同内容只存一份，多个「快照元数据」引用它。
- **价值**：历史功能会把存储量放大 10–100 倍，去重是让它可持续的前提。

#### B3. 备份索引与智能保留

- **场景**：
  - 按变量名/路径片段搜索所有备份；
  - 「保留最近 30 天 + 每天 1 份 + 最多 200 份」的时间/数量/大小混合策略；
  - 备份列表显示真实的变量数/差异摘要（现在 `variableCount` 恒为 0，因为列表只 stat 不解析）。
- **现状**：只枚举文件名与大小，不解析内容；保留策略只有「最近 N 份」。
- **SQLite**：备份索引表（kind/time/hash/size/path/snapshot_id）+ 查询；备份文件本身仍可保留为文件，DB 只做索引。
- **建议**：**保留文件**（用户要能手工打开、手工复制），DB 负责索引与去重。

#### B4. 配置方案的元数据、标签与版本

- **场景**：
  - 给配置方案加描述、标签（「公司」「家庭」「笔记本」）；
  - 按标签过滤、按最近使用排序；
  - 查看某配置的历史版本并回滚。
- **现状**：`ProfileMeta` 只有 `name/created/modified`；列表要逐个读文件解析。
- **SQLite**：`profiles`（description/tags/last_applied_at/apply_count）+ `profile_revisions`；查询与排序走索引。
- **UI**：标签 chips、搜索框、「最近使用」分组。

#### B5. N 路对比（配置 vs 配置 vs 当前状态）

- **场景**：
  - 「当前 PATH 与某个配置差在哪？」
  - 「A、B、C 三个配置哪个最接近当前？」
  - 「应用配置后新增/删除了哪些路径？」
- **现状**：两份快照可以在内存里比；N 份需要反复加载/解析，代码里没有通用 diff。
- **SQLite**：`JOIN`/`EXCEPT` 直接算集合差；配合 `content_hash` 快速判断「是否相同」。

#### B6. 全文检索（FTS5）

- **场景**：在全部历史/备份/配置里搜「`Python311`」「`%JAVA_HOME%`」「某天改过的变量」。
- **现状**：逐文件扫描 + 解析，数据量一大就不可接受。
- **SQLite**：FTS5 虚拟表（`rusqlite` 的 `bundled` 在 libsqlite3-sys 构建脚本中启用 `SQLITE_ENABLE_FTS5` 与 `SQLITE_ENABLE_JSON1`）。**验收口径**：FTS5 用 `SELECT sqlite_compileoption_used('ENABLE_FTS5')`；JSON **不要**用 `ENABLE_JSON1` 编译选项判断——SQLite ≥3.38 已内建 JSON，该选项可能返回 0，应实际执行 `SELECT json_valid('{"a":1}')`（核对轮小提醒）。
- **边界**：FTS 对中文分词一般，可用 `unicode61` + 前缀查询；不追求自然语言搜索。

#### B7. 统计与仪表盘

- **场景**：
  - PATH 总长度/条目数随时间的变化；
  - 最常被修改的变量；
  - 冲突出现频率与首次出现时间；
  - 配置应用次数、最近应用时间；
  - CLI vs GUI 的写入比例。
- **现状**：无持久化，每次重新扫描；历史数据根本不存在。
- **SQLite**：聚合查询 + 可选的每日汇总表；查询成本低。

#### B8. 环境变量引用关系图

- **场景**：
  - 存储 `%JAVA_HOME%` → `JAVA_HOME` 的引用边；
  - 删改变量前查「谁依赖我」（传递闭包）；
  - 检测循环引用；
  - 展示某个变量的影响范围。
- **现状**：`expand_env_vars` 按需展开，不持久化关系；每次都要重新解析。
- **SQLite**：`var_references(from_name, to_name, hive)` + 递归 CTE（`WITH RECURSIVE`）。
- **价值**：这是文件存储几乎不会去做的「图查询」，也是 SQLite 的强项。

#### B9. 冲突历史基线

- **场景**：「这个冲突是 3 天前出现的」「本次新增了哪些冲突」「冲突数量趋势」。
- **现状**：`scan_paths` 结果即用即弃（`core/src/scanner.rs:92-129`）。
- **SQLite**：`scan_runs` + `conflicts`；两次扫描做差集。

#### B10. 智能导入与合并

- **场景**：导入一份 PATH 列表时，按大小写不敏感匹配已有项，保留 `enabled` 状态，报告「新增/重复/冲突」，而不是整表替换。
- **现状**：导入是解析后交给上层处理，语义偏「替换/追加」；没有暂存表与合并审计。
- **SQLite**：先入 staging 表，再用 SQL 做匹配、去重、合并，最后一次性提交并写事件。

---

### C. SQLite 也不解决（明确排除，避免错误预期）

| 期望                            | 现实                                                                                             |
| ------------------------------- | ------------------------------------------------------------------------------------------------ |
| 让注册表写入变成原子/CAS        | 不能。Windows 注册表没有 CAS；读-比-写仍是两次调用，TOCTOU 窗口仍在。revision 校验只能缩小影响。 |
| 自动获得管理员权限              | 不能。权限仍由 `PathCapabilities` 与 OS 决定。                                                   |
| 实时知道路径是否存在 / 工具清单 | 不能。文件系统状态必须实时扫描；DB 只能缓存结果并标注时间。                                      |
| 自动加密敏感值                  | 不能。默认 SQLite 明文；SQLCipher 只保护 DB，**保护不了仍在磁盘上的明文备份文件**。              |
| 让备份更易读                    | 反而更难。备份/配置要保持用户可打开，应继续用 JSON/TXT 文件或提供导出。                          |
| 解决网络家目录的并发            | 不能。WAL 依赖本地文件系统锁；SMB 上需要降级或本地缓存。                                         |

---

## 四、v6.0.0 范围建议（三档，供拍板）

### Tier 1 — v6.0.0 MVP（推荐）

1. **统一快照模型**：`snapshots` + `path_entries` + `env_values`；把 `disabled.json`、`profiles/*.json`、备份统一抽象。
2. **事件与历史**：`events` + 历史时间线 + 任意历史点预览/恢复；跨会话撤销/重做。
3. **配置元数据**：description/tags/last_applied_at/apply_count/revisions。
4. **备份索引**：文件仍保留，DB 索引 kind/time/hash/size；列表显示真实摘要。
5. **一次性迁移**：导入现有 JSON/TXT，保留原件为 `.v5.bak`。
6. **可靠性**：WAL、`busy_timeout`、`foreign_keys`、`integrity_check`、迁移前 DB 备份。

### Tier 2 — v6.x（历史跑稳之后）

- FTS5 全文搜索；
- 统计仪表盘；
- N 路配置对比；
- 环境变量引用图；
- 冲突历史基线；
- 智能导入/合并。

### Tier 3 — v7 或更晚

- 跨设备同步/合并（UUID + 墓碑 + 变更集）；
- 策略/规则引擎（「禁止某类路径」「自动清理」）；
- 敏感值加密（若同时解决备份文件加密）。

> **不建议 v6.0.0 一次性全做**。Tier 1 已经足以证明迁移价值，且把风险控制在「历史/索引」层；Tier 2 的图查询/FTS 可以独立迭代。

---

## 五、最小数据模型草图（Tier 1）

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA user_version = 1;

CREATE TABLE snapshots (
  id           INTEGER PRIMARY KEY,
  created_at   INTEGER NOT NULL,              -- Unix ms
  source       TEXT NOT NULL,                 -- gui / cli / import / restore / migration
  kind         TEXT NOT NULL,                 -- current / profile / backup / history
  reason       TEXT,
  content_hash TEXT NOT NULL,                 -- 去重键
  parent_id    INTEGER REFERENCES snapshots(id),
  note         TEXT
);
CREATE INDEX idx_snapshots_time ON snapshots(created_at);
CREATE INDEX idx_snapshots_hash ON snapshots(content_hash);

-- G3：当前状态指针 + 版本；所有写事务用 BEGIN IMMEDIATE，读-改-写带 version CAS。
CREATE TABLE app_state (
  id                  INTEGER PRIMARY KEY CHECK (id = 1),
  current_snapshot_id INTEGER REFERENCES snapshots(id),
  version             INTEGER NOT NULL DEFAULT 1,
  migrated_v5         INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE path_entries (
  snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
  hive        TEXT NOT NULL CHECK (hive IN ('system','user')),
  position    INTEGER NOT NULL,
  path        TEXT NOT NULL,
  enabled     INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, hive, position)
);
CREATE INDEX idx_path_entries_path ON path_entries(path);

CREATE TABLE env_values (
  snapshot_id INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
  hive        TEXT NOT NULL,
  name        TEXT NOT NULL,
  kind        TEXT NOT NULL,
  value       TEXT,                           -- 敏感值/Unsupported 为 NULL；见 §七 R1、G1
  revision    TEXT NOT NULL,                  -- FNV-1a 弱摘要，不是加密（G5）
  sensitive   INTEGER NOT NULL DEFAULT 0,
  restorable  INTEGER NOT NULL DEFAULT 1,     -- Unsupported / 敏感值按策略置 0
  PRIMARY KEY (snapshot_id, hive, name),
  -- G1：Path 只能走 path_entries 专用通路，绝不进入 env_values
  CHECK (lower(name) <> 'path')
);

CREATE TABLE events (
  id              INTEGER PRIMARY KEY,
  created_at      INTEGER NOT NULL,
  source          TEXT NOT NULL,              -- gui / cli
  action          TEXT NOT NULL,              -- save / apply / restore / import / env_set / undo / redo
  hive            TEXT,
  target          TEXT,
  changed_fields  TEXT,                       -- JSON 数组：只放字段名/摘要，禁止明文值（G2）
  before_revision TEXT,                       -- 只存 revision，不存值（G2/G5）
  after_revision  TEXT,
  result          TEXT NOT NULL,              -- success / partial / failure
  error_code      TEXT,
  snapshot_id     INTEGER REFERENCES snapshots(id)
);
CREATE INDEX idx_events_time   ON events(created_at);
CREATE INDEX idx_events_target ON events(target, created_at);

CREATE TABLE profiles (
  id              INTEGER PRIMARY KEY,
  uid             TEXT NOT NULL UNIQUE,        -- Q5：稳定身份，为未来同步预留（UUID）
  name            TEXT NOT NULL UNIQUE,
  description     TEXT,
  tags            TEXT,                       -- JSON 数组；或独立 tags 表
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  last_applied_at INTEGER,
  apply_count     INTEGER NOT NULL DEFAULT 0,
  version         INTEGER NOT NULL DEFAULT 1  -- G3：CAS 用
);

CREATE TABLE profile_revisions (
  id          INTEGER PRIMARY KEY,
  profile_id  INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
  version     INTEGER NOT NULL,
  created_at  INTEGER NOT NULL,
  snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
  UNIQUE (profile_id, version)
);

CREATE TABLE backups (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL,                 -- env / path
  created_at   INTEGER NOT NULL,
  content_hash TEXT NOT NULL,
  size_bytes   INTEGER NOT NULL,
  file_path    TEXT NOT NULL,                 -- 保留文件；DB 只做索引
  snapshot_id  INTEGER REFERENCES snapshots(id)
);
CREATE INDEX idx_backups_kind_time ON backups(kind, created_at);

CREATE TABLE scan_runs (
  id             INTEGER PRIMARY KEY,
  created_at     INTEGER NOT NULL,
  conflicts_json TEXT NOT NULL,
  tools_json     TEXT NOT NULL
);

-- Tier 2：FTS5（外部内容表，避免重复存储）
-- CREATE VIRTUAL TABLE search_fts USING fts5(content, kind, ref_id, tokenize='unicode61');
```

**模型层的四条硬约束（G1/G2/G3/G5）**：

1. `env_values` 永远不含 `Path`（`CHECK` + 采集层双重排除）；`Unsupported` 只存 `kind`/`revision`，`value=NULL`、`restorable=0`。
2. `events` 永远不存 env 明文值；只存 `changed_fields` + `revision`。spec 需要一条守卫测试扫描事件写入路径。
3. 所有「读-改-写」必须在 `BEGIN IMMEDIATE` 事务内完成，并对 `app_state.version` / `profiles.version` 做 CAS；冲突返回 `CoreError.code = conflict`。
4. `revision` 是 FNV-1a 弱摘要，只用于变更检测与冲突判定，**不是加密/匿名化**。

---

## 六、迁移与兼容策略（草案）

1. **一次性导入**：首次启动检测旧文件，按 `disabled.json → snapshots(kind=current)`、`profiles/*.json → profiles + profile_revisions + snapshots`、`backups/* → backups + snapshots` 导入；原件保留为 `*.v5.bak`（不删除，符合仓库「不删文件」纪律）。导入时**排除 `Path`**（它只属于 `path_entries`），`Unsupported` 只写元数据；`env_backup_*.json` 中的敏感值按 Q2 只写 `revision` + `sensitive=1`（G1/G2）。
2. **幂等**：以「源文件路径 + content_hash」判重；迁移完成写 `app_state.migrated_v5 = 1`（单行 `app_state`，不是散落的 `meta` 键）。
3. **GUI/CLI 双入口**：两边都可能先启动；用文件锁或 `BEGIN IMMEDIATE` + `app_state` 标记保证只迁移一次。
4. **降级（G4）**：已发布的 v5 二进制不认识 DB，只会忽略并继续用旧文件；「拒绝启动」的守卫只能在 **v6 之前**由最后一个 v5.x 版本发出。因此二选一：① 先发一个 v5.x 守卫版本（检测 `~/.patheditor/patheditor.db` 后警告/拒绝）；② 明确「降级不受支持」，v6 提供「导出为 v5 格式」并保留 `.v5.bak`。
5. **测试**：
   - 旧格式夹具（无 `schemaVersion` 的 disabled.json、损坏 JSON、空备份）；
   - 迁移中断（在事务中 kill）后重跑；
   - 双进程并发（GUI mock + CLI 同时写）；
   - `PRAGMA integrity_check`；
   - 性能基线（启动耗时、迁移耗时）。
6. **打包**：`rusqlite = { version = "…", features = ["bundled"] }` 放进 **core**（不是前端 `tauri-plugin-sql`）；CI 的 MSVC 构建需验证 `cc` 编译；预计二进制 +1–2 MB。
7. **架构边界**：SQL 只在 core；GUI/CLI 仍经 `backend.ts` / core API；**不向前端暴露 SQL**。

---

## 七、风险与未决项

| #   | 风险/未决              | 说明                                                                                                                                                                           | 建议                                                                                                                                                                                                                                              |
| --- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R1  | **敏感值入库**         | `env_values.value` 若存明文，DB 与现有明文备份一样暴露；`revision` 是 FNV-1a 弱摘要，**不是加密**——保留 revision 仍可用候选值验证（G5）；SQLCipher 只保护 DB，保护不了备份文件 | **推荐 Tier 1**：非敏感值明文入库；敏感值 `value=NULL` + `revision` + `sensitive=1`，历史 UI 显示「敏感值，不可回放」，恢复仍走现有明文备份文件。加密（DPAPI/SQLCipher）若要做，必须与备份加密同一波（Tier 3）。**不要宣称「只存哈希 = 零泄漏」** |
| R2  | **网络家目录**         | `~/.patheditor` 可能落在漫游配置/SMB；WAL 锁不可靠                                                                                                                             | 检测网络路径 → 降级为 `journal_mode=DELETE` + 单进程提示，或把 DB 放本地缓存                                                                                                                                                                      |
| R3  | **DB 损坏/升级**       | 单文件损坏影响全部历史                                                                                                                                                         | 启动 `integrity_check`（可配置频率）；**WAL 下备份必须 `VACUUM INTO`（或先 checkpoint），禁止直接复制 `.db`**——直接复制会丢最近未 checkpoint 的事务；迁移前同样先 `VACUUM INTO`；保留 `.corrupt` 隔离思路                                         |
| R4  | **体积增长**           | 每次保存一个快照，历史会膨胀                                                                                                                                                   | `content_hash` 去重 + 保留策略（如事件 90 天、每日快照 1 年）+ `VACUUM`/`VACUUM INTO`                                                                                                                                                             |
| R5  | **注册表原子性**       | DB 事务管不到注册表                                                                                                                                                            | 保留 pending 机制；事件表记录 `result=partial` 以便审计                                                                                                                                                                                           |
| R6  | **迁移失败**           | 旧文件是唯一历史                                                                                                                                                               | 导入前复制 `~/.patheditor` 到 `*.v5.bak`；迁移事务失败回滚；提供导出                                                                                                                                                                              |
| R7  | **CLI 启动延迟**       | 打开 DB + 迁移可能拖慢 CLI                                                                                                                                                     | 迁移只在首次；后续只读连接；测量冷启动                                                                                                                                                                                                            |
| R8  | **测试设施**           | 现有测试用固定临时文件与 `test_persist_lock`                                                                                                                                   | 新增临时 DB 夹具 + 双进程并发测试；WAL 文件清理                                                                                                                                                                                                   |
| R9  | **依赖/许可**          | `rusqlite`/`libsqlite3-sys` 的 MIT/Apache 许可、C 编译                                                                                                                         | CI 增加 MSVC bundled 构建；`cargo deny` 检查                                                                                                                                                                                                      |
| R10 | **数据可移植性**       | 用户可能想手工备份/迁移                                                                                                                                                        | 提供「导出全部历史为 JSON」与「导入」；备份文件继续保留                                                                                                                                                                                           |
| R11 | **逻辑层 lost update** | WAL 只保证提交原子，不保证业务读的是最新状态（G3）                                                                                                                             | 所有写路径 `BEGIN IMMEDIATE` + 事务内读 + `version` CAS；冲突复用 `CoreError.code = conflict`，不要用 WAL 代替业务并发控制                                                                                                                        |

> **决策更新（2026-10-19）**：用户最终选择 **SQLCipher 全库加密**（见 §十一），R1 的「敏感值 `value=NULL`」推荐被覆盖；实际方案见 spec D5（加密入库 + UI 掩码 + 事件表不存明文）。本行保留为备选分析。

---

## 八、需要你拍板的问题（附核对轮建议）

| #   | 问题                                                    | 核对轮建议（开发窗口提出、审核窗口复核）                                                                                                                                                                                                                                                                           |
| --- | ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | **v6.0.0 范围**：只做 Tier 1，还是把 FTS/统计也放进来？ | **只做 Tier 1**。存储替换 + 一次性迁移已是本项目史上最大变更；FTS/统计是纯增量，`events`/`snapshots` 就位后随时可加。                                                                                                                                                                                              |
| 2   | **敏感值**：`env_values.value` 是否入库？               | **Tier 1 只存哈希/元数据**：非敏感值明文入库；敏感值 `value=NULL` + `revision` + `sensitive=1`，历史 UI 显示「敏感值，不可回放」，恢复仍走现有明文备份文件。**注意两点**：① 历史对敏感值不可回放，DB 历史不是完整备份；② `revision` 是弱摘要，不是零泄漏。DPAPI/SQLCipher 若要做，必须与备份加密同一波（Tier 3）。 |
| 3   | **备份文件形态**：保留文件还是全部入库？                | **保留文件**（`env_backup_*.json` / `path_backup_*.txt`），DB 只做索引。用户可手工打开/复制；DB 损坏时备份仍可用；符合「不删文件」纪律。                                                                                                                                                                           |
| 4   | **配置方案**：迁入 DB 还是继续一文件一配置？            | **迁入 DB**（获得标签/版本/最近使用/查询），保留「导出为 v5 JSON」兜住可移植性。                                                                                                                                                                                                                                   |
| 5   | **跨设备同步**：是否预留 UUID/墓碑/变更集？             | **只预留稳定 `uid`**（配置身份，建议 UUIDv7）；墓碑/变更集**不做**（YAGNI，同步属 Tier 3）。                                                                                                                                                                                                                       |
| 6   | **降级**：v5 遇到 v6 的 DB 怎么办？                     | **已发布的 v5 无法改变 → 只能忽略**。若要「拒绝启动」，守卫必须在 v6 之前发到 v5.x（见 G4）；否则明确「降级不受支持」，v6 提供导出 v5 JSON。                                                                                                                                                                       |

> **其中只有 Q2 会反向决定数据模型**（`value` 是否可空、`restorable` 语义、事件表字段）；建议最先定 Q1、Q2，其余四项按上表默认执行即可。
>
> **决策更新（2026-10-19）**：用户已选择 **单版本全量 + SQLCipher 全库加密 + 文件式变更包**，Q1/Q2 的「Tier 1 / 只存哈希」建议均被覆盖；最终方案见 `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` 与 §十一。

---

## 九、结论

- **SQLite 的最大价值是「把快照变成一等公民」**：统一 `disabled.json`、`profiles/*.json`、`env_backup_*.json`、`path_backup_*.txt` 四套重复结构，围绕 `snapshots` + `events` 解锁历史、审计、检索、对比、统计。
- **不建议一次性全量替换**：Tier 1 先落历史 + 配置元数据 + 备份索引；Tier 2 的 FTS/图查询/统计独立迭代。
- **注册表仍是真相源**；SQLite 是 sidecar / 历史 / 索引，不是第二个真相源。
- **下一步**：你对 §八 的 6 个问题拍板后，我写 v6.0.0 的 spec（数据模型、迁移、IPC、回滚）与实施计划，再按双窗口流程交给开发窗口。

## 十、核对轮裁断（2026-10-19）

开发窗口的 5 个缺口（G1–G5）与 2 个小提醒**全部采纳**，已并入上文：

| 项                    | 裁断 | 落点                                                                                             |
| --------------------- | ---- | ------------------------------------------------------------------------------------------------ |
| G1 Path / Unsupported | 采纳 | §五 `env_values` 的 `CHECK (lower(name) <> 'path')`、`value` 可空、`restorable`；§六 迁移排除    |
| G2 事件明文旁路       | 采纳 | §五 `events` 改为 `changed_fields` + `before_revision` / `after_revision`；§五 四条硬约束第 2 条 |
| G3 lost update / CAS  | 采纳 | §三 A2、§五 `app_state` / `profiles.version`、§七 R11                                            |
| G4 降级守卫时序       | 采纳 | §六 第 4 条、§八 Q6                                                                              |
| G5 `revision` 弱哈希  | 采纳 | §七 R1、§八 Q2；明确「只存哈希 ≠ 零泄漏」                                                        |
| JSON1 验收            | 采纳 | §三 B6 改为 `json_valid()`；FTS5 仍查编译选项                                                    |
| WAL 备份              | 采纳 | §七 R3 明确「必须 `VACUUM INTO`，禁止直接复制 `.db`」                                            |

**对 6 项建议的裁断**：1 / 3 / 4 / 5 直接采纳；2 采纳并加两点限定（敏感值历史不可回放；`revision` 不是加密）；6 采纳并绑定 G4 的发布时序。**本轮未改代码；调研文档已修订，未提交。**

## 十一、用户拍板（2026-10-19）

| 问题      | 用户选择                               | 备注                                                                                                                                                                                               |
| --------- | -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q1 范围   | **全量 Tier 1+2+3**，**单版本 v6.0.0** | 与开发窗口建议（仅 Tier 1）不同。spec 将按 M1–M4 里程碑组织（存储迁移 → 历史/配置/备份索引 → FTS/统计/图查询 → 同步），但发布号仍为 v6.0.0；每个里程碑独立可验证、可回滚。                         |
| Q2 敏感值 | **SQLCipher 全库加密**                 | 密钥来源：**DPAPI 保护的密钥文件**（用户选择）。敏感值可入库并回放（与「只存哈希」不同）。                                                                                                         |
| Q3 旧备份 | **开发期间保持明文，开发完成后删除**   | 登记为**有条件删除**：先导入新库并抽样验证 → 保留可回滚副本 → 用户明确确认后才删除；不在开发过程中删。                                                                                             |
| Q4 同步   | **文件式变更包**                       | 包用用户口令派生密钥（Argon2id）加密；本地 DB 仍用 DPAPI。因此 schema **从第一天**必须含 `uid` / `updated_at` / `deleted_at` / `changes` / `change_sets`；核对轮「只预留 uid」的建议被此选择覆盖。 |

**新发现的结构性冲突（必须先拍板）**：

> DPAPI 密钥绑定当前 Windows 用户/机器，**跨设备同步的 SQLCipher 数据库在另一台机器上无法解密**。因此「单版本全量（含 Tier 3 同步）+ DPAPI」不能直接同时成立。同步必须采用**独立的便携密钥**（用户口令派生，如 Argon2id）或**局域网内明文/TLS 交换后各端本地重加密**；本地 DB 仍可用 DPAPI。详见 §十二。

---

## 十二、同步模型待定（Tier 3 的前置）

| 方案                        | 做法                                                                                                                           | 优点                                               | 代价                                                  |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------- | ----------------------------------------------------- |
| **A. 文件式变更包（已选）** | 导出/导入加密变更包（change bundle）到用户指定目录（OneDrive/Syncthing/NAS）；包用**用户口令派生密钥**加密；本地 DB 仍用 DPAPI | 无服务器；跨设备可用；与现有「文件可复制」习惯一致 | 需要口令 UX；冲突需 LWW/字段级合并 UI；包的回放要幂等 |
| B. 局域网点对点             | mDNS 发现 + TLS 交换变更                                                                                                       | 无需云盘                                           | 配对/证书/防火墙；实现量大                            |
| C. 自建服务器               | NAS/服务器 API 存储变更                                                                                                        | 体验最好                                           | 认证、存储、冲突、运维，完全超出当前项目范围          |
| D. v6.0.0 不含同步          | 只预留 UUID/墓碑，Tier 3 延后                                                                                                  | 风险最低                                           | 与「单版本全量」冲突，需要用户改口                    |

> **无论选哪个**：同步的「变更集」要求从第一天就有稳定 `uid`、`updated_at`、墓碑（`deleted_at`）与逐字段变更记录；否则后补会重写所有表。若坚持单版本全量，**A 是唯一在 v6.0.0 内可落地的同步形态**（B/C 的工程量另算）。

## 十三、实施边界最终拍板（2026-10-19）

| 项                 | 最终决定                                                                                        | 落点             |
| ------------------ | ----------------------------------------------------------------------------------------------- | ---------------- |
| **B1 NASM**        | 本地 M0 先 no-asm（`openssl-src` 自动降级）；**CI 显式安装 NASM**；构建失败或性能不足再本地安装 | spec §3.4 / M0   |
| **B6 新 env 备份** | **DPAPI 加密文件 `.pebak`**；遗留 `.json` 一次性恢复；M4 用户确认后删除                         | spec §5.3 / §7.9 |
| **B7 DB 故障**     | **fail-open + 降级 `disabled.json` 兜底**；不静默重建空库                                       | spec §7.10       |
| **同步**           | 文件式变更包（Argon2id + ChaCha20-Poly1305）；本地 DB 用 DPAPI                                  | spec §6.3 / §6.4 |

> 调研文档的职责到此为止；后续以 `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` 为准。

---

**审核窗口**：2026-10-19
