# PathEditor v6.0.0 M1 缺口补齐 — 开发回执

- **提交窗口**：开发窗口
- **日期**：2026-10-20
- **交付对象**：审核窗口
- **依据**：`docs/审核和开发/2026.10.20/PathEditor-v6-M1-缺口清单.md`（274 行）、`PathEditor-v6-M1-缺口补齐轮审查报告.md`、`PathEditor-v6-M1-缺口补齐轮复审结论.md`
- **分支**：`v6/m1-storage-history`（基于 `main @ 9e674ec`）
- **状态**：缺口清单 9 项 + D-1 / D-2 / D-6 **全部完成**，质量门全绿，**未推送**
- **版本**：未升级（仍为 `5.1.7`）
- **注册表**：零写入（全部测试用 tempfile / 注入 fake）

---

## 0. 先说出事故

**审核窗口在验证 D-1 时用探针覆盖了用户真实的 `C:\Users\33644\.patheditor\disabled.json`。**

| 项               | 覆盖前 | 覆盖后                       |
| ---------------- | ------ | ---------------------------- |
| `systemSnapshot` | 43 条  | 1 条（`C:\D1REPRO`，假数据） |
| `userSnapshot`   | 11 条  | 10 条                        |

**实际影响：零。** 我的独立核实：

```
注册表 system PATH: 43 条实条目，无 C:\D1REPRO
注册表 user PATH:   12 条，无 C:\D1REPRO
load_path_snapshot(): system=43 user=12 disabled_total=0 has_D1REPRO=false
disabled.json / .bak / pre-wave30 三份的「禁用项」计数：全部为 0
```

**为什么零影响**：`systemSnapshot` 是注册表状态的**缓存**，只在「被禁用的孤儿条目」时参与计算（`merge_hive`）。用户从未禁用过任何条目，所以这份坏缓存在当前状态下不被使用。

**没有恢复文件**。理由：唯一可用的旧副本是 9-20 的 `pre-wave30`，用它覆盖会引入新错误（9-20 之后 PATH 变过）。

**审核窗口的正确定性值得记录**：「这是用未重定向的路径写真实用户数据的错误，定论是『碰巧没有造成伤害』，而不是『没有错误』。」

**遗留**：`disabled.json` 仍含 `C:\D1REPRO` 一条脏数据。当前无影响，**未清理，待用户决定**（见 §5）。

---

## 1. 交付内容

### 1.1 缺口清单 9 项

| #        | 项                         | 实现                                                                                                                                                                                                                                  | 测试                                  |
| -------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| **P0-1** | 配置方案接通 DB（方案 A）  | UI `use-profiles.ts` 与 CLI `profile_ops.rs` **双端**切 DB；v5 命令从 `invoke_handler` 摘除并加禁重注册注释；新增 `profile_save` / `profile_paths` / `profile_rename` / `profile_delete_by_name` / `profile_mark_applied_by_name` IPC | `db_profiles_by_name`                 |
| **P0-2** | `snapshotUid` 回填         | `DbHistory::with_snapshot`；命令层在调服务**前**建「操作前」快照并绑定                                                                                                                                                                | `db_event_snapshot_link`              |
| **P0-3** | `history_restore_snapshot` | 走既有 `save_path_with_sidecar` 编排（不新增写注册表路径）；append-only；UI 加恢复按钮 + 二次确认                                                                                                                                     | `db_restore_append_only`              |
| **P1-1** | EventFilter 补 4 维        | `hive` / `target` / `result` / `from_ms` / `to_ms`；`total` 与分页共用同一段 WHERE；UI 加 3 个筛选下拉                                                                                                                                | `db_event_filter`                     |
| **P1-2** | 备份索引接线               | `setup_database` 启动时 + `backup_list` **每次调用前**增量索引                                                                                                                                                                        | `db_backup_index_wiring`              |
| **P1-4** | v5 文件停写（H7）          | `set_v5_writes_disabled` + 启动序列设置；**配套** D-1 读路径切换                                                                                                                                                                      | `v5_writes_disabled_skips_file_write` |
| **P1-5** | 修正错误引用               | `migrate_v5.rs` 头部把「spec §7.4 授权」改写成「§7.4 是迁移规则本身；推迟是**实现取舍**」                                                                                                                                             | —                                     |
| **P1-6** | 历史面板 E2E               | 空态 / 降级条幅 / 展开详情（含掩码与恢复按钮）                                                                                                                                                                                        | `history.spec.ts` 3 条                |
| **P1-7** | 跨会话撤销                 | `undoLast`：**事件流即撤销栈**（不重建 `OpRecord`）；跳过 restore 自身                                                                                                                                                                | 4 条单测                              |

