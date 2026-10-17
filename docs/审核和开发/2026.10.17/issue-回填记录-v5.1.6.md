# v5.1.6 issue 回填记录

- **日期**：2026-10-17
- **版本**：v5.1.6（[Release](https://github.com/LHY0125/PathEditor/releases/tag/v5.1.6)）
- **结论**：**本轮无 issue 可回填**，未新建、未关闭任何 issue。

---

## 1. 核实过程

发版后按 5.1.5 的先例检查是否有需要回填的 issue。核实方法：`gh issue list --state all`（全量，不限状态）+ 关键词搜索（`PATH 保存` / `快照` / `顺序` / `死锁` / `拒绝` / `Scoop` / `clean` / `侧车`）。

### 全仓 issue 现状（8 条）

| #      | 标题                                                                                                                    | 状态     |
| ------ | ----------------------------------------------------------------------------------------------------------------------- | -------- |
| 38     | [待确认] env add 未提供 VALUE 时，stdin 非 TTY 环境下可能阻塞                                                           | CLOSED   |
| 37     | 备份目录缺少保留策略，长期使用会持续累积                                                                                | CLOSED   |
| 36     | 破坏性变更（env set / remove / add）缺少 dry-run 或变更预览                                                             | CLOSED   |
| 35     | env add 缺少 revision 并发守卫，与 set / remove 不对称                                                                  | CLOSED   |
| 34     | env get 读取不存在的变量时，把 Windows 原生 os error 2 直接抛给用户                                                     | CLOSED   |
| 33     | env list 声称「不含明文」，但 PREVIEW 列与 --json 的 preview 字段输出的是完整明文                                       | CLOSED   |
| **32** | **env set/remove 的 revision 守卫可用性：错误提示未说明如何获取 + help 措辞与 list --json 实际结构不符 + 批量往返过多** | **OPEN** |
| 26     | [Bug] 写入 PATH 时注册表值类型被降级为 REG_SZ，导致 %VAR% 条目无法展开                                                  | CLOSED   |

## 2. 为什么 #32 不回填

#32 的三个诉求**都不是 5.1.6 的内容**：

| #32 的诉求                                    | 5.1.6 是否涉及 | 说明                                                      |
| --------------------------------------------- | -------------- | --------------------------------------------------------- |
| ① 错误提示未说明 revision 从哪来              | ❌ 否          | 属 `env` 子命令文案，5.1.6 未触碰 env 通路                |
| ② `--revision` 的 help 措辞与 JSON 结构对不上 | ❌ 否          | 同上                                                      |
| ③ 批量往返过多（需跨命令 session）            | ❌ 否          | #32 自己已判「属架构决定，建议单独开 issue 走 spec 流程」 |

**决定性证据**：`git log --oneline v5.1.5..v5.1.6 -- cli/src/env_ops.rs core/src/env_var.rs core/src/registry/env_store.rs` **无任何输出** —— 该版本区间内 env 通路零改动。

**#32 保持开放**，其三个诉求仍待处理。

## 3. 为什么没有新建 issue

5.1.6 修复的两个缺陷（**顺序敏感死锁**、**`patheditor clean` 后侧车快照陈旧**）是 F-08 收口过程中**内部发现**的，**没有对应的既有 issue**。

**处置（用户 2026-10-17 决定）：不补开 issue，仅在文档中记录结论。**

理由：这两个缺陷由内部评审与真机回归发现，补开「自问自答」的 issue 对使用者无增量信息；CHANGELOG 的 5.1.6 段落已完整记录其现象、根因与修复，Release 页面即为公开可查的记录。

两个缺陷的公开记录位置：

| 缺陷                        | 公开记录                                                    |
| --------------------------- | ----------------------------------------------------------- |
| PATH 保存的顺序敏感死锁     | `CHANGELOG.md` 5.1.6 段落 `### 修复` 第 1 条 + Release 页面 |
| `patheditor clean` 侧车陈旧 | `CHANGELOG.md` 5.1.6 段落 `### 修复` 第 2 条 + Release 页面 |

## 4. 附：本轮一并完成的文档同步

发版后发现 `CLAUDE.md` / `AGENTS.md` 与代码不符，根因是 **`6d83dac` 的提交信息与实际改动不符**——其信息声称「命令表移除已退役的 save_system_paths…服务层说明改为 GUI 已接线」，但 `git show --stat 6d83dac` 显示**只改了 4 行**（每个文件 2+/2−），命令表与「暂未接线」说明**根本没动**。本次补齐：

| 项                             | 修正                                                                                                                     |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| 项目概述版本号                 | `v5.1.4` → `v5.1.6`                                                                                                      |
| `core/src/` 模块树             | 补 `service.rs` / `error.rs` / `reg_store.rs` / `persist.rs`，`registry.rs` → `registry/`                                |
| 前端 `src/core/` 描述          | 移除已删的 `path-manager`                                                                                                |
| IPC 表：`save_system_paths` 等 | 移除 5 个已退役命令，补 `save_path_with_sidecar` / `apply_path_snapshot` / `retry_pending_path_state` / `read_text_file` |
| IPC 表注：GUI 接线状态         | 「**暂未接线**」→「**已接线**」（`path-session.ts:135` 实调 `backend.savePathWithSidecar`）                              |
| 版本号升级清单                 | 补 `Cargo.lock`（本轮实测需 `cargo update -p` 同步三个 crate）                                                           |
| 发布流程示例                   | `v5.1.4` → `v5.1.6`                                                                                                      |
| README 测试徽章                | `305 passed` → `582 passed`（core 227 + CLI 65 + 前端 260 + E2E 30，实测）                                               |

**`CLAUDE.md` 与 `AGENTS.md` 保持字节一致**（md5 同步校验通过）。
