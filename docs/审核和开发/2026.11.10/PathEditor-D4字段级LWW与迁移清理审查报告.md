# PathEditor D4 字段级 LWW 与迁移清理审查报告（修订版）

- **审查日期**：2026-11-10
- **被审对象**：当前工作区未提交改动；基准提交 `f21480e`
- **功能**：字段级 LWW（D4）、同步协议 v2、旧库/旧包兼容层清理、迁移备份命令移除
- **规格来源**：`docs/superpowers/specs/2026-11-08-v7-db-program-design.md` §12、`docs/superpowers/plans/2026-11-08-v7-db-program-implementation.md`、用户 2026-11-10「不兼容旧版本、精简迁移代码」指令
- **规范来源**：`AGENTS.md` / `CLAUDE.md`
- **范围**：38 个已跟踪文件（+541 / −761），新增 3 个测试文件（其中一个为改名）；工作区未提交

## 修订说明

初版报告有三处需要纠正，本版已改写：

1. 初版把 `P1-2 GUI 完全静默` 写错了。项目已有 `DegradedBanner` 并常驻 `AppShell`；真实问题是**错误原因丢失、提示不可行动、重启建议误导**。
2. 初版把 `deleted_at + content` 与「其它 profile 字段必须带 content」并列。后者会误杀合法包：配置软删后 `get_profile` 返回 `None`，导出方对**所有** profile 旧变更都只能带 `content = null`。本项降为 P2，只保留 `deleted_at` 带 content 这一条不变量。
3. 初版漏掉了发布版本事实与 `seq` 溢出。已补：v6.2.2 schema 4、v7.0.0 schema 5、schema 6 只存在于未发布的 `f21480e`。

### 第二次修订（2026-11-10 定稿前）

- P1-1 拆为 P1-1a（schema 6 版本标识）与 P1-1b（4/5 已发布库拒绝），明确“抬到 7 不恢复旧库”。
- P1-1b 的测试缺口改为“现有用例只覆盖空库 + user_version = 5；需扩展真实结构并覆盖 4/6”，不再声称从零新增。
- P1-3 补充 device_id 可被同时伪造、`seq <= i64::MAX - 2` 边界与复现测试；seq 溢出标注为按同类 SQL 路径推断、待测试固定。
- P1-4 验收谓词改为“任何 `schema_version != 7` 的 v2 包均拒绝”。
- `profile.content` 的 snapshotUid = null 条目移出 P2，单列 P3/待确认。
- README 旧语义行号改为 431（430-432 整段）。

## 事实基线（已核对 tag）

| 版本       | 来源                                         | `CURRENT_SCHEMA_VERSION` |
| ---------- | -------------------------------------------- | ------------------------ |
| v6.2.2     | `git show v6.2.2:src/core/src/db/schema.rs`  | 4                        |
| v7.0.0     | `git show v7.0.0:src/core/src/db/schema.rs`  | 5                        |
| 未发布     | `git show f21480e:src/core/src/db/schema.rs` | 6                        |
| 当前工作区 | `src/core/src/db/schema.rs:7`                | 6                        |

## 一、验证证据

| 命令                                                                | 实际结果                                                                         |
| ------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `cargo test --workspace`                                            | **362 passed / 0 failed / 2 ignored**                                            |
| `npx vitest run`                                                    | **42 files / 466 passed**                                                        |
| `npm run test:coverage`                                             | 行覆盖率 **80.2%**（阈值 80%）                                                   |
| `npx playwright test --config src/gui/web/e2e/playwright.config.ts` | **59 passed**                                                                    |
| `npm run verify`                                                    | 通过（一致性 / Prettier / ESLint / build / coverage / fmt / clippy / Rust 全量） |

质量门为报告时点结果，**不能替代契约判断**。下述 P1 均未被现有测试捕获。

## 二、P1

### P1-1a 版本语义：schema 6 已不唯一，必须抬到 7

**事实**

- `src/core/src/db/schema.rs:7` 仍为 6。
- `src/core/src/db/mod.rs:99-119`：`current == 6` 直接通过；`current != 0 && != 6` 一律 `Migration`。
- schema 6 只存在于未发布的 `f21480e`；v7.0.0 是 5，v6.2.2 是 4。
- 本批删除了 `migration_imports`，并把 `pinned` 从 ALTER 折入基础 DDL。即使 `user_version = 6`，新库与旧 schema 6 开发库的物理结构也不同；版本 6 已不能唯一标识 DDL。

**故障链：schema 6 开发库会自产不可导入的 v2 包**

