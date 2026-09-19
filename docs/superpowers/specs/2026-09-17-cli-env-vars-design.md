# CLI 环境变量管理 — 设计文档

**日期**: 2026-09-17
**分支**: `worktree-all-env-vars`（延续全环境变量特性分支）
**状态**: 已确认（4 项核心决策 + 2 项小取舍均经用户拍板）
**关联**: 前置规格 `docs/superpowers/specs/2026-09-16-all-env-vars-design.md`（GUI 侧，已实现并通过复审）

## 概述

全环境变量特性已完成 GUI 侧交付（core 读写 + 5 个 IPC 命令 + 前端「全部变量」Tab）。本设计把同一能力延伸到 CLI，让脚本与自动化场景获得与 GUI 同等的变量管理能力。

**架构既定**：仓库规范（AGENTS.md/CLAUDE.md）已锁定「gui 和 cli 只做参数转换、命令分派和错误呈现，业务规则放在 core」。core 的 5 个公开 API 已全部就绪并经 97 个测试覆盖，CLI 侧**零新增核心代码**，只做薄壳。

| core API（`core/src/registry.rs`）                      | 服务的 CLI 命令 |
| ------------------------------------------------------- | --------------- |
| `list_all_env_vars() -> Result<EnvVarSnapshot, String>` | `env list`      |
| `reveal_env_var(hive, name) -> Result<String, String>`  | `env get`       |
| `update_env_var(hive, name, value, expected_revision)`  | `env set`       |
| `create_env_var(hive, name, value, kind)`               | `env add`       |
| `delete_env_var(hive, name, expected_revision)`         | `env remove`    |

保留名（`Path`）、保护名单、`Unsupported` 类型、hive 写权限、`[E_CONFLICT]` revision 校验**全部由 core 判定**，CLI 零重复实现，错误消息透传。

## 已确认的决策

| #   | 决策点     | 结论                                                                                                              |
| --- | ---------- | ----------------------------------------------------------------------------------------------------------------- |
| 1   | 命令形态   | `env` 子命令组（对齐 `profile` 先例），不用扁平 `env-xxx` 命名                                                    |
| 2   | 并发语义   | 双模式强制显式选择：`--revision`（CAS）或 `--force`（跳过 revision 校验，最后写入者胜），两者都不给报错、都给报错 |
| 3   | 值输入通道 | argv + `--stdin` + `--value-file` 三通道（敏感值可绕开 shell 历史与进程列表）                                     |
| 4   | hive 选择  | 默认 user，`--system` 切换；写操作绝不跨 hive 兜底                                                                |
| 5   | 冲突退出码 | 新增退出码 3 表示 revision 冲突（仅 env 命令，PATH 命令不动）                                                     |
| 6   | 敏感值读取 | `env get` 直接打印明文（显式 get 即授权，不加 `--reveal` 确认标志）                                               |

## 命令规格

```text
patheditor env list   [--system|--user] [--json]
patheditor env get    <NAME> [--system]
patheditor env set    <NAME> [--value <V>|--stdin|--value-file <F>] (--revision <R>|--force)
patheditor env add    <NAME> [<VALUE>|--stdin|--value-file <F>] [--kind <string|expand>]
patheditor env remove <NAME> (--revision <R>|--force)
```

### `env list`

- 默认列出两个 hive（对齐 PATH `list` 的 `system || !user` 惯例）；`--system`/`--user` 过滤单侧
- 人类可读输出：表格列 `NAME / KIND / PREVIEW`；命中敏感规则的变量 preview 显示 `(敏感)`；`can_edit=false` 的变量名后加 `(只读)`；`Unsupported` 类型 KIND 列显示 `unsupported`
- `--json`：serde 直接序列化 core 的 `EnvVarSnapshot`（camelCase，契约单一来源）；单 hive 过滤时只保留对应字段

### `env get`

- **唯一明文出口**，调用 `reveal_env_var`
- **stdout 只打印值本身 + 换行，零装饰**（管道友好，对齐 `git config --get`）；错误走 stderr
- 敏感变量也打印（决策 #6）；`Unsupported` 类型报错（继承 core 语义）
- 仅 get（只读）在当前 hive 未命中时查询另一 hive，用于错误提示「该变量存在于系统 hive，请加 --system」；**写操作不做此提示也不跨 hive**

### `env set`

- 更新**已存在**变量，调用 `update_env_var`；类型跟随注册表现状，不可更改（与 GUI 一致）
- 值三通道互斥：`--value` / `--stdin` / `--value-file` 同时给出多个则报错
- `--revision <R>`：CAS 校验（从 `env list --json` 的 `revision` 字段获取），与 GUI 同强度
- `--force`：跳过 revision 校验直接覆盖（最后写入者胜；仍受保留名 / 保护名单 / 类型 / 权限判定）

### `env add`

- 新建变量，调用 `create_env_var`；`--kind` 默认 `string`，可选 `string` / `expand`（`string` → `REG_SZ`，`expand` → `REG_EXPAND_SZ`）
- 值通道同 `set`：位置参数 `<VALUE>`、`--stdin`、`--value-file` 三选一
- 保护名单 / 保留名 / 重名拒绝由 core 返回错误
- `env add` 的 `<VALUE>` 可省略（与 `set` 的值三通道规则一致，零通道时建空值变量）

### `env remove`

- 删除变量，调用 `delete_env_var`；需 `--revision <R>` 或 `--force`

### 公共行为

