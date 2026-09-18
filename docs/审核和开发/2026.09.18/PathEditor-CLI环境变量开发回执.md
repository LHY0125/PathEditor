# CLI 环境变量管理 — 开发回执

**提交窗口**: 开发窗口（Claude Code）
**日期**: 2026-09-18
**交付对象**: 审核窗口
**分支**: `worktree-all-env-vars` → 已合并回 `main`
**版本**: 5.1.3
**状态**: 开发完成，质量门全绿，真实注册表闭环验证通过，**未推送远端**

---

## 1. 交付内容概述

为 `patheditor` CLI 新增 `env` 子命令组，把既有的全环境变量管理能力（GUI 侧已交付）延伸到脚本与自动化场景。

**核心架构原则**：CLI 侧**零新增核心代码**。core 的 5 个公开 API 在 GUI 特性中已就绪并经测试覆盖，CLI 只做参数转换、命令分派与错误呈现。保留名、保护名单、`Unsupported` 类型、hive 写权限、revision 校验**全部由 core 判定**，CLI 仅透传错误文本。

### 命令面

| 命令                                                                                | 作用                                   |
| ----------------------------------------------------------------------------------- | -------------------------------------- |
| `env list [--system\|--user] [--json]`                                              | 列出变量元数据（不含明文）             |
| `env get <NAME> [--system]`                                                         | 读取单个变量明文（CLI 侧唯一明文出口） |
| `env set <NAME> [--value <V>\|--stdin\|--value-file <F>] (--revision <R>\|--force)` | 修改已有变量值（类型不变）             |
| `env add <NAME> [<VALUE>] [--kind string\|expand] [--system]`                       | 新建变量                               |
| `env remove <NAME> (--revision <R>\|--force)`                                       | 删除变量                               |

### 关键设计决策（均经用户拍板）

1. **值输入三通道**：位置参数 / `--stdin` / `--value-file`，互斥。后两者让敏感值可绕开 shell 历史与进程列表。
2. **并发双模式强制显式选择**：`--revision`（CAS）或 `--force`（跳过校验），两者都不给报错、都给也报错。CLI 是一次性进程，静默降级为「现读现写」会让用户在毫秒级竞态窗口下最后写入者胜，与仓库 `verify_and_save` 的安全文化相悖。
3. **退出码 3** 表示 revision 冲突（仅 env 命令），使脚本可凭退出码区分「重试后可恢复」与致命错误，不必 grep 中文文案。PATH 命令保持退出码 1 不动（向后兼容）。
4. **`env add --kind` 默认 `string`（选填）**：spec 原写「必填」，实现为 `default_value = "string"`。用户裁决保留选填——符合 CLI 惯例，`string` 覆盖绝大多数变量类型。已完成 spec 与计划文档的相应回改，文档与实现一致。
5. **`--force` 的实现方式**：core 的 `update_env_var` / `delete_env_var` 签名恒要求 `expected_revision`。CLI 的「跳过校验」语义由「立即重新读取当前 revision 再传入」表达，而非给 core 新增 force 参数——core 的并发校验契约保持唯一。

---

## 2. 代码改动清单

合并提交 `edc0635`（merge --no-ff），改动 **10 个文件，+2778 / −7**。

### Rust 核心改动（916 行）

| 文件                   | 改动             | 说明                                                                     |
| ---------------------- | ---------------- | ------------------------------------------------------------------------ |
| `cli/src/env_ops.rs`   | **+764**（新建） | env 子命令实现 + 纯函数格式化器（值通道、并发选项、表格渲染、JSON 组装） |
| `cli/src/main.rs`      | +97              | Clap `EnvCmd` 枚举 + `Command::Env` 变体 + match 分派                    |
| `cli/src/runtime.rs`   | +50              | `exit_conflict`（退出码 3）、`is_conflict`、`CONFLICT_PREFIX`            |
| `core/src/registry.rs` | +5               | `conflict_message()` —— 冲突文案唯一读取源，供 CLI/GUI 判定前缀          |

### 文档改动

| 文件                                                               | 说明                                                          |
| ------------------------------------------------------------------ | ------------------------------------------------------------- |
| `README.md`                                                        | CLI 命令表追加 env 组、说明段、退出码约定、命令数与测试数修正 |
| `AGENTS.md` / `CLAUDE.md`                                          | CLI 命令节 + 错误处理节（退出码）；两份保持一致               |
| `docs/superpowers/specs/2026-09-17-cli-env-vars-design.md`         | 设计文档（新建入库）                                          |
| `docs/superpowers/plans/2026-09-17-cli-env-vars-implementation.md` | 实施计划（新建入库）                                          |
| `docs/审核和开发/2026.09.18/PathEditor-CLI环境变量闭环测试记录.md` | 真实注册表闭环测试记录（新建归档）                            |

### 额外提交（合并后）

