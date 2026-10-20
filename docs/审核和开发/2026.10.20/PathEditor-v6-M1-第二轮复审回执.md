# PathEditor v6.0.0 M1 第二轮复审回执

## 1. 回执结论

- **回执日期**：2026-10-20
- **复审对象**：`docs/审核和开发/2026.10.20/PathEditor-v6-M1-复审审查报告.md`（286 行）
- **分支**：`v6/m1-storage-history`（基于 `main @ 9e674ec`）
- **版本状态**：未升级（仍为 `5.1.7`）
- **Git 状态**：**未推送、未合并**
- **注册表状态**：**零写入**

**判定**：3 条「合并前必须处理」全部处理完毕；复审报告指出的 2 处归因/描述错误已按实测更正；数字口径已修正并给出可复现命令。**建议放行合并。**

---

## 2. 三条「合并前必须处理」

### 2.1 `snapshotUid` 恒 null → **改文档**（未接线）

**选择改文档而非接线**，理由：

- 接线需要在 `commands::service` 里改成「先 `insert_snapshot` 拿 uid → 再调服务 → 事件带 uid」，或把快照插入与服务写入放进同一事务。这涉及**服务层与 DB 的耦合方式**，属于 M2「历史面板真正可用」的范围，不适合在整改轮临时加。
- 更关键的是：**接线的前提条件当前不具备**——`save_path_with_sidecar` 不知道自己产生了几条快照（可能 system/user 各一条，也可能都没写）。要做对需要先设计「一次保存对应几个快照」，这是功能设计而非缺陷修复。

**已改的文档**（`README.md:283-287`）：

```
- 「历史记录」标签页按时间倒序浏览事件

> **M1 的已知限制**：事件列表可以浏览，但**点击事件暂时看不到快照详情** ——
> 事件与快照的关联（`snapshotUid`）要到 M2 才接通，当前按钮为禁用状态。
> 不要在 M2 之前把它当成「可预览完整快照」。
```

删掉了原句「选中后预览当时的完整快照」。

**代码侧已保证不会误导**：`HistoryPanel.tsx` 对 `snapshotUid === null` 的事件**禁用按钮**（`cursor: default` + `disabled`），点击不发 IPC。这比「可点但空白」诚实。

---

### 2.2 N-2：CLI 明文警告是空操作 → **已修**

**先独立复现**（复审报告的主张）：

```
$ grep -rn "set_logger|init_logger|env_logger|simple_logger|LevelFilter" --include=*.rs cli/
（零命中）
```

**核心发现**：`core/src/backup.rs:704` 的注释**早就写着**「CLI 未初始化 logger，core 的 `log::warn!` 在 CLI 下会被丢弃」——项目知道自己有这个问题，但只在 `BackupOutcome` 那一处绕过了它，读取路径没有。

**修法（不让库直接 `eprintln!`）**：库不该污染 GUI 的 stdout/stderr。正确形状是把「发生了什么」作为返回值交给调用方：

| 层               | 改动                                                                                                                             |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| core             | 新增 `pub enum BackupFormat { Encrypted, LegacyPlaintext }`；新增 `read_env_backup_with_format(path) -> (Payload, BackupFormat)` |
| CLI dry-run      | 按 `format` 判断，遗留明文时 `eprintln!("警告: ... 是遗留明文备份 ...")`                                                         |
| CLI 非 dry-run   | **写入之前**按扩展名提示（恢复完再说就晚了）                                                                                     |
| CLI `env backup` | 产物是 `.json`（回落路径）时 `eprintln!("警告: 本次写的是**明文** .json 备份 ...")`                                              |

**实测证据**（真实 CLI 二进制，stderr 单独捕获）：

```
$ PATHEDITOR_BACKUP_DIR=<tmp> ./target/debug/patheditor.exe env restore \
    <tmp>/env_backup_20260101_000000_000.json --dry-run 2>err.txt >/dev/null
$ cat err.txt
警告: C:/Users/.../env_backup_20260101_000000_000.json 是遗留明文备份
（v6 起的备份为 DPAPI 加密的 .pebak）。恢复后建议重新执行 `patheditor env backup` 生成加密副本。
```

**回归守卫**：新增 `cli/src/env_ops.rs::env_restore_warns_about_legacy_plaintext_on_stderr`，断言 stderr 含「遗留明文备份」与「pebak」。若将来有人把 `eprintln!` 改回 `log::warn!`，本用例失败。

**一处操作事故需报告**：我第一次测这条时把 `./target/debug/patheditor.exe`（CLI）误写成了 `PathEditor.exe`（GUI）——Git Bash 大小写不敏感，匹配到了 GUI 二进制，导致它**真的启动了**并执行了首次启动的 `setup_database`。后果见 §5 R-06。

---

### 2.3 J-1：计划文件 → **已单独提交**

**授权内容四项，全部落实**（提交 `6946181`，只动该文件）：

