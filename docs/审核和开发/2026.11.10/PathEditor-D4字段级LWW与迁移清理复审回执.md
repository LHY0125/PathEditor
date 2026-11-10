# PathEditor D4 字段级 LWW 与迁移清理复审回执

## 1. 回执结论

| 项         | 内容                                                                                                                       |
| ---------- | -------------------------------------------------------------------------------------------------------------------------- |
| 回执日期   | 2026-11-10                                                                                                                 |
| 复审对象   | `docs/审核和开发/2026.11.10/PathEditor-D4字段级LWW与迁移清理审查报告.md`（修订版）                                         |
| 回执结论   | **P1 全部整改；P2 除“规则应用审计”待产品确认外全部整改；P3/待确认项保持不动**。判定：整改完成，建议复核后进入合并/发布流程 |
| 版本状态   | **7.0.1**（按用户 2026-11-10 明确指令升级；未提交、未打 tag）                                                              |
| Git 状态   | **未提交、未推送**                                                                                                         |
| 注册表状态 | 未触碰；E2E 仍使用 mock IPC                                                                                                |

### 第二轮复审整改（2026-11-10）

复审审查报告的两项 P2 已整改：

- **P2-1**：`RulesDialog` 的 5 处错误转换全部改用 `errorMessage()`；新增 `rules-dialog.test.tsx`，覆盖结构化 `{code, message}` rejection，断言不出现 `[object Object]`。
- **P2-2**：降级条幅只在 `migration` 或“旧版加密数据库”文案时建议“备份后删除旧库”；普通 `database` 错误改为“检查并先备份，不要直接删除数据库”；新增对应条幅用例。
- **P3 顺手修**：`ProfileDetail` 的描述行仅在任一侧非空时显示；`next_seq` 耗尽错误补充“检查本机 device_id 的 seq 是否被外部工具写入超大值”的清理指引。

质量门更新为 Rust **370 / 2 ignored**、前端 **45 files / 471 passed**、行覆盖率 **80.2%**、E2E **59**；README 徽章总数 **841**。

---

## 2. 整改核对

| 报告项                            | 处理                                                                                                                                                                                                                    | 证据                                                                                                              |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| P1-1a schema 6 版本标识           | **已改**：`CURRENT_SCHEMA_VERSION = 7`；4/5/6 库启动即拒绝，非法版本不改写 `user_version`                                                                                                                               | `src/core/src/db/schema.rs:7`；`src/core/tests/suites/schema/db_schema_open.rs`（版本循环 + 非空库用例）          |
| P1-1b 已发布库拒绝的用户面        | **已改文案/入口**：CLI 警告改为“数据库结构不可用”；GUI 降级条幅展示根因并给出“打开数据目录”；只有 schema 不兼容/旧加密库提示“备份后删除旧库”，普通 database 错误只提示检查并先备份                                      | `src/cli/src/runtime.rs:63`；`DegradedBanner.tsx:61-98`；`db_state.rs:10-37`                                      |
| P1-2 降级提示                     | **已改**：`DbState` 保存初始化错误的 `code/message`；`with_db`/`with_db_mut` 回传根因；banner 接受 `migration` 错误                                                                                                     | 同上；新增 `src/gui/web/tests/unit/degraded-banner.test.tsx`                                                      |
| P1-3 lamport/seq/deviceId         | **已改**：导入校验 `header.schemaVersion == 7`、`deviceId == header.deviceId`、拒绝本机 deviceId、`0 <= lamport <= baseLamport`、`0 <= seq <= i64::MAX - 2`；`next_seq`/`next_lamport` 改为 checked add，饱和时显式报错 | `src/core/src/db/sync/import.rs:177-298,322-339`；`change_log.rs:79-159`；`db_sync_import_limits.rs` 新增边界用例 |
| P1-4 header schema 未校验         | **已改**：任何 `schema_version != 7` 的 v2 包均 `SyncFormat`；8 个 import fixture 改为 7；`db_bundle_crypto` 的 1 只用于 `open`，未改                                                                                   | `import.rs:177-187`；`src/core/tests/suites/sync/*`                                                               |
| P2 deleted_at + content           | **已改**：`field == "deleted_at"` 且 `content` 非空 → `SyncFormat`；其它 profile 字段 `content = null` 仍按“无载荷”允许                                                                                                 | `import.rs:288-296`；新增测试 `profile_tombstone_must_not_carry_content`                                          |
| P2 seen_fields/seen_entities 注释 | **已改注释**：明确“导出侧 content 是当前值，取第一条/最后一条等价；不承诺后到覆盖”                                                                                                                                      | `import.rs:352-358,479-481`                                                                                       |
| P2 migrate 命名/CLI 文案          | **已改**：`migrate_to_current` → `ensure_schema`；`open_and_migrate` → `open_and_ensure_schema`；CLI、bootstrap、docs/dev、当前 plan/spec 同步                                                                          | `src/core/src/db/mod.rs:99`、`:285`；`src/cli/src/runtime.rs:49-63`；全仓 `migrate_to_current` 仅剩历史 Changelog |
| P2 不可达 profile None 分支       | **已改**：`import.rs` 的 profile `None` 分支改为显式 `SyncFormat`；`conflict.rs` 的 profile 整实体 fallback 改为显式 `Internal` 错误                                                                                    | `import.rs:382-397`；`conflict.rs:286-301`                                                                        |
| P2 文档旧语义                     | **已改**：README 430-444、AGENTS/CLAUDE 123 改为字段级 LWW；schema 6→7；spec 顶部加 2026-11-10 superseded 说明；storage/architecture 同步                                                                               | `README.md:430-444`；`AGENTS.md:121-123`；`CLAUDE.md:121-123`；`docs/dev/*`；spec 第 9-11 行                      |
| P2 规则启停/删除无反馈            | **已改**：`RulesDialog` 两个动作加 `.catch` 写入 `error`，且全部错误文案改用 `errorMessage()`（不再出现 `[object Object]`）                                                                                             | `RulesDialog.tsx:28-146`；`rules-dialog.test.tsx`                                                                 |
| P2 N 路对比只显示计数             | **已改**：`MultiCompareDialog` 增加 `PathDiffDetails`，逐条渲染 added/removed/reordered/toggled                                                                                                                         | `MultiCompareDialog.tsx:12-59,125-126`；新增 `multi-compare-dialog.test.tsx`                                      |
| P2 配置差异漏描述/重排/启停       | **已改**：`ProfileDetail` 渲染 `leftDescription/rightDescription`，并逐条渲染 `reordered`/`toggled`                                                                                                                     | `ProfileDetail.tsx:180-218`                                                                                       |
| P2 deleted_at i18n                | **已改**：zh/en 增加 `sync.field.deleted_at`                                                                                                                                                                            | `zh-CN.json:487`；`en.json:487`                                                                                   |
| P2 搜索起止日期                   | **已改**：起止反了显示 `search.dateRangeInvalid`                                                                                                                                                                        | `SearchPanel.tsx:32,209-213`；locale 新增键                                                                       |
| P2 规则应用 `record=true` 审计    | **未改**：报告明确要求产品确认；本轮不改变“应用到草稿即记 hit”的语义                                                                                                                                                    | `RulesDialog.tsx:55`                                                                                              |
| P3 `snapshotUid = null`           | **未改**：报告定为 P3/待确认；未定义“完整落地”的契约                                                                                                                                                                    | `profiles.rs:434-452`                                                                                             |

