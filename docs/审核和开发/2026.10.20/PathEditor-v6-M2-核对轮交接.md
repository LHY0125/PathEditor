# PathEditor v6.0.0 M2 计划核对 — 交接文档

- **提交窗口**：开发窗口
- **日期**：2026-10-20
- **交付对象**：审核窗口
- **核对对象**：`docs/superpowers/plans/2026-10-20-v6-sqlite-m2-plan.md`（1440 行，13 个 Task）
- **对照权威**：`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` §4.1 / §4.2 / §6.2 / §3.2.1
- **判定**：**可以开工，建议先修 1 处阻塞 + 4 处一致性**。架构方向、任务切分、质量门与 Global Constraints 我全部认同。
- **状态**：本轮**未写任何代码**，未提交、未推送、未升级版本号、未碰注册表。

> **本文档有过一版作废稿。** 初版里 S-1（`var_references` 列数矛盾）、S-3（`?` 错误转换）、
> M-3（`parse_refs` 边界）三条是**我凭记忆写下的、未经核实的主张**，逐字比对计划原文后发现
> **三条都不成立**。已全部删除，更正记录见 §9。留下的每一条都附了可复核的行号或命令。

---

## 0. 结论摘要

| 类别                               | 数量 | 是否需要先修 |
| ---------------------------------- | ---- | ------------ |
| 阻塞（测试会写用户真实目录）       | 1    | **是**       |
| 计划内部自相矛盾（能编译，自打嘴） | 4    | 建议先修     |
| 语义/口径问题（需写清，非 bug）    | 6    | 建议同轮补   |
| 待裁断                             | 3    | 是           |

---

## 1. 已核实的事实核对表

下表每一条都实际打开文件比对过，行号可复查。

| #   | 计划原文位置                        | 仓库实际                                                                                                                                    | 结论                         |
| --- | ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- |
| ✅  | Task 1 `DDL_V2`（113–144 行）       | `var_references` 四列含 `snapshot_id`，主键与 spec §4.1 一致；`scan_runs` 额外加了 `idx_scan_runs_time`                                     | **正确**（初版误报，已撤销） |
| ✅  | Task 2 `rebuild_index`（296–360）   | 三处 `query_map` 都写了 `.map_err(\|e\| super::db_err(...))?`                                                                               | **正确**（初版误报，已撤销） |
| ✅  | Task 4 `parse_refs`（757–778）      | 逐字复刻成 JS 跑边界：`%A%%B%` → `["A","B"]`；`%` → `[]`；`%A% %B%` → `["A","B"]`                                                           | **正确**（初版误报，已撤销） |
| ❌  | Task 8「测试断言往返一致」          | `core::profiles::load_profile`（`core/src/profiles.rs:203`）内部走 `profiles_dir()` = `dirs::home_dir()`（同文件 13–18 行），**无法重定向** | **阻塞**，见 B-1             |
| ⚠️  | Task 3 接口声明（437 行）           | `Dashboard` 声明有 `days` / `top_targets` / `profile_applies` 三字段                                                                        | 见 C-1                       |
| ⚠️  | Task 3 实现里的 `Dashboard`（551）  | 实际只定义了 `days` 一个字段                                                                                                                | 见 C-1                       |
| ⚠️  | Self-Review #1（1424 行）           | 说「在 `dashboard()` 里实时算 `avg_len`」，但 `dashboard()` 只查 `daily_stats`，该表无长度列                                                | 见 B-2                       |
| ⚠️  | Task 2 接口声明（196 行）           | 声明了 `index_snapshot_tx`，Task 2 的代码块里**没有实现它**；Task 12（1374 行）却依赖它                                                     | 见 C-2                       |
| ⚠️  | Task 5 `use` 列表（1028 行）        | 引入了 `ErrorCode`，但全文件未使用；结尾还有一行自认的「占位注释」（1115 行）                                                               | 见 C-3                       |
| ⚠️  | Task 10「新增 `TabId`」（1324 行）  | `TabId` 已存在于 `src/core/path-capabilities.ts:3`，且 `app-store.ts:16` 已 re-export                                                       | 措辞需改，见 C-4             |
| ⚠️  | Task 6 测试（1160–1178 行）         | 只断言 `scan_runs` 与 `diff_runs`，**没有断言 `daily_stats.conflicts` 被重建**                                                              | 见 M-6                       |
| ✅  | Task 12 `scan_runs` / `daily_stats` | 表在 Task 1 一次建出；Task 3 的 SQL 引用 `scan_runs` 时它已存在                                                                             | 正确                         |
| ✅  | Task 6「不能嵌套事务」              | 计划已明确写「`with_transaction` 写完后，**再单独调** `rebuild_daily`」                                                                     | 正确（初版误报，已撤销）     |
| ℹ️  | Task 13「用简单 bar/line 元素」     | 依赖里无任何图表库（`@tanstack/react-virtual` 是唯一 UI 库）                                                                                | 方向对，工作量需再估         |

