# PathEditor v6.0.0 SQLite 调研 — 核对轮交接

**提交窗口**：开发窗口
**日期**：2026-10-19
**交付对象**：审核窗口
**核对对象**：`docs/审核和开发/2026.10.19/PathEditor-v6-SQLite功能调研.md`（389 行，未提交）
**基线**：`main @ aae0909`（Wave 4.1 已合并，未推送）
**结论**：调研的**事实主张全部成立**（逐条核实到 file:line，含外部库编译选项）。另发现 **5 个需要 spec 补上的技术缺口**，并对 §八 的 6 个拍板问题给出开发窗口建议。**本轮未改任何代码、未提交任何文件。**

---

## 1. 事实核实（全部逐条实测）

| #   | 调研主张                                                             | 核实结果 | 证据                                                                                                                  |
| --- | -------------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------- |
| 1   | `disabled.json` 路径与内容                                           | ✅ 属实  | `core/src/disabled.rs:14-18`；含 `systemSnapshot`/`userSnapshot` 完整有序快照（CLAUDE.md 亦载明）                     |
| 2   | `pending_path_snapshot.json`                                         | ✅ 属实  | `core/src/disabled.rs:27-31`                                                                                          |
| 3   | `profiles/<name>.json` 一配置一文件                                  | ✅ 属实  | `core/src/profiles.rs:13-17`（目录）、`:66-68`（`profile_path`）                                                      |
| 4   | `ProfileMeta` 只有 name/created/modified                             | ✅ 属实  | `core/src/profiles.rs:75-79`                                                                                          |
| 5   | `env_backup_*.json` 含明文值 + revision                              | ✅ 属实  | `core/src/backup.rs:336-357`；payload 结构见 `:200-204`                                                               |
| 6   | `path_backup_*.txt`                                                  | ✅ 属实  | `core/src/backup.rs:1250-1267`                                                                                        |
| 7   | `Versioned<T>` + schemaVersion                                       | ✅ 属实  | `core/src/persist.rs:35-50`                                                                                           |
| 8   | 写前 `.bak` 轮换                                                     | ✅ 属实  | `core/src/persist.rs:65-77`                                                                                           |
| 9   | `atomic_write` = 临时文件 + rename，无 fsync                         | ✅ 属实  | `core/src/fs.rs:18-24`（`write` + `rename`，无 `sync_all`）                                                           |
| 10  | 解析失败隔离 `.corrupt-<ts>`                                         | ✅ 属实  | `core/src/persist.rs:102-129`                                                                                         |
| 11  | 撤销栈只在内存、最多 50 步                                           | ✅ 属实  | `src/core/undo-redo.ts:39`（`DEFAULT_MAX_SIZE = 50`）；`src/store/app-store.ts:77`（`new UndoRedoManager(...)`）      |
| 12  | 扫描结果不落盘                                                       | ✅ 属实  | `core/src/scanner.rs:92-129`（`scan_paths` 纯计算，无持久化）                                                         |
| 13  | 零 SQLite 依赖；无文件锁依赖                                         | ✅ 属实  | `core/Cargo.toml` 仅 serde/serde_json/log/winreg/dirs/chrono/windows-sys；全 workspace 无 `fs2`/`fd-lock`             |
| 14  | `variableCount` 恒为 0（列表只 stat 不解析）                         | ✅ 属实  | `core/src/backup.rs:555`（`variable_count: 0, // 不解析内容：见 S5`）；`src/services/backend.ts:428/444` 亦注明该契约 |
| 15  | rusqlite `bundled` 启用 `SQLITE_ENABLE_FTS5` / `SQLITE_ENABLE_JSON1` | ✅ 属实  | 拉取 `rusqlite/libsqlite3-sys/build.rs`（master）确认两个 define 均在 bundled 分支；锁版本后仍应按调研建议实测        |

> 外部库核实方法：`web_fetch` 读取 `https://raw.githubusercontent.com/rusqlite/rusqlite/master/libsqlite3-sys/build.rs`。**master ≠ 未来锁定的版本**，spec 里应保留一条编译选项验收。

---

## 2. 需要 spec 补上的技术缺口（核对新增）

### G1 —— `env_values` 草图未排除保留名与 Unsupported 类型

调研的数据模型把「全部环境变量」都写进 `env_values`，但现有契约明确：

- `Path` 在 `RESERVED_NAMES` 中，`list_all_env_vars` **过滤掉它**，Rust 写入口拒绝；PATH 只能走 `PathEntry` 通路（CLAUDE.md「通用环境变量通路与 PATH 通路并存」）。
- `Unsupported` 类型（如 `REG_MULTI_SZ`）**不可恢复**，现有备份/恢复也按不可写处理。

**建议**：`env_values` 显式排除 `Path`；`Unsupported` 只存元数据（`kind`/`revision`），`value` 置 NULL 且标记 `restorable=0`。否则迁移会把 `Path` 复制成第二个真相源，违反现有不变式。

### G2 —— `events.before_json/after_json` 是敏感值的旁路

即使 `env_values` 按 §八-Q2 选了「只存哈希」，**事件表的 before/after JSON 仍可能写入明文**（例如 `env set` 的 before/after 值）。现有设计里 `EnvVarMeta` 契约不含 `value`、明文只能经 `reveal_env_var` 获取——事件表不能绕开这个约束。

**建议**：事件只存**变更摘要**（变量名、hive、revision 前后值、结果），不存明文值；需要回放时经 `snapshots` 引用，敏感项按同一策略脱敏。spec 应把这条写成显式约束 + 一条守卫测试。

### G3 —— WAL 只保证「写不撕裂」，不自动消除 lost update

