# PathEditor v6.0.0 M1 缺口清单与后续里程碑归属

- **编制**：审核窗口
- **日期**：2026-10-20
- **依据**：`specs/2026-10-19-v6-sqlite-design.md`（下称 spec）、`plans/2026-10-19-v6-sqlite-m1-plan.md`（下称计划）、三轮审查报告、`PathEditor-v6-M1-复审回执.md`
- **用途**：交给开发窗口继续开发。**每一条都带 `file:line` 证据与验收判据**，不需要回看聊天记录。
- **当前分支**：`v6/m1-storage-history` @ `be88f35`，领先 `main` 24 个提交，未推送、未合并，版本仍是 `5.1.7`。

---

## 零、先说结论

**M1 的代码骨架完成了，但 M1 不是「差一点」，而是「三个核心承诺只兑现了一个」。**

| 承诺                                                         | 状态                                                                 |
| ------------------------------------------------------------ | -------------------------------------------------------------------- |
| 加密存储基础设施（SQLCipher / DPAPI / DACL / schema / 迁移） | ✅ 完成，质量高                                                      |
| 用户能用上「历史记录」                                       | ❌ **几乎为零**：列表为空、详情禁用、无恢复                          |
| 配置与备份迁入 DB                                            | ⚠️ **半成品且有害**：DB 里存了数据，但 UI 仍读写 v5 文件，两边不同步 |

**最危险的不是缺功能，是 P0-1**：迁移把 `profiles/*.json` 导入 DB 后，UI 仍在写 v5 文件，**DB 里的数据从此冻结、永不更新**。这是「同一份数据两个真相源」，比没迁移更糟。

下面按 **P0（阻塞）/ P1（M1 内必须补）/ P2（可移入 M2）** 和 **M2/M3 归属** 分组。

---

## 一、P0 —— 阻塞，必须先修

### P0-1【数据不一致】配置方案存在两个真相源，DB 侧冻结

**现象**

- DB 侧：`core/src/db/profiles.rs` 实现了 `save_profile` / `list_profiles` / `get_profile` / `delete_profile`，IPC 命令 `profile_list` / `profile_delete` 已注册（`gui/src/lib.rs:116-118`）。
- UI 侧：配置对话框走的是**旧的 v5 文件通路**——
  - `src/components/dialogs/profile/use-profiles.ts:19` → `backend.listProfiles()` → `invoke('list_profiles')`（`backend.ts:786`）→ `gui/src/commands/profiles.rs:4` → `core::profiles::list_profiles()`（**文件**）
  - `:32` → `backend.saveProfile()` → `invoke('save_profile')`（`backend.ts:788`）→ `core::profiles::save_profile()`（**写文件**，`core/src/profiles.rs:136-152`）
- 同时 `migrate_legacy` 已在启动时把 `profiles/*.json` **一次性导入 DB**（`gui/src/lib.rs:23`）。

**后果链**

1. 首次启动：`profiles/*.json` → 导入 DB 的 `profiles` + `profile_revisions` + `snapshots`。
2. 用户在 UI 里新建/改名/删除配置 → **只改文件，DB 不变**。
3. 用户查看 `profile_list`（未来任何走 DB 的面板）→ 看到的是**迁移那一刻的快照**，与文件内容已经完全脱节。
4. 由于 `migration_imports` 幂等，**DB 永远不会重新导入**，这个脱节是永久的。

**为什么是 P0**：这不是「功能没做」，是「做了一半且制造了新的不一致源」。`docs/superpowers/specs/...-design.md` §7.8 明确写了「**迁移成功后，正常模式不再写 `disabled.json`**」——同一条原则适用于 `profiles/*.json`，而实现只做了一半（导入做了，停写没做）。

**验收判据**

- 二选一，且必须在文档里写清选了哪个：
  - **(A) 接通 DB**：`use-profiles.ts` 改调 `backend.profileList()`（DB 版），`save_profile` 实现为「PATH 条目 + 元数据一次写入」的 IPC，同时停止写 `profiles/*.json`。
  - **(B) 明确暂缓**：把 `profile_list` / `profile_delete` IPC 与 `gui/src/commands/history.rs` 里的相关命令**下线**（不注册），并在 README/CHANGELOG 写明「v6.0.0 的配置仍由 v5 文件承载，DB 迁移仅作归档」。
- 无论选哪个，**都必须停止「导入后永不更新」的静默状态**。

---

### P0-2【功能不可用】`snapshotUid` 恒为 `null`，历史面板详情永远打不开

**现象**

