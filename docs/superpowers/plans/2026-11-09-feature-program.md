# 功能扩展批次（2026-11-09）

> 用户清单 15 项；按依赖与风险分两批。本文件记录当前落地状态，未完成项保持明确标记，不写成“已完成”。

## 已落地

| #   | 功能                        | 落地内容                                                                                                                                            |
| --- | --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | 环境变量修改写入历史/快照   | `service::{update,create,delete}_env_with_db`（+ force 变体）：写前/写后完整快照 + `EnvSet/EnvAdd/EnvRemove` 事件；GUI/CLI 在 DB 可用时自动走该路径 |
| 2   | 环境变量撤销/重做           | 环境变量工具栏撤销/重做；会话栈保存前后快照 uid，调用 `history_restore_env_snapshot`（只恢复 env，不碰 PATH）                                       |
| 3   | 配置方案包含并应用 env      | 配置快照已含 env；`profile_env` 读取；`apply_profile` 在 system/user 两侧都完整时先应用 env；应用后 GUI 刷新 env                                    |
| 4   | env 恢复审计                | `EventAction::EnvRestore`（`envRestore`）；快照 env 恢复与整份恢复都会写事件                                                                        |
| 5   | 一次性恢复 PATH+env         | `restore_snapshot_full` + IPC `history_restore_full_snapshot` + CLI `history-restore-full` + 快照详情主按钮                                         |
| 6   | 环境变量 N 路对比           | `compare_many` + 历史面板多选，以第一份快照为基准                                                                                                   |
| 7   | 配置方案对比                | core `diff_profiles` + CLI `profile diff` + GUI 双选差异摘要                                                                                        |
| 8   | 搜索增强                    | 快照备注 FTS、类型筛选、时间范围、保存搜索                                                                                                          |
| 9   | 规则引擎                    | schema v6 + `rules`/`rule_hits` + GUI「整理规则」（预览/应用到草稿/命中历史）                                                                       |
| 10  | 冲突解决引导                | 多选批量解决 + 远端原始内容展开                                                                                                                     |
| 12  | 保留策略 UI                 | 统计页卡片读写 `snapshot_keep_days/max`、`event_keep_days`                                                                                          |
| 13  | 健康打开目录 / 迁移备份还原 | 健康卡片「打开数据目录」；CLI `maintenance backups` / `restore-backup`（覆盖前留 `pre-restore.bak`）                                                |
| 14  | 深色模式跟随系统            | `themeMode=system` + `matchMedia` 监听；菜单「跟随系统主题」；兼容旧 `darkMode` 键                                                                  |
| 15  | winget/choco 识别           | 扫描分析探测 `WinGet\Links` 与 `chocolatey\bin`，未在 PATH 时标记                                                                                   |

## 未完成

| #   | 功能              | 缺口                                            |
| --- | ----------------- | ----------------------------------------------- |
| 11  | 字段级 LWW（D4）  | 仍是整实体 LWW；需 bundle v2 + profile 字段维度 |
| 13  | 迁移备份 GUI 还原 | 只有 CLI；GUI 还原需处理“应用运行中”的替换风险  |

## 验证

- Rust：`cargo test --workspace` → **360 passed / 2 ignored**
- 前端：41 files / 448 passed；E2E 59 passed；build/lint/format/一致性全绿