### 1.2 审查轮追加的三项

| #       | 项                    | 实现                                                                                                  | 测试                        |
| ------- | --------------------- | ----------------------------------------------------------------------------------------------------- | --------------------------- |
| **D-1** | 停写后读路径也切 DB   | `load_path_snapshot()` 在停写模式走注入的加载器；取不到回落文件                                       | `db_v5_read_from_db`        |
| **D-2** | 迁移取当前快照        | SQL 改为 `WHERE kind = 'current'`；**并修了 `ON CONFLICT` 不更新 `current_snapshot_id` 的第二层缺陷** | `db_migrate_current_pick`   |
| **D-6** | 无 current 快照的基线 | `setup_database` ③b 加 `ensure_current_snapshot_baseline`                                             | `db_migrate_v5`（边界断言） |

---

## 2. 设计决策（选了什么 + 为什么）

| #   | 决策                             | 选择                                                  | 理由                                                                                                         |
| --- | -------------------------------- | ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| 1   | P0-1 的范围                      | **GUI + CLI 同时切 DB**                               | 只改 GUI 会把双真相源从「GUI 文件 vs DB」变成「CLI 文件 vs DB」——问题没解决，只是换了个入口                  |
| 2   | 快照记录「操作前」还是「操作后」 | **两个都建**                                          | 「操作前」绑给事件（撤销语义）；「操作后」指向 `current_snapshot_id`（当前状态）。职责不同，不能共用         |
| 3   | 停写开关的类型                   | `AtomicBool` 而非 `OnceLock`                          | 生产仍只设一次，但 `OnceLock` 无法在测试里验证开/关两种状态                                                  |
| 4   | D-1 的依赖方式                   | **注入加载器**（`set_current_snapshot_loader`）       | `db::migrate_v5` 已依赖 `disabled`，反向依赖会成环                                                           |
| 5   | DB→disabled 的桥接               | 进程级 `RwLock` 缓存                                  | 加载器是 `Fn()` 类型，拿不到 `Db`；缓存只存一份 `(Vec<PathEntry>, Vec<PathEntry>)`，**不是「全局 Db 单例」** |
| 6   | `ON CONFLICT` 的更新策略         | `COALESCE(app_state.current_snapshot_id, excluded.…)` | 只在仍为空时填入，不覆盖运行时已维护的值                                                                     |
| 7   | D-6 基线放哪                     | **启动序列**，不是迁移                                | 迁移的职责是「导入 v5 文件」，读注册表不属于它                                                               |
| 8   | `backup_list` 索引时机           | **每次调用前**增量                                    | 只在启动时索引会漏掉「启动后新做的备份」：用户启动 → `env backup` → 打开列表 → 空                            |
| 9   | P1-7 的实现形态                  | 事件流即撤销栈                                        | 不重建 `OpRecord`（那是 UI 即时栈）；P0-2 已经让每条事件指向「操作前」快照                                   |

---

## 3. 偏离记录

| #   | 计划/清单原文                                      | 实际实现                        | 处理方式与理由                                                                                                                                   |
| --- | -------------------------------------------------- | ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | P1-4 写「写路径前置检查 `if migrated_v5 == 1`」    | 先补「当前快照进 DB」，再做停写 | **清单漏了前置依赖**：DB 里没有「当前 PATH 状态」落点（`set_current_snapshot` 零生产调用者），直接停写会让禁用状态彻底丢失。经用户确认采用方案 A |
| 2   | 清单 P1-2「启动时或 `backup_list` 首次调用时触发」 | 启动时 **+ 每次调用前**         | 「首次调用」仍有时间窗：用户启动 → 做备份 → 打开列表，索引没刷新                                                                                 |
| 3   | D-2 只要求「SQL 改对」                             | 同时修了 `ON CONFLICT` 分支     | **补测试时抓出来的第二层缺陷**：建库种子先插了 `id=1`，迁移的 `INSERT` 永远走 `ON CONFLICT`，而那个分支只更新了 `migrated_v5`                    |
| 4   | 清单未提 D-6                                       | 在启动序列补基线                | 审查窗口指出：新装用户无 `disabled.json` → 迁移后 `current_snapshot_id` 为 NULL → 读路径回落空状态 → 禁用标记丢失                                |
| 5   | 无既有计划（本轮由缺口清单驱动）                   | 见 §3.1                         | 清单是审核窗口产出的任务书，本回执按它执行                                                                                                       |