- `core/src/service.rs:351` 与 `:486` 两处 `HistoryEvent` 都写死 `snapshot_uid: None`。
- `src/components/history/HistoryPanel.tsx:89` 的 `clickable = e.snapshotUid !== null` 因此恒为 `false`，按钮禁用。
- 三个写命令（`save_path_with_sidecar` / `apply_path_snapshot` / `apply_profile`）都**只记事件、不建快照**。

**已就位的部分**（不要重做）

- `HistoryEvent.snapshot_uid` 字段（`core/src/history.rs:65-90`）
- `DbHistory::record` 会把 uid 解析成 `snapshot_id` 并落库（`core/src/db/history_sink.rs:44-53`）
- `EventView.snapshot_uid` + LEFT JOIN（`core/src/db/history.rs:337-386`）
- 前端形状校验与 store 用法（`backend.ts:612`、`history-store.ts:77-85`）

**缺的是最后一段**：在写注册表的调用点上「先建快照、再记事件」。

**实现建议**

在 `gui/src/commands/service.rs` 的 `with_history` 内，把「建快照」和「调服务」放进同一事务：

```rust
// 伪代码：以 save_path_with_sidecar 为例
let snapshot_uid = history::insert_snapshot(db, NewSnapshot { /* 当前 PATH */ })?;
service::save_path_with_sidecar(system, user, &DbHistory::new_with_snapshot(db, snapshot_uid))?;
```

注意：`insert_snapshot` 自带事务，`DbHistory::record` 也开事务——**不能让两者嵌套**（SQLite 不支持）。需要改用 `insert_snapshot_tx` + 与 `record` 合并到一个事务，或先提交快照再记事件（接受「有快照无事件」的窗口，比「有事件无快照」安全）。

**验收判据**

- 手工保存一次 PATH → 历史面板出现一条事件 → **点击能展开快照详情**（PATH 条目 + 环境变量，敏感值仍被掩码）。
- 新增测试：断言落库后 `events.snapshot_id` 不为 NULL。

---

### P0-3【功能不可用】`history_restore_snapshot` 完全不存在

**现象**：全仓库 `grep -rn "history_restore_snapshot"` → **零命中**。

**为什么是 P0**：spec §十三 的 M1 验证门写的是「迁移夹具 + **历史/恢复** + 并发 CAS + 降级 + `verify:all`」。**「恢复」是 M1 验收门的组成部分**，不是 M2 的延伸。spec §6.1「历史时间线」第 3 条也明确列出 `restore_snapshot(uid)`。

**验收判据**

- `history_restore_snapshot(uid)` → 把该快照交给**既有注册表写入编排**（`service::apply_path_snapshot` 或 `save_path_with_sidecar`），**不新增写注册表的代码路径**。
- 成功后 append 新快照与新事件（**append-only，不回滚 DB**）。
- UI：历史面板详情区加「恢复到此快照」按钮，带二次确认。
- 测试：恢复后 `app_state.current_snapshot_id` 指向新快照，旧快照仍在（append-only）。

---

## 二、P1 —— M1 范围内必须补

### P1-1 `EventFilter` 缺 4 个筛选维度

**现象**：`core/src/db/history.rs` 的 `EventFilter` 只有 `limit` / `offset` / `action` / `source`；`list_events` 的 WHERE 也只拼了这两个（`:352`）。

**spec 要求**（§6.1）：「支持 `source` / `action` / `hive` / `target` / `result` / **时间范围** / 分页」——**缺 `hive` / `target` / `result` / 时间范围**（4 项）。

**验收判据**

- `EventFilter` 增 `hive: Option<String>` / `target: Option<String>` / `result: Option<String>` / `from_ms: Option<i64>` / `to_ms: Option<i64>`。
- WHERE 与 `total` 的 count 查询**用同一段 WHERE**（`history.rs:352` 已有注释警告过这一点，别破坏）。
- `gui/src/commands/history.rs` 的 `EventFilterRequest` 同步扩充。
- UI 至少提供 `action` / `source` 两个下拉（其余可延后，但 **core 与 IPC 必须支持**）。

### P1-2 `index_backups` 无生产调用者，`backup_list` 永远返回空

**现象**

- `core/src/db/backups.rs:267` 的 `index_backups` 只被 `core/tests/db_backups_index.rs` 调用。
- `backup_list` IPC（`gui/src/commands/history.rs:142`）读 `backups` 表，但**没人往里写**。
- 前端也没有任何地方调 `backend.backupList`。

**后果**：spec §6.1「备份索引」整节未兑现，`backup_list` 是死命令。

**验收判据**

- 启动时（`setup_database`）或 `backup_list` 首次调用时触发 `index_backups(&mut db, &env_backup_dir())`。
- 或明确：**M1 不提供备份索引 UI**，则把 `backup_list` 下线并在文档登记，避免留下「命令在、数据恒空」的陷阱。

