# PathEditor v6.0.0 M1 — 第三轮审查报告（复审 II）

- **审查日期**：2026-10-20
- **被审分支 / 提交**：`v6/m1-storage-history` @ `460fbec`
- **基准点**：`main @ 9e674ec`
- **被回复对象**：`docs/审核和开发/2026.10.20/PathEditor-v6-M1-第二轮复审回执.md`（含 `460fbec` 的 R-06 更正）
- **规格来源**：`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`、`docs/superpowers/plans/2026-10-19-v6-sqlite-m1-plan.md`（已被 `6946181` 修订）
- **规范来源**：`AGENTS.md` / `CLAUDE.md`、`patheditor-two-window-flow`、`patheditor-review-window`

---

## 提交清单（本轮增量）

```
460fbec docs(v6): 更正 R-06 —— patheditor.db 是正常产物，无需处理
d10f759 docs(v6): M1 第二轮复审回执
6946181 docs(v6): M1 计划修订（J-1 交付物名、启动序列三步、DACL API、三行裁断）
b876c03 fix(v6): N-2 CLI 明文警告走 stderr，README 去掉两处不实声明
42f981f docs(v6): M1 复审回执                      ← 上一轮审查点
```

---

## 一、验证证据（审核窗口独立执行）

| #   | 命令                                                    | 结果                                               |
| --- | ------------------------------------------------------- | -------------------------------------------------- |
| 1   | `cargo fmt --all -- --check`                            | exit 0                                             |
| 2   | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0                                             |
| 3   | `cargo test --workspace`                                | 15 binaries，**305 passed / 0 failed / 2 ignored** |
| 4   | `npm run verify`                                        | exit 0                                             |
| 5   | `npx vitest run`                                        | 27 files / **332** tests                           |
| 6   | `npm run test:e2e`                                      | **41 passed**                                      |

**关于连续三轮的「差 1」——开发窗口的解释正确，我上一轮的 304 是错的。**

回执 §4 给出了可复现命令并把 `passed`/`failed`/`ignored` 三栏显式拆开：

```
binaries=15 passed=305 failed=0 ignored=2
```

我上一轮用 `awk '{p+=$4}'` 求和，只累加了每行第 4 个字段，漏掉了跨行口径（多个二进制 + 一个 `0 passed` 的 doc-test 行）。**回执的 305 是正确的**。三轮口径问题至此闭合：首轮回执 301 错（应该 302）、上一轮我 304 错（应该 305）、本轮 305 对。

这条值得单记：**我在两轮审查里都用了同一个不可靠的计数方式**，而开发窗口在回执里主动把口径固定成命令 + 显式拆分，这才是「数字可复现」的正确形态。

---

## 二、本轮三条「合并前必须」的复核

### 2.1 J-8 / README 不实 —— ✅ 已用「改文档」解决

`b876c03` 把 `README.md:284` 的「选中后预览当时的完整快照」删除，改为：

```markdown
- 「历史记录」标签页按时间倒序浏览事件

> **M1 的已知限制**：事件列表可以浏览，但**点击事件暂时看不到快照详情** ——
> 事件与快照的关联（`snapshotUid`）要到 M2 才接通，当前按钮为禁用状态。
> 不要在 M2 之前把它当成「可预览完整快照」。
```

**这是正确的选择**（我上一轮推荐的 (B) 方案）。实测代码确认表述成立：`service.rs:351`、`:486` 仍写 `snapshot_uid: None`，`HistoryPanel.tsx:89` 的 `clickable = e.snapshotUid !== null` 因此恒为 `false`，按钮禁用、光标 `default`。

**但 CHANGELOG 有一处过时表述（新增发现）**：

`CHANGELOG.md:38`：

> 历史事件的 `snapshot_id` 暂未回填（`EventView` 不含 `snapshotUid`）。

**后半句现在是错的**。`backend.ts:505`、`:555`、`:612` 都已有 `snapshotUid: string | null`，`EventView` **确实包含**这个字段。真实状态是「字段有了、链路通了，但服务层不填值」。

准确写法应为：

> 历史事件的 `snapshotUid` 在服务层尚未回填（字段与 IPC 链路已在 M1 就位，`commands::service` 的两处 `HistoryEvent` 仍传 `None`），因此历史面板的详情按钮当前恒为禁用。真正回填需要「先建快照再记事件」，属 M2。

**不阻塞合并**（这是文档措辞精度问题，不是不实承诺），但既然本轮专门修了一轮文档不实，这条漏网就该一并修。

### 2.2 N-2 / CLI 明文警告 —— ✅ 已修复且实测有效

`b876c03` 的修法比我建议的更讲究。我建议的是「`log::warn!` 改 `eprintln!`」，开发窗口没有照做，而是：

