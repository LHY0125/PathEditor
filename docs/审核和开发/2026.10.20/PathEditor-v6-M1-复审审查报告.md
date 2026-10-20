# PathEditor v6.0.0 M1 — 第二轮审查报告（复审）

- **审查日期**：2026-10-20
- **被审分支 / 提交**：`v6/m1-storage-history` @ `42f981f`（整改提交 `f7581dc`）
- **基准点**：`main @ 9e674ec`
- **范围**：整改增量 + 首轮审查报告 16 条整改项的复核
- **被回复对象**：`docs/审核和开发/2026.10.20/PathEditor-v6-M1-复审回执.md`（362 行）
- **规格来源**：`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`、`docs/superpowers/plans/2026-10-19-v6-sqlite-m1-plan.md`
- **规范来源**：`AGENTS.md` / `CLAUDE.md`、`patheditor-two-window-flow`、`patheditor-review-window`

---

## 提交清单（整改增量）

```
42f981f docs(v6): M1 复审回执
f7581dc fix(v6): 整改审查报告 V-1/V-2/V-3 与 J-2/J-3/J-4/J-5/J-7/J-8/J-9
5277407 chore(v6): 锁定 SQLCipher 依赖树        ← 首轮审查点
```

新增文件：`core/src/db/dacl.rs`、`core/tests/db_app_state.rs`、`core/tests/db_backup_production.rs`、`src/components/layout/DegradedBanner.tsx`。

---

## 一、验证证据（审核窗口独立执行）

| #   | 命令                                                    | 结果                                           |
| --- | ------------------------------------------------------- | ---------------------------------------------- |
| 1   | `cargo fmt --all -- --check`                            | exit 0                                         |
| 2   | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0                                         |
| 3   | `cargo test --workspace`                                | exit 0；**304** 个 `test ... ok`，9 个集成文件 |
| 4   | `cargo test -p patheditor-cli --bins`                   | 65 passed                                      |
| 5   | `npm run verify`                                        | exit 0                                         |
| 6   | `npx vitest run`                                        | 27 files / **332** tests                       |
| 7   | `npm run test:e2e`                                      | **41 passed** (1.2m)                           |
| 8   | 9 个 `core/tests/db_*.rs` 逐文件                        | 各 1 test（`ls` 确认为 9 个文件）              |

**与回执的差异**：回执称 `cargo test --workspace` 合计 **305**；实测为 **304**（229 + 9 + 65 = 303，再加 1 个内联）。连续两轮都是这个位置差 1，说明是回执的算术口径问题（把 `2 ignored` 或某个内联测试重复计入），不是功能问题。**这属于如实性问题：回执里的总数与证据对不上。**

---

## 二、三条阻塞项的复核

### V-1 DACL —— 已实现，但判据有盲区

**已确认落地的部分**：

| 要求           | 证据                                                                   |
| -------------- | ---------------------------------------------------------------------- |
| 写完立即设置   | `crypto.rs:118-131`，`sync_all()` 后 `drop(f)` 再 `set_key_dacl(path)` |
| 失败删半成品   | `crypto.rs:122-129`，`fs::remove_file(path)` + 返回 `Crypto`           |
| 启动校验不修改 | `mod.rs:203-221`，`Ok(false)` 只 `log::warn!`                          |
| 测试           | `db_key_lifecycle.rs:45-52`                                            |

审核窗口实测该文件真实 ACL：

```
$ icacls C:/Users/33644/.patheditor/keys/db.key.dpapi
NT AUTHORITY\SYSTEM:(F)
LHY\LHY:(F)
```

**只有 SYSTEM + 当前用户，没有任何继承条目。** DACL 实现是对的。

**关于 `SetFileSecurityW` vs `SetNamedSecurityInfoW`（回执请确认的第 1 点）**：

**采纳，判断正确。** 而且这个判断的严重性被低估了。我做了实验，把文件 DACL 设为 NULL（`SetNamedSecurityInfoW(..., si=4|0x80000000, dacl=NULL)`），然后按 Rust 侧同样的方式序列化：