---

## 2. 阻塞项

### B-1 Task 8 的测试会写进用户真实配置目录

```rust
// core/src/profiles.rs:13-18
fn profiles_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor").join("profiles")
}
```

`load_profile(name)`（同文件 203 行）只接受**名字**，路径由 `profiles_dir()` 算出。
计划 Task 8 的验收是「export → 写成文件 → `load_profile` 读回」——照写会在
`C:\Users\33644\.patheditor\profiles\` 下留下测试文件，且测试之间互相覆盖。

**这正是 Global Constraints 第 2 条要禁止的事**（`config_dir()` 不可重定向）。
计划正文写了约束，Task 8 的验收方式却违反了它。

注意 `core/src/profiles.rs:20-23` 有一个 `#[cfg(test)] fn profiles_dir()` 指向 temp 目录的
特判——但它只对 **crate 内的单测**生效，`core/tests/*.rs` 是**外部集成测试**，用的是正式版本。
所以「现有测试没出过事」不能作为「这样写安全」的证据。

**修法（二选一）**：

- **A（推荐）**：给 `core::profiles` 加 `pub fn load_profile_from(path: &Path) -> Result<ProfileData, CoreError>`，
  让 `load_profile(name)` 变成「算路径 + 调它」。测试只调新函数读 `TempDir`。
  这顺带补上 `profiles.rs` 现有可测性缺口。
- **B**：Task 8 的测试只断言 JSON 结构与字段值，不经过 `load_profile`。代价是丢掉「往返一致」这个最有价值的断言。

---

## 3. 计划内部自相矛盾（能编译，但自打嘴巴）

> B-2 是 B-1 之外的第二处阻塞（`avg_len` 无数据源），归入本节因为它同时是 Self-Review 的矛盾。

### C-1 `Dashboard` 三处定义不一致

| 位置                      | 内容                                                           |
| ------------------------- | -------------------------------------------------------------- |
| Task 3 接口（437 行）     | `{ days, top_targets, profile_applies }`                       |
| Task 3 实现（551 行）     | `{ days }`                                                     |
| Self-Review #2（1425 行） | 「Task 3 的 `Dashboard` 增加 `top_targets: Vec<TargetCount>`」 |

三处说的是三件事。`profile_applies` 在实现里彻底消失，`top_targets` 只在接口和 Self-Review 里出现。

**修法**：先定 `Dashboard` 的最终形状，再让接口、实现、Self-Review 三处引用同一个定义。
我建议最终形状是 `{ days, top_targets, profile_applies, avg_len }`（`avg_len` 见 B-2）。

### B-2 Self-Review #1 的 `avg_len` 在 `dashboard()` 里算不出来

`dashboard()` 的 SQL 是 `SELECT day, path_items, env_items, events, conflicts FROM daily_stats`。
`daily_stats` 没有长度列，`dashboard()` 又**不接触 `path_entries`**。
「实时算」在这个函数里**没有数据源**。

**修法（二选一）**：

- **A**：给 `daily_stats` 加 `avg_path_len` 列 —— **要改 spec §4.1**，必须由审核窗口落笔，开发窗口不能私改；
- **B（我推荐）**：`dashboard()` 另发一条
  `SELECT AVG(length(path)) FROM path_entries WHERE snapshot_id = <current>`，
  结果放进 `Dashboard.avg_len`。不动表结构，但「当前」口径要明确（见 M-1）。

选 B 的理由：spec 已写明「派生表可重建、不是真相」，为一个展示字段改表结构不划算；
且 B 的「当前」口径与 M-1 是同一个问题，可以一次讲清。

