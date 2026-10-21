# PathEditor v6.0.0 M3 复审结论

- **复审日期**：2026-10-21
- **被审对象**：`main` @ `66a2953`（整改提交，14 文件，+487 / −55）
- **复审依据**：`docs/审核和开发/2026.10.21/PathEditor-v6-M3-审查报告.md`（220 行）
- **开发回执**：`docs/审核和开发/2026.10.21/PathEditor-v6-M3-复审回执.md`（166 行，已随整改提交）

## 一、判定

**通过。** 审查报告的四项发现全部成立并已落到代码；R-1 采用 B' 而非建议 A，
**我复核后接受**（理由见 §三）。无阻塞项。

可以进入下一阶段。**提交与推送仍需用户单独授权**（本轮授权不延续）。

## 二、审核窗口独立验证（不采信回执数字）

| #   | 命令                                                    | 结果                                             |
| --- | ------------------------------------------------------- | ------------------------------------------------ |
| 1   | `cargo fmt --all -- --check`                            | exit 0                                           |
| 2   | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0，无 error / warning                       |
| 3   | `cargo test --workspace`                                | **48 binaries, 340 passed, 0 failed, 2 ignored** |
| 4   | `npx tsc --noEmit`                                      | exit 0                                           |
| 5   | `npx prettier --check`（全部改动文件）                  | All matched files use Prettier code style        |
| 6   | `npx vitest run`                                        | **33 files, 397 passed**                         |
| 7   | `npm run test:e2e`                                      | **54 passed (1.4m)**                             |

与回执 §4.1 完全一致：Rust 339→340（+1 新测试文件，binaries 47→48），
前端 395→397（+2），E2E 54→54。**无一项下降。**

## 三、逐条核实（file:line）

### R-1 —— 成立，已整改（方案 B'）

| 位置                                       | 内容                                                                        |
| ------------------------------------------ | --------------------------------------------------------------------------- |
| `core/src/db/sync/import.rs:56`            | `APPLICABLE_ENTITY_TYPES = &["snapshot"]`，与白名单分离                     |
| `:60-68`                                   | `KNOWN_ENTITY_TYPES` 决定「包是否合法」，前者决定「能否落库」               |
| `:147-150`                                 | 幂等检查（`change_exists_tx`）在前                                          |
| `:157-160`                                 | 不支持类型：`skipped_unsupported += 1; continue;`——**不写 `changes`**       |
| `:213-225`                                 | `record_remote_change_tx` 与 `applied += 1` 只对通过检查的类型执行          |
| `:297-321`                                 | `apply_entity_tx` 的 `_ => Ok(())` 改为返回错误（将来加类型忘实现会立刻炸） |
| `cli/src/sync_ops.rs:75-81`                | CLI 在 `skipped_unsupported > 0` 时向 stderr 明示                           |
| `src/components/sync/SyncCenter.tsx:26-35` | GUI 走 `noticeImportedUnsupported` 专用文案                                 |

**第二层陷阱（回执 §3.2 第 3 点）验证成立**：`change_exists_tx` 按 uid 判定幂等
（`import.rs:147`），因此「跳过但照样记 `changes`」会让 uid 被占住，M4 支持配置同步后
同一包会被永久跳过。`db_sync_import_limits.rs:88-91` 断言重复导入仍计
`skippedUnsupported`，正是守这一点。

**为什么接受 B' 而不坚持我原推荐的 A**：
`save_profile_tx` 在同一事务内写**两条**变更——`core/src/db/profiles.rs:147`（profile）
与 `:157`（profile_revision）。因此任何存过配置的用户，其待导出队列必然含这两类；
A 会把整包（含本可同步的 PATH 快照）全部拒绝。这不是「更严格」，是「让同步对
主要用户群不可用」。**回执的技术理由成立，我撤回 A 的推荐。**

### F-3 —— 成立，已删

`#[allow(dead_code)] fn _unused` 已删除；它维持的 `BundleHeader` import 也一并删除
（`import.rs:25` 现为 `use crate::db::sync::bundle::open;`）。「死函数维持一个假用途的
import」这个观察准确——同一判据被骗过去了。

### F-4 —— 成立，已改（比建议更彻底）

- `src/store/sync-store.ts:67-72`：`SyncNotice` 结构化联合类型，store 不产出面向用户的文案
- `src/components/sync/SyncCenter.tsx:15-42`：`renderNotice()` 经 `t()` 渲染
- `zh-CN.json` / `en.json` 各新增三键，键名一致

