# Wave 2 架构收口 — 真实注册表闭环测试记录

**日期**: 2026-09-20
**分支**: `worktree-wave2`（worktree 隔离）
**被测版本**: HEAD `69e2159`（Wave 2 Task 1-7 完成后，Task 8 收口轮）
**测试对象**: Wave 2 改造后的 PATH 专用通路 + `env` 通用通路在真实注册表上的端到端行为
**测试用 CLI**: 仓库构建的 `target/release/patheditor.exe`（`cargo build --release -p patheditor-cli`，非 scoop 旧版）

## 1. 结论

**2 项真实注册表写入验证全部通过。** 测试结束后注册表回到操作前状态（system/user PATH 快照与 `env list` 全量导出与操作前内容级逐字节一致，条目数与顺序零变化），零残留、零污染。

格式沿用 `docs/审核和开发/2026.09.19/PathEditor-Wave1数据一致性闭环测试记录.md`。

## 2. 授权与前置准备

用户于 2026-09-20 显式授权本轮真实注册表写入测试（Task 8 Step 2 前置授权，开发窗口按授权直接执行），条件：记录备份、操作前后快照、回滚预案，全程零污染收尾。

### 2.1 备份与快照

| 准备项           | 结果                                                                                                                                                            |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 注册表备份       | `C:\Users\33644\.patheditor\backups\path_backup_20260920_152446_043.txt`（`patheditor backup` 退出码 0，文件名符合 `path_backup_YYYYMMDD_HHMMSS_mmm.txt` 契约） |
| PATH 操作前快照  | system 43 项（全启用）+ user 11 项（全启用），`list --system/--user --json` 全量导出                                                                            |
| env 操作前快照   | system 23 变量 + user 29 变量，`env list --json` 全量导出                                                                                                       |
| 临时名冲突检查   | user PATH 无 `C:\nonexistent-wave2-verify-0x9e`；两个 hive 均无 `PATHEDITOR_WAVE2_*` 变量                                                                       |
| pending 文件预检 | `~/.patheditor/pending_path_snapshot.json` 不存在                                                                                                               |

**测试范围限定**：只用临时 PATH 条目 `C:\nonexistent-wave2-verify-0x9e` 与自建临时变量 `PATHEDITOR_WAVE2_TEST`，不触碰任何既有变量与既有 PATH 条目；`Path` 仅经 PATH 专用命令操作，其他变量仅经 `env` 通用命令操作。

## 3. 验证项与证据

### 3.1 验证 1 —— user PATH add/remove 回环（含 remove 默认 user hive 语义）

```text
$ patheditor add "C:\nonexistent-wave2-verify-0x9e" --user
已添加到用户 PATH: C:\nonexistent-wave2-verify-0x9e       退出码 0

$ patheditor list --user --json（中期快照）
user 条目 11 → 12，末位为测试条目（enabled=true）✅ 写入确实生效

$ patheditor remove 11        （remove 默认操作用户 PATH，未传 --system）
已删除: C:\nonexistent-wave2-verify-0x9e                   退出码 0

$ patheditor list --user --json
user 条目 12 → 11 ✅；残留检查：测试条目出现次数 0 ✅
```

**结论**：Wave 2 服务层改造后 PATH 通路的 add/remove 在真实注册表上行为正确；`remove` 不带 `--system` 时默认操作 user hive，与 CLAUDE.md 契约一致。

### 3.2 验证 2 —— env 变量 add/get/remove 回环（user hive）

```text
$ patheditor env add PATHEDITOR_WAVE2_TEST "wave2-roundtrip"
已新建用户变量: PATHEDITOR_WAVE2_TEST                      退出码 0

$ patheditor env get PATHEDITOR_WAVE2_TEST
wave2-roundtrip                                            退出码 0（stdout 裸值，管道友好）

$ patheditor env list --json
user 中出现 PATHEDITOR_WAVE2_TEST 恰好 1 次 ✅

$ patheditor env remove PATHEDITOR_WAVE2_TEST --force
已删除用户变量: PATHEDITOR_WAVE2_TEST                      退出码 0

$ patheditor env list --json
残留出现次数 0 ✅

$ patheditor env get PATHEDITOR_WAVE2_TEST
退出码 1（变量已不存在，get 报错）✅
```