### C-2 `index_snapshot_tx` 声明了但没实现

Task 2 的 Interfaces（196 行）声明：

```text
pub fn index_snapshot_tx(tx: &Transaction, snapshot_id: i64) -> Result<(), CoreError> — 增量
```

Task 2 的**代码块里没有它**。Task 12（1374 行）的重建策略表却写着「每次写入快照后增量（`index_snapshot_tx`）」。

后果：Task 12 接线时会发现没有这个函数，要么临时发明一个，要么放弃增量改成每次全量重建
（FTS 全量重建随快照线性增长，每次保存都做会明显变慢）。

**修法**：在 Task 2 里补齐实现，或把 Task 12 的策略改成「启动时若为空则全量；否则不做增量」并写明代价。

### C-3 Task 5 引入了未使用的 `ErrorCode`

```rust
// 计划 1028 行
use crate::error::{CoreError, ErrorCode};
```

`compare()` 的错误都来自 `get_snapshot`，`ErrorCode` 全程未用。计划结尾（1115 行）那行
「占位：`ErrorCode` 未在纯函数路径使用」说明作者知道，但**代码块里没删**。

`cargo clippy --workspace --all-targets -- -D warnings` 会直接红。

**修法**：改成 `use crate::error::CoreError;`，删掉占位注释。

### C-4 `TabId` 不是「新增」，是「扩展」

`src/core/path-capabilities.ts:3` 已有：

```ts
export type TabId = 'system' | 'user' | 'allVars' | 'merged' | 'history';
```

`app-store.ts:16` 已 re-export。计划写「`src/core/path-capabilities.ts`（新增 `TabId`）」会让人以为从零建。

**修法**：改成「在既有 `TabId` 联合类型里追加 `'search' | 'stats'`，并同步 `AppShell.tsx:131` 的
`tabConfig` 与 i18n 的 `tab.*` 键」。

---

## 4. 语义与口径问题（不是 bug，但必须写清）

### M-1 `daily_stats.path_items` 的口径

Task 3 的 SQL 用 `ROW_NUMBER() OVER (PARTITION BY day ORDER BY created_at DESC)` 取
**当天最后一条快照**的条目数。

这是个**口径选择**，不是缺陷。但它有个反直觉之处：用户第二天打开统计面板，问的通常是
「**现在**有多少条」，而图上的「今天」显示的是「今天最后一次保存时的条目数」。

建议在计划里补一句口径说明，并让「当前值」单独走 `app_state.current_snapshot_id`
（与 M1 的 `READ_SNAPSHOT_CACHE` 同源）。改不改 SQL 由你定，**但口径必须写下来**。

### M-2 `date(..., 'localtime')` 依赖进程时区

`'localtime'` 取的是 C 运行时时区（`TZ` / 系统设置）。项目约定 `Asia/Shanghai`，
但代码里没有任何地方保证这一点。CI 或跨时区运行时，同一条事件可能落到不同的 `day`。

**这不是要你现在改实现**，而是：Task 3 的测试要**避开跨日边界**（别在 23:59 前后写事件，
或让断言只比较「天数」不比较具体日期）。这条不依赖实测，可以直接采纳。

### M-3 `dependents` / `dependencies` / `find_cycles` 没有快照过滤

表的 DDL 主键是 `(snapshot_id, from_hive, from_name, to_name)`，理论上可容纳多份快照的边。
`rebuild_refs` 的做法是「清空全表 + 只插当前快照」，所以**正常运行下**表里只有一份快照的边，
查询不带 `snapshot_id` 不会出错。

但这个不变量是**隐式的**：它依赖「每次重建都全量清空」。若将来有人为了增量改成「只删当前快照的边」，
这三条查询会静默混入历史数据。

**建议**：在 `rebuild_refs` 的文档注释里写死这个不变量（「本表**只保存当前快照的边**，
由全量重建保证；查询可省略 `snapshot_id` 过滤」）。一行注释，避免未来踩坑。

### M-4 CLI 侧没有任何派生表重建点

`set_current_snapshot` 只在 GUI 出现（`gui/src/commands/service.rs:136`、`gui/src/lib.rs:148`）。
CLI 的写入口（`cli/src/runtime.rs:78-83`）用的是 `NoopHistory`，**不碰 DB 的当前快照**。