```
MakeNull rc = 0
ACL SDDL  = D:PAINO_ACCESS_CONTROL
```

NULL DACL 序列化成 `NO_ACCESS_CONTROL`，**不含任何主体**。而 `key_dacl_is_restricted` 的判据是「黑名单命中则过宽」——`NO_ACCESS_CONTROL` 一个都不命中，**会被判为「受限」并返回 `Ok(true)`**。

也就是说：回执里说的「我第一版写错了，编译能过、测试也会『通过』」，**不是错觉**。如果当初用了 `SetNamedSecurityInfoW(..., null)`，`key_dacl_is_restricted` 会**明确确认通过**，而这个文件实际是所有人可访问的。这是一次真实的、被正确识别并修正的假绿灯，值得记为正面案例。改用 `SetFileSecurityW` 让「传错东西」变成「语义上不可能」。

**但判据本身仍有盲区（新增发现，非阻塞，建议 M2 修）**：

黑名单 `[";;;WD)", ";;;BU)", ";;;AU)", ";;;BG)", ";;;PS)"]`（`dacl.rs:224`）**漏了 `;;;CO)`（CREATOR OWNER）**。`ConvertSecurityDescriptorToStringSecurityDescriptorW` 默认按账户名解析 SDDL，解析不了的主体回落为 SID 形式（`;;;S-1-5-21-...`）——**任何具名 SID 形式都不命中黑名单**。

后果：若密钥文件的 DACL 被改成 `D:(A;;GA;;;CO)` 或 `D:(A;;GA;;;S-1-5-21-<别人的 SID>)`，`key_dacl_is_restricted` 返回 `Ok(true)`，启动时**不告警**，而这正是「另一个用户可删除/替换密钥文件」的威胁本身。

正确判据应是**白名单**：读取 ACL 的每个 ACE，断言主体集合 ⊆ {当前用户 SID, SYSTEM}。计划 `:678` 原本写的就是白名单（「断言只有当前用户 SID 与 SYSTEM」），实现改成了黑名单。M2 建议改回白名单，并把盲区写成测试。

> 不做阻塞的理由：这个盲区只在**文件被外部篡改后**才有意义，而 DACL 的主要作用是防篡改。防御的第一层（`set_key_dacl` 写入时的正确 SDDL）已经成立，实测 ACL 也正确。但「启动校验」这个第二层目前**基本测不出真实威胁**，只测得出最粗心的一种。

### V-2 `DbHistory` 接线 —— 已落地

| 要求                        | 证据                                                                                                                                           | 判定 |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| 4 个写命令走真实 sink       | `service.rs:33-35` 的 `with_history`；`save_path_with_sidecar` / `apply_path_snapshot` / `retry_pending_path_state` / `apply_profile` 全部使用 | ✅   |
| 降级回落不阻断编辑          | `service.rs:36-40`，锁中毒或 `None` 时 `f(&NoopHistory)`                                                                                       | ✅   |
| `migrate_legacy` 进启动序列 | `gui/src/lib.rs:11-45` 的 `setup_database`：pending 回放 → `migrate_to_current` → `migrate_legacy`                                             | ✅   |
| 迁移失败不阻断              | `lib.rs:34-40` 只 `log::warn!`                                                                                                                 | ✅   |

`with_history` 的文档注释准确解释了为什么需要这个辅助函数（`DbHistory<'a>` 借 `&'a mut Db`，`MutexGuard` 生命周期跨不过 `tauri::command` 返回），是个干净的解法。

**一处值得记录的连带后果**：`apply_profile` 现在会在持有 `State` 锁的情况下再取一次锁（`service.rs:82`，`with_history` 内部已经释放，所以不是死锁）。但**每次记应用次数都要 `list_profiles` 全表扫描再按名字线性查找**（`service.rs:88-92`）。配置文件通常不多，M1 可接受；M2 应加 `find_profile_by_name` 或直接让 `mark_profile_applied` 接受名字。

