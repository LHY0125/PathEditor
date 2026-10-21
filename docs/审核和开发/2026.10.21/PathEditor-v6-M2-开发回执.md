# PathEditor v6.0.0 M2 检索与统计 — 开发回执

- **提交窗口**：开发窗口
- **日期**：2026-10-21
- **交付对象**：审核窗口
- **依据**：`docs/superpowers/plans/2026-10-20-v6-sqlite-m2-plan.md`（1967 行）、`PathEditor-v6-M2-开工提示词.md`（220 行）
- **分支**：`main`
- **状态**：13 个 Task **全部完成**，质量门全绿，**未推送**
- **版本**：未升级（仍为 `5.1.7`）
- **注册表**：零写入（全部测试用 `TempDir` / mock IPC）

---

## 0. 开工前的核对

按开工提示词 §九 的三件事：

1. **基线确认**：`cargo test --workspace` → 22 binaries / 313 passed / 0 failed / 2 ignored；
   前端 27 files / 336 tests；E2E 44 passed。**与 §二 完全一致**。
2. **两张裁断表**：22 条逐条读过（14 + 8），无理解偏差。
3. **Task 1 的 DDL 与 spec §4.1 逐字比对**：`scan_runs` / `var_references` / `daily_stats` /
   `search_fts` 四张表的列名与类型**一致**。唯一的额外项是 `CREATE INDEX idx_scan_runs_time`
   （spec 未写，但 Task 3 按天 group by `created_at` 需要它）。

---

## 1. 交付内容（逐 Task）

| Task               | 提交                  | 交付物                                                                                   | 新增测试                                                     |
| ------------------ | --------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| 1 schema v2        | `a2d26a2`             | `DDL_V2` + `CURRENT_SCHEMA_VERSION = 2` + 迁移分支                                       | `db_schema_v2`、`db_schema_v1_to_v2`                         |
| 2 FTS5 检索        | `bf025bd`             | `search.rs`：`rebuild_index` / `index_snapshot_tx` / `search` / `escape_query`           | `db_search_fts`、`db_search_incremental`、`db_search_escape` |
| 3 统计聚合         | `7d840ac`             | `stats.rs`：`rebuild_daily` / `dashboard` + `Dashboard` 五字段                           | `db_stats_daily`                                             |
| 4 引用图           | `0030d5c`             | `refs.rs`：`parse_refs` / `rebuild_refs` / `dependents` / `dependencies` / `find_cycles` | `db_var_references`                                          |
| 5 快照对比         | `845f959`             | `compare.rs`：双 hive `CompareResult` + `PathDiff.toggled`                               | `db_compare`、`db_compare_user_only`、`db_compare_toggle`    |
| 6 冲突历史         | `d90e9ca`             | `scan_history.rs`：`record_run` / `list_runs` / `diff_runs` + `first_seen`/`last_seen`   | `db_scan_runs`                                               |
| 7 reveal           | `0bea4aa`             | `history::reveal_env_value` + IPC + `SnapshotDetail` 的「显示」按钮                      | `db_reveal_value`、`backend-reveal.test.ts`                  |
| 8 export v5        | `e00a49e`             | `v5_export.rs` + `profiles::load_profile_from` + CLI `profile export-v5`                 | `db_profile_export_v5`                                       |
| 9 IPC + 形状校验   | `f8d5ff5`             | `m2.rs` 12 命令 + `backend.ts` 8 类型 / 7 `parseXxx` / 12 方法                           | `backend-m2-shapes.test.ts`（24 条）                         |
| 10 搜索与统计 UI   | `cab8908`             | `search-store` / `SearchPanel` / `StatsPanel` / `TabId` 扩展                             | `search-store.test.ts`（5 条）                               |
| 11 对比与引用图 UI | `86f4777`             | `CompareDialog` / `RefGraphDialog` / `HistoryPanel` 多选                                 | `m2-dialogs.test.tsx`（5 条）                                |
| 12 派生表接线      | `f81d9e2` + `92b2858` | `setup_database` 第 ⑥ 步 + `service.rs` 的增量索引接线                                   | `db_derived_rebuild`、`db_search_write_path`                 |
| 13 E2E + 文档      | `4c90444`             | `search.spec.ts` / `stats.spec.ts` + README/CHANGELOG                                    | E2E 6 条                                                     |