1. 新增 `BackupFormat` 枚举 + `read_env_backup_with_format()`（`backup.rs:1062-1096`），把「读到了什么格式」作为**返回值**交给调用方；
2. 在 CLI 侧（`env_ops.rs`）打印到 stderr；
3. 注释里写明理由：「库不该直接 `eprintln!`（会污染 GUI 的 stdout/stderr），正确做法是把『发生了什么』作为返回值交给调用方」。

**这个判断比我的建议更正确**。core 是库，GUI 与 CLI 同为调用方；库直接写 stderr 会把警告塞进 Tauri 应用的终端输出。返回值形态让两个调用方各自决定呈现方式。

**实测验证**：

```
$ PATHEDITOR_BACKUP_DIR=<tmp> ./patheditor.exe env restore <legacy>.json --dry-run
警告: ...legacy.json 是遗留明文备份（v6 起的备份为 DPAPI 加密的 .pebak）。
      恢复后建议重新执行 `patheditor env backup` 生成加密副本。
将新增 0 个、修改 0 个、删除 0 个；冲突 0 个（未写入任何内容）

$ （只取 stderr）
警告: ...legacy.json 是遗留明文备份……
```

警告确实走 **stderr**，stdout 保持纯净契约。`README.md:297` 的「读取时会在 stderr 打印明文警告」**现在成立**。

降级回落的警告（`backup.rs:320-327` 的 `log::warn!` + CLI 侧 `cmd_env_backup` 的 `eprintln!`）也已接上，`README.md:298` 成立。

### 2.3 J-1 计划文件 —— ✅ 四条全部落地

`6946181` 对 M1 计划的修订（+169/−18），逐条核对：

| 授权内容                                               | 证据                                                                                                                                                                      |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Task 12 交付物名 → `开发回执`                          | `- Create: …PathEditor-v6-M1-完成报告.md` → `+ …YYYY.MM.DD-…-开发回执.md`，并附「命名遵循 `patheditor-two-window-flow` 固定约定」说明；Step 3 标题与 `git add` 命令同步改 |
| Task 9 Step 4 补 `setup_database` 三步                 | 新增 `setup_database(&mut db)` 片段，并注明「三步**缺一不可**（V-2 整改的教训：首版计划漏了第 ③ 步，导致 `migrate_legacy` 在生产中根本没有调用者）」                      |
| DACL 骨架 `SetNamedSecurityInfoW` → `SetFileSecurityW` | 骨架代码与注释都改了，并把 `D:PAI(NO_ACCESS_CONTROL)` 的假绿灯机制写进理由                                                                                                |
| 裁断表补三行                                           | 已见 `V-1 判据盲区` 一行（含 `;;;CO)` 与具名 SID 的说明 + 「M2 应改为白名单」）                                                                                           |

**修订质量高于要求**：不只改了名字，还把「为什么」留在了计划里。下一轮 M2 计划起草时，这三条教训会被直接读到。

---

## 三、开发窗口的自我更正 —— 核实成立

回执里那段关于「31 条记录」的自我更正，我独立复现了：

```
$ grep -c "INSERT INTO events" core/src/db/migrate_v5.rs
0

$ sed -n '48,52p' core/src/db/migrate_v5.rs
pub fn migrate_legacy(db: &mut Db, home: &Path) -> Result<MigrationReport, CoreError> {
    migrate_legacy_with(db, home, &NoopHistory)
}

$ grep -n "migrate_legacy" gui/src/lib.rs
23:    match path_editor_core::db::migrate_v5::migrate_legacy(db, &home) {
```

**三条全部成立**：

1. 迁移只写 `snapshots` / `path_entries` / `profiles`，**不写 `events`**；
2. `migrate_legacy`（无 `_with`）传 `NoopHistory`，`migrate_legacy_with` 里的 `history.record(...)` 不落库；
3. `setup_database` 调的正是无后缀版本。

**所以历史面板首次打开显示「还没有历史记录」是当前代码的正确行为，不是 bug。** 如果它显示了内容，反而是异常。

这个自我更正的价值在于：**它把「看到东西 = 验证成功」这个直觉预期，纠正成了可证伪的预期**。上一版回执里那句话如果留着，下一轮真去手工验证的人会把它当成成功判据——面板空了会以为坏了，而我这种审报告的人也可能把「空了」读成「功能没接上」。

**这与本轮 §2.1 的 J-8 状态是同一件事的两面**：事件不写库（迁移用 Noop） + 服务层 `snapshot_uid: None`，两者共同决定了 M1 的历史面板在首次启动后必然是空的且不可点。回执把它写清楚了，这是正面的。

---

## 四、R-06 的更正 —— 同意，但对照表有一处需要补正

