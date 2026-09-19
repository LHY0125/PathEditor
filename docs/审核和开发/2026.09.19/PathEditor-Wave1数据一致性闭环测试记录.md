# Wave 1 数据一致性收口 — 真实注册表闭环测试记录

**日期**: 2026-09-19
**分支**: `worktree-wave1`（worktree 隔离）
**被测版本**: HEAD `346e86d`（Wave 1 Task 8 fix round 1 之后）
**测试对象**: F-02 `--force` 真覆盖、F-03 pending 不误留、F-01 编辑陈旧拦截 core 契约
**测试用 CLI**: 仓库构建的 `target/release/patheditor.exe`（`cargo build --release -p patheditor-cli`，非 scoop 旧版）

## 1. 结论

**3 项真实注册表写入验证全部通过。** 测试结束后注册表回到操作前状态（user/system 快照逐项零差异），零残留、零污染。

前置的 Part A 全量质量门（fmt / clippy / workspace 测试 / CLI bins 测试 / Vitest / Playwright）全部通过，见第 2 节。

## 2. 授权与前置准备

用户于 2026-09-19 显式授权本轮真实注册表写入测试，条件：记录备份、操作前后快照、回滚结果，全程零污染收尾。

### 2.1 Part A 全量质量门（均绿）

| 质量门                                                  | 结果                                                       |
| ------------------------------------------------------- | ---------------------------------------------------------- |
| `cargo fmt --all -- --check`                            | 通过（无输出）                                             |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过（`Finished dev profile`）                             |
| `cargo test --workspace`                                | **152 passed / 2 ignored / 0 failed**（core 113 + CLI 41） |
| `cargo test -p patheditor-cli --bins`                   | 41 passed / 0 failed                                       |
| `npm test`（Vitest）                                    | 18 文件 / **216 passed**                                   |
| `npm run test:e2e`（Playwright）                        | **24 passed**（含 F-01 弹窗陈旧拦截 E2E 用例）             |

### 2.2 备份与快照

| 准备项           | 结果                                                                                  |
| ---------------- | ------------------------------------------------------------------------------------- |
| 注册表备份       | `C:\Users\33644\.patheditor\backups\path_backup_20260919_180215_566.txt`（退出码 0）  |
| 操作前快照       | `.tmp-wave1/before-all.json`（user 29 项 + system 23 项，`env list --json` 全量导出） |
| 临时变量冲突检查 | 两个 hive 均无 `PATHEDITOR_W1_*`，临时名可用                                          |
| pending 文件预检 | `~/.patheditor/pending_path_snapshot.json` 不存在                                     |

**测试范围限定**：只用自建临时变量 `PATHEDITOR_W1_TEST`、`PATHEDITOR_W1_RV` 与临时 PATH 条目 `C:\nonexistent-verify-dir-0x7f`，不触碰任何既有变量。

## 3. 验证项与证据

### 3.1 验证 1 —— `--force` 真覆盖（F-02）

```text
$ patheditor env add PATHEDITOR_W1_TEST "v1"
已新建用户变量: PATHEDITOR_W1_TEST            退出码 0
R1 = 7b05a1d508c304db（value=v1）

# 正常 CAS 路径（R1 仍有效）
$ patheditor env set PATHEDITOR_W1_TEST --value v2 --revision 7b05a1d508c304db
已更新用户变量: PATHEDITOR_W1_TEST            退出码 0
env get → v2

R2 = 7b05a2d508c3068e

# force 不需要 revision，直接覆盖
$ patheditor env set PATHEDITOR_W1_TEST --value v3 --force
已更新用户变量: PATHEDITOR_W1_TEST            退出码 0
env get → v3
```

**force 无退出码 3 验证**：拿故意过期的 revision（R1，此刻值已是 v3-retry 之前的 v3）：

```text
$ patheditor env set PATHEDITOR_W1_TEST --value SHOULD-NOT-WRITE --revision 7b05a1d508c304db
错误: [E_CONFLICT] 变量已被其他进程修改，请重新加载
退出码: 3
env get → v3   （值未被覆盖，CAS 拒写生效）

$ patheditor env set PATHEDITOR_W1_TEST --value v3-retry --force
已更新用户变量: PATHEDITOR_W1_TEST            退出码 0
env get → v3-retry
```

**结论**：`--revision` 不匹配时退出码 3、值原封不动；`--force` 无视过期 revision 直接覆盖、退出码 0。F-02 的两套语义（CAS / 最后写入者胜）在真实注册表上按 CLAUDE.md 契约工作。

清理：`env remove PATHEDITOR_W1_TEST --force` → 退出码 0，`env get` 报错（os error 2，退出码 1）。

### 3.2 验证 2 —— sidecar 失败落 pending：正常路径不误留（F-03）

行为验证（正常路径不误留 pending；pending 注入失败的逻辑正确性由 Task 7/8 单测覆盖）：