调研 A2 说「WAL 写串行化」是对的，但**逻辑层的读-改-写竞争仍在**：两个进程可以各自读到状态 A、各自算出 B/C，先后提交，后提交者覆盖前者。SQLite 的事务串行化只保证提交原子性，不保证你的业务判断基于最新状态。

**建议**：spec 给 `snapshots`/`profiles` 加 `version`（或 `generation`）列，写路径用 `BEGIN IMMEDIATE` + 事务内读 + 版本比对（compare-and-swap），冲突时返回 `conflict` 复用现有 `CoreError.code` 语义。**不要**宣称「SQLite 天然解决多进程并发」。

### G4 —— 降级守卫必须由 v5 先发，否则 Q6 没有可执行答案

已经发布的 v5 二进制**不可能认识未来的 DB 文件**——它只会忽略并继续用旧文件。「v5 遇到 v6 DB」的行为只能在 **v6 发布之前**由最后一个 v5.x 版本加上检测（见到 `~/.patheditor/patheditor.db` 就警告/拒绝启动）。

**建议**：若重视可降级性，在 v6 发版前发一个 v5.x 小版本携带该守卫；否则明确「降级不受支持」，并让 v6 迁移把旧文件改名留档（`.v5.bak`），文档写清降级后果。

### G5 —— `revision` 本身是弱哈希，敏感值策略不要过度承诺

`revision` 是 `name+type+value` 的 FNV-1a 散列（CLAUDE.md 载明）。即使 `env_values.value` 只存 NULL，只要保留 `revision`，拿到候选值的人仍可**验证**某个敏感值是否等于候选。这与现有 env 备份（本就存 revision）一致，不是新增泄漏，但 spec 不应把「只存哈希」说成「零泄漏」。

> 另两条小提醒（不阻塞 spec）：
>
> - JSON1 自 SQLite ≥3.38 起为内建，`sqlite_compileoption_used('ENABLE_JSON1')` 可能返回 0 而 JSON 函数可用；验收应实际执行 `SELECT json_valid('{"a":1}')`，FTS5 用编译选项查即可。
> - WAL 下备份单文件必须 `VACUUM INTO`（或先 checkpoint），直接复制 `.db` 会丢最近事务——调研 R3 已提到 `VACUUM INTO`，spec 需写成硬性步骤。

---

## 3. 对 §八 六个拍板问题的开发窗口建议

> 以下为实施视角的建议，最终由用户拍板。

| #   | 问题           | 建议                                                                                                                                 | 理由与实施影响                                                                                                                                                                                                                  |
| --- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | v6.0.0 范围    | **只做 Tier 1**                                                                                                                      | 存储层替换 + 一次性迁移是本项目史上最大变更；FTS/统计是纯增量（events/snapshots 就位后随时可加），放进 v6.0.0 只会放大风险面。                                                                                                  |
| 2   | 敏感值策略     | **Tier 1 只存哈希/元数据**：非敏感值明文入库；敏感值 `value=NULL`，只存 `revision` + `sensitive=1`，历史 UI 显示「敏感值，不可回放」 | 与现有掩码模型一致（`EnvVarMeta` 无 value、明文只经 `reveal_env_var`）；DPAPI/SQLCipher 是**两层假安全**——DB 加密了，磁盘上的明文备份仍在；两者主要成本是密钥管理与构建复杂度。若将来要加密，必须与备份加密同一波做（Tier 3）。 |
| 3   | 备份文件形态   | **保留文件**，DB 只做索引                                                                                                            | 用户可手工打开/复制；DB 损坏时备份仍可用；符合仓库「不删文件」纪律。调研 B3 的推荐正确。                                                                                                                                        |
| 4   | 配置方案       | **迁入 DB**，保留「导出为 v5 JSON」                                                                                                  | 标签/版本/最近使用/查询正是迁 DB 的收益；继续一文件一配置等于保留「逐个解析」的老问题。导出能力兜住可移植性。                                                                                                                   |
| 5   | 跨设备同步预留 | **只预留稳定 `uid`（配置身份）**；墓碑/变更集**不做**                                                                                | `uid` 一列成本极低；墓碑/变更集会污染所有查询与删除语义（`WHERE deleted_at IS NULL`），在同步真正排期前属于 YAGNI。同步仍留在 Tier 3。                                                                                          |
| 6   | 降级行为       | **已发布 v5 无法改变 → 只能忽略**；若要「拒绝启动」，守卫必须在 v6 之前发到 v5.x（见 G4）；否则登记「降级不受支持」                  | 这是发布时序问题，不是 v6 能自己解决的实现问题。                                                                                                                                                                                |

---

## 4. 结论

- 调研文档的**事实层全部扎实**：14 条 file:line 引用逐条对上，外部库编译选项也核实无误。四个「快照各存一套」的判断与代码一致。
- **方向认可**：Tier 1（统一快照 + 事件历史 + 配置元数据 + 备份索引）是合理的 v6.0.0 边界；「SQLite 是 sidecar，注册表仍是真相源」的定位正确。
- **spec 需要补 5 条**：G1（保留名/Unsupported）、G2（事件明文旁路）、G3（lost update 与 CAS）、G4（降级守卫时序）、G5（revision 是弱哈希的措辞边界）。
- 6 个拍板问题里，**只有 Q2（敏感值）会反向决定数据模型**，建议最先定；Q1 定边界；其余四项均有低风险的推荐路径。

**本文件不改动调研文档、不写 spec、不改代码、未提交任何文件。**

**签章**：以上结论基于 2026-10-19 在 `main @ aae0909` 上实际执行的代码核对，以及 libsqlite3-sys 构建脚本的在线核实。