- 所有子命令支持 `--system`（默认 user）
- `set` / `add` / `remove` 成功后 `broadcast_env_change()`
- `list` / `get` 为只读，不广播

## 并发与安全语义

1. **双模式强制显式选择**：`--revision` 与 `--force` 互斥；都不给时报错并提示两种用法：

   ```text
   错误: 需要提供 --revision（并发校验）或 --force（跳过校验）
   ```

   设计意图：CLI 是一次性进程，静默降级为「现读现写」会让用户在毫秒级竞态窗口下最后写入者胜，与仓库 `verify_and_save` 的安全文化相悖；强制显式选择让每次覆盖都是知情决策。

2. **敏感值通道**：`patheditor env set MY_TOKEN sk-abc123` 会把明文写进 shell 历史与进程列表（任务管理器可见），GUI 无此泄露面。`--stdin` 与 `--value-file` 提供绕开通道：
   - `--stdin`：读到 EOF，按 UTF-8 解码，去除末尾一个换行序列（`\r\n` 或 `\n`）——管道 `echo value |` 的标准约定
   - `--value-file <F>`：读取整个文件内容，同样去末尾换行序列；**不限制文件所在目录**（区别于 import 的安全限制——值文件是用户主动指定的单文件，非批量导入）

3. **安全判定单一实现处**：保留 / 保护 / 敏感 / 权限 / revision 校验全部在 core，CLI 不复制任何判定规则，仅透传错误文本（含 `[E_CONFLICT]` 前缀）。

## 错误处理与退出码

| 情形                                                                | 退出码 | 输出                                 |
| ------------------------------------------------------------------- | ------ | ------------------------------------ |
| 成功                                                                | 0      | stdout                               |
| 一般错误（参数、校验、找不到、权限、Unsupported、保护名、保留名等） | 1      | stderr，沿用 `exit_err`              |
| revision 冲突（core 返回 `[E_CONFLICT]` 前缀）                      | **3**  | stderr，保留 `[E_CONFLICT]` 前缀原文 |

- 退出码 3 的动机：脚本可凭退出码区分「重试后可恢复」（重新 list 取 revision）与致命错误，不必 grep 中文文案。**仅 env 命令引入，PATH 命令保持退出码 1 不动**（向后兼容）。
- `--force` 不携带 revision，**不会**因并发冲突产生退出码 3；退出码 3 仅在 `--revision` 不匹配时出现。

## 实现布局

```text
cli/src/
├── main.rs       # 仅新增 Clap Env 枚举 + match 分派（薄）
└── env_ops.rs    # 新文件：命令实现 + 纯函数格式化器
```

`env_ops.rs` 内的纯函数（可独立单测，不碰注册表）：

- 值通道解析：三通道互斥校验、stdin/file 读取与末尾换行剥离
- 并发选项解析：`--revision`/`--force` 互斥与缺失校验
- 表格渲染：`NAME / KIND / PREVIEW` 行组装（含 `(敏感)` / `(只读)` 标记）
- JSON 输出组装：单 hive 过滤

## 测试策略

- **Rust 单测**（`env_ops.rs` `#[cfg(test)]`）：上述纯函数全覆盖——三通道互斥、通道缺失、revision/force 互斥、二者皆缺的报错文案、stdin 末尾换行剥离（`\n` / `\r\n` / 无换行）、表格格式化（敏感标记、只读标记、Unsupported 显示）、单 hive JSON 过滤
- **不新增注册表集成测试**：写路径语义（冲突拒写、DWORD 拒写、保护名拒绝、Path 过滤等）已由 core 侧 97 个测试保障；CLI 薄壳的职责是参数转换与错误透传，透传正确性由单测覆盖
- 质量门：`cargo fmt` / `clippy -D warnings` / `cargo test --workspace` 全绿；README 测试计数同步

## 文档同步

- `README.md`：CLI 命令表追加 `env` 子命令组
- `AGENTS.md` / `CLAUDE.md`：CLI 命令节追加 env 组（两文件内容保持一致，**顺带修复遗留的 IPC 表格分隔行宽度不一致**）
- 错误码约定（退出码 3）写入 CLAUDE.md 错误处理节

## 范围外（本版不做）

- `env rename`：可由 add + remove 组合，但涉及类型迁移与大小写语义，单列后续 issue
- 环境变量导入 / 导出（PATH 既有 import/export 不扩展到通用变量）
- profile 集成（profile 只管 PATH 快照，不纳入变量）
- 交互式确认提示（CLI 保持非交互；`--force` 即显式确认）

## 验收标准

1. 5 个子命令按规格工作，默认 user hive、`--system` 切换正确；`env add --kind` 合法值为 `string`（默认）/ `expand`，非法值由 clap `value_parser` 拒绝
2. `--revision` 与 `--force` 互斥、皆缺报错；revision 冲突时（仅 `--revision` 模式）退出码 3 且 stderr 含 `[E_CONFLICT]`
3. `--stdin` / `--value-file` 读值正确并剥离末尾换行；三通道互斥报错
4. `env get` 输出裸值；`env list --json` 序列化 core 契约（camelCase）
5. 保留名 / 保护名 / Unsupported / 权限不足的错误透传自 core，CLI 无重复判定代码
6. `set` / `add` / `remove` 成功后环境变更广播生效（由 core 写入口内部调用 `broadcast_env_change`，CLI 不重复广播）
7. 新增纯函数单测全绿，`cargo clippy -D warnings` 零警告
8. README / AGENTS.md / CLAUDE.md 同步，两文件内容一致