`460fbec` 把 `keys/db.key.dpapi` 从「需要处置的产物」改为「GUI 正常启动的产物」。**结论我同意**（上一轮我已明确说「保留、不授权删除」），但对照表里有一格不准确：

|              | `keys/db.key.dpapi` | `patheditor.db`      |
| ------------ | ------------------- | -------------------- |
| **怎么来的** | 测试写了用户目录    | GUI 启动写了用户目录 |

**`keys/db.key.dpapi` 不是 GUI 写的。** 实测该文件 mtime 是 `2026-10-20 11:25:40`，而 `~/.patheditor/` 下**从未出现过 `patheditor.db`**（`ls *.db*` 为空）。GUI 从未启动过这个库，文件是测试（`backup_env_vars()` 走 `config_dir()`）留下的。

准确的表述应该是：

|          | `keys/db.key.dpapi`                                | `patheditor.db`  |
| -------- | -------------------------------------------------- | ---------------- |
| 谁写的   | **测试**（`backup_env_vars` → `config_dir()`）     | GUI/CLI 首次启动 |
| 该不该有 | 测试不该写，但文件本身合法                         | 该有             |
| 处置     | **保留**（格式与 DACL 都正确，GUI 首次启动会复用） | 无需处理         |

**结论不变**（保留），只是归因要写准——否则下一轮有人照这个表去查「GUI 为什么生成了密钥却没生成库」，会发现对不上。

### 补充：N-1 的更正还没进文档

上一轮我实测发现回执 R-05 对 `dirs::home_dir()` 的描述有偏差：

- 回执说「Windows 上读 `USERPROFILE` 不读 `HOME`」；
- 实测 `HOME` 与 `USERPROFILE` **都不读**（走 `SHGetKnownFolderPath(FOLDERID_Profile)`，读注册表）。

结论更重要：**`config_dir()` 无法被任何环境变量重定向**，所以「用环境变量隔离密钥目录」这条路对 `core` 根本不成立。这条经验目前只在我上一轮的审查报告里，**没有进任何回执或 CLAUDE.md**。建议下一轮把它落进 `path-editor` 的测试约定，否则每个新写的备份测试都会重复踩一次。

---

## 五、新增发现（均为非阻塞）

### N-3 `CHANGELOG.md:38` 过时表述

见 §2.1，`EventView` 现在的确含 `snapshotUid`，原文「`EventView` 不含 `snapshotUid`」已不成立。

### N-4 `README.md:245` 的轮换描述只写了 `.json`

```
轮换只删除本工具生成的 `env_backup_*.json`
```

实际 `rotate_env_backups` 同时收集 `.json` 与 `.pebak`（这正是 `.pebak` 上线后必须做的，否则新备份永不轮换）。**这句话没有写错（`.json` 确实会被删），但不完整**——读者会以为 `.pebak` 不参与轮换。

同一段的 `README.md:247`、`:313` 也有类似的「`env_backup_*.json` 含明文」表述，在 `.pebak` 成为默认格式后需要区分：`.json` 含明文，`.pebak` 不含。

**不是不实陈述**（`.json` 确实含明文、确实会被删），是**覆盖不全**。M1 合并前改不改都可以，但 README 是用户唯一的文档入口，建议一并梳理。

### N-5 `README.md:239-242` 的示例用旧格式路径

```
patheditor env restore ~/.patheditor/backups/env_backup_20260921_200437_062.json --dry-run
```

这个路径在示例语境下是合理的（演示「恢复一份遗留备份」），但读者照抄会发现自己的备份是 `.pebak`、路径对不上。建议把示例改成 `.pebak`，并把 `.json` 作为「遗留格式同样支持」的补充说明。

### N-6 工作区遗留：`SnapshotDetail.tsx` 的 U+FFFD 修复未提交

`git status` 显示一份未提交改动：

```diff
-/** 变量值区：敏感值在 Rust 侧已掩码��� `null`，这里只做「不可用」的呈现。 */
+/** 变量值区：敏感值在 Rust 侧已掩码为 `null`，这里只做「不可用」的呈现。 */
```

**这个修复是对的**：`git show HEAD:src/components/history/SnapshotDetail.tsx` 确认已提交版本第 8 行含 **3 个 U+FFFD**（「为」字被截成三个替换字符）。这是本轮唯一的源码级编码损坏。

我全仓扫描过一遍（`rs/ts/tsx/json/md/css`，排除 `node_modules`/`target`/`.git`），**源码目录只有这一处**；其余 8 个文件都是 `docs/` 与 `.superpowers/` 下的历史文档，不影响构建。

**建议**：这份修复该提交，但**它是开发窗口的领域**（源码提交），我不代为提交。请让开发窗口补一个 `fix(v6): 修复 SnapshotDetail 注释中的编码损坏` 提交。