1. `f21480e` 写入的 profile 变更是 `field = NULL`（整实体）。
2. 当前导出不检查字段形状，`export_bundle` 以 `FORMAT_VERSION = 2` 写出（`src/core/src/db/sync/export.rs`）。
3. 当前导入的 `open_bundle` 明确要求 profile 带字段名（`src/core/src/db/sync/import.rs:199-221`）。
4. 结果：同一台机器升级后，把未导出的旧变更导出，再导入会被自己的包拒绝。

**结论**

只保留一个方案：**把 schema 基线抬到 7，旧库（4/5/6）一律拒绝，不写迁移**。不要再把“恢复 `field = null` 兼容”作为并列方案——它与用户 2026-11-10 指令冲突，并会长期维护两套 profile 语义。

**注意**：抬到 7 只解决 schema 6 的**版本标识**问题，不会让已发布的 4/5 库恢复可用；这是 P1-1b 的既定策略。

**触发条件**：未发布的 schema 6 开发库被继续接受，或导出其旧 pending profile 变更。

**后果**：字段 LWW 误判或自产包不可导入。

**测试缺口**：没有“schema 6 旧字段变更导出后自导入”用例。

**验收标准**：新库写 7；schema 6 库拒绝；schema 7 的 v2 包可往返；文档中的 6 全部改 7。

---

### P1-1b 已发布库全量拒绝：4/5 库不可用，这是策略不是 bug

**事实**

- `v7.0.0`（`abced04`）的 schema 是 5，`v6.2.2` 是 4；当前 `migrate_to_current` 对 `current != 6` 全部拒绝。
- 因此所有已发布用户升级后，历史/同步/配置库不可用；这是本批最大的用户面 breaking change。
- 抬到 7 不会改变这一点；4/5 库仍会被拒绝。需要发布说明与 GUI 可行动指引（与 P1-2 联动），不能把“抬版本”宣传成迁移恢复。

**触发条件**：用户从 v7.0.0 / v6.2.2 升级。

**后果**：历史/同步/配置能力重置；若没有可行动提示，用户无法自助恢复。

**测试缺口（修正初版）**：`src/core/tests/suites/schema/db_schema_open.rs:90-105` 已有 `old_schema_versions_are_rejected_without_mutation`，但它只覆盖“空库 + `PRAGMA user_version = 5`”的版本拒绝，不覆盖真实 v5/v6 结构库，也未覆盖 4/6 边界。应扩展该用例，而不是声称从零新增。

**验收标准**：4/5/6 库启动即拒绝且 `user_version` 不改；测试覆盖 4、5、6 三个版本（至少一个带真实结构的 fixture）；发布说明明确“旧库不迁移，需备份后重建”。

**文档同步清单**：`docs/dev/architecture.md:27`、`docs/dev/sqlite-storage.md:13`、`AGENTS.md:121`、`CLAUDE.md:121` 均写死 6。

---

### P1-2 降级提示存在，但原因丢失、不可行动、重启建议误导

**事实（修正初版）**

- `src/gui/web/src/components/layout/DegradedBanner.tsx:23` 已实现，`AppShell.tsx:258` 常驻挂载。
- `history_get_current` 在 DB 为 `None` 时返回 `database` 错误（`src/gui/src-tauri/src/commands/history.rs:94`），banner 会显示。
- 真实缺陷在 `src/gui/src-tauri/src/commands/db_state.rs:25/41`：`with_db` 把打开错误替换成通用文案“历史库不可用，当前处于降级模式”，原始 `Migration` / `Crypto` 原因丢失。
- `DegradedBanner.tsx:69-75` 的提示是“重启后可重试”；对 schema 不兼容或旧加密库，重启永远不会成功。
- 没有“打开数据目录”入口；`open_data_dir` 已存在（`src/gui/src-tauri/src/commands/fs.rs:21`），只是没接到 banner。
- CLI 反而保留了原始错误：`src/cli/src/runtime.rs:63` 打印具体 message。

**触发条件**：旧 schema 库或旧 SQLCipher 库启动。

**后果**：用户看到“降级”但不知道原因，按“重启”操作无效，也无法定位数据库文件。

**验收标准**：`DbState` 保存初始化错误的 `code/message`；banner 展示根因；按错误码提供“打开数据目录 / 备份后删除旧库”操作；去掉无条件的“重启后可重试”。

**测试缺口**：没有前端测试断言 banner 显示 `Migration` 根因和操作按钮。

---

### P1-3 `lamport` 与 `seq` 都可被单条变更打爆

**成立：单条 `lamport` 无上限**