**一处措辞不准确（非阻塞）**：回执 §3.3 称「store 里**一个中文字符都没有**」。
实际 `sync-store.ts:39/41/42` 仍有错误兜底中文（`'同步不可用（降级模式）'` /
`'同步包无效'` / `'同步操作失败'`），`:55` 还有 `'(空)'`。本次 F-4 的**主反馈**
（导入完成提示）已正确迁移，但该句表述与事实不符。建议 M4 把这些兜底也按
`err.code` 交给组件渲染。

### F-5 —— 成立，已改

`gui/build.rs:62-71`：`Err` 分支给出 MinGW bin 路径、MSVC 判断、运行期症状
（`0xC0000139`）与修复动作。静默降级现在有可操作提示。

### 白名单缺测试 —— 已补

`core/tests/db_sync_import_limits.rs`（122 行，**1 个 `#[test]`**）覆盖两段：

1. 混合包（1 快照 + 1 配置）→ `applied = 1`、`skipped_unsupported = 1`，
   且 `changes` 中 `entity_type = 'profile'` 行数为 **0**（守第二层陷阱）；
   重复导入 → `applied = 0`、`skipped = 1`、`skipped_unsupported = 1`
2. 未知 `entityType = "backups"` → `SyncFormat`，导入前后 `changes` 行数不变（整包回滚）

**一文件一测试**：`core/tests/db_sync_*.rs` 五个文件均恰好 1 个 `#[test]`。

## 四、复审中的新观察（非阻塞，归 M4）

| #   | 观察                                   | 证据                                                                                                            | 建议                                                         |
| --- | -------------------------------------- | --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| N-1 | `header.schema_version` 只记录、不校验 | `import.rs` 只用 `change_count` / `device_id` / `base_lamport`；`bundle.rs:168` 校验的是二进制 `FORMAT_VERSION` | M4 明确其语义，或改为拒绝不兼容版本                          |
| N-2 | `content = null` 的墓碑不触发本地删除  | `import.rs:206-211` 仅在 `Some(content)` 时调 `apply_entity_tx`                                                 | M3 的快照没有删除路径，当前无影响；**M4 做配置同步前必须补** |
| N-3 | `sync-store.ts` 仍有中文错误兜底       | 见 F-4 备注                                                                                                     | 与错误码本地化同批处理                                       |

## 五、审核窗口本轮所做的文档修订（未提交）

1. `docs/superpowers/plans/2026-10-21-v6-sqlite-m3-plan.md`
   - `:917` `#1#1` → `#1`（该损坏由**我自己的**提交 `62c6438` 引入）
   - Task 6 Interfaces 的 `ImportReport` 补 `skipped_unsupported` 及其语义
   - Execution Notes：补偏离 6（R-1 整改）；测试数更新为 340 / 397 / 54；
     把「R-1 是最主要残余风险」改为「已闭环，配置同步本身待 M4」
2. `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` §6.4
   - `Change` 补 `content`（实体当前内容）与 `seq`；删除已废弃的 `deletedAt`；
     `entityType` 枚举移除 `event`
   - `schemaVersion` 例值 1 → 3（实现写的是库结构版本），并注明当前不校验
   - 导入顺序去掉 `events`；错误码 `Sync`/退出码 4 → `SyncFormat`/退出码 1
   - 新增「当前可落库类型（M3）」条目，记录 `skippedUnsupported` 语义与不写 `changes` 的原因

两份文件均过 Prettier；U+FFFD 与 `#N#N` 复扫均为 0。

## 六、工作区状态

```text
 M docs/superpowers/plans/2026-10-21-v6-sqlite-m3-plan.md      ← 本轮审核窗口修订
 M docs/superpowers/specs/2026-10-19-v6-sqlite-design.md       ← 本轮审核窗口修订
?? docs/审核和开发/2026.10.21/PathEditor-v6-M3-审查报告.md     ← 未跟踪
?? docs/审核和开发/2026.10.21/PathEditor-v6-M3-复审结论.md     ← 本文档
```

`CHANGELOG.md` 已与 HEAD 一致（`git diff` 为空）：`#34#34` / `#38#38` 不再存在。
`66a2953` 未推送。

## 七、下一步

- M3：**通过**，可进入 M4。
- 待用户授权：
  1. 提交本轮三份审核文档（审查报告、复审结论、计划与 spec 修订）；
  2. 推送 `main`（含 `66a2953`）。
- M4 范围建议（供规划参考）：`.v5.bak` 归档、遗留明文清理、版本号 6.0.0、
  配置同步落库（含 N-2 墓碑语义与 `uq_profiles_name_active` 同名冲突）。