### V-3 `.pebak` 接生产 —— 已落地，且发现一个真实缺陷

**实测（审核窗口）**：

```
$ PATHEDITOR_BACKUP_DIR=<tmp> ./target/debug/patheditor.exe env backup
备份已保存到: ...\env_backup_20261020_120420_045.pebak
```

产物确为 `.pebak`，`README`/`CHANGELOG` 的声明现在是**真的**。

对回执请确认的第 2 点（**文件名用写入时刻**）：

**采纳，且这是本轮最有价值的发现。** `rotate_env_backups` 按文件名字典序降序排列、保留前 `keep` 份（`backup.rs` 的 `candidates.sort_by(|a,b| b.0.cmp(&a.0))`）。文件名内嵌时间戳即排序键。若用 `payload.captured_at` 命名，导入一份旧载荷会生成一个「看起来最旧」的文件，写入后立刻被轮换删掉——用户会看到「备份成功了但文件不见了」。改用 `Local::now()` 是对的。

**但要纠正回执里的一处表述**：回执 §3.1 V-3 说「这是审查报告没发现的缺陷」。准确地说，**这是 `.pebak` 从不被生产写入的必然结果**——首轮报告 V-3 指出的是「写入器没有调用者」，而这个命名缺陷只有在调用者接通后才会暴露。两者是同一条链上的前后环，不是两个独立发现。

---

## 三、「应该改」项复核

| 编号 | 项                            | 状态        | 证据                                                                                                                                                                                                                |
| ---- | ----------------------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| J-2  | 硬编码中文                    | ✅ 已改     | `zh-CN.json`/`en.json` 各加 `history` 段；`HistoryPanel.tsx:89`、`SnapshotDetail.tsx` 全部改 `t()`                                                                                                                  |
| J-3  | 注释与实际不符                | ✅ 已改     | `service.rs:7` 重写；`migrate_v5.rs:53` 补生产入口                                                                                                                                                                  |
| J-4  | `[object Object]`             | ✅ 已改     | `history-store.ts:13-25` 新增 `isCoreError`；`:26-35` 的 `errorMessage` 对 `crypto` 单独出文案                                                                                                                      |
| J-5  | `apply_count` 恒 0            | ✅ 已改     | `service.rs:76-105`                                                                                                                                                                                                 |
| J-7  | `app_state` 无行 → CAS 恒失败 | ✅ 已改     | `schema.rs:136-138` 的 `SEED_APP_STATE_SQL` + `mod.rs:97-108` 建库事务内 `INSERT OR IGNORE`；新增 `db_app_state.rs` 测 CAS **能成功一次**且陈旧 version 失败                                                        |
| J-8  | 详情恒空                      | ⚠️ **部分** | 链路打通（`EventView.snapshot_uid` + LEFT JOIN + 前端用 `snapshotUid`），且 `snapshotUid === null` 时 `HistoryPanel.tsx:89` 禁用按钮。但**服务层两处仍写死 `snapshot_uid: None`**（`service.rs:351`、`:486`），见下 |
| J-9  | 条幅非常驻                    | ✅ 已改     | 新增 `DegradedBanner.tsx`，挂在 `AppShell`；`README` 表述同步                                                                                                                                                       |
| J-1  | 计划文件命名                  | ❌ 未改     | 回执说明理由并要求授权                                                                                                                                                                                              |

### J-1 的处理 —— 授权，但理由需要更正

回执说那个计划文件的未提交改动「不是我造成的」。**这是事实性误判**：那 +93 行是我在上一轮（压缩记忆前）按用户要求写入的密钥存储规格与核对轮裁断表（`Local\PathEditor.KeyInit`、`set_key_dacl`、补充-1/补充-2 等）。不是 `/init` 残留，也不是开发窗口的改动。