**另有 1 个提交**：`701e538` 把「没碰用户目录」的元层声明改成行为断言（审核窗口上一轮的建议）。

---

## 2. 设计决策（选了什么 + 为什么）

| #   | 决策                             | 选择                                                                                                                                                 | 理由                                                                                                                                                                                                                                                                                                                                                                                         |
| --- | -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `PRAGMA user_version` 的位置     | 移到两个分支**之外**                                                                                                                                 | M1 原写在 `if current < 1` 内部；v1→v2 升级会执行 DDL 但不落版本号 → 下次启动重复建表报错。已做反向验证（移回内部则 `db_schema_v1_to_v2` 红）                                                                                                                                                                                                                                                |
| 2   | Task 3 的 SQL 合并方式           | `UNION ALL` 摊长表 + `GROUP BY`，**不用** `FULL OUTER JOIN`                                                                                          | 语义相同、不依赖可选特性、与聚合 CTE 混用时更易读。（**更正**：我初期把理由写成「不确定是否编进 SQLite」，那是错的——审核窗口实测支持。真实理由是**可读性**）                                                                                                                                                                                                                                 |
| 3   | FTS5 查询转义的位置              | **core 侧**（`escape_query` 包成 phrase）                                                                                                            | CLI 与任何未来调用方都走同一个 `search()`；放在调用方等于要求每个调用方各实现一遍，忘掉的那个不会被测试抓到                                                                                                                                                                                                                                                                                  |
| 4   | 空 phrase 的写法                 | `""""`（内容为一个引号字符），**不是**空 phrase                                                                                                      | 空 phrase 匹配不到任何行；用户输入一个引号时期望「搜含引号的内容」                                                                                                                                                                                                                                                                                                                           |
| 5   | `first_seen`/`last_seen` 的口径  | 按**全历史**算，不只两次扫描之间                                                                                                                     | spec §6.2 原文是「两次扫描 diff；「首次出现/最近出现」」——若限定在两次扫描内，`first_seen` 等价于 `appeared`（本次是否新出现），与已有字段重复。**只有跨全历史才有独立价值**。这是对规格的解释，不是偏离计划<br><br>**验证**：`db_scan_runs` 构造三段历史（r1/r2/r3），断言 `dup` 的 `first_seen.runUid == r1` 且 `last_seen.runUid == r3`——两者指向**不同**的 run，证明两个字段语义确实不同 |
| 6   | 环检测的返回值                   | 旋转归一化去重（`A→B→A` 与 `B→A→B` 是同一个环）                                                                                                      | 计划版的 `DISTINCT path` 会把同一环按起点重复报 N 次，UI 上显示成 N 个环                                                                                                                                                                                                                                                                                                                     |
| 7   | 对比的维度                       | **重构** `CompareResult`：`{path_added, path_removed, path_reordered}` → `{system: PathDiff, user: PathDiff, env_*}`，并在 `PathDiff` 内加 `toggled` | 两件事：(a) 双 hive 分离是 N-8 裁断，**字段名全变，是 breaking change**；(b) `PathEntry { path, enabled }` 是 `CLAUDE.md` 写死的跨层契约，只比路径串会让「同一条路径从启用变禁用」显示「没有差异」——与 N-8 同一类静默错误                                                                                                                                                                    |
| 8   | `with_db` 的处理                 | 提升 `history.rs` 的为 `pub(crate)` 供 `m2.rs` 复用，**不复制**                                                                                      | 降级判定（`DbState::None` → `code=database`）必须只有一处；两份判断会让同一个 DB 故障在不同命令上给出不同错误码                                                                                                                                                                                                                                                                              |
| 9   | `load_profile_from` 的实现       | 直接复用 `read_profile_file`，**不新写解析**                                                                                                         | 新写一份会让版本迁移规则将来分叉。净效果：`read_profile_file` 一行未动，解析逻辑仍只有一份                                                                                                                                                                                                                                                                                                   |
| 10  | `export_profile_v5` 的无快照配置 | 返回 `NotFound`，**不导出空数组**                                                                                                                    | `{sys:[],user:[]}` 会让用户误以为「配置就是空的」并据此覆盖；真相是数据缺失                                                                                                                                                                                                                                                                                                                  |
| 11  | 三张派生表的重建时机             | `search_fts` 仅空时全量；`daily_stats` / `var_references` 每次全量                                                                                   | 三者成本不同：FTS 重建随快照数线性增长且写入路径已增量维护；另两张只是聚合                                                                                                                                                                                                                                                                                                                   |
| 12  | 增量索引的挂载点                 | `service.rs` 的两处快照创建后，**不在** `insert_snapshot` 内部                                                                                       | 后者是 core 通用入口，`migrate_v5` 也会用；在那里做增量既没必要（迁移后启动序列会全量重建）又会让 core 依赖自己的派生模块                                                                                                                                                                                                                                                                    |
| 13  | 图表                             | CSS 宽度条，**不引入图表库**                                                                                                                         | 依赖里没有任何图表库；为几条柱状图引入一个与「M2 不引入新依赖」冲突                                                                                                                                                                                                                                                                                                                          |
| 14  | 统计的口径                       | 「当前状态」与「历史序列」**分两块显示** + 显式文案                                                                                                  | M-1 的裁断：某天的 `pathItems` 是「那天最后一次保存时」的值，不是「现在」                                                                                                                                                                                                                                                                                                                    |