**结论**：`env add` → `env get` → `env remove --force` 全链路在真实注册表上按契约工作；`--force` 删除不产生退出码 3（最后写入者胜语义）；`env get` 在变量不存在时退出码 1。

## 4. 快照对比 — 零污染证明

测试结束后重新全量导出三份快照，与操作前快照做内容级比对（操作前快照由 PowerShell `>` 重定向写入，带 CRLF 行尾；后态快照重定向伪影产生 LF 行尾差异，属 shell 伪影非内容差异——比对前统一行尾后逐字节比对，条目集合、顺序、enabled 状态、变量元数据均无差异）：

```text
system PATH: 操作前 43 → 操作后 43 ✅ 内容级逐字节一致（行尾规范化后）
user   PATH: 操作前 11 → 操作后 11 ✅ 内容级逐字节一致（行尾规范化后）
env 全量导出: system 23 + user 29 ✅ 内容级逐字节一致（行尾规范化后）

注册表残留检查：
$ env list 中 PATHEDITOR_WAVE2_TEST 出现次数 0 ✅
$ user PATH 中 nonexistent-wave2-verify-0x9e 出现次数 0 ✅

pending 文件：~/.patheditor/pending_path_snapshot.json 不存在 ✅
```

**结论**：注册表完全回到操作前状态，无非预期新增、消失或修改；全程未产生 pending 文件。

## 5. 未覆盖项

- **system hive（HKLM）的写入验证**：本轮会话未持有管理员提升上下文（`IsInRole(Administrator)` 返回 False），如实记录 "system hive skipped (no elevation)"。system PATH 仅做只读快照与逐字节比对（零差异），未做写入验证。
- sidecar 写失败的注入式端到端验证（需磁盘故障注入，由 Task 5 单测 + 正常路径不误留 pending 的行为覆盖）
- `--stdin` / `--value-file` 值通道端到端读数（本轮 `env add` 用位置参数传值，测试值为非敏感临时串）
- GUI 侧经 Tauri IPC 的真实注册表写入（CLI 与 GUI 共用 core 通路，本轮以 CLI 验证 core 行为）

## 6. 回滚资源与预案

| 资源       | 位置                                                                                                                                           |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| 注册表备份 | `C:\Users\33644\.patheditor\backups\path_backup_20260920_152446_043.txt`                                                                       |
| 操作前快照 | `%TEMP%\wave2-closed-loop\`（path-\*-before.json / env-all-before.json）⚠️ 临时工件，会话收尾清理；持久证据是第 4 节零差异比对结果与注册表备份 |

**回滚手段声明**：若需回滚，可用注册表备份文件 `path_backup_20260920_152446_043.txt` 恢复两个 hive 的 PATH 值（该文件含 `[System PATH]` / `[User PATH]` 两个 section 的完整逐行内容；可通过 `patheditor import` 导入，或手工用 `reg restore` / PowerShell `Set-ItemProperty` 按 section 重建 `HKLM\...\Session Manager\Environment\Path` 与 `HKCU\Environment\Path`）；环境变量可按操作前 `env list` 快照用 `patheditor env add` 重建。本轮终态快照 diff 为零，回滚预期不会被触发。

## 7. 清理确认

- 临时 PATH 条目 `C:\nonexistent-wave2-verify-0x9e`：已 remove，残留 0 ✅
- 临时变量 `PATHEDITOR_WAVE2_TEST`：已 `env remove --force`，残留 0 ✅
- 注册表备份文件：**保留不删**（回滚资源，位于 `~/.patheditor/backups/`，不进仓库）
- 临时快照目录 `%TEMP%\wave2-closed-loop\`：会话收尾清理（未入库）