`f85ee04` —— 更新 `CLAUDE.md` / `AGENTS.md`，补 `env_ops.rs` 到结构树、标注 `patheditor-cli` 为 bin-only crate、补充环境变量并发契约与退出码 2 说明、补 `pub fn` 文档注释要求。

---

## 3. 提交清单

分支 `worktree-all-env-vars` 相对 `main` 共 **15 个提交**，全部随 `edc0635` 合并：

```
a68cede  feat(cli): 新增冲突退出码与 [E_CONFLICT] 前缀判定
a8db28b  feat(cli): 值输入三通道与末尾换行剥离
d92f5aa  feat(cli): 并发选项互斥校验与冲突退出码映射
a1c416e  feat(cli): 环境变量表格渲染与只读/敏感标记
5dbd325  feat(cli): env list 的 JSON 输出按 hive 过滤
4767adc  feat(cli): env list 与 env get 命令实现
0e76d37  feat(cli): env set / add / remove 命令实现
e327838  feat(cli): 接入 env 子命令组到命令分派
7b410b6  refactor(cli): 移除 env 写命令中冗余的环境变更广播
bbd1585  docs: 同步 CLI env 子命令与环境变量管理说明
382c891  docs: 入库 CLI env 计划与设计文档并修正事实错误
8a66241  docs: 修正 README 的 CLI 命令数与 Rust 测试数
7ff2f69  docs: README 命令数改用可复现口径（17 条顶层命令）
dfa3dcf  docs: 归档 CLI 环境变量真实注册表闭环测试记录
7e4365f  fix(test): 同步 main 的 vitest exclude 规则（排除 .claude/ 嵌套目录）
```

其中 `7e4365f` 属分支维护（同步 main 已有的 vitest exclude 修复），非特性交付内容。

`main` 上另有合并后的文档提交 `f85ee04`。

---

## 4. 测试与质量门

### 新增测试

| 位置                 | 数量            | 覆盖内容                                                                                                                                                                                                                              |
| -------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cli/src/env_ops.rs` | 38 个 `#[test]` | 三通道互斥、通道缺失、末尾换行剥离（`\n`/`\r\n`/`\r`/无）、revision/force 互斥与缺失、kind 解析、`find_revision` 大小写匹配、表格渲染（敏感/只读/Unsupported/空表）、单 hive JSON 过滤、camelCase 契约校验、`get` 裸值输出、hive 选择 |
| `cli/src/runtime.rs` | 3 个 `#[test]`  | 冲突前缀判定、非冲突消息排除、与 core 常量契约的一致性断言                                                                                                                                                                            |

**全部为纯函数单测，不触碰注册表。**

设计取舍：不为 CLI 新增注册表集成测试。写路径语义（冲突拒写、DWORD 拒写、保护名拒绝、Path 过滤等）已由 core 侧 97 个测试保障；CLI 薄壳的职责是参数转换与错误透传，透传正确性由单测覆盖。

### 质量门（合并后于 main 实测）

| 检查                                                    | 结果                                               |
| ------------------------------------------------------- | -------------------------------------------------- |
| `cargo fmt --all -- --check`                            | 通过                                               |
| `cargo clippy --workspace --all-targets -- -D warnings` | **零警告**                                         |
| `cargo test --workspace`                                | core **97 passed**（1 ignored）+ cli **41 passed** |
| `npm test`（Vitest）                                    | **213 passed** / 18 files                          |
| 版本号一致性                                            | `package.json`、`Cargo.toml` 均为 `5.1.3`          |

---

## 5. 真实注册表闭环测试

**测试已于 2026-09-18 完成，4 项全部通过，注册表零污染。** 完整记录见 `docs/审核和开发/2026.09.18/PathEditor-CLI环境变量闭环测试记录.md`。

### 授权与前置

用户显式授权执行真实写入，并要求逐步进行、遇异常即停下求证。前置已完成：注册表备份（`C:\Users\33644\.patheditor\backups\path_backup_20260918_141256_062.txt`）、操作前后快照、临时变量名冲突检查（两个 hive 均无 `PATHEDITOR_*`）。测试范围限定为自建临时变量 `PATHEDITOR_VERIFY_TMP`，不触碰任何既有变量。

### 验证结果摘要

| #   | 验证项                             | 结果                                                                                                 |
| --- | ---------------------------------- | ---------------------------------------------------------------------------------------------------- |
| 1   | `env add`（HKCU，`--kind expand`） | ✅ 退出码 0；`get` 读回正确；`kind: expandString`，**未被降级为 `REG_SZ`**（Issue #26 回归点未复现） |
| 2   | `env set --force`                  | ✅ 退出码 0；值更新；**类型保持 `expandString`**（set 不改类型）                                     |
| 3   | 退出码 3 真实冲突                  | ✅ stderr 含 `[E_CONFLICT]` 前缀；**退出码 3**；**冲突被拒后值未被覆盖**                             |
| 4   | `env remove --revision`            | ✅ 退出码 0；删除后 `get` 以退出码 1 报错；列表无残留                                                |