### P1-3 `profile_save` IPC 缺失（若选 P0-1 的方案 A 则是阻塞）

**现象**：`gui/src/commands/history.rs:137-138` 只有一条注释说明它没实现。

**验收判据**：见 P0-1 的方案 A。

### P1-4 v5 文件停写（H7）未实现

**现象**

- spec §7.8：「迁移成功后，正常模式**不再写 `disabled.json`**；`disabled.json` / `profiles/*.json` 只读归档」。
- 实际：`core/src/disabled.rs` 与 `core/src/profiles.rs` 的写函数**零改动**，仍然正常写文件。
- `app_state.migrated_v5` 字段存在（`schema.rs:33`），但**没有任何代码读它来做分支**。

**验收判据**

- 写路径前置检查：`if app_state.migrated_v5 == 1 { 降级为只写 DB }`。
- 降级模式（DB 不可用）时恢复写文件（spec §7.8 的兜底要求）。

### P1-5 `.v5.bak` 归档未做，且模块文档引用错了 spec 章节

**现象**

- `core/src/db/migrate_v5.rs:6` 写：「**不动原件**：只读 + 登记导入记录；`.v5.bak` 归档与删除留给 M4（**spec §7.4**）」。
- 实际 spec 里 §7.4 是「**原件：保留为 `*.v5.bak`；不自动删除**」，属于**迁移规则本身**；授权 M4 做的是 §7.6 的「清理 `env_backup_*.json`」。**引用标错了。**

**判定**：归档动作本身推迟到 M4 是合理的（M1 不该改用户文件），但**文档必须改对**，否则下一个人会以为 §7.4 授权了推迟。

**验收判据**：把 `migrate_v5.rs:6` 的引用改为「spec §7.6 授权 M4 清理；§7.4 的 `.v5.bak` 归档同样推迟到 M4，理由：不在迁移中改用户文件」。

### P1-6 历史面板 E2E 为零

**现象**：`grep -rln "history" e2e/` → 零命中。

**spec 要求**（§3.2.1 测试要求）：「E2E 只覆盖 3–4 条主路径（**历史面板、恢复、配置应用**）」。

**验收判据**：至少 3 条：

1. 打开历史标签页 → 空态正确（不是错误）
2. 降级模式 → 常驻条幅可见
3. 保存一次后 → 出现一条事件 → 点击展开详情（依赖 P0-2）

### P1-7 跨会话撤销/重做未实现

**现象**：`src/core/undo-redo.ts:41` 有内存版 `UndoRedoManager`；DB 侧无任何读取 `events` 重建撤销栈的代码。

**spec 要求**（§6.1「跨会话撤销/重做」）：「UI 的撤销栈从事件流重建；进程重启后仍可用」「v6.0.0 保留内存栈作为即时 UI，**持久化事件作为跨会话恢复来源**」。

**判定**：这是 M1 明确列出的功能。但**它依赖 P0-2**（事件必须带快照才能重建撤销）。**建议排在 P0-2 之后**。

**验收判据**：重启后打开历史面板，能从 `events` 读到最后 N 次操作；`undo` 行为对得上。

---

## 三、P2 —— 可移入 M2，但必须显式登记

以下条目**不在 spec 的 M1 验收门里**，移入 M2 是合理的。但移入时必须在 M2 计划里列出，不能像现在这样「没人管」。

| #    | 项                                                                                                  | 证据                                                              | 说明                                                                                                          |
| ---- | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| P2-1 | `history_reveal_env_value`                                                                          | 全仓库零命中                                                      | spec §3.2.1 要求「需要明文时走独立的 `history_reveal_env_value`」。M1 面板只能看掩码值。                      |
| P2-2 | `profile_export_v5` / `import_profile_v5`                                                           | 全仓库零命中                                                      | spec §6.1「配置元数据」明确要求。**这是 v5 用户的退路**（降级到 v5 时能导出），不应无限推迟。                 |
| P2-3 | 列表分页                                                                                            | `backup_list` / `profile_list` 无分页参数（`history.rs:114,142`） | spec §3.2.1 规则 1 要求所有列表支持 `{limit, offset}`。数据量小时无感，但契约不一致。                         |
| P2-4 | v5 导出（降级通路）                                                                                 | spec §7.5「v6 提供『导出 v5 JSON』」                              | 与 P2-2 同源。                                                                                                |
| P2-5 | `changes` / `sync_conflicts` / `daily_stats` / `scan_runs` / `var_references` / `search_fts` 六张表 | 实际 `schema.rs` 只建了 9 张表；spec §4.1 定义了 15 张            | **M2 建 `scan_runs` / `var_references` / `daily_stats` / `search_fts`；M3 建 `changes` / `sync_conflicts`**。 |