### 2.1 关键实现说明

- **schema 7**：没有写迁移；`ensure_schema` 只接受空库或当前版本。非空库 + `user_version = 6` 的回归用例确认拒绝发生在任何表访问之前。
- **header/schema 校验顺序**：`open` 解密并验证 AAD 后，先校验 schema，再解析 payload；schema 检查不会对旧包产生额外解密面。
- **seq 防护**：导入侧拒绝 `seq > i64::MAX - 2`；本地 `next_seq` 先读 `MAX(seq)`，再 `checked_add(1)`，不依赖 SQLite 的 `+1` 溢出行为。
- **本机 deviceId 防护**：`import_bundle` 在进入事务前拒绝 header 等于本地 `app_state.device_id`；同时 `open_bundle` 要求每条变更的 `deviceId == header.deviceId`。
- **墓碑形状**：只拒绝 `deleted_at` 带 `content`；软删配置后普通字段 `content = null` 仍合法（导出侧 `get_profile` 返回 `None` 时，所有 profile 旧变更都只能带 null）。

---

## 3. 验证证据

| 命令                                                                                           | 实际结果                                                                                   |
| ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `cargo test --workspace`                                                                       | **370 passed / 0 failed / 2 ignored**                                                      |
| `node node_modules/vitest/vitest.mjs run`                                                      | **45 files / 471 passed**                                                                  |
| `node node_modules/vitest/vitest.mjs run --coverage`                                           | 行覆盖率 **80.2%**（阈值 80%）                                                             |
| `node node_modules/@playwright/test/cli.js test --config src/gui/web/e2e/playwright.config.ts` | **59 passed**                                                                              |
| `cargo fmt --check`                                                                            | 通过                                                                                       |
| `cargo clippy --workspace --all-targets -- -D warnings`                                        | 通过                                                                                       |
| `node node_modules/prettier/bin/prettier.cjs --check ...`                                      | 通过                                                                                       |
| `node node_modules/eslint/bin/eslint.js .`                                                     | **0 errors，3 个既有 React Compiler 警告**（`EnvVarTable` / `MergePreview` / `PathTable`） |
| `node node_modules/typescript/bin/tsc -b && node node_modules/vite/bin/vite.js build`          | 通过                                                                                       |
| `node scripts/check-consistency.mjs`                                                           | 通过（版本 / schema / 基线 / 指令文件）                                                    |