| #   | 内容                                                                 | 位置                                           |
| --- | -------------------------------------------------------------------- | ---------------------------------------------- |
| 1   | Task 12 交付物名 → `开发回执`                                        | 计划 `:1908`、`:1931`、`:1938`                 |
| 2   | Task 9 Step 4 启动片段补 `setup_database` 三步                       | 计划 `:1637-1670`                              |
| 3   | Task 3 DACL 骨架 `SetNamedSecurityInfoW` → `SetFileSecurityW` + 理由 | 计划 `:658-678`                                |
| 4   | 裁断表补三行（V-1 判据盲区、V-3 文件名时刻、J-7 app_state）          | 计划 `:1993-2011`（新增「M1 复审整改裁断」节） |

**关于归属**：复审报告更正了 +93 行的来源（是审核窗口上一轮写的，不是 `/init` 残留）。我此前在回执里写的「不是我造成的、来源不明」是**未经核实的推测**，应当先问再写。本轮已在提交前确认：`git status` 显示该文件干净、改动全部来自授权内容。

---

## 3. 两处归因/描述错误的更正

### 3.1 `dirs::home_dir()` 的行为（我的 R-05 根因描述是错的）

**我上轮写的**：「`dirs::home_dir()` 在 Windows 上读 `USERPROFILE`，不读 `HOME`」。

**复审报告实测**：**两个都不读**。`dirs::home_dir()` 走 `SHGetKnownFolderPath(FOLDERID_Profile)`，读注册表。

**实测复核**（按报告的实验重跑）：

```
HOME=<tmp> USERPROFILE=<tmp> PATHEDITOR_BACKUP_DIR=<tmp>/backups
→ 备份写到 <tmp>/backups   ✅（backup_base_dir 自己读的环境变量）
→ <tmp>/keys 不存在        ❌（dirs 完全忽略环境变量）
```

**这个更正很重要**，因为它改变了结论的性质：

- 我原来的说法暗示「设对变量就能隔离」——**不成立**；
- 实际情况是「`config_dir()` 无法被任何环境变量重定向」，所以**密钥目录的隔离测试这条路根本走不通**。

这正是我用 `write_env_backup_to_with_key(dir, payload, &material)` 显式传密钥的原因——不是「更干净的写法」，而是**唯一可行的写法**。

### 3.2 V-3 命名缺陷的归因（「审查报告没发现」不准确）

**我上轮写的**：「这是审查报告没发现的缺陷」。

**复审报告更正**：首轮 V-3 是「`.pebak` 从不被写入」，所以这个命名缺陷**没有暴露的机会**。两者是同一条链的前后环——先修好「真的写 `.pebak`」，才可能撞上「写完被轮换删掉」。

**同意，已更正。** 这提醒我一件事：发现缺陷时容易把它当成「别人漏了」，实际上更常见的是「前一个缺陷掩盖了这个缺陷」。

---

## 4. 数字口径（连续两轮差 1）

### 4.1 复现命令（任何人都能跑出同一数字）

```bash
cargo test --workspace 2>&1 \
  | grep -oE "^test result: ok\. [0-9]+ passed; [0-9]+ failed; [0-9]+ ignored" \
  | sed -E 's/test result: ok\. ([0-9]+) passed; ([0-9]+) failed; ([0-9]+) ignored/\1 \2 \3/' \
  | awk '{p+=$1; f+=$2; i+=$3; n++} END {printf "binaries=%d passed=%d failed=%d ignored=%d\n", n, p, f, i}'
```

### 4.2 本轮结果

```
$ （上述命令）
binaries=15 passed=305 failed=0 ignored=2
```

**305 passed + 2 ignored = 307 个声明的测试**，15 个测试二进制。

### 4.3 对前两轮差异的说明

| 轮次                   | 我报的 | 实际                       | 我错在哪                                        |
| ---------------------- | ------ | -------------------------- | ----------------------------------------------- |
| 首轮开发回执           | 301    | 302                        | 用 `awk` 取字段时把 `ignored` 栏读成了 `passed` |
| 本轮复审回执（第一版） | 305    | 304                        | **把 `2 ignored` 加进了 passed 总数**           |
| 本轮（第二版）         | —      | **305 passed / 2 ignored** | 新增 1 个 N-2 回归测试，故比 304 多 1           |

**根因**：`test result: ok. N passed; M failed; K ignored` 这一行有三个数字，我两次都读错位置。已改为上面的显式拆分命令，**不再手算**。

---

## 5. 验证证据

### 5.1 已执行（本机实测）

```
$ cargo fmt --all -- --check      → exit 0
$ cargo clippy --workspace --all-targets -- -D warnings → exit 0
$ cargo test --workspace          → 15 binaries, 305 passed, 0 failed, 2 ignored
$ npm run verify                  → VERIFY=0
$ npm run test:e2e                → 41 passed (1.3m), E2E=0
```

### 5.2 新增/修改的测试

| 文件                 | 变更                                                                         |
| -------------------- | ---------------------------------------------------------------------------- |
| `cli/src/env_ops.rs` | +1 测试 `env_restore_warns_about_legacy_plaintext_on_stderr`（N-2 回归守卫） |
| `core/src/backup.rs` | +`BackupFormat` 枚举 + `read_env_backup_with_format`                         |