---

## 3. 偏离记录

| #   | 计划原文                                     | 实际实现                                      | 处理方式与理由                                                                                                 |
| --- | -------------------------------------------- | --------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| 1   | Task 4 的测试：插完快照直接调 `rebuild_refs` | 补 `set_current_snapshot`                     | `rebuild_refs` 只处理 `app_state.current_snapshot_id`；不设就是 0 条边。**这是测试的前提，不是放宽断言**       |
| 2   | Task 4 的测试：`env` 里放 `name: "PATH"`     | 换成 `JAVA_HOME` 等合法名字                   | `insert_snapshot` 的 `CHECK (lower(name) <> 'path')` 会拒绝；计划自己的注释也建议如此                          |
| 3   | Task 3：`FULL OUTER JOIN`                    | `UNION ALL` + `GROUP BY`                      | 见设计决策 #2。**理由已按审核窗口的更正修正**（初版误写为「特性不确定」）                                      |
| 4   | Task 5：`CompareResult` 三字段               | 五字段（加 `system`/`user` 分离与 `toggled`） | `system`/`user` 分离是 N-8 裁断；`toggled` 见设计决策 #7                                                       |
| 5   | Task 8：新写 `load_profile_from` 的完整实现  | 只做「存在性检查 + 调 `read_profile_file`」   | N-1 裁断要求复用；**新增函数里的每一行都应当在回答原函数没回答的问题**，这里只多回答了「路径能否由调用方指定」 |
| 6   | Task 9：`m2.rs` 内含 `with_db` 副本          | 提升 `history.rs` 的为 `pub(crate)`           | 见设计决策 #8                                                                                                  |
| 7   | Task 11：三个方向分别取                      | `Promise.all` 并行                            | 三个请求互不依赖，串行只会让对话框更慢。**不是语义变更**                                                       |
| 8   | Task 12：只写 `setup_database` 第 ⑥ 步       | **补上 `service.rs` 的增量索引接线**          | **计划策略表写了「每次写入快照后增量」，我首版只做了「启动时空则全量」。详见 §5 事故与自纠**                   |

---

## 4. 质量门（实际输出）

