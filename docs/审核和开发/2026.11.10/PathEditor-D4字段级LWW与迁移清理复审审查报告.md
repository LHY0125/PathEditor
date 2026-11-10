# PathEditor D4 字段级 LWW 与迁移清理复审审查报告

- **复审日期**：2026-11-10
- **复审对象**：`docs/审核和开发/2026.11.10/PathEditor-D4字段级LWW与迁移清理复审回执.md`（更新版）
- **被审状态**：工作区未提交、未推送；版本仍 7.0.0
- **基准**：`f21480e`
- **结论**：**复审通过。P1/P2 全部关闭；发布前必须升版本号（需用户授权）。**

## 1. 独立验证

未采信回执数字，重新执行：

| 命令                                                          | 实际结果                               |
| ------------------------------------------------------------- | -------------------------------------- |
| `cargo test --workspace`                                      | **370 passed / 0 failed / 2 ignored**  |
| `node node_modules/vitest/vitest.mjs run`                     | **45 files / 470 passed**              |
| `node node_modules/vitest/vitest.mjs run --coverage`          | 行覆盖率 **80.2%**                     |
| `node node_modules/@playwright/test/cli.js test --config ...` | **59 passed**                          |
| `cargo fmt --all -- --check`                                  | 通过                                   |
| `cargo clippy --workspace --all-targets -- -D warnings`       | 通过                                   |
| Prettier 全量检查                                             | 通过                                   |
| ESLint                                                        | 0 errors，3 个既有 React Compiler 警告 |

`npm run verify` 仍受本机 npm/npx shim 的 `/dev/null` 问题影响；等价质量门已用底层二进制逐项执行。

## 2. 整改核对

### 2.1 P1

| 项                      | 核对结果       | 证据                                                                                                        |
| ----------------------- | -------------- | ----------------------------------------------------------------------------------------------------------- |
| P1-1a schema 6 标识     | **通过**       | `CURRENT_SCHEMA_VERSION = 7`；`ensure_schema` 对 4/5/6 拒绝且不改写版本号；非空库用例存在                   |
| P1-1b 已发布库拒绝      | **策略已落实** | v4/v5/v6 空库与“当前 DDL + user_version=6”非空库均拒绝；回执如实说明未复制真实 v6 DDL                       |
| P1-2 降级提示           | **通过**       | `DbState` 保存 `init_error`；`DegradedBanner` 显示根因、打开数据目录、移除无条件重启提示                    |
| P1-3 lamport/seq/device | **通过**       | header schema、device 一致性、拒绝本机 device、`0..=base`、`seq <= MAX-2`、checked_add 均落地；边界测试存在 |
| P1-4 header schema      | **通过**       | 任何 `schema_version != 7` 的 v2 包均 `SyncFormat`；fixture 已同步                                          |

### 2.2 P2

| 项                                   | 核对结果                                                                                                               |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------- |
| `deleted_at` 带 content              | **通过**：仅拒绝这一种；普通字段 content=null 仍合法                                                                   |
| seen_fields/seen_entities 注释       | **通过**：改为“导出内容为当前值，取第一条/最后一条等价”                                                                |
| `migrate_to_current` 命名与 CLI 文案 | **通过**：`ensure_schema` / `open_and_ensure_schema`；CLI 显示“数据库结构不可用”                                       |
| 不可达 profile fallback              | **通过**：import 显式 `SyncFormat`；conflict 显式 `Internal`                                                           |
| 文档旧语义                           | **通过**：README、AGENTS/CLAUDE、spec superseded、schema 6→7 已同步                                                    |
| 规则启停/删除反馈                    | **通过**：5 处改为 `errorMessage(err)`；新增 `rules-dialog.test.tsx` 验证普通对象 rejection 不再显示 `[object Object]` |
| 降级删除指引                         | **通过**：仅 `migration` / “旧版加密数据库”提示备份后删除；普通 `database` 提示备份、不要直接删除；新增测试            |
| N 路对比/配置差异/i18n/日期          | **通过**：逐条渲染、描述仅在非空时显示、`deleted_at`、日期先后校验均已落地                                             |

## 3. 剩余项

### P3

- `DegradedBanner` 的“打开数据目录”调用 `.catch(() => {})`，失败无反馈；不影响主流程。
- 真实 v6 历史 DDL fixture 未复制；当前用“当前 DDL + user_version=6”验证版本闸门，回执已如实披露，可接受。
- `next_seq` 耗尽错误已补清理指引；若外部工具把本机 seq 写成超大值，仍会显式报错，不会静默损坏。

### 发布门禁（非代码缺陷）

- `package.json` / `Cargo.toml` / `tauri.conf.json` 仍为 **7.0.0**。
- 本批包含 schema 4/5/6 全量拒绝、同步包 schema 7、GUI 降级行为变化，属于 breaking change；**不能以 7.0.0 发布**。发布前必须升版本（建议 7.1.0 或 8.0.0，按项目语义），并同步 README / CHANGELOG / Release notes。
- 按仓库边界，升版本需要用户明确授权；本轮未改版本号。

### 已接受的产品取舍

- `R-01`：拒绝 header 等于本机 deviceId 的包，牺牲“同机灾难恢复自导入”；回执已列为非阻塞。若产品需要同机恢复，应另设显式恢复入口，而不是放宽 seq 防护。
- 规则应用在保存 PATH 前写 `rule_hits`：已按报告要求留待产品确认，不作为本轮缺陷。

## 4. 判定

**复审通过。** P1/P2 全部关闭；剩余为 P3 细节与发布版本门禁。代码可进入合并准备，但发布前必须完成版本号升级并取得授权。

本轮未提交、未推送、未写真实注册表；CI 未触发。