- `src/core/src/db/sync/import.rs:229` 只钳制 header 的 `base_lamport`。
- `src/core/src/db/sync/import.rs:338/436` 直接使用包内 `c.lamport`、`c.seq`、`c.device_id` 落库。
- `resolve` 高者胜，`lamport = i64::MAX` 会让本地后续所有变更永远输。

**新增：`seq` 溢出会永久卡死本地记账**

- `src/core/src/db/sync/change_log.rs:109-115` 用 `MAX(seq) + 1` 取本机下一个序号。
- 恶意包使用**本地 device_id** 写入 `seq = i64::MAX` 后，SQLite 的 `+1` 溢出为 REAL，`r.get::<i64>` 抛类型错误，之后本地每次写变更都失败。
- 本地 device_id 可从本机 `changes` / `app_state` / 已导出包获得，不是秘密。
- **只校验 `c.device_id == header.device_id` 不够**：攻击者可以把两者都伪造成接收方的本地 device_id。必须显式拒绝 `c.device_id == local_device_id` 的远端变更，或更彻底地把本地 `next_seq` 改为 `app_state` 中的独立计数器（schema 7 一并加列）。
- **`0 <= c.seq < i64::MAX` 仍留窗口**：`i64::MAX - 1` 会通过，下一次本地 `+1` 仍溢出。要么要求 `c.seq <= i64::MAX - 2`，要么在饱和时返回明确错误并停止写入，不能只做静默饱和。

**触发条件**：持有同步口令的一方提供 device_id 伪造成本地、seq 接近上限的构造包。

**后果**：LWW 永久失效或本地同步记账永久卡死。

**证据级别**：`seq` 溢出结论按 `record_change_tx` 对 `lamport` 的同类 SQL 溢出路径推断，**尚未实测**；整改时必须用复现测试固定错误形态。

**复现测试**：导入 `device_id = 本地`、`seq = i64::MAX` 的包后，执行一次本地 `rename_profile`，断言不会永久失败。

**验收标准**：导入时校验 `0 <= c.lamport <= 钳制后的 base_lamport`、`0 <= c.seq <= i64::MAX - 2`，并拒绝 `c.device_id == local_device_id` 的远端变更（或改用 app_state 独立计数器）；`next_seq` 饱和时返回明确错误；补负数、`i64::MAX`、`i64::MAX - 1`、header base 超半区测试。

---

### P1-4 `BundleHeader.schema_version` 导入侧完全未校验

- 字段定义在 `src/core/src/db/sync/bundle.rs:55`，导入侧从未读取。
- v2 文件格式 + `schema_version = 5/7` 的包仍可进入已知类型分支，可能按错误 schema 语义落库。
- 用户已选择不兼容旧版本，谓词应明确为 `header.schema_version == CURRENT_SCHEMA_VERSION`，而不是 `<=`。
- **影响面**：现有 sync fixture 大量写死 `schema_version: 3`（`db_sync_profile_idempotent.rs:23`、`db_sync_profile_invalid_name.rs:28`、`db_sync_profile_name_conflict.rs:59`、`db_sync_profile_order.rs:16`、`db_sync_profile_resurrect.rs:23`、`db_sync_profile_tombstone.rs:49`、`db_sync_import_limits.rs:26`；`db_sync_profile_fields.rs:18/270` 为 6）。加校验要同批改这些 fixture，否则测试全红。
- `db_bundle_crypto.rs:9` 的 header 为 1，但只测 `open` 不测 import；若把校验放进 `import_bundle`，不受影响。

**验收标准**：任何 `schema_version != 7` 的 v2 包均报 `SyncFormat`；7 通过。4/5/6/8 全部拒绝，不只 5/7；fixture 全部改为 7。

---

## 三、P2

1. **文档仍在宣传旧语义**（必须与本批同改）：
   - `README.md:431`（430-432 整段）、`README.md:443` 仍写“`snapshot`/`profile` 按整实体 LWW”“已知限制：整实体，不是字段级”。
   - `AGENTS.md:123`、`CLAUDE.md:123` 仍写“`snapshot`/`profile` 按整实体比较”。
   - 这不是文案问题：用户和后续 AI 会按错误语义行动。