### N-7 `CLAUDE.md` / `AGENTS.md` 在 HEAD 上**不是字节级一致**

这是**我上一轮漏报的**。`diff <(git show HEAD:CLAUDE.md) <(git show HEAD:AGENTS.md)`：

```
237c237
< | `package-lock.json`   | `version`（…同步）              |     ← 多一个空格
---
> | `package-lock.json`   | `version`（…同步）             |
```

`patheditor-two-window-flow` 的硬约束是「两份文档必须**字节级一致**」。当前状态：

- **HEAD（已提交）**：不一致，差 1 个空格；
- **工作区**：一致（`diff CLAUDE.md AGENTS.md` → exit 0）。

原因：工作区里那份 OpenSSL 章节的改动（我上一轮写的）同时写进了两份文件，顺带把那行表格也对齐了。

**所以只要那批未提交改动被提交，这条就自动修复**。但需要明确：**这三份未提交改动（`CLAUDE.md`、`AGENTS.md`、`SnapshotDetail.tsx`）目前无人认领**——`CLAUDE.md`/`AGENTS.md` 是我写的（OpenSSL 章节），`SnapshotDetail.tsx` 是开发窗口相关的编码修复。

我上一轮的审查报告没有查 HEAD 版的字节一致性（只查了工作区），这是我的漏洞。报告里写「`CLAUDE.md` 与 `AGENTS.md` 是否字节级一致」是必查项，我用了工作区版本回答 HEAD 的问题。

---

## 六、总结

### 本轮三条「合并前必须」的状态

| #   | 项                 | 状态                                                           |
| --- | ------------------ | -------------------------------------------------------------- |
| 1   | J-8 / README 不实  | ✅ 已改文档（正确选择），CHANGELOG 一处措辞待修（N-3，非阻塞） |
| 2   | N-2 / CLI 明文警告 | ✅ 已修复，实测有效，且修法优于我的建议                        |
| 3   | J-1 计划文件       | ✅ 四条全部落地，质量高于要求                                  |

### 合并前建议处理（都在 5 分钟量级）

1. **N-6**：让开发窗口补提交 `SnapshotDetail.tsx` 的编码修复；
2. **N-7**：把 `CLAUDE.md` / `AGENTS.md` 的未提交改动提交（顺带修好 HEAD 版本的字节差异）；
3. **N-3**：`CHANGELOG.md:38` 的 `EventView` 表述更新。

### 可延后到 M2

- V-1 判据改白名单（`;;;CO)` 与具名 SID 盲区）；
- `config_dir()` 支持 `PATHEDITOR_HOME` 重定向（N-1 的落地方案）；
- `snapshotUid` 真正回填；
- 历史面板 E2E（spec §3.2.1 要求 3–4 条主路径，仍是 0）；
- N-4 / N-5 的 README 覆盖度梳理。

### 本轮判定

**通过。可以合并。**

三条阻塞项在上一轮已全部落地并经我独立复现；本轮的三条「合并前必须」中，两条代码修复实测有效、一条文档修订完整落地。质量门的 6 项我全部独立跑过，305 passed 的口径由开发窗口用可复现命令固定下来——**这一轮的数字我第一次不需要打折扣**。

**这轮最值得记的是两处「开发窗口比我判断得更好」**：

1. **N-2 的修法**：我建议 `log::warn!` → `eprintln!`，开发窗口改成「库返回 `BackupFormat`，调用方决定呈现」。后者才是对的——core 是库，GUI 与 CLI 是对等调用方，库直接写 stderr 会把警告塞进 Tauri 的终端输出。我提的是一个能跑但不能扩展的方案。
2. **R-05 的自我更正**：把「看到东西 = 验证成功」纠正成「面板应该就是空的，有内容反而是异常」。这个更正让 M1 唯一缺失的端到端证据变成了可证伪的判据。

以及一处**我自己的错误**：连续两轮用 `awk '{p+=$4}'` 数测试，都数错了。`patheditor-review-window` 要求「任何『绿灯』结论必须附实际执行过的命令与输出」——我附了输出，但**用了错误的聚合方式去读正确的输出**。数字可复现的标准不是「我跑了命令」，而是「别人照同样的命令能得出同样的结论」。

另外，**N-7 是我上一轮的真实漏报**：必查清单里写着「`AGENTS.md` 与 `CLAUDE.md` 是否字节级一致」，我用工作区版本回答了 HEAD 的问题，给出了「IDENTICAL」的结论而不成立。

---

**审查窗口签章**：本报告所有「实测」结论均附审核窗口独立执行的命令与输出（§一、§2.1、§2.2、§三、§四、§五、N-6、N-7）。测试计数由开发窗口提供的可复现命令得出，我已按该命令复跑并核对。未引用回执中任何未经复现的数字。
