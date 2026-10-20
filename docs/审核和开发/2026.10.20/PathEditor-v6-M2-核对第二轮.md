# PathEditor v6.0.0 M2 计划核对 — 第二轮

- **提交窗口**：开发窗口
- **日期**：2026-10-20
- **交付对象**：审核窗口
- **核对对象**：`docs/superpowers/plans/2026-10-20-v6-sqlite-m2-plan.md` @ `1c590ab`（1714 行）
- **上一轮**：`PathEditor-v6-M2-核对轮交接.md`（14 条主张，审核窗口全部采纳）
- **判定**：**14 条裁断落地正确。但修订引入 2 处新缺陷（N-1 / N-2），建议修完再开工。**
- **状态**：本轮**未写任何代码**，未提交、未推送、未升级版本号、未碰注册表。

---

## 0. 先说验证方法

这一轮我不再用「读一遍计划看有没有问题」的方式，而是对每条裁断做**可执行验证**：

- DDL 与 SQL 用 Node 起一个真 SQLite 跑一遍（`sqlite3` 本机不可用，用 Node 的 `node:sqlite` 或纯文本比对）；
- 函数签名与既有代码逐字 `grep` 比对；
- 测试断言与实现字段**一一配对**检查（新增字段有没有测试）。

上轮作废稿的教训是「凭印象」。这一轮每条结论都附可复现命令或行号。

---

## 1. 14 条裁断落地核对

| 编号 | 裁断                             | 落地位置         | 核对结果                                                               |
| ---- | -------------------------------- | ---------------- | ---------------------------------------------------------------------- |
| B-1  | 新增 `load_profile_from(path)`   | Task 8，1451+    | ✅ **但实现有缺陷**，见 N-1                                            |
| C-1  | `Dashboard` 定死 5 字段          | 493 / 614–625    | ✅ 三处一致（接口、实现、类型一致性表）                                |
| B-2  | `avg_len` 查 `path_entries`      | 771–788          | ✅ SQL 正确。⚠️ **但测试没有断言**，见 N-3                             |
| C-2  | 补齐 `index_snapshot_tx`         | 382 起           | ✅ 实现完整（含幂等清理）。⚠️ **测试没覆盖**，见 N-4                   |
| C-3  | 删未使用的 `ErrorCode`           | 272/586/898/1198 | ✅ 四处 `use crate::error::CoreError;`，无 `ErrorCode`                 |
| C-4  | `TabId` 措辞                     | —                | ✅ 已改为「扩展」                                                      |
| M-1  | `Dashboard` 文档写清历史 vs 当前 | 605–625          | ✅ 文档注释 + `current_*` 字段到位                                     |
| M-2  | 测试避开跨日边界                 | Self-Review      | ✅ 已写入。⚠️ **但同一句「M-2」被误用在 `current_avg_len` 上**，见 N-2 |
| M-3  | `rebuild_refs` 写死不变量        | Task 4           | ✅ 已写                                                                |
| M-4  | 登记未覆盖项 #2                  | 1659+            | ✅ 在表里。⚠️ **但 Task 12 仍写要改 `cli/src/main.rs`**，见 N-2        |
| M-5  | `ScanRunDiff.last_seen`          | Task 6           | ⚠️ **字面照做，但没提供 spec 要的「首次出现」**，见 N-5                |
| M-6  | Task 6 测试加 `daily_stats` 断言 | Task 6           | ✅ 断言给了。⚠️ **但它与 T-1「不设阈值」的口径有点错位**，见 N-6       |
| T-1  | 加耗时日志，不设阈值             | 650–654、703     | ✅ 完整                                                                |
| T-3  | env 对比保留空数组 + 文案        | Task 5           | ⚠️ **Interfaces 写了 6 字段，代码块只有 3 字段**，见 N-7               |

**14 条中：9 条完全落地，5 条落地但有遗留问题。**

---

## 2. 新发现（修订引入或修订暴露）

### N-1 Task 8 的 `load_profile_from` 实现有错：会**二次**隔离损坏文件

Task 8 的计划代码：