不过回执的做法仍然正确：**在归属不明时不擅自改动别人的未提交内容**。这条纪律值得肯定。

**现在授权**：请单独提交一个只改该文件的 commit，内容包含：

1. Task 12 的交付物名 `PathEditor-v6-M1-完成报告.md` → `PathEditor-v6-M1-开发回执.md`；
2. Task 9 Step 4 的启动片段补上 `setup_database` 的三步（pending → `migrate_to_current` → `migrate_legacy`）——这是首轮报告指出的计划缺陷；
3. Task 3 的 DACL 骨架把 `SetNamedSecurityInfoW` 改为 `SetFileSecurityW`，并注明理由（NULL DACL 会序列化成 `NO_ACCESS_CONTROL`、被黑名单判据误判为受限）；
4. 裁断表补三行：V-1 判据盲区（黑名单漏 `;;;CO)` 与具名 SID）、V-3 文件名时刻、J-7 `app_state` 建库即写。

### J-8 的残留 —— 这是本轮唯一需要在合并前决定的事项

`core/src/service.rs:351` 与 `:486` 仍写 `snapshot_uid: None`。所以**生产路径上 `EventView.snapshotUid` 恒为 `null`**，前端按钮恒被禁用。

结果是：历史面板现在**只能看事件列表，点不开任何一条**。比首轮「点了静默空白」诚实（禁用 + 光标 `default`），但**功能上仍然不可用**。

而 `README.md:284` 写的是「选中后预览当时的完整快照」——**这句话在当前实现下不成立**。

这一条必须二选一：

- **(A) 合并前接上**：在 `commands::service` 的写路径里「先 `insert_snapshot` 拿 uid → 调服务 → 事件带 uid」。回执说这是 M2 的事，但它其实是 V-2 接线的**自然收尾**——`with_history` 里已经有 `&mut Db`，加一个 `insert_snapshot_tx` 调用不需要新架构。
- **(B) 明确降级**：改 `README.md:284` 为「按时间倒序浏览事件（快照预览见 M2）」，并在 CHANGELOG「已知限制」里写清。

我倾向 **(B) 合并、M2 做 (A)**：M1 的验收点是「存储 + 历史记录基础设施」，快照详情确实可以推。但**文档不能留着不实的句子合并**——这是首轮 V-3 已经证明过的教训。

---

## 四、新增发现

### N-1【非阻塞，但需登记】`dirs::home_dir()` 在 Windows 上**忽略** `HOME` 与 `USERPROFILE`

这是回执 R-05 事故的根因，但回执的描述有偏差。回执说「`dirs::home_dir()` 在 Windows 上读 `USERPROFILE` 不读 `HOME`」——我实测发现**它两个都不读**：

```
$ HOME=<tmp> USERPROFILE=<tmp> PATHEDITOR_BACKUP_DIR=<tmp>/backups ./patheditor.exe env backup
备份已保存到: <tmp>/backups/env_backup_...pebak
$ ls <tmp>/keys
no key under fake home -> dirs::home_dir ignores env vars
```

`dirs::home_dir()` 走 `SHGetKnownFolderPath(FOLDERID_Profile)`，读的是注册表/用户配置文件，**修改环境变量对同一用户无效**。所以：

- 回执里「我设了 HOME（无效）」——对，但原因不是「Windows 读 USERPROFILE」；
- 「PATHEDITOR_BACKUP_DIR（有效）」——对，但那是 `backup_base_dir()` 自己读的环境变量（`backup.rs:148`），**和 `dirs` 无关**。

**后果**：`config_dir()`（`crypto.rs:45-49`）**无法被任何环境变量重定向**。这意味着**所有触及真实 `~/.patheditor/` 的生产路径都无法在测试里被隔离**。

我复核了当前测试是否还有这个风险：