```
$ cargo fmt --all -- --check
exit 0

$ cargo clippy --workspace --all-targets -- -D warnings
exit 0

$ cargo test --workspace
binaries=37  passed=329  failed=0  ignored=2

$ npx vitest run
Test Files  31 passed (31)
Tests  374 passed (374)

$ npx tsc -b
exit 0

$ npx eslint .
✖ 3 problems (0 errors, 3 warnings)

$ npm run test:e2e
50 passed (1.5m)

$ npm run verify:all
EXIT=0
```

### 4.1 与基线的对照

| 项            | 基线 | 完成    | 变化 |
| ------------- | ---- | ------- | ---- |
| Rust          | 313  | **329** | +16  |
| Rust binaries | 22   | **37**  | +15  |
| 前端          | 336  | **374** | +38  |
| 前端 files    | 27   | **31**  | +4   |
| E2E           | 44   | **50**  | +6   |

**没有任何一项下降。**

### 4.2 ESLint 的 3 个 warning

```
src/components/env-list/EnvVarTable.tsx:52   react-hooks/incompatible-library
src/components/path-list/MergePreview.tsx:39 react-hooks/incompatible-library
src/components/path-list/PathTable.tsx:91    react-hooks/incompatible-library
```

**这 3 个 warning 全部是既有代码，M2 未新增。** 成因是 `@tanstack/react-virtual` 与
React Compiler 不兼容。已用 `git stash` 对比确认：基线同样为 3 个。

### 4.3 一文件一测试

`core/tests/db_*.rs` 共 **31 个文件**，逐个 `grep -c '#\[test\]'` 全部为 **1**，零违规。

新增 15 个测试文件（其中 12 个是本轮新建，其余为前序轮次）。

---

## 5. 事故与自纠（放在显眼处）

### 5.1 `index_snapshot_tx` 首版零生产调用者

**这是本轮最严重的漏项，性质与 M1 的 V-3 相同。**

计划 Task 12 的策略表写的是：

> `search_fts`：启动时若 `count(*) = 0` 则全量重建；**每次写入快照后增量（`index_snapshot_tx`）**

我首版只实现了**前半句**。`index_snapshot_tx` 有实现、有单测（`db_search_incremental`），
但 `grep -rn "index_snapshot_tx" --include="*.rs" | grep -v core/src | grep -v core/tests`
**零命中**。

**后果**：用户改完 PATH 立刻去搜索，找不到刚保存的内容，要等下一次启动的全量重建才出现。

**发现方式**：写 CHANGELOG 的「已知限制」节时，要如实描述它的接线状态——**写文档的诚实要求逼出了代码缺陷**。

**修复**：`92b2858` 在 `service.rs` 的两处快照创建后调 `index_snapshot_by_uid`，
并补 `db_search_write_path.rs` 守「新写入的快照不重启就能搜到」。

**验证**：`grep -rn "index_snapshot_by_uid" gui/src/` → 2 处调用（77 行、155 行）。

### 5.2 两次误删代码（已恢复，无遗留）

用 `Edit` 做「插入新函数」时，`old_string` 写成「被插入位置的开头几行」，
替换后原文被吃掉。两次都靠诊断/`git diff` 及时发现并恢复。

**规范**（审核窗口上一轮给的，我这一轮仍违反了一次）：`new_string` 必须**完整包含** `old_string`。

### 5.3 计划里没有、我加的维度

- **`CompareResult` 重构 + `PathDiff.toggled`**（设计决策 #7）——已在 §3 偏离记录 #4 中列出。
  ⚠️ 注意这是 **breaking change**（字段名全变），不是新增字段。
- `with_db` 复用（设计决策 #8）
- `find_cycles` 的旋转归一化（设计决策 #6）

---

## 6. 未覆盖项

### 6.1 计划指定的 4 项（**原样抄录，不得省略**）