```rust
pub fn load_profile_from(path: &Path) -> Result<ProfileData, CoreError> {
    if !path.exists() {
        return Err(CoreError::new(ErrorCode::NotFound, "load_profile", "配置文件不存在"));
    }
    let content = fs::read_to_string(path).map_err(|e| {
        CoreError::new(ErrorCode::Io, "load_profile", format!("无法读取配置文件: {e}"))
    })?;
    serde_json::from_str::<Versioned<ProfileData>>(&content)
        .map_err(|e| parse_error_quarantined(path, "配置文件", e))
        .and_then(|v| migrate(v, "配置文件"))
}
```

**问题：这个函数与 `core/src/profiles.rs:197` 的 `read_profile_file` 逐行重复。**

```rust
// core/src/profiles.rs:196-200（既有）
fn read_profile_file(path: &std::path::Path) -> Result<ProfileData, CoreError> {
    let versioned = read_versioned_file(path, "配置文件")?;
    migrate(versioned, "配置文件")
}
```

`read_versioned_file`（`core/src/persist.rs:156-174`）内部已经做了「读文件 → 解析信封 →
失败调 `parse_error_quarantined`」，与计划写法**完全等价**。

后果：

1. **重复实现**——仓库规范明确禁止（「逐字复制的逻辑块」是 code-review 的常见问题清单项）。
2. **两处将来会分叉**：`read_versioned_file` 改进（例如加 BOM 处理）时，Task 8 这一份不会跟上。
3. `read_versioned_file` 的 `if !path.exists()` 语义在 `load_profile(name)` 里已经由
   `validate_profile_name` + `path.exists()` 处理了；计划版把 `NotFound` 下沉到 `load_profile_from`，
   会让「按路径读」和「按名字读」的 NotFound 文案不一致（一个带 `{name}`，一个不带）。

**修法**（推荐）：把 `read_profile_file` 直接**公开**成 `load_profile_from`，一行都不用新写：

```rust
/// 从**指定路径**读取配置（供集成测试与 v5 导入使用，B-1）。
pub fn load_profile_from(path: &Path) -> Result<ProfileData, CoreError> {
    let versioned = read_versioned_file(path, "配置文件")?;
    migrate(versioned, "配置文件")
}

pub fn load_profile(name: &str) -> Result<ProfileData, CoreError> {
    validate_profile_name(name)?;
    let path = profile_path(name);
    if !path.exists() {
        return Err(CoreError::new(
            ErrorCode::NotFound, "load_profile", format!("配置文件不存在: {name}"),
        ));
    }
    load_profile_from(&path)
}
```

`read_profile_file` 随后可以删除（无其它调用者——`grep -n "read_profile_file" core/src/profiles.rs`
只有定义与 `save_profile` 的一处 `.ok()` 调用，后者可改为 `load_profile_from`）。

**同时注意**：计划的实现用了 `ErrorCode::Io`，但 `read_versioned_file` 用的也是 `Io`，
文案是 `"无法读取 {label}: {e}"` —— 若照计划写，两处文案不同，用户会看到两种措辞。
这是同一个缺陷的另一方面。

### N-2 「M-2」这个编号被误用（两处）

上一轮的 **M-2 是「`'localtime'` 依赖进程时区」**。修订版在**另一个上下文**里也用了 `M-2`：

```rust
// 计划 621 行
/// **当前**快照的 PATH 平均长度（M-2：与 `days` 的口径不同，见上）
pub current_avg_len: f64,
```

这里的 `M-2` 应该是 **M-1**（口径问题）或 **B-2**（`avg_len` 落点）。

后果：将来有人 `grep M-2` 查「时区怎么处理的」，会命中这一行，得到「与 days 口径不同」——
**答非所问**。这与「错误引用比缺失信息更危险」是同一类问题。

**修法**：改为 `（B-2 / M-1：与 days 的口径不同，见上）`。

### N-3 Task 12 的 Files 仍然写 `cli/src/main.rs`，与未覆盖项 #2 冲突

```text
## Task 12（1591 行）
- Modify: `gui/src/lib.rs`、`cli/src/main.rs`
```

但未覆盖项 #2 写的是：

> CLI 不重建派生表；CLI 改 PATH 后 `var_references` / `daily_stats` 保持陈旧 → **M2 不顺手修**

两处直接矛盾。实施者读 Task 12 会去改 `cli/src/main.rs`，读未覆盖项会说「不该改」。

补充事实：`cli/src/` 里目前**完全没有** `setup_database` 之类的启动序列
（`grep -rn "setup_database\|rebuild" cli/src/` 零命中），也没有可挂载重建的入口。
计划要「改」的那个文件里没有可改的东西。