后果（**这是 M1 就存在的，不是 M2 引入的**）：

- CLI 改了 PATH 后，`var_references` 仍是旧的；
- D-1 之后 GUI 读路径从 DB 来，CLI 改了注册表但 DB 的 current 快照没变 → **GUI 读到 CLI 改动前的状态**。

这条已登记在 M1 缺口补齐轮回执 §6 R-02。M2 的引用图会把它放大（图上显示的是旧路径的引用关系）。

**建议**：Task 12 里明确写「CLI 不重建派生表；多进程混用时的陈旧读为已知残余风险 R-02，
本轮不解决」。**不要试图在 M2 里顺手修**——那需要 CLI 也接 DB 的 current 快照，
属于范围扩张，应当单独评估。

### M-5 spec 的「首次出现/最近出现」没有对应实现

spec §6.2 原文：`scan_runs` 持久化；两次扫描 diff；**「首次出现/最近出现」**。

计划 Task 6 的 `ScanRunDiff` 只有 `appeared` / `disappeared` / `persistent`。
`appeared` ≈「本次新出现」，但「**最近出现**」（persistent 但已连续 N 次）没有对应字段。
Self-Review 的 4 条缺口里也没列这一条。

**这是范围缩减，必须写进计划的未覆盖项**——理由同你在 M1 反复强调的那条：
不记录的范围缩减会伪装成「做完了」。补起来也便宜：`list_runs` 已经能取全部历史，
`diff_runs` 多返回一个 `last_seen_uid` 即可，不需要改表。

### M-6 Task 6 的测试没有验证「触发了 daily_stats 重建」

计划说 `record_run` 要「同时触发 `stats::rebuild_daily`」，但 Task 6 的测试（1160–1178 行）
只断言了 `scan_runs` 与 `diff_runs`，**没有任何断言检查 `daily_stats.conflicts` 变了**。

后果：「触发重建」这件事没有证据，接没接上都会「测试全绿」。

**修法**：测试末尾加一条 `daily_stats` 查询断言（同一文件内沿用同一个 `db`，不额外开
SQLCipher 连接，不违反一文件一测试）。

---

## 5. 待裁断

### T-1 `daily_stats` 全量重建的阈值（你问的那个）

**我的答复：现在不要定阈值，但请现在就加观测。**

理由：

1. **定阈值需要知道「多慢算慢」，这个数只有真实数据能给。** M2 阶段 `events` 上限是
   「用户操作次数」，重度用户一年几千条，全表扫描是毫秒级。
2. **定一个现在跑不到的阈值，等于埋一条永远不会被测到的分支。** 没跑过的分支比
   「看起来对、语义错」的代码更危险——它连「看起来对」都没人验过。
3. 增量重建的正确性依赖「哪些天的数据变了」，目前 `events` 没有能直接回答这个问题的索引。

**建议写法**：在 `rebuild_daily` 里加

```rust
let started = std::time::Instant::now();
// ... 重建 ...
log::info!("daily_stats 重建完成: {n} 天, 耗时 {:?}", started.elapsed());
```

三行代码，零风险。等真的出现启动瓶颈时，日志里已有基线数据，那时再定阈值有依据。

**如果你仍想现在定**：我建议 `events > 50_000` 且**只加 `log::warn!`、不改逻辑**。
这条我按你的裁断执行。

### T-2 `avg_len` 的落点（B-2 的解法）

已并入 §3 的 B-2，结论：推荐 B（`dashboard()` 另发一条查询，不改表结构）。
需要你确认，因为这涉及「spec §4.1 是权威还是可改」。

### T-3 env 的 N 路对比推迟

Self-Review #4 已把这条写成「范围缩减，必须写进 M2 回执的未覆盖项」。**我认同推迟**——掩码值在
对比结果里怎么呈现（显示 `••••`？显示「已修改但不可见」？两边都敏感且都改了但无法判断是否相同时怎么办？）
确实是产品语义问题，硬做出来可能误导用户。

**但建议再收窄一点**：`CompareResult` 保留 `env_added` / `env_removed` / `env_modified`
三个**空数组**，UI 上明确显示「环境变量对比将在 M3 提供」。