### 3.1 一处需要注意的边界

`db_migrate_v5` 里我加了一条断言：**无 `disabled.json` 时迁移不产生 `kind=current` 快照**。

这是**已知边界**，不是缺陷——建基线是启动序列的职责。写进测试是为了防止下一个人误以为迁移会兜底。

---

## 4. 质量门（实际输出）

```
$ cargo fmt --all -- --check
exit 0

$ cargo clippy --workspace --all-targets -- -D warnings
exit 0

$ cargo test --workspace
binaries=22  passed=313  failed=0  ignored=2

$ npm run verify
VERIFY=0

$ npm run test:e2e
44 passed (1.3m)
E2E=0
```

计数口径（可复现，避免前两轮的差 1）：

```bash
cargo test --workspace 2>&1 \
  | grep -oE "^test result: ok\. [0-9]+ passed; [0-9]+ failed; [0-9]+ ignored" \
  | sed -E 's/test result: ok\. ([0-9]+) passed; ([0-9]+) failed; ([0-9]+) ignored/\1 \2 \3/' \
  | awk '{p+=$1; f+=$2; i+=$3; n++} END {printf "binaries=%d passed=%d failed=%d ignored=%d\n", n, p, f, i}'
```

前端：`Test Files 27 passed (27)`、`Tests 336 passed (336)`。

### 4.1 一文件一测试（23 个 DB 测试文件，各 1 个 `#[test]`）

`db_app_state` / `db_backup_index_wiring` / `db_backup_production` / `db_backups_index` / `db_event_filter` / `db_event_snapshot_link` / `db_history_roundtrip` / `db_history_sink` / `db_key_lifecycle` / `db_migrate_current_pick` / `db_migrate_v5` / `db_profiles` / `db_profiles_by_name` / `db_restore_append_only` / `db_schema_open` / `db_v5_read_from_db` 等。

逐文件执行命令：

```powershell
foreach ($f in (Get-ChildItem core/tests/db_*.rs | ForEach-Object { $_.BaseName })) {
  cargo test -p path-editor-core --test $f
  if ($LASTEXITCODE -ne 0) { throw "失败: $f" }
}
```

---

## 5. 未覆盖项（如实列出）

| #     | 项                                                                                                           | 说明                                                                                                         |
| ----- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------ |
| **1** | `disabled.json` 的 `C:\D1REPRO` 脏数据                                                                       | 探针写入，当前无影响（`merge_hive` 会过滤不存在的路径），**未清理，待用户决定**                              |
| **2** | D-6 基线的降级路径                                                                                           | 读注册表失败时只告警 → 读路径回落文件（此时文件已停写，可能是旧数据）。这条降级路径**未测试**                |
| **3** | 历史面板 E2E 的「恢复」路径                                                                                  | 3 条 E2E 覆盖空态 / 降级 / 展开详情，**没有覆盖「点击恢复 → 注册表被写入」**——那需要真实注册表               |
| **4** | CLI 的 `profile` 子命令无自动化测试                                                                          | CLI 已切 DB，但 `profile_ops.rs` 的 6 个函数**没有单测**（它们直接开真实 DB，测试会碰用户 `~/.patheditor/`） |
| **5** | `history_reveal_env_value` / `profile_export_v5` / 列表分页 / v5 导出                                        | P2，仍属 M2（spec §3.2.1 / §6.1）                                                                            |
| **6** | 六张未建的表（`search_fts` / `daily_stats` / `var_references` / `scan_runs` / `changes` / `sync_conflicts`） | M2（前四张）与 M3（后两张）                                                                                  |
| **7** | CI（MSVC）                                                                                                   | 未推分支，CI 未跑。本机为 GNU                                                                                |
| **8** | 真实注册表闭环测试                                                                                           | 未做（需用户显式授权）                                                                                       |