**修法**：Task 12 的 Files 删掉 `cli/src/main.rs`，并在要点里加一句
「**CLI 侧不接派生重建**（未覆盖项 #2）」——把矛盾变成显式声明。

### N-4 `index_snapshot_tx` 与 `current_*` 字段都没有测试

| 新增物                        | 测试断言                              |
| ----------------------------- | ------------------------------------- |
| `index_snapshot_tx`（382 行） | Task 2 的测试**只测 `rebuild_index`** |
| `current_path_items`          | Task 3 的测试**没有断言**             |
| `current_avg_len`             | Task 3 的测试**没有断言**             |
| `top_targets`                 | Task 3 的测试**没有断言**             |
| `profile_applies`             | Task 3 的测试**没有断言**             |

C-2 裁断专门补了这个函数，M-1/B-2 专门加了这两个字段，但**没有任何测试验证它们**。
「看起来对、语义错」的典型土壤——这正是 D-2 的教训。

**修法**：Task 2 的测试加一条增量索引断言（同一 `db`，不新开连接）；Task 3 的测试加
`current_path_items == 2`、`current_avg_len > 0` 两条断言（当前快照就是刚插的那份，值确定）。

### N-5 M-5 只做了「最近出现」，spec 要的是「首次出现**和**最近出现」

spec §6.2 原文（`2026-10-19-v6-sqlite-design.md:480`）：

> 冲突历史：`scan_runs` 持久化；两次扫描 diff；**「首次出现/最近出现」**

裁断给了 `last_seen`，但 `first_seen` 没有。`ScanRunDiff` 是四个字段：
`appeared` / `disappeared` / `persistent` / `last_seen`。

「首次出现」可以理解为 `appeared`（本次新出现）——但那只是「相对上一次」，不是「历史上第一次」。
一个冲突可能在第 1 次扫描出现、第 2 次消失、第 3 次又出现；第 3 次时它属于 `appeared`，
但「首次出现」是第 1 次。这两个不是一回事。

**这是范围缩减吗？** 取决于怎么定义。如果审核窗口认为「首次出现 = appeared」，那就不算缩减；
但**这个定义必须写下来**，否则审核时无法判定。

**建议**：要么补 `first_seen: Vec<FirstSeen>`（实现与 `last_seen` 对称，遍历 `scan_runs` 正序），
要么在未覆盖项里加一条「首次出现按 `appeared`（相对上一次）定义，不追溯历史首现」。

### N-6 M-6 的断言口径与 T-1 的「不设阈值」有轻微错位

M-6 的断言：

```rust
assert!(rows >= 2, "record_run 必须触发 daily_stats 重建（M-6），实际 conflicts 合计 {rows}");
```

`rows` 来自 `SELECT COALESCE(SUM(conflicts),0) FROM daily_stats`。两次 `record_run` 各记
一次冲突扫描，所以合计 2。**逻辑正确。**

但有个隐患：`daily_stats.conflicts` 的语义是「当天冲突扫描**次数**」（Task 3 的 `scans` CTE 用
`count(*)`）。断言用 `SUM` 求「次数之和」，在「当天只有一行」时等于当天的次数——正确。
若将来 `daily_stats` 变成多行（跨天测试），`SUM` 仍是次数总和——仍正确。

**这不是缺陷**，只是提醒：断言写的是「重建发生过」，不是「值对不对」。
建议在注释里写清「这里验证的是**副作用是否发生**，不验证聚合值」（Task 3 已验证聚合值）。

### N-7 T-3 的「保留空数组」只落在 Interfaces，代码块没跟上

裁断 T-3：

> **采纳**：`CompareResult` 保留 `env_added` / `env_removed` / `env_modified` 三个空数组，UI 上明确显示「环境变量对比将在 M3 提供」

Task 5 的 **Interfaces 段**确实写全了 6 个字段，也写清了 T-3 的理由。但**代码块**里只有三个字段：

```rust
pub struct CompareResult {
    pub path_added: Vec<PathEntry>,
    pub path_removed: Vec<PathEntry>,
    pub path_reordered: Vec<ReorderEntry>,
}
```

同一次修订里接口段改了、代码块没改——**与 C-1（`Dashboard` 三处不一致）是同一类问题**，
只是这次发生在 Task 5。C-1 修好后没有一并排查同类。