理由：只有 PATH 的对比结果，用户会以为「env 没变化」，而不是「env 没对比」。
空数组 + 显式文案，比「字段不存在」诚实。

---

## 6. 我核对通过的（计划的强项）

记下来是为了让你知道我逐条核过，不是只挑刺：

- **Global Constraints 六条全部成立。** 其中「`config_dir()` 不可重定向」与「一文件一测试」两条，
  仓库里都有对应事故记录（`CLAUDE.md`「测试与质量门」节、M0 结果 §5.3）——写进计划是正确的落点。
- **SQLite 实测结论**（3.51.3 / FTS5 = 1 / FULL OUTER JOIN = true）与我掌握的一致，
  删掉「若低于 3.39 就改写法」的降级分支是对的。
- **Task 顺序**依赖关系正确，没有「B 用 A 的产物但 A 在后面」的情况。
- **Task 5 的 `compare_paths` 算法**：按小写路径做键、三集合运算、结果稳定排序——正确。
  一个口径提醒：`reordered` 用**绝对 index** 比较，中间插入一条会把后面全部判为重排。
  这不是 bug，但 Task 11 的 UI 建议显示「移动了几位」而不是「位置变了」。
- **Task 7 的 `history_reveal_env_value` 独立命令**：与 spec §3.2.1「掩码默认、明文显式」完全对齐，
  前端「不写入 store」的要求也对。
- **Task 4 用 `parse_refs` 解析 env 的 `value` 时跳过自引用**——细节到位。

---

## 7. 建议的修订顺序

1. **B-1**（Task 8 测试路径，决定加 `load_profile_from` 还是放弃往返断言）
2. **C-1 + B-2**（`Dashboard` 形状一次定死，含 `avg_len`）
3. **C-2**（`index_snapshot_tx` 补齐或改策略）
4. **C-3 / C-4**（机械修改）
5. **M-1 / M-2 / M-3 / M-5 / M-6**（写清口径、补测试断言、登记未覆盖项）
6. **T-1 / T-3**（你的裁断）

收敛后我按 `superpowers:subagent-driven-development` 逐任务开工，每个 Task 结束跑
`cargo fmt --check` / `clippy -D warnings` / `cargo test --workspace` / `npm test`。

---

## 8. 本次核对**没有**做的事

- **没有写任何代码**，没有建分支，没有提交。
- **没有测 SQLite 的运行时行为**（FTS5 的 MATCH 转义、`'localtime'` 的实际时区）——
  实现时用探针验证。M-2 的「测试避开跨日边界」不依赖实测，可直接采纳。
- **没有核对 spec 的 M3/M4 部分**（§6.3 同步、§6.4 维护）——M2 计划不涉及。
- **没有检查 `--app-*` token 的具体名字**——Task 10/11 实现时再对照 `globals.css`。

---

## 9. 关于本文档的作废初版（自我更正记录）

初版里我写了三条「阻塞项」，逐字比对计划原文后**全部不成立**：

| 初版主张                                 | 实际情况                                                                   |
| ---------------------------------------- | -------------------------------------------------------------------------- |
| Task 1 的 `var_references` 只有 3 列     | 计划 126–130 行是 4 列，含 `snapshot_id`，与 spec §4.1 一致                |
| Task 2 的 `?` 无法转换 `rusqlite::Error` | 三处 `query_map` 都带了 `map_err`；`db.prepare(...)?` 本就返回 `CoreError` |
| `parse_refs("%A%%B%")` 会漏掉 `B`        | 逐字复刻成 JS 实测返回 `["A","B"]`；`%`、`%%`、`%A-B%` 等边界行为均正确    |

这三条的共同点是：**我凭对「这类计划容易犯什么错」的印象下结论，而没有打开文件逐字比对。**
按你教我的判据——「错误引用比缺失信息更危险，因为它让人以为已经查过了」——初版如果直接交给你，
你会花时间去核实三条不存在的问题。

本版所有条目都附了行号或可复现命令。**若仍有未核实的，请当场退回。**

---

**签章**：本文档的每一条「仓库实际」都来自对具体文件的实际读取或实际执行，行号已标注。
本轮未产生任何代码改动，未提交、未推送、未升级版本号、未写注册表。