| #   | 未覆盖项                                                                   | 归属              | 原因                                                                                                                                  |
| --- | -------------------------------------------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | env 变量的 N 路对比（`env_added`/`env_removed`/`env_modified` 恒为空数组） | M3                | 掩码值在对比结果里如何呈现没有定义（显示 `••••`？显示「已修改但不可见」？两边都敏感且都改了无法判断是否相同时怎么办），硬做会误导用户 |
| 2   | CLI 不重建派生表；CLI 改 PATH 后 `var_references` / `daily_stats` 保持陈旧 | 未排期（M1 R-02） | 需 CLI 也接 DB 的 current 快照，属范围扩张；**M2 不顺手修**                                                                           |
| 3   | 真实注册表闭环测试                                                         | 用户授权后        | E2E 用 mock IPC                                                                                                                       |
| 4   | CI（MSVC）验证                                                             | 推送后            | 本机为 GNU                                                                                                                            |

### 6.2 本轮额外发现的

| #   | 未覆盖项                                                        | 说明                                                                                                                                                                                  |
| --- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 5   | `gui::rebuild_derived_tables` 的**编排顺序**未测试              | `db_derived_rebuild.rs` 守的是「三个 core 函数能独立完成重建」；`setup_database` 里「search 仅空时重建、daily/refs 每次重建」这层编排没有测试（`gui` 的私有函数，外部集成测试拿不到） |
| 6   | `db_search_write_path.rs` 守的是「core 的两个函数放一起能工作」 | 它**不**证明「gui 真的把它们放在一起了」。后者的证据是 `service.rs` 的 2 处调用（grep 可查），不是测试                                                                                |
| 7   | 统计面板与搜索面板的真实交互细节                                | E2E 覆盖了「能显示」与「不发请求」，没覆盖分页、滚动、键盘导航                                                                                                                        |
| 8   | 引用图对话框的 `refs_cycles` 显示的是**全部**环                 | 组件内按变量名过滤出「该变量参与的环」，但全局环列表本身没有独立的 UI 入口                                                                                                            |

---

## 7. 残余风险

| #    | 风险                                 | 等级   | 说明                                                                                       |
| ---- | ------------------------------------ | ------ | ------------------------------------------------------------------------------------------ |
| R-01 | FTS5 的 `unicode61` 分词对中文不友好 | 非阻塞 | 中文会被当整段 token，搜「环境」匹配不到「环境变量」。当前 PATH 场景以英文路径为主         |
| R-02 | `rebuild_daily` 每次启动全表扫描     | 非阻塞 | 已按 T-1 加耗时日志；数据量增长后的阈值判断留待日志数据                                    |
| R-03 | `'localtime'` 依赖进程时区           | 非阻塞 | 跨日边界的测试已避开（M-2）；生产环境若时区变化，历史 `daily_stats` 会重算                 |
| R-04 | 增量索引与全量重建可能不一致         | 非阻塞 | `db_search_incremental` 断言了「同一快照两种路径产出行数一致」，但没覆盖「多快照混合场景」 |
| R-05 | 引用图的表只保存当前快照的边         | 非阻塞 | 不变量已在文档注释写死；若将来改增量必须同步给三条查询加 `snapshot_id` 过滤                |
| R-06 | 快照对比的 `reordered` 用绝对 index  | 非阻塞 | 中间插入一条会把后面全部判为重排。UI 已用「第 N 位 → 第 M 位」呈现，不误导                 |

---

## 8. 需要审核窗口重点关注的项

1. **`first_seen`/`last_seen` 的口径**（设计决策 #5）：按**全历史**实现。
   理由：若限定在两次扫描内，`first_seen` 与 `appeared` 重复，没有独立价值。
   已用 `db_scan_runs` 的三段历史（r1/r2/r3）锁定：`dup` 的 `first_seen` 指向 r1、
   `last_seen` 指向 r3，**两者必须是不同的 run**。
   这条是**对规格的解释而非偏离**，如果你的理解不同需要改。