---

## 四、M2 / M3 归属（明确划界，避免再次悬空）

### M2：检索与统计

**spec §十三 原文**：「FTS5、统计、N 路对比、引用图、冲突历史」

| 项                                                     | spec 位置  | 当前状态       |
| ------------------------------------------------------ | ---------- | -------------- |
| FTS5 全文检索（`search_fts` 虚拟表 + `search(query)`） | §4.1、§6.1 | 未建表、未实现 |
| 统计仪表盘（`daily_stats`）                            | §4.1、§6.2 | 未建表         |
| N 路对比                                               | §6.2       | 未实现         |
| 环境变量引用图（`var_references`）                     | §4.1、§6.2 | 未建表         |
| 冲突历史（`scan_runs`）                                | §4.1       | 未建表         |
| **M1 移入项**：P2-1 ~ P2-4、以及 P1-2 若选「延后」     | —          | 见上表         |

**M2 验证门**（spec §十三）：`查询/图/统计测试 + UI E2E`

### M3：同步

**spec §十三 原文**：「changes/墓碑/bundle/合并/冲突 UI」

| 项                                  | spec 位置 | 当前状态             |
| ----------------------------------- | --------- | -------------------- |
| `changes` / `sync_conflicts` 表     | §4.1      | 未建表               |
| Argon2id + ChaCha20-Poly1305 同步包 | §5.2      | 未引入依赖           |
| 文件式变更包导出/导入               | §6.4      | 未实现               |
| 冲突合并 UI                         | §6.4      | 未实现               |
| `ErrorCode::Sync → exit 4`          | §8        | 码已定义，映射未实现 |

**M3 验证门**：`两库同步 + 并发/篡改/幂等测试`

> ⚠️ **M2 / M3 的计划文档目前都不存在**（`plans/` 下只有 `m0-plan` 与 `m1-plan`）。在写 M2 计划之前，上面这些项没有人认领——这正是 M1「恢复」被推给 M2、而 M2 又不存在的原因。

---

## 五、建议的开发顺序

```
① P0-1  配置双真相源（决定方案 A 还是 B，先止血）
② P0-2  snapshotUid 回填（打通详情）
③ P0-3  history_restore_snapshot（M1 验收门要求）
④ P1-1  EventFilter 补 4 维（恢复之后的筛选才有意义）
⑤ P1-6  E2E 3 条（spec §3.2.1 硬要求）
⑥ P1-4  v5 文件停写（H7）
⑦ P1-7  跨会话撤销（依赖 ②）
⑧ P1-2  备份索引接线（或明确下线）
⑨ P1-5  修正 migrate_v5.rs:6 的错误引用（一行改动，随时可做）
```

**③ 完成后 M1 才满足 spec 的验证门**，可以合并。④⑤ 是补充，**⑥⑦⑧ 建议同批**（都涉及「v5 与 DB 的边界」，一起做避免二次返工）。

---

## 六、给开发窗口的三条提醒

1. **不要再把 M1 的项推给 M2。** 本轮的教训是：`history_restore_snapshot` 被标「M2」，但 spec §十三 的 M1 验证门明确包含「历史/恢复」，而 M2 计划又不存在——推出去的东西会直接消失。**要推，先在文档里写下接收方。**

2. **P0-1 是最容易被漏掉的**，因为它「看起来已经做完了」：DB 表有、core 函数有、IPC 有。只有把 UI 的实际调用链跟一遍（`use-profiles.ts` → `backend.ts:786` → `commands/profiles.rs:4` → `core/profiles.rs`）才能发现它读的是文件。**判定「接通」的标准是调用链闭环，不是函数存在。**

3. **修正 `migrate_v5.rs:6` 的错误引用要顺手做**。它已经误导过一轮审查（我据此判「不符合 spec」，开发窗口据此认为「已处理」）。错误的指针比缺失的信息更危险，因为它让人以为查过了。

---

## 七、本清单的判定依据与局限

- 所有「未实现 / 无调用者」结论均用 `grep -rn` 在 `core/ gui/ cli/ src/` 全集上验证过，命中数为 0 或只命中测试。
- 「M1 验收门包含恢复」直接引自 spec §十三 的 M1 行，非推断。
- 本轮**未执行**：真实 GUI 端到端手工验证、CI（MSVC）验证、真实注册表写入。三者都不在本清单的判定范围内，如需可作为补充证据。
- 本清单**不评价**已完成的加密/迁移/备份部分——三轮审查报告已确认它们正确，此处不重复。