| 测试                                  | 是否触碰真实 `~/.patheditor/` | 证据                                                                              |
| ------------------------------------- | ----------------------------- | --------------------------------------------------------------------------------- |
| `db_backup_production.rs`             | ❌ 不碰                       | 用 `write_env_backup_to_with_key(dir, payload, load_or_create_key(tmp))`          |
| `cli/src/env_ops.rs` 的 `make_backup` | ⚠️ **碰**                     | 跑真实 `env backup` 子进程，写 `.pebak` 需要密钥 → 落到真实 `~/.patheditor/keys/` |
| `write_env_backup_for_test`           | ⚠️ **碰**                     | `load_or_create_key(&config_dir())`                                               |

**当前不造成新污染的证据**：我在跑完 `cargo test --workspace` 与 `cargo test -p patheditor-cli --bins` 之后检查，`C:/Users/33644/.patheditor/keys/db.key.dpapi` 的 mtime **仍是 11:25:40**（即回执那次的创建时刻），没有变化。原因是 `load_or_create_key` 读到已有文件就返回，不重写。

**但这是一颗定时炸弹**：任何一次「删掉密钥文件后跑测试」或 CI 上的干净环境，都会重新创建真实密钥。建议 M2 把 `config_dir()` 改为支持 `PATHEDITOR_HOME` 重定向（与 `PATHEDITOR_BACKUP_DIR` 对称），并让 `<home>/keys` 的解析走同一个函数。

### N-2【非阻塞】CLI **没有日志后端**，`log::warn!` 全部丢弃

`grep -rn "set_logger|init_logger|env_logger::init" --include=*.rs .` 只有一处命中：`backup.rs:1919`，且**在 `#[cfg(test)]` 内**。GUI 注册了 `tauri_plugin_log`（`lib.rs:52`），CLI **没有**。

这意味着生产 CLI 里所有 `log::warn!` 都是空操作。具体受影响的有：

- `backup.rs:320-325`：**拿不到密钥回落明文 `.json` 的告警**——这是唯一能让用户知道「磁盘上新增了明文敏感文件」的通道，实际不会显示；
- `backup.rs:215-218`（`read_env_backup_any`）：**读取遗留明文备份的警告**——spec §7.9 与 README 都承诺「会提示明文警告」，实际不会提示。

**实测**：

```
$ PATHEDITOR_BACKUP_DIR=<tmp> ./patheditor.exe env restore <legacy>.json --dry-run
将新增 0 个、修改 0 个、删除 0 个；冲突 0 个（未写入任何内容）
（stderr 为空）
```

**零警告**。`README.md:292`「遗留的 `env_backup_*.json` 仍可一次性恢复（会提示明文警告）」**不成立**。

这一条与首轮 V-3 是同一类问题：文档承诺了一个实现里不存在的行为。只是这次藏在 `log` 宏后面，`grep 明文` 搜不到。

**整改建议（低成本）**：把这两处从 `log::warn!` 改为 `eprintln!`（CLI 侧已有 `eprintln!("警告: ...")` 的既有风格，见 `env_ops.rs:653`）。或者 M2 引入 `env_logger` 并支持 `--verbose`。

### N-3【非阻塞】`plan` 里的 DACL 骨架与实际实现已分叉

计划 `:656-664` 写的仍是 `SetNamedSecurityInfoW`，实现在 `dacl.rs` 用 `SetFileSecurityW`。见 J-1 整改第 3 条。

### N-4【非阻塞】`profile_mark_applied` 的 IPC 与库函数职责重复

`commands::service::apply_profile` 内部直接调了 `db::profiles::mark_profile_applied`（`service.rs:95`），而 `commands::history::profile_mark_applied`（`history.rs:131`）与 `backend.profileMarkApplied` 都没有调用者。回执 R-02 已登记，**判定合理**（M2 手动物理重新应用时会用到）。不阻塞。

---

## 五、关于污染文件（回执请确认的第 3 点）

`C:/Users/33644/.patheditor/keys/db.key.dpapi`（278 字节，2026-10-20 11:25:40）。

**我的判断：保留。**

理由：