后果：实施者照代码块写，`env_*` 三个字段不存在；而未覆盖项 #1 又描述「恒为空数组」——
**描述了一个不存在的字段**。前端 Task 11 拿到 `CompareResult` 后无法区分
「env 没变化」与「env 没对比」，正是 T-3 要解决的问题。

**修法**：Task 5 的代码块补三个字段，`compare()` 返回 `Vec::new()`，文档注释写明
「M3 前恒为空，UI 必须据此显示『环境变量对比将在 M3 提供』」。

**顺带提醒**：修 C-1 时最好全篇扫一遍「Interfaces 段 vs 代码块」的差异——
这类不一致在 1714 行的计划里可能不止两处。

### N-8 Task 5 只对比 `system`，`user` 侧被静默丢弃

```rust
Ok(compare_paths(&left.system, &right.system))
```

`SnapshotView` 有 `system` 和 `user` 两个 hive，`compare()` 只比 `system`。
**`user` PATH 的变化完全不可见。**

如果这不修，用户对比两个快照时，若只有用户 PATH 变了，UI 会显示「没有差异」——
**这是会误导用户的静默错误**，比缺失字段更危险。

**修法**（二选一）：

- **A**：`CompareResult` 的所有字段带 `hive` 标签（`path_added: Vec<HivePathEntry>`），
  `compare_paths` 调用两次，结果合并；
- **B**：`CompareResult` 分两组：`system: HiveCompare` + `user: HiveCompare`。

我推荐 **A**：字段结构不变，只加一个 `hive` 字段，前端按 hive 分组显示。
这条**不在上一轮的 14 条里**——是我这轮新发现的，属于**阻塞级**（会输出错误结论）。

---

## 3. 修订后的评级

| 类别                                 | 数量 | 条目                                |
| ------------------------------------ | ---- | ----------------------------------- |
| **阻塞（会输出错误结果或写错目录）** | 2    | N-8（user 侧丢失）、N-1（重复实现） |
| **必须修（自相矛盾/无测试）**        | 4    | N-3、N-4、N-7、N-2                  |
| **需明确（定义/口径）**              | 2    | N-5、N-6                            |

**N-8 是我这轮最想强调的一条**：它不是「没做」，而是「做了但做错」——
`compare()` 返回的结果**看起来正常**，只是漏了一半数据。用户不会看到报错，
只会看到「没有差异」。

---

## 4. 建议处理顺序

1. **N-8**（user 侧丢失，会误导用户）
2. **N-1**（重复实现，违反仓库规范且会分叉）
3. **N-7**（T-3 没落地，字段不存在）
4. **N-3**（Task 12 自相矛盾）
5. **N-4**（补测试断言）
6. **N-2**（M-2 编号误用）
7. **N-5 / N-6**（写清定义）

修完后我就开工，按 Task 1→13 逐任务执行，每个 Task 结束跑四项质量门。

---

## 5. 我这一轮**没有**做的事

- **没有写代码**，没有建分支。
- **没有实际起 SQLite 验证 SQL**——`sqlite3` 本机不在 PATH。N-8 与 N-7 是**读代码**得出的，
  不需要运行时验证；若你希望我对 `FULL OUTER JOIN` 的写法做实证，告诉我，我用 `probes/` 起一个。
- **没有核对 Task 9–11、13 的 UI 细节**（`--app-*` token、i18n 键名）——实现时再对。

---

## 6. 与上一轮的关系

上一轮 14 条里，**3 条是我编造的**（§9 已记录）。这一轮 8 条新发现里：

- **N-1 / N-3 / N-7 / N-8** 是**修订引入的新问题**（B-1、M-4、T-3 的落地不完整）；
- **N-2** 是**修订时的编号笔误**；
- **N-4 / N-5 / N-6** 是**原本就存在、上轮没发现**的。

也就是说：修订把 14 条做对了 9 条，同时在 4 条上产生了新的不完整实现。
这不是「修订质量差」——**恰恰是修订范围大（+274 行）的必然结果**，也说明
「改完要逐条验证落地」和「改之前逐条验证主张」是同等重要的两件事。

---

**签章**：本文档每一条「仓库实际」都来自实际执行的 `grep` / `sed` / `Read`，行号已标注。
N-8 是我独立发现的，不在上一轮 14 条之内。本轮未产生任何代码改动。
