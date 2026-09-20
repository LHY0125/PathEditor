# 备份体系未覆盖通用环境变量（登记文档）

- **登记日期**：2026-09-19
- **登记人**：审核窗口
- **状态**：待开发窗口评估；本轮（三波次收口）**不处理**，开发完后再议
- **来源**：审核窗口在答疑中发现的复审未覆盖项

## 1. 问题陈述

PathEditor v5.2 将管理范围从 PATH 扩展到通用环境变量，但**备份体系没有跟着扩**，存在两个独立缺口：

### 缺口 1：备份内容只有 PATH

`core/src/backup.rs:38-39` 的 `backup_registry()` 只读两个键：

```rust
let sys_paths = registry::load_paths(HKEY_LOCAL_MACHINE, SYS_REG_PATH, "系统")?;
let user_paths = registry::load_paths(HKEY_CURRENT_USER, USER_REG_PATH, "用户")?;
```

备份文件 `path_backup_<时间戳>.txt` 只有 `[System PATH]` / `[User PATH]` 两段。其他环境变量（`JAVA_HOME` 等）不进备份。

### 缺口 2：环境变量写入路径不触发备份

全库 `backup_registry` 调用点仅 3 处，全部绑定 PATH 保存流程：

| 调用点                                | 场景                            |
| ------------------------------------- | ------------------------------- |
| `src/services/path-session.ts:131`    | GUI 保存 PATH 前备份            |
| `cli/src/main.rs:461`（`cmd_backup`） | 仅 `patheditor backup` 手动命令 |
| `gui/src/commands/backup.rs`          | 上一条的 IPC 转发               |

CLI `env set` / `env add` / `env remove`（`cli/src/env_ops.rs`，grep `backup` 零命中）与 GUI `update_env_var` / `create_env_var` / `delete_env_var` 均无备份调用。**改坏一个环境变量，事前没有任何备份**；PATH 改坏至少有「保存前旧值」可手工恢复。

## 2. 定性

- 不在本次 11 项整改清单内：复审报告 F-10 只提「备份格式与恢复可用性」（指现有 `.txt` 能否用于恢复），未把「环境变量纳入备份范围」列为缺陷。
- 属于 5.2 全环境变量扩展的遗留缺口，**复审未覆盖的新发现**。

## 3. 待议方案（开发完成后再评估）

1. **内容**：扩展备份文件为环境变量段，或另出 `env_backup_*.json`（带 hive、类型 `REG_SZ`/`REG_EXPAND_SZ`、revision——当前 `.txt` 丢失类型信息，恢复时无从得知原类型）。
2. **触发**：`env set/add/remove`（CLI 与 GUI）写前触发对应 hive 的备份；**敏感变量明文是否进备份文件需单独决策**（备份文件目前是明文，写入敏感值会扩大暴露面）。
3. **归属**：本轮三波次不放；作为独立小项处理，或并入 F-10「备份格式」一起收口。

## 4. 处理约定

- 本文档只登记，不改任何代码、不进本轮计划。
- 待当前三波次开发完成后，由用户决定是否立项。