1. 它是**一个格式完全正确、DACL 也正确**的密钥文件（`icacls` 实测见 §二）。GUI 首次启动会直接复用它，不会产生任何异常。
2. 当前 `~/.patheditor/` 下**没有 `patheditor.db`**（实测 `ls *.db*` 为空），所以这把密钥目前**没有加密任何东西**——删与不删对用户数据都没有影响。
3. 删掉反而有风险：如果将来某个中间状态已经用它建了库，删密钥 = 历史永久不可读。现在保留是零风险选择。
4. 全局规则「未经书面同意不得删除任何文件」在此成立，**不授权删除**。

回执按「不删除 + 如实报告」处理是**正确的**，且 R-05 的自我剖析（「识别了风险，没把防范核对到每个路径来源」）比事故本身更有价值。结合 N-1 的实测更正，这条经验应该更新为：**`dirs::home_dir()` 在 Windows 上不可重定向，所以「用环境变量隔离测试」这个思路对 `config_dir()` 根本不成立**。

---

## 六、总结

### 首轮 16 项整改清单的状态

| 类别                       | 已改 | 部分     | 未改               |
| -------------------------- | ---- | -------- | ------------------ |
| 阻塞（V-1/V-2/V-3）        | 3    | —        | —                  |
| 应该改（J-1…J-5, J-7…J-9） | 6    | 1（J-8） | 1（J-1，现已授权） |
| 可延后（J-6/J-10, a-\*）   | —    | —        | 按约定延后         |

### 合并前必须处理

1. **J-8 / README 不实**：`snapshotUid` 恒 null + `README.md:284`「选中后预览完整快照」。二选一——接上（建议 M2）或改文档（建议现在做）。
2. **N-2 / README 不实**：`README.md:292`「会提示明文警告」在 CLI 上不成立。改 `log::warn!` → `eprintln!`（约 3 行），或改文档。
3. **J-1 计划文件**：已授权，单独 commit（内容见 §三）。

### 可以合并后做（M2）

- V-1 判据改白名单（黑名单漏 `;;;CO)` 与具名 SID）；
- `config_dir()` 支持 `PATHEDITOR_HOME` 重定向；
- `snapshotUid` 真正回填；
- 历史面板 E2E（spec §3.2.1 要求 3–4 条主路径，目前仍是 0）。

### 本轮判定

**整改质量高，判定可以进入合并流程，但需先处理上面 3 条「合并前必须」。**

三条阻塞项全部真实落地，且我逐条独立复现过：

- V-1 的 `icacls` 实测产出正是「仅当前用户 + SYSTEM」；
- V-2 的 `with_history` 与 `setup_database` 都在生产调用链上；
- V-3 的 `env backup` 实测确实产出 `.pebak`。

回执对 `SetFileSecurityW` 的判断**比我预期的更重要**——我用 NULL DACL 做了实验，确认旧判据会把它误判为「受限」（`D:PAINO_ACCESS_CONTROL` 不命中任何黑名单项）。这是一个真实的假绿灯，被正确识别并消除。

回执在诚实度上也有明显提升：主动登记了 R-05 污染事故、`snapshotUid` 恒 null、历史面板零 E2E、DACL 启动分支未测。这些都没有被包装成「已完成」。

但**「文档承诺 vs 实现」的问题这轮又出现了两处新的**（README 的快照预览、README 的明文警告），而且是同一类问题——首轮 V-3 已经因为「README 说写 `.pebak` 但实际写 `.json`」被判过。说明整改时的核对范围停在了「代码改了没有」，没有延伸到「文档里每句话现在还成立吗」。建议把「文档声明逐句核对」加进开发窗口的自查清单，而不是每轮靠审查窗口抓。

---

**审查窗口签章**：本报告所有「实测」结论均附有审核窗口独立执行的命令与输出（§一、§二、§四、§五）。`icacls` 与 NULL DACL 实验是在本机 PowerShell 上跑的，命令见正文。未引用回执中的任何未经复现的数字。