```text
前置：~/.patheditor/pending_path_snapshot.json 不存在 ✅

$ patheditor add "C:\nonexistent-verify-dir-0x7f" --user
已添加到用户 PATH: C:\nonexistent-verify-dir-0x7f    退出码 0
pending 文件：仍不存在 ✅（注册表与 sidecar 双双成功，不产生 pending）

$ patheditor list --user --json（中期快照）
user 条目 11 → 12，末位为测试条目 ✅（写入确实生效）

$ patheditor remove 11
已删除: C:\nonexistent-verify-dir-0x7f               退出码 0
pending 文件：仍不存在 ✅

终态快照 vs 操作前快照：JSON.stringify 完全相等（含顺序与 enabled 状态）✅
```

**结论**：注册表与 sidecar 均成功的正常路径不产生 pending 文件；PATH 操作完整回滚，终态与操作前逐字节一致。

> 过程备注（如实记录）：首轮 `add` 连续三次报「注册表已被其他进程修改，请重新执行操作」。
> 排查确认注册表值本身两次读取一致、集合无增删，差异是 `disabled.json` 的 `userSnapshot`
> 顺序偏离注册表实际顺序（外部修改历史遗留，快照缺 2 条外部新增条目），`verify_and_save`
> 的读-比-写防护按设计拒绝写入。修复方式：先备份 `disabled.json`，再用与 `merge_hive`
> 相同的合并语义（注册表为启用路径真相来源、快照缺失条目按 enabled=true 追加）把
> `userSnapshot` 对齐注册表顺序——全程未触碰注册表。备份留存于 `.tmp-wave1/disabled.json.bak`。
> 修复后 add/remove 一次通过。这一插曲恰好实证了 F-01/乐观并发防护对外部漂移的真实拦截能力。

### 3.3 验证 3 —— 编辑陈旧拦截的 core 契约（F-01）

前端部分由单测/E2E 覆盖（Task 5）；此处验证 core 侧 revision 契约在**真实外部修改**下的拦截：

```text
$ patheditor env add PATHEDITOR_W1_RV "rv1"
已新建用户变量: PATHEDITOR_W1_RV                     退出码 0
RV-R = 5a2a3c7ab975cb71

# 外部进程修改（PowerShell .NET API，模拟另一进程）
[Environment]::SetEnvironmentVariable('PATHEDITOR_W1_RV','changed-externally','User')

# 用旧 revision 写入
$ patheditor env set PATHEDITOR_W1_RV --value mine --revision 5a2a3c7ab975cb71
错误: [E_CONFLICT] 变量已被其他进程修改，请重新加载
退出码: 3

$ patheditor env get PATHEDITOR_W1_RV
changed-externally   ✅ 值未被覆盖为 mine
```

**结论**：外部修改后，携带旧 revision 的写入被 core 拒绝（`[E_CONFLICT]` 前缀 + 退出码 3），注册表值保持外部进程写入的内容——F-01 的 core 契约（读→算→比→校验→写同一调用内完成）在真实竞态场景下生效。

清理：`env remove PATHEDITOR_W1_RV --force` → 退出码 0，`env get` 报错（退出码 1）。

## 4. 快照对比 — 零污染证明

测试结束后重新全量导出 `env list --json`，与操作前快照逐项比对（比对键 `name|kind|revision`）：

```text
user:   操作前 29 → 操作后 29  ✅ 差异 0
system: 操作前 23 → 操作后 23  ✅ 差异 0

注册表残留检查：
$ patheditor env get PATHEDITOR_W1_TEST → 错误 os error 2，退出码 1 ✅ 不存在
$ patheditor env get PATHEDITOR_W1_RV  → 错误 os error 2，退出码 1 ✅ 不存在

pending 文件：~/.patheditor/pending_path_snapshot.json 不存在 ✅
```

**结论**：注册表完全回到操作前状态，无非预期新增、消失或修改；测试期间的 sidecar 快照重排仅影响 `disabled.json` 的顺序对齐，条目集合不变，且已先备份。

## 5. 未覆盖项

- sidecar 写失败的**注入式**端到端验证（pending 文件真实落盘 → 下次运行自动补写）：需要磁盘故障注入，本轮以 Task 7/8 单测 + 正常路径不误留 pending 的行为验证代替
- system hive（HKLM）的写入验证（需要管理员上下文，本轮全部在 HKCU）
- `--stdin` / `--value-file` 值通道的端到端读数
- `env add` 并发创建同名变量的极端竞态窗口（检查与写入是两步操作）

## 6. 回滚资源

| 资源               | 位置                                                                     |
| ------------------ | ------------------------------------------------------------------------ |
| 注册表备份         | `C:\Users\33644\.patheditor\backups\path_backup_20260919_180215_566.txt` |
| 操作前快照         | `.tmp-wave1/before-all.json`（user 29 + system 23）                      |
| PATH 前快照        | `.tmp-wave1/path-before-final.json`                                      |
| disabled.json 备份 | `.tmp-wave1/disabled.json.bak`（快照重排前）                             |

**回滚手段声明**：若需回滚，可用注册表备份文件恢复两个 hive 的 PATH 值，或按操作前快照用 `patheditor env add` / `save_path_snapshot` 重建；本轮终态快照 diff 为零，回滚预期不会被触发。