---

## 6. 残余风险

| #    | 风险                                        | 等级       | 说明                                                                                                                                        |
| ---- | ------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| R-01 | `update_current_snapshot` 的 CAS 失败时放弃 | 非阻塞     | 并发写时 `current_snapshot_id` 这一次不更新。历史仍正确（事件与快照都在），下次操作重新对齐。**不做重试是刻意的**——强行覆盖可能指向错误快照 |
| R-02 | 读缓存与 DB 可能短暂不一致                  | 非阻塞     | 缓存由写路径 `refresh_read_snapshot` 刷新；进程外的写入（CLI）不会刷新 GUI 的缓存。**多进程同时用 GUI+CLI 时读到旧状态**，重启后对齐        |
| R-03 | `AtomicBool` 的 `Relaxed` 序                | 非阻塞     | 只用于「开关」语义，不含需要同步的数据。若要跨线程严格可见性需改 `SeqCst`，但当前只有启动时写一次                                           |
| R-04 | `profile_save` 不检查重名描述/标签冲突      | 非阻塞     | 只对 `name` 做 `NameExists` 检查（spec B5 的部分唯一索引）                                                                                  |
| R-05 | `.pebak` 的 `payload_hash` 不是 MAC         | 沿用       | 完整性由 DPAPI 提供，但 DPAPI 不抗篡改                                                                                                      |
| R-06 | DACL 判据是黑名单                           | 沿用（M2） | `NO_ACCESS_CONTROL` 会被误判为受限；漏 `;;;CO)`。第一层 `set_key_dacl` 正确，故不阻塞                                                       |

---

## 7. 需要审核窗口重点关注的项

1. **D-2 的第二层缺陷**：`ON CONFLICT` 只更新 `migrated_v5` 是**审查报告和清单都没提到的**，补测试时才暴露。请确认 `COALESCE` 的写法（只在为空时填充）是否符合你的预期。
2. **CLI 侧完全不受 D-1 影响**：`cli/src/` 里既没有 `set_v5_writes_disabled` 也没有 `set_current_snapshot_loader`，所以 `v5_writes_disabled()` 恒为 `false`，走文件路径——**行为与改动前完全一致**。你上一轮已经核实这一点，此处确认我的理解一致。
3. **`profile_paths` 返回 `updatedAt`**：UI 原来展示 `data.modified`（字符串），DB 版是 Unix 毫秒，前端用 `new Date(...).toLocaleString()` 格式化。请确认展示口径可接受。
4. **P1-7 的语义边界**：`undoLast` 跳过 `action === 'restore'` 的事件（不「撤销撤销」）。这是我的判断，清单没规定。

---

## 8. 提交清单

**本轮全部改动尚未提交**（46 份文件改动 + 7 个新增测试文件 + 4 份审查文档）。按用户指示等统一提交。

前序已提交（上一轮）：

```
be88f35 docs: 让 Prettier 跳过 CLAUDE.md/AGENTS.md，保证字节一致
f8858cc docs: 修正 AGENTS.md 与 CLAUDE.md 字节一致（表格对齐 + 行尾）
8106ff5 docs: 同步 CLAUDE.md/AGENTS.md（OpenSSL 依赖说明）并修正 CHANGELOG 过时描述
ef10bc6 fix(v6): 修复 SnapshotDetail 注释中的编码损坏
460fbec docs(v6): 更正 R-06
d10f759 docs(v6): M1 第二轮复审回执
```

---

## 9. 分支状态

- 分支：`v6/m1-storage-history`
- HEAD：`be88f35`（**本轮改动未提交**）
- 未推送、未合并、未升级版本号

---

**签章**：以上「已修复」的结论均来自本机实际执行的命令输出（§4）；§5 的 8 项未覆盖与 §6 的 6 项残余风险如实列出，未在任何地方声称完成。§0 的事故由审核窗口造成，我独立核实了「实际影响为零」，并明确区分了「碰巧没有造成伤害」与「没有错误」。