> 本机 `npm`/`npx` shim 在当前 shell 报 `/dev/null` 路径错误，因此 `npm run verify` 未按原命令执行；上述每一项质量门已用对应的 `node`/`cargo` 原二进制逐项执行，命令与结果见上表。

### 3.1 新增/修改的测试

- `src/core/tests/suites/sync/db_sync_import_limits.rs`：新增 header schema 不符、lamport 负数/超 base、seq 负数/`i64::MAX`/`i64::MAX-1`、deviceId 不一致/伪造本机、墓碑带 content、本机 seq 溢出、header base 超半区共 7 个用例。
- `src/core/tests/suites/schema/db_schema_open.rs`：旧版本拒绝扩展为 4/5/6 循环；新增非空 schema 6 库拒绝用例。
- `src/gui/web/tests/unit/degraded-banner.test.tsx`：断言根因文案、移除“重启后可重试”、打开数据目录按钮；新增“普通 database 错误不提示删除旧库”。
- `src/gui/web/tests/unit/rules-dialog.test.tsx`：断言结构化 rejection 显示 message，不出现 `[object Object]`。
- `src/gui/web/tests/unit/multi-compare-dialog.test.tsx`：断言 N 路对比逐条渲染 added/reordered/toggled。
- 8 个 sync import fixture 的 `schema_version` 更新为 7；`db_sync_profile_name_conflict` 的 `base_lamport` 从 10 修正为 20，以符合“变更 lamport ≤ base”契约。

### 3.2 基线同步

- `docs/dev/testing.md`、`README.md` 测试表/徽章、`AGENTS.md`、`CLAUDE.md`、`CHANGELOG.md` 已同步：Rust 370、前端 45 files/471、E2E 59；README 徽章总数 841。

---

## 4. 未覆盖项（如实列出）

1. **真实 v6 历史 DDL fixture 未复制**：新增用例用“非空当前 DDL + `user_version = 6`”覆盖版本闸门；没有从 `f21480e` 复制旧 v6 DDL。拒绝逻辑发生在表访问之前，结构差异不影响该断言；若审核要求逐表旧结构，需要另建夹具。
2. **P2 规则应用审计未改**：`RulesDialog` 在用户保存 PATH 前以 `record=true` 写 `rule_hits`，是产品语义问题，等产品确认后决定是否改为“保存后记账”。
3. **P3 `snapshotUid = null` 未改**：profiles 表没有 current_snapshot 列、revision 是 append-only；未定义“完整落地”前不改。
4. **真实注册表未测**：本轮所有测试仍用 TempDir 与 mock IPC；未写真实注册表。
5. **CI 未跑**：未提交/推送，未触发 GitHub Actions。
6. **版本号已升到 7.0.1**（用户明确指令）：`package.json` / `package-lock.json` 根版本 / `Cargo.toml` / `Cargo.lock` 三个 workspace crate / `tauri.conf.json` / README 徽章与安装包名 / AGENTS/CLAUDE 当前版本 / CHANGELOG 顶部段落均已同步；`check-consistency` 通过。发布前仍需用户授权提交、推送与打 tag。

---

## 5. 残余风险

- `R-01 [非阻塞]`：`import_bundle` 拒绝 header 等于本机 deviceId 的包；若用户把同一设备的旧包用于灾难恢复，会被拒绝。当前同步语义是跨设备，不接受自导入；需要恢复时应直接恢复数据库文件。
- `R-02 [非阻塞]`：schema 4/5/6 库全部拒绝是既定策略；升级用户必须备份后重建，历史/同步不会自动迁移。已通过 CHANGELOG、README、GUI 条幅提示。
- `R-03 [已修复]`：`ProfileDetail` 描述行改为仅在任一侧非空时显示。
- `R-04 [非阻塞]`：`next_seq` 采用 `MAX(seq)+1`；若数据库已被外部工具写入 `i64::MAX - 1` 的本机序号，仍会在下一次写入时显式报错（不会静默错误），错误文案已提示检查 `changes` 表中本机 `device_id` 的超大 `seq`。
- `R-05 [非阻塞]`：`npm`/`npx` shim 在本机 shell 异常；本轮用底层二进制执行等价命令，CI 不受影响。

---

## 6. 回执判定

**整改完成，建议复核。** P1 全部关闭；第二轮 P2-1/P2-2 已修复；“规则应用审计”按报告要求留待产品确认；P3 中的描述行视觉噪声已顺手修复，`snapshotUid = null` 仍待确认。质量门全绿，但上述未覆盖项与残余风险不应被“绿灯”掩盖。

本轮已按用户指令升级版本到 7.0.1；未提交、未推送、未打 tag、未写真实注册表。