2. **`CompareResult` 是 breaking change，不是「多了字段」**（设计决策 #7）：

   ```text
   旧：{ path_added, path_removed, path_reordered }
   新：{ system: { added, removed, reordered, toggled }, user: {...}, env_added, env_removed, env_modified }
   ```

   **字段名全变**（`path_added` → `system.added`），不只是新增 `toggled`。任何第三方调用
   `compare_snapshots` 都会直接解析失败。

   影响范围已核实：`grep -rn "path_added\|path_removed\|path_reordered" --include=*.ts --include=*.tsx --include=*.rs`
   → 只有 `backend.ts` 的 `parseCompareResult` 与 `CompareDialog.tsx`（均已同步为
   `system.*` / `user.*`），**CLI 不调 compare，无其它消费方**。

3. **`load_profile` 的错误文案变化**：从 `配置文件不存在: {name}` 变成 `配置文件不存在: <完整路径>`。
   你上一轮确认过保留完整路径，且 `grep -rn "配置文件不存在" core/tests/ cli/src/` 零命中——
   **这条作为「行为变化」记录在此**。

4. **ESLint 的 3 个 warning**（§4.2）：全部既有，M2 未新增。

5. **§5.1 的自我纠错**：`index_snapshot_tx` 首版零调用者，是我写 CHANGELOG 时发现的。
   建议下一轮把「每个新增的 `pub fn` 是否有生产调用者」列为固定检查点——
   这已经在 M1 的 V-3 与本轮的 §5.1 上各出现过一次。

---

## 9. 提交清单与状态

```
4c90444 test(v6): 搜索与统计 E2E；文档同步
92b2858 fix(v6): 接上 index_snapshot_tx 的生产调用者
f81d9e2 feat(v6): 派生表重建策略与启动接线
86f4777 feat(v6): 快照对比与引用图 UI
cab8908 feat(v6): 搜索与统计面板
f8d5ff5 feat(v6): M2 IPC 命令与前端形状校验
701e538 test(v6): 把「没碰用户目录」从元层声明改成行为断言
e00a49e feat(v6): profile_export_v5 —— 降级到 v5 文件的退路
0bea4aa feat(v6): history_reveal_env_value —— 敏感值显式 reveal
057e1b2 docs(v6): 修正 escape_query 的注释措辞
d90e9ca feat(v6): 冲突历史持久化与两次扫描 diff
845f959 feat(v6): N 路快照对比（双 hive + 启用状态）
0030d5c feat(v6): %VAR% 引用图（解析/递归依赖/环检测）
7d840ac feat(v6): daily_stats 聚合与统计面板数据
bf025bd feat(v6): FTS5 检索与索引重建（不索引敏感值）
a2d26a2 feat(v6): schema v2 —— M2 派生表
```

**16 个提交。** 未推送、未合并、未升级版本号、未写注册表。

---

## 10. 交付物位置索引

| 交付物    | 路径                                                                                                 |
| --------- | ---------------------------------------------------------------------------------------------------- |
| schema v2 | `core/src/db/schema.rs`、`core/src/db/mod.rs`                                                        |
| FTS5      | `core/src/db/search.rs`                                                                              |
| 统计      | `core/src/db/stats.rs`                                                                               |
| 引用图    | `core/src/db/refs.rs`                                                                                |
| 对比      | `core/src/db/compare.rs`                                                                             |
| 冲突历史  | `core/src/db/scan_history.rs`                                                                        |
| v5 导出   | `core/src/db/v5_export.rs`、`core/src/profiles.rs`                                                   |
| IPC       | `gui/src/commands/m2.rs`、`gui/src/commands/history.rs`                                              |
| 启动接线  | `gui/src/lib.rs`、`gui/src/commands/service.rs`                                                      |
| 前端      | `src/services/backend.ts`、`src/store/search-store.ts`、`src/components/{search,stats,history,env}/` |
| E2E       | `e2e/tests/search.spec.ts`、`e2e/tests/stats.spec.ts`                                                |
| 文档      | `README.md`、`CHANGELOG.md`                                                                          |

---

**签章**：§4 的每个「通过」都来自实际执行的命令输出。§6 的 8 项未覆盖与 §7 的 6 项残余风险如实列出，
未在任何地方声称完成。§5.1 的事故由我自己发现并修复，已附验证命令。
本轮未推送、未合并、未升级版本号、未写真实注册表。