2. **`deleted_at + content` 形状不变量**（由初版 P1 降级）：
   - 触发：构造 `field = "deleted_at"` 且 `content` 非空。
   - 当前路径：墓碑分支不成立（`import.rs:383`）→ 走 `apply_entity_tx` → `import_profile_content_tx` 清空 `deleted_at`（`profiles.rs:434-452`），复活配置。
   - **不要扩大为“所有 profile 字段必须有 content”**：`get_profile` 过滤软删（`profiles.rs:591`，底层 `list_profiles` 在 `:562` 过滤），导出方对已删除配置的所有字段变更都会返回 `content = null`（`export.rs:111`）。合法增量包可以同时含 `name content=null` 和 `deleted_at content=null`。
   - 正确不变量：`field == "deleted_at"` ⇒ `content` 必须为 null；其它字段 `content = null` 只表示“实体已删除/无载荷”，最多计数为 `skipped_no_content`，不要当丢数据拒绝整包。
3. **`seen_fields` 与 `seen_entities` 都是先到先得**（`import.rs:409` 等），与“后到 content 才是当前值”的注释相反。当前 exporter 给每条变更附当前内容，通常等价；手写/未来包会出现实体与变更日志不一致。
4. **`migrate_to_current` 改名影响面**：除 `runtime.rs:63`，还包括 `open_and_migrate`、CLI `runtime` 三处调用、`bootstrap.rs` 注释、测试名与 `docs/dev/*`。只改一处会让命名继续分裂。
5. **规则应用审计**：`RulesDialog.tsx:55` 在用户保存 PATH 前就以 `record=true` 写 `rule_hits`。这是产品语义问题，列“待产品确认”，不直接判缺陷。
6. **规则启停/删除无 `.catch`**：`RulesDialog.tsx:123/134`，后端失败时无用户反馈。
7. **N 路对比只显示计数**：`MultiCompareDialog.tsx:52-75` 没有逐条路径/重排/启停差异。
8. **配置差异漏内容**：`ProfileDetail.tsx:164-207` 未渲染 `leftDescription/rightDescription`，`reordered/toggled` 只显示数量。
9. **冲突 `deleted_at` 无 i18n**：`SyncCenter.tsx:313-316` + locale 只有 `name/description/tags/content`，墓碑显示原始 key。
10. **搜索起止日期无先后校验**：`SearchPanel.tsx:246-252`，起止反了只得到空结果。

## 三之二、P3 / 待确认

- **`profile.content` 在 `snapshotUid = null` 时只更新 `updated_at/version`，不更新 revision**（`profiles.rs:434` 起）。profiles 表没有 current_snapshot 列、revision 是 append-only，何为“完整落地”需先定义；补可达场景后再决定是否升 P2。

## 四、Spec 轴

- **D4 旧包回退要求已被 2026-11-10 用户指令覆盖**：spec §12 仍写“旧包 `field = null` 视为 content”，当前实现直接拒绝。计划已更新，但 spec 未加 superseded 标记；下次审核会继续按旧文判。必须在 spec 顶部标注。
- **编号纠正**：初版把 #13 写成 D5。设计文档中 D5 是“新增读查询必须带 LIMIT”（`design.md:48`），#13 才是“同步包依赖自动补齐”（`:166`）。D4 依赖的是 #13，不是 D5。
- **#13 未实现**：导入只含后续变更、缺少前置快照的包仍会失败；发布说明必须写清，不能宣称“任意一包可导入”。
- **缺失测试**：`tags` 字段独立合并、`description/tags` 冲突解决、A→B→A 双向幂等、`deleted_at+content` 拒绝、头部 schema 版本、lamport/seq 边界均无测试。
- **合规亮点**：v1 包在解密前按版本拒绝；新库单事务建全；profile 字段写入使用参数化 SQL；冲突“采纳远端”只写冲突字段；现有字段级测试覆盖不同字段/同字段冲突/墓碑/v2 导出往返。

## 五、未跟踪测试与复现清单

- `src/core/tests/suites/sync/db_sync_profile_fields.rs`
- `src/core/tests/suites/schema/db_schema_derived.rs`（由 `db_schema_v2.rs` 改名）
- `src/gui/web/tests/unit/backend-new-methods.test.ts`
- 用户未跟踪的 `.agents/skills/*` 不属于本批代码，未纳入审查。

## 六、结论与整改顺序

**判定：需要整改后复审；当前状态不建议合并/发布。**

1. schema 7 + `BundleHeader.schema_version == CURRENT` + 旧 DB/v2 包拒绝测试 + 所有文档版本同步。
2. lamport / seq / device_id 上限与冒用校验，补边界测试。
3. GUI 错误传播与可行动指引（Open Data Dir / 删除旧库），去掉误导性重启提示。
4. 只拒绝 `deleted_at + content` 这一种形状；其它 `content = null` 按无载荷处理。
5. 文档旧语义、CLI 命名、不可达分支、测试补全与 UI 细节。

质量门全绿只能证明现有断言通过，不能证明上述契约成立。