**快照对比（零污染证明）**：

```text
user:   操作前 29 → 操作后 29  ✅
system: 操作前 23 → 操作后 23  ✅
新增项：无    消失项：无
```

### 未覆盖项（如实列出）

以下情形本轮未验证，需要时另立授权测试：

- `env add` / `env set` / `env remove` 在 **HKLM（系统 hive）** 的行为——本轮全部在 HKCU 进行
- `--stdin` 与 `--value-file` 两个值输入通道的端到端读数（本轮只用位置参数与 `--value`）
- 敏感变量（命中 `is_sensitive`）的 `env get` 读回
- 保护名单变量被写入时的拒绝错误透传
- 并发创建同名变量的极端竞态（`create_env_var` 的检查与写入是两步操作，存在窗口）

---

## 6. 需要审核窗口重点关注的项

### 6.1 已知保留项（非缺陷，但请确认措辞是否如实）

- **Windows 注册表无 CAS**：`update_env_var` / `delete_env_var` 的「读 → 算 revision → 比对 → 校验 → 写」中，读与写是两次独立注册表调用，**仍有毫秒级竞态窗口**。revision 校验缩小影响，**不能完全消除 TOCTOU**。代码注释与文档均已如实措辞，未宣称原子性。
- **`create_env_var` 的重名检查与写入是两步操作**，并发创建同名的极端场景可能后写覆盖。当前未做原子 CAS，代码注释已说明。

### 6.2 本轮做的两处清理（请复核判断是否恰当）

1. **移除 CLI 侧冗余环境广播**（`7b410b6`）：计划要求三个写命令成功后调 `broadcast_env_change()`，但 core 的 `update_env_var` / `create_env_var` / `delete_env_var` 在写入成功后**各自已调用**该广播。CLI 侧是重复的 Win32 广播，经用户裁决移除。语义等价，少一次系统调用。
2. **README 命令数改用可复现口径**（`7ff2f69`）：原文 `18` 为口径不明的历史估值。统一为「`patheditor --help` 中 `Commands:` 段不含 clap 自动生成的 `help` 行」，实测 **17 条**，并在正文注明另有 `env` / `profile` 子命令组。

### 6.3 计划/规格文档中已修正的事实错误

执行过程中发现并修正了计划与 spec 中的 4 处错误，已在文档内标注（详见计划文档 Execution Notes）：

| #   | 错误                                            | 修正                                                             |
| --- | ----------------------------------------------- | ---------------------------------------------------------------- |
| 1   | 计划中所有 `cargo test -p patheditor-cli --lib` | 该 crate 是 bin-only，无 lib target；改为 `--bins`               |
| 2   | 分支名 `workspace-all-env-vars`                 | 实际为 `worktree-all-env-vars`                                   |
| 3   | `--kind` 表述为「必填」                         | 实际默认 `string`（选填），按用户裁决统一                        |
| 4   | 验证命令 `env get Path --user`                  | `env get` 无 `--user` 参数（仅 `--system`），改为 `env get Path` |

### 6.4 未推送

所有提交**留在本地**，`main` 领先 `origin/main` **17 个提交**。推送时机由用户决定。

---

## 7. 交付物位置索引

| 交付物         | 路径                                                                     |
| -------------- | ------------------------------------------------------------------------ |
| 设计文档       | `docs/superpowers/specs/2026-09-17-cli-env-vars-design.md`               |
| 实施计划       | `docs/superpowers/plans/2026-09-17-cli-env-vars-implementation.md`       |
| 闭环测试记录   | `docs/审核和开发/2026.09.18/PathEditor-CLI环境变量闭环测试记录.md`       |
| 命令实现       | `cli/src/env_ops.rs`                                                     |
| 命令分派       | `cli/src/main.rs`                                                        |
| 退出码基础设施 | `cli/src/runtime.rs`                                                     |
| 冲突文案读取器 | `core/src/registry.rs`（`conflict_message()`）                           |
| 注册表备份     | `C:\Users\33644\.patheditor\backups\path_backup_20260918_141256_062.txt` |

---

## 8. 合并与分支状态

- **合并方式**：`git merge --no-ff worktree-all-env-vars`，merge commit `edc0635`
- **合并前 main**：`8481a9f`
- **合并后 main**：`f85ee04`（含合并提交与后续文档更新）
- **工作树状态**：干净
- **worktree 分支**：`.claude/worktrees/all-env-vars` 与本地分支 `worktree-all-env-vars` **保留未删**（已合并，可安全删除，待用户决定）

---

**开发窗口签章**：本回执所述全部结论均基于实际执行的命令输出，未经推测。质量门与闭环测试的原始输出见上述归档文档。