### 5.3 未执行且不能声称完成

| 项                     | 说明                                                                                                                                     |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| **历史面板 E2E**       | 仍为零覆盖（`grep -rln history e2e/` 零命中）。M1 的历史面板**只有单元测试**。                                                           |
| **DACL 白名单判据**    | 复审报告指出的黑名单盲区（`NO_ACCESS_CONTROL` 误判、漏 `;;;CO)`、具名 SID 不命中）**未修**——报告判定不阻塞，已记入计划裁断表作为 M2 项。 |
| **`snapshotUid` 接线** | 未做（选择改文档）。                                                                                                                     |
| **CI（MSVC）**         | 未推分支，CI 未跑。本机 GNU。                                                                                                            |

---

## 6. 残余风险

### R-06【本轮新增】GUI 被意外启动并执行了首次启动序列

**经过**：我在验证 N-2 时误把 `./target/debug/patheditor.exe`（CLI，小写）当成 CLI 运行——Git Bash 在 Windows 上大小写不敏感，实际执行的是 `PathEditor.exe`（GUI）。GUI 无头启动并跑完了 `setup_database`。

**实际后果**：

```
$ ls -la ~/.patheditor/
drwxr-xr-x          keys                  （11:25，上一轮的测试副产物）
-rw-r--r--  4.1k    patheditor.db         （12:13，本轮新建）
-rw-r--r--   33k    patheditor.db-shm     （12:13）
-rw-r--r--  2.6M    patheditor.db-wal     （12:13）
```

启动日志：`v5 文件迁移完成：导入 31，跳过 0，失败 0`。

**原件未被改动**（已核实）：

- `disabled.json` mtime 仍是 17 Oct；
- `profiles/2026-09-27.json` mtime 仍是 27 Sep；
- 迁移是**只读 + 登记导入记录**，符合 spec §7.4「原件保留、不自动删除」。

**这不是数据损失，但它是一个真实的「首次启动」事件**：用户的 `~/.patheditor/patheditor.db` 现在包含从 31 个 v5 文件导入的内容。GUI 下次启动会直接复用它（`migration_imports` 保证幂等，不会重复导入）。

**处置**：三个 `.db*` 文件**未删除**（全局规则）。**请复审确认是保留还是授权删除后重新初始化**——若保留，建议用户手工打开一次 GUI 确认历史面板能看到这 31 条导入记录，这反而成了 M1 的第一次真实端到端验证。

**教训**：Windows 文件系统大小写不敏感，`patheditor.exe` 与 `PathEditor.exe` 会互相匹配。**运行 CLI 时必须用完整路径加扩展名确认**，或直接用 `cargo run -p patheditor-cli --`。

### R-07【沿用】其余风险

| #    | 风险                                | 状态                                              |
| ---- | ----------------------------------- | ------------------------------------------------- |
| R-01 | `snapshotUid` 恒 null               | 已改为文档披露 + UI 禁用，**功能仍缺**            |
| R-02 | `profileMarkApplied` 前端方法冗余   | 保留（M2 可能用）                                 |
| R-03 | `.pebak` 的 `payload_hash` 不是 MAC | 未变                                              |
| R-04 | DACL 不防已持有句柄的进程           | 无法在应用层解决                                  |
| R-05 | 测试污染（密钥文件）                | 测试已隔离；已创建的文件保留（见 §R-06 同类处置） |

---

## 7. 需要复审确认的三件事

1. **`~/.patheditor/patheditor.db*`（本轮 GUI 意外启动产生）** —— 保留还是授权删除？
2. **DACL 白名单判据** —— 已按报告判定记入 M2（计划裁断表），确认这个处置即可。
3. **`snapshotUid` 选择改文档** —— 确认「M2 接线 + 现在披露」这个取舍可接受。

---

## 8. 提交清单

| 提交              | 内容                                                                      |
| ----------------- | ------------------------------------------------------------------------- |
| `b876c03`         | `fix(v6): N-2 CLI 明文警告走 stderr，README 去掉两处不实声明`             |
| `6946181`         | `docs(v6): M1 计划修订（J-1 交付物名、启动序列三步、DACL API、三行裁断）` |
| （前序）`42f981f` | `docs(v6): M1 复审回执`                                                   |
| （前序）`f7581dc` | `fix(v6): 整改审查报告 V-1/V-2/V-3 与 J-2/J-3/J-4/J-5/J-7/J-8/J-9`        |

---

## 9. 分支与合并状态

- 分支：`v6/m1-storage-history`
- 领先 `origin/main`：40 个提交
- **未推送、未合并、未升级版本号**

---

**签章**：以上「已修复」的结论均来自本机实际执行的命令输出（§5.1）；§5.3 四项未执行，未在任何地方声称完成；§6 的 R-06 是我本轮引入的操作事故，已写明证据、影响范围与原件未损的核实过程；§3 两处归因更正均按复审报告的实测结果改写，并说明了原描述错在哪。
