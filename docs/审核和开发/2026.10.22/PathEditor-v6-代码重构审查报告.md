# PathEditor v6 代码重构审查报告

- **审查日期**：2026-10-22
- **被审分支 / 提交**：`main` @ `30bfb75`（v6.0.0 tag `2899462` 之后 1 个文档提交）
- **审查范围**：全仓库源码（`core/`、`gui/`、`cli/`、`src/`、`tests/`、`e2e/`、构建与 CI 配置）
- **代码规模**：132 个 Rust 文件 / 27,448 行；117 个 TS/TSX 文件 / 16,763 行；500 个受版本控制文件
- **审查方式**：静态审查 + 行数/注释度量 + 关键调用链逐行追踪 + `cargo fmt --check` / `npm run format:check`
- **规范来源**：`CLAUDE.md` / `AGENTS.md`、`docs/dev/*`、`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`
- **边界声明**：本轮只出报告，**未改任何代码**；未启动 GUI、未执行真实注册表写入、未运行 CLI 跨进程联调。所有涉及运行时行为的结论均标注证据链，并在「未覆盖项」中如实说明验证缺口。

---

## 〇、执行摘要

### 结论先说

1. **这不是一座 C/IUP 时代遗留的屎山。** C 与 IUP 的实现早已被替换；当前目录是标准的 Cargo workspace + Tauri 布局。真正的残留是 **v5 → v6 过渡层**：两套配置通路、两套状态源、两套错误模型，以及开发里程碑（M0–M4、Wave、Task、F/J/D/S/R/N 编号）直接写进了生产代码注释。
2. **代码量本身不臃肿。** 19,328 行 Rust 生产代码支撑 v6 的注册表、加密库、同步、检索、统计与 GUI/CLI 双入口，属于合理规模。问题不在「行数多」，而在 **概念层数多**：同一件事（保存 PATH）在 core、GUI 命令层、CLI 运行时、前端 store 四处各有一层编排。
3. **发现 4 个 P0 / 6 个 P1 逻辑问题。** 最严重的是 **CLI 的 PATH 写入与 v6 数据库彻底脱节**：CLI 既不更新 `app_state.current_snapshot_id`，也不写 `changes`，还把「快照已保存」当成成功返回；而 GUI 在 v5 停写后只认数据库。两套入口对「当前 PATH 是什么」会给出不同答案。
4. **目录迁移（`src/{core,cli,gui}`）可以做，但不应该现在做。** 它只改路径、不改行为，风险却横跨 Cargo、Tauri CLI、Vite、tsconfig、CI、发布流水线与 137 份文档。建议把它放在逻辑修复与注释清理之后，单独立项、单独验证。
5. **注释清理是低风险、高收益，可以立刻开始。** Rust 生产代码 19,328 行里有 3,975 行注释（20.6%），其中 370 行包含里程碑或审查编号；前端 `src/` 还有 61 行。清理的目标不是「少写注释」，而是让每条注释只回答「这段代码做什么、为什么必须这样做」。

### 判定

| 维度       | 判定               | 说明                                                        |
| ---------- | ------------------ | ----------------------------------------------------------- |
| 可维护性   | **需要整改**       | 注释考古层、死代码、超大文件、双通路                        |
| 逻辑正确性 | **需要整改（P0）** | CLI/DB 状态分裂、事件结果分类、v5 apply 陷阱                |
| 安全性     | **基本合格**       | `unsafe` 全部有 `SAFETY`；CSP 未放松；导入路径有 scope 校验 |
| 测试       | **覆盖广但有盲区** | 350 Rust / 398 前端 / 54 E2E，但没有 CLI→GUI 跨入口回归     |
| 目录结构   | **可优化，非缺陷** | 当前布局是合法 Tauri 约定；`src/` 整合是偏好，不是屎山      |

---

## 一、度量基线

### 1.1 代码与注释

| 指标                              |           Rust |   TypeScript / TSX |
| --------------------------------- | -------------: | -----------------: |
| 受版本控制文件                    |            132 |                117 |
| 总行数                            |         27,448 |             16,763 |
| 生产代码行（`#[cfg(test)]` 之前） |         19,328 | 8,722（仅 `src/`） |
| 测试代码行（含 `tests/`、`e2e/`） |          8,120 |              8,041 |
| 生产代码注释行                    | 3,975（20.6%） |       941（10.8%） |
| 含里程碑/审查编号的注释行         |    370（1.9%） |                 61 |

> 注释密度 20.6% 本身不是问题。PathEditor 有大量「为什么不能这样做」的约束（SQLCipher 密钥顺序、TOCTOU、墓碑语义、一文件一测试），这些必须写。问题在于其中相当一部分写的是「这是 M3 Task 7 改的」，而不是「这段代码保证什么」。

### 1.2 生产代码超过 300 行的文件

|  行数 | 文件                          | 判断                                                                      |
| ----: | ----------------------------- | ------------------------------------------------------------------------- |
| 1,442 | `core/src/backup.rs`          | **必须拆分**：配置解析、env 备份、PATH 备份、轮换、恢复、路径校验混在一起 |
|   705 | `core/src/db/history.rs`      | 可接受；快照/事件/reveal 同居一个领域，拆分收益有限                       |
|   699 | `cli/src/main.rs`             | **应拆分**：clap 定义 + 分派 + 6 个命令实现                               |
|   692 | `cli/src/env_ops.rs`          | **应拆分**：list/get/set/add/remove/backup/restore 七个子命令             |
|   602 | `core/src/db/profiles.rs`     | 可接受；保存/查询/改名/墓碑同居                                           |
|   536 | `core/src/service.rs`         | 偏大，但它是唯一保存编排，先保持完整更利于审计                            |
|   499 | `core/src/db/sync/import.rs`  | 可接受；单事务导入本就应集中                                              |
|   479 | `core/src/db/backups.rs`      | 可接受；`.pebak` 头解析与索引                                             |
|   382 | `gui/src/lib.rs`              | 可拆分启动序列到 `setup.rs`                                               |
|   378 | `gui/src/commands/history.rs` | **必须拆分**：配置方案命令不应住在 history 里                             |
|   310 | `core/src/fs.rs`              | 可接受；导入导出解析                                                      |

> `core/src/registry/golden_tests.rs`（478 行）与 `golden/merge_golden_tests.rs` 是 `#[cfg(test)]` 模块，不计入生产代码。

### 1.3 注释考古层计数

同一行可能命中多个模式，以下为命中行数（Rust + TS 生产代码）：

| 模式                  | 行数 | 典型含义         |
| --------------------- | ---: | ---------------- |
| `M0`–`M4`             |  163 | 里程碑编号       |
| `spec §`              |  130 | 设计文档章节引用 |
| `F-xx`                |   86 | 审查发现编号     |
| `Task N`              |   84 | 实施计划任务号   |
| `P0-x` / `P1-x`       |   66 | 优先级/欠账编号  |
| `Wave`                |   36 | 开发波次         |
| `N-x`                 |   36 | 复审观察项       |
| `D-x`                 |   16 | 核对轮裁断编号   |
| `J-x`                 |   14 | 复审发现编号     |
| `欠账`                |   11 | 未完成事项       |
| `T-x`                 |   10 | 技术债编号       |
| `S-x` / `R-x` / `C-x` |   13 | 核对轮/复审编号  |

这些编号对当时的开发窗口是有用的，但它们不应该出现在 v6.0.0 的最终源码里——它们描述的是「谁在哪一轮改的」，而不是「这段代码现在保证什么」。

---

## 二、注释考古层（用户问题 1）

### 2.1 现象

最典型的几处：

```rust
// core/src/db/mod.rs:41
/// 打开序列与 M0 探针（`probes/v6-sqlcipher/src/lib.rs`）已验证的顺序一致：

// core/src/db/mod.rs:108
// J-7：建库即写 app_state 单行。否则 `set_current_snapshot` 的
// CAS 永远匹配 0 行、恒返回 false（spec §十二 的 CAS 验收不可达）。

// core/src/db/profiles.rs:133
// ── M3 S-1 接线 ──

// gui/src/commands/m2.rs:1
//! M2 的 Tauri 命令：检索 / 统计 / 对比 / 引用图 / 冲突历史。
```

这四类注释分别对应：**开发工具链**、**审查工单**、**里程碑**、**模块命名**。它们共同的效果是：一个 2026 年之后接手的人，必须先去读 `docs/审核和开发/` 的十几份报告，才能知道 `M3 S-1` 是什么。

### 2.2 分类与处置

| 类别                 | 例子                                                       | 处置                                                           |
| -------------------- | ---------------------------------------------------------- | -------------------------------------------------------------- |
| **A. 里程碑 / 波次** | `M2 的 Tauri 命令`、`Wave 3.1`、`M4 起`                    | 删除编号，保留行为描述                                         |
| **B. 审查工单号**    | `J-7`、`F-04`、`S-1`、`R-1`、`N-8`                         | 删除编号；把「这条约束为什么存在」改写成一句不依赖编号的规则   |
| **C. 计划 Task 号**  | `Task 5 复审 Minor`、`Task 7 证词`                         | 删除；历史记录留在 `docs/`                                     |
| **D. 历史叙事**      | `此前这里写成…`、`Task 6 全面迁移后已是死代码`、`欠账`     | 删除；如果代码真的死了，删代码，不要留注释                     |
| **E. spec 章节引用** | `spec §7.8 H7`                                             | 每个模块头保留一行「设计依据：`docs/...`」，正文内不再逐句引用 |
| **F. 过期/错误注释** | `Sync 仍映射为 1`（实际是 4）、`环境变量对比将在 M3 提供`  | **必须修**；这类注释会主动误导                                 |
| **G. 与代码重复**    | `// 写入`、`// 读取`、`// 返回结果`                        | 删除                                                           |
| **H. 有价值注释**    | `// SAFETY:`、TOCTOU、墓碑、一文件一测试、错误码不匹配文案 | **保留并加强**                                                 |

### 2.3 必须先修的过期注释（P0）

这些不是风格问题，是**错误信息**：

| 位置                                         | 注释说的                                                     | 代码实际做的                            | 影响                                                 |
| -------------------------------------------- | ------------------------------------------------------------ | --------------------------------------- | ---------------------------------------------------- |
| `core/src/error.rs:94`                       | `spec §8：4 预留给同步/合并冲突（M3），当前 Sync 仍映射为 1` | `:100` `ErrorCode::Sync => 4`           | 读者会以为退出码 4 未生效，可能改错 CLI 契约         |
| `core/src/db/compare.rs:81`                  | `T-3：本轮恒为空，UI 必须显示「环境变量对比将在 M3 提供」`   | M3/M4 已发布，字段仍恒为空              | 用户看到的是「将在 M3 提供」——一个永远不会兑现的承诺 |
| `src/components/history/HistoryPanel.tsx:16` | `恢复操作与筛选留到 M2`                                      | 恢复与筛选早已实现                      | 让维护者误判功能边界                                 |
| `src/components/history/HistoryPanel.tsx:90` | `target 与时间范围留到 M2 的筛选面板`                        | 当前筛选已支持全部维度                  | 同上                                                 |
| `core/src/env_var.rs:12-13`                  | 保留 `Path` 是为了不绕过 `_pendingSys` / `_pendingUser`      | 这两个字段在前端已恒为 `null`，是死状态 | 安全理由引用了不存在的机制                           |
| `src/services/backend.ts:235`                | `双形状兼容（过渡期，Wave 2 收口时复核是否收紧）`            | Wave 2 早已结束，兼容层仍在             | 过渡期没有终点                                       |
| `README.md:13`                               | `tests-582 passed`                                           | Rust 350 + 前端 398 = 748（E2E 另 54）  | 徽章数字失真                                         |
| `README.md:14`                               | coverage 徽章指向 `branch/v5.1`                              | 当前是 v6.0.0                           | 覆盖率徽章长期不更新                                 |

> **建议**：把「过期注释扫描」做成一次独立提交，不和重构混在一起。每一处都附上「注释原文 → 实际行为 → 新注释」三行对照，方便复核。

### 2.4 注释规范（建议写入 `CONTRIBUTING.md`）

1. **注释只回答两个问题**：这段代码做什么？为什么必须这样做（而不是另一种做法）？
2. **禁止出现开发过程标识**：`M0`–`M4`、`Wave`、`Task N`、`F/J/D/S/R/N/P/T` 编号、「本轮」「此前」「欠账」「复审」。
3. **模块头固定三段**：职责、边界（不做什么）、设计依据（一条文档链接）。不写变更史。
4. **公开函数用 `///` 写契约**：参数语义、返回值、错误码、副作用（是否写注册表 / 是否落盘 / 是否广播）。不写「Task 7 实现」。
5. **行内注释只写非显而易见的 `why`**：并发窗口、Windows 注册表怪癖、SQLCipher 顺序、加密 AAD 覆盖范围。`// 写入`、`// 读取` 一律删除。
6. **`unsafe` 必须有 `// SAFETY:`**，且说明的是「为什么这些前置条件成立」，不是「这里调用了 Win32」。
7. **安全判定注释不得引用已删除的机制**。例如 `_pendingSys` 已死，就不能再拿它当保留 `Path` 的理由。
8. **历史决策去 `docs/dev/decisions/` 或 CHANGELOG**，源码里只留一句结论。

### 2.5 清理步骤与验收

1. **先建基线**：用脚本统计当前 370 + 61 处标记，生成 `docs/dev/comment-archaeology-baseline.md`（只列位置，不改代码）。
2. **按文件清理**：优先 `core/src/service.rs`、`core/src/backup.rs`、`core/src/db/*`、`gui/src/lib.rs`、`gui/src/commands/*`、`src/services/backend.ts`。
3. **每清一个文件，跑一次 `cargo fmt --check` / `npm run format:check`**，确保只动注释。
4. **加一个软门禁**：`scripts/check-comment-archaeology.mjs` 扫描生产代码中的上述模式，命中即失败；先在 CI 里设为 `continue-on-error`，清理完成后再转硬门禁。
5. **验收标准**：`core/`、`gui/`、`cli/`、`src/` 生产代码中上述模式命中数为 **0**；`docs/` 不受限；测试文件允许保留「这条测试防的是什么」的行为描述，但不允许只写编号。

> **不要做的事**：不要用正则一次性删除所有含 `M[0-4]` 的注释。有些注释同时承载约束，例如 `// M0 实测 ON 在受限进程里 VirtualLock 失败`，删掉编号后要保留「为什么默认 OFF」。机械替换会把有效信息一起删掉。

---

## 三、工程结构与目录（用户问题 2）

### 3.1 当前布局是什么

```text
PathEditor/
├── core/        # path-editor-core（Rust 库）
├── gui/         # patheditor（Tauri Rust bin）+ tauri.conf.json
├── cli/         # patheditor-cli（Rust bin）
├── src/         # React 前端（Vite root）
├── tests/       # 前端单测
├── e2e/         # Playwright
├── index.html   # 前端入口
├── vite.config.ts
├── package.json # 根目录，前端依赖与脚本
└── Cargo.toml   # workspace
```

这是一个**合法的 Cargo workspace + Tauri 2 布局**。`npx tauri info` 能正确识别 `gui/tauri.conf.json` 的 `frontendDist: ../dist`，说明 Tauri CLI 当前能定位配置（本轮已实际执行 `npx tauri info` 验证）。

### 3.2 真正的「没整合到一起」在哪里

用户说的「core、cli、gui 没整合到一起」，在代码层面表现为：

| 现象                          | 证据                                                                                   | 影响                                                         |
| ----------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| GUI 命令层承载业务编排        | `gui/src/commands/service.rs` 271 行，含快照创建、`current_snapshot` CAS、FTS 增量索引 | 违反 `CLAUDE.md`「gui/cli 只做参数转换、命令分派、错误呈现」 |
| CLI 与 GUI 各有一套保存后处理 | GUI 在 `commands/service.rs` 更新 DB；CLI 在 `runtime.rs` 只调 core，不碰 DB           | 两入口状态不一致（详见 L-1/L-2）                             |
| 配置方案有两套实现            | `core/src/profiles.rs`（v5 文件）与 `core/src/db/profiles.rs`（DB）                    | `service::apply_profile` 仍读 v5 文件，是活陷阱              |
| 历史/备份/配置命令混居        | `gui/src/commands/history.rs` 同时含 `history_*`、`profile_*`、`backup_list`           | 模块名与职责不符，找代码要靠 grep                            |
| 里程碑成为模块名              | `gui/src/commands/m2.rs`                                                               | 唯一一个以开发阶段命名的生产模块                             |

> 这些是**模块边界问题**，不是目录层级问题。即使把目录改成 `src/core`、`src/cli`、`src/gui`，只要上述边界不改，代码仍然「没整合」。

### 3.3 用户目标布局的可行性

目标：

```text
PathEditor/
├── src/
│   ├── core/                 # path-editor-core
│   ├── cli/                  # patheditor-cli
│   └── gui/
│       ├── src-tauri/        # patheditor（Tauri Rust）
│       └── web/              # React（原 src/、index.html、vite.config.ts、tsconfig*）
├── Cargo.toml
├── package.json
└── ...
```

可行性逐项：

| 系统            | 影响                                                                                                         | 风险                                                                  |
| --------------- | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------- |
| Cargo workspace | `members = ["src/core","src/cli","src/gui/src-tauri"]`；包名不变，`cargo test -p ...` 不变                   | 低；`git mv` 保留历史                                                 |
| Tauri CLI       | 默认约定是 `src-tauri/`；`src/gui/src-tauri` 符合约定，但**从仓库根运行 `npx tauri` 是否会自动定位**必须实测 | **中**；本轮未迁移，不能假设                                          |
| Tauri 配置      | `frontendDist` 改为 `../web/dist`；`beforeBuildCommand` 需要 `npm --prefix ../.. run build` 或等价写法       | 中；命令执行目录需实测                                                |
| Vite            | root 改为 `src/gui/web`；`dist`、`public`、`index.html`、alias `@` 全部随之移动                              | 中；`vite.config.ts`、`tsconfig*`、ESLint/Prettier/Vitest glob 都要改 |
| CI / release    | `.github/workflows/*.yml` 中 `gui/tauri.conf.json`、`target/...`、`cargo build --path` 等路径全部要改        | **高**；漏一处就是发布失败                                            |
| 文档            | 137 份 `docs/` 中大量 `core/src/...`、`gui/...` 路径引用                                                     | 中；文档漂移比代码更难发现                                            |
| Git 历史        | 用 `git mv` 可保留 rename 检测；跨目录移动可能降低 `git log --follow` 效果                                   | 低                                                                    |

### 3.4 两个方案

**方案 A（用户目标，推荐作为最终形态）**

```text
src/
├── core/
├── cli/
└── gui/
    ├── src-tauri/   # Rust + tauri.conf.json
    └── web/         # React + Vite
```

优点：符合用户「一个 src 装下所有源码」的直觉；Rust 与前端各自有清晰子目录。
缺点：`src/` 在 Tauri 社区惯例里通常指前端源码，把 Rust crate 放进去会让第一次接触的人短暂困惑；Tauri 配置发现路径需要实测。

**方案 B（低风险替代，供比较）**

```text
crates/
├── core/
└── cli/
apps/
└── gui/
    ├── src-tauri/
    └── web/
```

优点：Rust 与 Tauri 社区惯例一致；CI 路径改动少；`cargo` 与 `tauri` 工具链零意外。
缺点：不是用户明确要的 `src/`。

**我的建议**：如果用户坚持 `src/`，采用方案 A，但**必须先做一个 30 分钟的 spike**：在临时分支上只移动 `gui`（不动 `core`/`cli`），跑通 `npx tauri info`、`npm run build`、`npx tauri build --debug`，确认 Tauri 的配置发现与 `beforeBuildCommand` 执行目录。spike 不通过就不要继续移动 `core`/`cli`。

### 3.5 迁移步骤（若采用方案 A）

1. **Step 0：冻结功能**。迁移分支只做路径变更，不合入任何逻辑修复。
2. **Step 1：移动前端**。`git mv src src/gui/web/src`、`git mv index.html src/gui/web/`、`git mv public src/gui/web/`、`git mv vite.config.ts src/gui/web/`、`git mv tsconfig*.json src/gui/web/`、`git mv tests src/gui/web/tests`、`git mv e2e src/gui/web/e2e`。更新 `package.json` 脚本为 `vite --config src/gui/web/vite.config.ts` 或把 `package.json` 也移入 `src/gui/web`（推荐后者，保持 npm 项目自包含）。
3. **Step 2：移动 Tauri Rust**。`git mv gui src/gui/src-tauri`。更新 `tauri.conf.json` 的 `frontendDist: "../web/dist"`、`beforeBuildCommand`、`beforeDevCommand`。
4. **Step 3：移动 core/cli**。`git mv core src/core`、`git mv cli src/cli`。更新 `Cargo.toml` members。
5. **Step 4：更新所有引用**。CI、release workflow、`README`、`docs/dev/*`、`cargo install --path` 文档、`.gitignore`（`target/`、`dist/`、`coverage/` 路径）。
6. **Step 5：全量验证**。`cargo metadata`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`npm run build`、`npm test`、`npm run test:e2e`、`npx tauri build --debug`。任何一步失败，回滚到 Step 4 之前。

**不要做的事**：不要在同一个提交里同时移动目录和改逻辑。一旦出问题，无法判断是路径还是代码导致的。

---

## 四、结构精简（用户问题 3.1）

### 4.1 判断：代码量不需要「瘦身」，概念层需要

19,328 行 Rust 生产代码不算多。真正可以删/并/拆的是以下 15 项。按收益排序：

| 编号 | 位置                                                                             | 现状                                                                                                                                                                                                                                                            | 动作                                                                                                                    |     预计影响 | 风险 |
| ---- | -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | -----------: | ---- |
| S-1  | `gui/src/commands/profiles.rs`                                                   | 80 行死模块，`#![allow(dead_code)]`，命令未注册                                                                                                                                                                                                                 | **删除**（需用户书面同意）；v5 读逻辑留在 core                                                                          |       −80 行 | 低   |
| S-2  | `src/store/app-store.ts`、`src/services/path-session.ts`                         | `_pendingSys` / `_pendingUser` 恒为 `null`，但参与 `markDirty`、undo、redo、save                                                                                                                                                                                | **删除字段与全部管道**；`SaveSnapshotOutcome` 去掉两个字段                                                              |       −60 行 | 低   |
| S-3  | `gui/src/lib.rs` invoke_handler                                                  | 注册了前端从未调用的 IPC：`apply_profile`、`apply_path_snapshot`、`retry_pending_path_state`、`save_disabled_state`、`load_disabled_state`、`save_path_snapshot`、`profile_export_v5`、`stats_rebuild`、`refs_rebuild`、`search_rebuild`、`scan_history_record` | **从 `invoke_handler` 移除**；core 函数保留给 CLI/调试/测试                                                             |  缩小 IPC 面 | 低   |
| S-4  | `gui/src/commands/m2.rs`                                                         | 以里程碑命名的 6 个命令                                                                                                                                                                                                                                         | **改名 `analytics.rs`**，或按域拆成 `search.rs` / `stats.rs` / `refs.rs` / `scan_history.rs`                            |         命名 | 低   |
| S-5  | `gui/src/commands/history.rs`                                                    | 378 行，含 `profile_*`、`backup_list`                                                                                                                                                                                                                           | **拆成 `history.rs` / `profiles.rs` / `backups.rs`**；与 S-1 合并                                                       |         职责 | 低   |
| S-6  | `core/src/backup.rs`                                                             | 1,442 行，6 类职责                                                                                                                                                                                                                                              | **拆成 `backup/{mod,config,env,path,restore}.rs`**                                                                      |       可读性 | 中   |
| S-7  | `core/src/registry/env_var.rs`                                                   | 1,753 行；normal 与 force 两套写路径约 200 行重复                                                                                                                                                                                                               | **合并为 `write_env_var_with_policy(store, hive, name, value, policy)`**，`policy` 为 `RequireRevision(&str)` / `Force` |  −100~150 行 | 中   |
| S-8  | `cli/src/main.rs`                                                                | 699 行：clap 定义 + 分派 + 6 个 `cmd_*`                                                                                                                                                                                                                         | **拆成 `cli.rs`（clap） + `path_ops.rs`（命令实现）**                                                                   |       可读性 | 低   |
| S-9  | `cli/src/env_ops.rs`                                                             | 692 行，7 个子命令 + 打印                                                                                                                                                                                                                                       | **拆成 `env/{list,get,set,backup,restore}.rs`**，或至少把打印抽到 `output.rs`                                           |       可读性 | 低   |
| S-10 | `src/services/backend.ts`                                                        | 1,643 行，59 个 invoke + 约 1,000 行运行时校验                                                                                                                                                                                                                  | **拆成 `backend/{paths,env,history,analytics,sync,profiles}.ts` + 一个 barrel**；共享 `parseCoreError` / 形状校验工具   |       可读性 | 中   |
| S-11 | `core/src/reg_store.rs`                                                          | 414 行；只有 core 内部与测试使用，却在 `lib.rs` 公开                                                                                                                                                                                                            | **移到 `core/src/registry/store.rs`，改为 `pub(crate)`**；测试通过 `registry::store` 访问                               |   缩小公开面 | 低   |
| S-12 | `core/src/profiles.rs` + `core/src/db/v5_export.rs` + `core/src/db/v5_import.rs` | v5 兼容孤岛，命名与 DB 版冲突                                                                                                                                                                                                                                   | **归入 `core/src/legacy_v5/{profiles,export,import}.rs`**；`service::apply_profile` 要么删除，要么改读 DB               | 消除同名歧义 | 中   |
| S-13 | `cli/src/runtime.rs:12-26`                                                       | `exit_persist_error` 与 `exit_core_error` 实现完全相同                                                                                                                                                                                                          | **合并为一个 `exit_core_error`**；`exit_err` 改为构造 `CoreError` 后调用它                                              |       −10 行 | 低   |
| S-14 | `cli/src/env_ops.rs`、`profile_ops.rs`、`scan_ops.rs`、`main.rs`                 | 8 处 `serde_json::to_string_pretty(...).unwrap()`                                                                                                                                                                                                               | **抽 `cli/src/output.rs::print_json<T: Serialize>`**，失败打印错误并以 1 退出                                           |   消除 panic | 低   |
| S-15 | `core/src/disabled.rs`、`core/src/service.rs` 测试                               | 固定 `temp_dir` 路径 + `test_persist_lock` 26 处引用                                                                                                                                                                                                            | **注入 `TempDir` 或 `PathStateStore`**，取消全局测试锁                                                                  | 消除测试竞态 | 中   |

### 4.2 可以合并的模块（不是必须）

| 现状                                                                            | 建议                                                                           | 理由                                     |
| ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ | ---------------------------------------- |
| `core/src/env_var.rs`（契约/安全判定）与 `core/src/registry/env_var.rs`（CRUD） | 合并为 `core/src/env/{model,validate,read,write}.rs`                           | 两个同名 `env_var` 模块是导航负担        |
| `core/src/history.rs`（trait/事件）与 `core/src/db/history.rs`（实现）          | 保留分层，但把 trait 文件改名为 `history/events.rs`                            | 同名不是错，但要让人一眼看出是端口与实现 |
| `core/src/backup.rs`（文件备份）与 `core/src/db/backups.rs`（备份索引）         | 保留分离，改名 `backup/files.rs` 与 `db/backup_index.rs`                       | 两者不是同一件事，合并会制造更大文件     |
| `core/src/lib.rs` 的 16 个 `pub mod` + 1 个 `pub(crate) mod`                    | 只保留 GUI/CLI 真正使用的 10 个；其余 `pub(crate)` 或经 `registry`/`db` 重导出 | 公开面越大，未来重构越难                 |

### 4.3 不建议做的「精简」

- **不要为了行数合并 `core/src/db/*` 的小模块。** `schema.rs`、`compare.rs`、`refs.rs` 各自职责清晰，合并只会制造一个 2,000 行的 `db.rs`。
- **不要删除 v5 导入/导出。** 它是降级与迁移的退路，属于产品能力，不是死代码。要删的是 `apply_profile` 这种「读 v5 文件但对外声称应用配置」的活陷阱。
- **不要把 `tests/` 内联进源码。** 当前「一文件一测试」是 SQLCipher 的硬约束，内联会让测试文件失去独立进程隔离。

---

## 五、逻辑审查（用户问题 3.2）

> 本节按严重度排序。P0 = 会导致用户状态错误或数据不一致；P1 = 设计缺陷或明确违反仓库约束；P2 = 可维护性/性能问题。

### L-1【P0】CLI 的 PATH 写入与 v6 数据库彻底脱节

**证据链**

1. CLI 的所有 PATH 写入口最终调用 `cli/src/runtime.rs:76-99` 的 `save_path_write`，它直接调用 `core::service::save_path_with_sidecar`，传入 `&core::history::NoopHistory`（`:81`）。
2. core 的 `commit_sidecar`（`core/src/service.rs:251-298`）调用 `disabled::save_path_snapshot`。
3. `disabled::save_path_snapshot`（`core/src/disabled.rs:265-279`）调用 `write_state`；`write_state` 在 `v5_writes_disabled()` 为真时**直接返回 `Ok(())`，不落任何盘**（`:139-145`）。
4. CLI 进程从未调用 `set_v5_writes_disabled(true)`，所以它实际会写 `disabled.json`；但 GUI 进程在启动时已把它归档为 `.v5.bak` 并只读 DB。
5. CLI 从未更新 `app_state.current_snapshot_id`；全仓库唯一更新点是 `gui/src/commands/service.rs:111-161`。
6. `gui/src/lib.rs:130-133` 自己承认：「CLI 的写路径用 `NoopHistory`，不碰 DB 的当前快照」，但把它归类为「引用图/统计仍旧」的已知风险——实际影响远大于派生表。

**影响**

- CLI `add/remove/edit/move/clean/enable/disable/import/profile apply` 之后，DB 的当前快照仍是旧的。
- CLI 返回的 `SidecarOutcome::Saved` 是**假成功**：core 以为快照已保存，实际什么都没写。
- GUI 重启后读 DB 旧快照，与真实注册表不一致；被 CLI 禁用的条目在 GUI 里表现为「消失」，而不是「已禁用」。
- 跨设备同步看不到 CLI 的 PATH 修改（CLI 不产生 `changes` 行，见 L-7）。

**建议**

把「保存 PATH 后的 DB 后处理」从 GUI 命令层下沉到 core，并让 GUI/CLI 共用：

```rust
// core/src/service.rs（示意）
pub trait PathStateStore {
    fn load_current(&self) -> Result<Option<PathSnapshot>, CoreError>;
    fn save_current(&mut self, system: Option<Vec<PathEntry>>, user: Option<Vec<PathEntry>>)
        -> Result<(), CoreError>;
}

pub fn save_path_with_state(
    state: &mut dyn PathStateStore,
    history: &dyn HistorySink,
    source: EventSource,
    system: Option<PathHiveWrite>,
    user: Option<PathHiveWrite>,
) -> Result<ApplyOutcome, CoreError>;
```

- GUI 注入 `DbPathStateStore`；CLI 在 DB 可用时注入同一个 store，DB 不可用时注入 `V5FileStateStore`。
- `disabled.rs` 退回纯 v5 文件模块，删除全局 `V5_WRITES_DISABLED` 与 `CURRENT_SNAPSHOT_LOADER`。
- `save_path_with_state` 内部负责：写前备份 → 并发校验 → 写注册表 → 广播 → **更新 current 快照** → 记录事件/变更。GUI 与 CLI 都只做参数转换。

**验证**

新增集成测试：打开临时 DB → 调 `save_path_with_state`（fake 注册表 IO）→ 再读 `current_snapshot`，断言两边一致；然后用 `disabled::load_path_snapshot`（注入 DB loader）断言读到的启用/禁用/顺序与写入一致。该测试不需要真实注册表。

---

### L-2【P0】CLI 读不到 DB 的禁用状态，`disabled.json` 归档后索引会错位

**证据链**

1. `disabled::load_path_snapshot`（`core/src/disabled.rs:282-309`）在 `v5_writes_disabled()` 为假时读文件；为真时读注入 loader，取不到再回落文件。
2. CLI 进程从未设置该标志，也从未注入 loader（全仓库 `set_current_snapshot_loader` 只在 `gui/src/lib.rs:112` 调用）。
3. GUI 启动末尾把 `disabled.json` 改名为 `disabled.json.v5.bak`（`gui/src/lib.rs:146-153`，`core/src/db/archive_v5.rs:43-83`）。
4. 因此在 GUI 迁移并归档之后，CLI 的 `load_path_snapshot` 读到的是**不存在的 `disabled.json`** → `DisabledState::default()` → 所有禁用条目消失。
5. CLI `cmd_list`（`cli/src/main.rs:354-378`）直接展示这份快照；`cmd_toggle`（`:552-575`）用这份快照的下标定位条目。

**影响**

- CLI 的索引与 GUI 的索引不一致：用户在 GUI 看到「第 5 条禁用」，在 CLI 里第 5 条可能是另一条。
- CLI `disable` 会基于「没有禁用项」的快照重建 `disabled.json`，覆盖掉迁移前的状态（虽然原文件已归档，但新文件又会被下一次 `archive_v5` 跳过，形成孤儿）。
- 这是 v6「注册表是唯一真相源、DB 是 sidecar」设计的直接破坏：CLI 读到的是两份都不完整的视图。

**建议**

- 与 L-1 同一套 `PathStateStore` 修复：CLI 启动时尝试打开 DB，成功则注入 `DbPathStateStore`；失败才退回文件。
- 在 `ensure_current_snapshot_baseline` 之外增加 **reconcile**：启动时比较 DB 当前快照的启用路径与注册表；不一致就按 `merge_hive` 语义重建当前快照，而不是只在「没有 current」时建。
- 归档 `disabled.json` 之前，先确认 DB 里已有可用 current 快照（现有 S-2 已保证顺序，但没有校验内容一致性）。

**验证**

构造临时目录：写入 v5 `disabled.json`（含一条禁用）→ 迁移 → 归档 → 用 CLI 路径读快照，断言禁用条目仍在。当前测试 `core/tests/db_v5_read_from_db.rs` 只测了注入 loader 的正向路径，没有测「CLI 没有 loader」的负向路径。

---

### L-3【P0】GUI 的进程级快照缓存跨进程陈旧，重启也不能自愈

**证据链**

1. `gui/src/lib.rs:105-116` 在启动时把 `current_snapshot` 读进 `Arc<RwLock<Option<PathPair>>>`，并注册为 `disabled` 的 loader。
2. 只有 GUI 自己的写路径会刷新它：`gui/src/commands/service.rs:157-160` → `crate::refresh_read_snapshot`。
3. CLI 或其他进程修改 DB 后，GUI 的缓存不会更新；`load_path_snapshot` 继续返回旧值。
4. 更糟的是：由于 L-1，CLI 根本不更新 DB，所以 GUI 重启后读到的仍是旧值——**重启不是解药**。

**影响**

- 用户在 GUI 打开时用 CLI 改 PATH，GUI 的重新加载（状态栏重试 / 重启）看不到变化。
- 如果 GUI 之后保存，它会用旧快照 + 旧 `_savedSys` 做并发校验，可能拒绝保存或覆盖 CLI 的修改。

**建议**

- 删除进程级缓存，改为每次 `load_path_snapshot` 直接查 DB（SQLite 读很快，索引在 `app_state` 单行）。
- 如果必须缓存，加一个 `Db` 变更版本号（`app_state.version`）并在每次读时比对；版本变了就重读。
- 长期方案仍是 L-1 的 `PathStateStore`：GUI 不再自己维护缓存，store 负责一致性。

**验证**

集成测试：同一个 `Db` 句柄，先 `save_path_with_state` 写一次，再通过另一个连接/另一个 store 写一次，断言 `load_path_snapshot` 读到第二次的结果。

---

### L-4【P0】事件结果分类错误：两个 hive 全失败也记为 `Partial`

**位置**：`core/src/service.rs:345-348`

```rust
result: match (&outcome.system, &outcome.user) {
    (HiveOutcome::Failed(_), _) | (_, HiveOutcome::Failed(_)) => EventResult::Partial,
    _ => EventResult::Success,
},
```

**问题**

- 两个 hive 都 `Failed` → `Partial`，但实际是**完全失败**。
- 一个 `Skipped` + 一个 `Failed` → `Partial`，也语义不明（Skipped 表示没请求写，不是「部分成功」）。
- 两个都 `Skipped`（没有请求写）→ `Success`，会记一条「成功」的空事件。

**影响**

- 历史面板的「部分成功/成功」筛选会误报；用户以为至少有一个 hive 写成功了。
- 事件是同步与审计的输入，错误结果会污染统计。

**建议**

```rust
fn event_result(system: &HiveOutcome, user: &HiveOutcome) -> EventResult {
    let failed = matches!(system, HiveOutcome::Failed(_)) || matches!(user, HiveOutcome::Failed(_));
    let applied = matches!(system, HiveOutcome::Applied) || matches!(user, HiveOutcome::Applied);
    match (applied, failed) {
        (true, true) => EventResult::Partial,
        (false, true) => EventResult::Failure,
        (true, false) => EventResult::Success,
        (false, false) => EventResult::Success, // 或跳过记录
    }
}
```

并补一条表驱动单测：`Applied+Applied`、`Applied+Failed`、`Failed+Failed`、`Skipped+Failed`、`Skipped+Skipped`。

---

### L-5【P1】`service::apply_profile` 仍读 v5 文件，是活陷阱

**位置**：`core/src/service.rs:25`、`:520-534`；注册于 `gui/src/lib.rs:344`。

```rust
pub fn apply_profile(name: &str, history: &dyn HistorySink) -> Result<ApplyOutcome, CoreError> {
    let data = profiles::load_profile(name)...;  // core::profiles，v5 文件版
    save_path_with_sidecar(...)
}
```

**问题**

- v6 的配置方案已经统一到 DB（`core/src/db/profiles.rs`），GUI 前端的「应用配置」实际走 `profile_paths` + `savePaths`，CLI 的 `profile_apply` 走 DB。这个 IPC 命令是**第三条路**，而且读的是已停写的 v5 文件。
- 前端从未 `invoke('apply_profile')`（本轮已 grep 确认），所以它是**未被调用的公开 API**。
- 一旦有人重新注册或调用它，会表现出「配置明明在 GUI 里，apply 却报不存在」。

**建议**

- 删除 `service::apply_profile` 与对应 IPC；如果确实需要「按名字应用配置」，在 `core::db::profiles` 上实现 DB 版本，并由 GUI/CLI 共用。
- 同时删除 `core/src/service.rs:25` 对 `crate::profiles` 的依赖，让 `service` 不再知道 v5 文件。

---

### L-6【P1】保存失败时仍会创建 current 快照并推进 `app_state.version`

**位置**：`gui/src/commands/service.rs:89-101`

```rust
let result = f(&DbHistory::with_snapshot(db, snapshot_uid));

if let Some((system, user)) = snapshot_of_after() {
    update_current_snapshot(db, system, user);
}
result
```

**问题**

- `f` 返回 `Err`（例如参数非法、`expected` 冲突导致两个 hive 都失败）时，仍然会读一次注册表、插入一条 `kind=current` 快照、CAS 更新 `app_state.version`。
- 这会制造无意义的快照与变更行，并让 `current_snapshot_id` 指向「没有发生任何写入」的状态。

**建议**

- 只在 `result.is_ok()` 且至少一个 hive `Applied` 时更新 current 快照。
- 或者把「是否写入」作为 `ApplyOutcome` 的显式字段，由调用方判断。

---

### L-7【P1】CLI 写操作不产生 `changes`，跨设备同步看不到 CLI 的修改

**证据链**

- `insert_snapshot_tx` 会为快照写 `changes`（`core/src/db/history.rs:250-260`、`:360`）。
- CLI 的 PATH 写路径传 `NoopHistory`，从不调用 `insert_snapshot`。
- CLI 的 `env` 写路径直接调 `registry::*`，也不写事件/变更。
- `export` 只导出 `origin='local' AND exported_at IS NULL` 的 `changes`。

**影响**

- 用户在设备 A 用 CLI 改 PATH，导出同步包时看不到这次修改；设备 B 导入后也不会得到它。
- README 说「同步范围不含 events 与备份索引」，但没有说「CLI 的 PATH 修改不同步」——这是文档与行为之间的空白。

**建议**

- 与 L-1 一并修复：CLI 写入后通过 `DbHistory`/`insert_snapshot` 记录快照与变更。
- 如果产品决定「同步只覆盖配置方案，不覆盖命令行 PATH 修改」，必须在 README 与 `docs/dev/sqlite-storage.md` 明确写出，并让 `sync status` 显示「本次导出不含 CLI 修改」。

---

### L-8【P1】GUI 命令层承载业务编排，违反仓库自己的分层约束

**位置**：`gui/src/commands/service.rs` 全文（271 行）。

**问题**

- `CLAUDE.md` 明确要求：`gui` 和 `cli` 只做参数转换、命令分派和错误呈现，业务规则放在 `core`。
- 该文件实际做了：快照创建、`current_snapshot` CAS、FTS 增量索引、读缓存刷新、配置应用计数。这些都是业务规则。
- 直接后果就是 L-1/L-2/L-3：CLI 无法复用这套编排，只能各写一套，最终分裂。

**建议**

- 把 `with_history_and_snapshot`、`update_current_snapshot`、`index_snapshot_by_uid`、`current_path_snapshot` 整体迁入 `core::service`（或 `core::db::save_orchestrator`）。
- GUI 只保留 `State<DbState>` 的解包与错误映射。
- 迁移后 `gui/src/commands/service.rs` 应降到 ~80 行。

---

### L-9【P1】环境变量对比字段恒为空，UI 却承诺「M3 提供」

**位置**：`core/src/db/compare.rs:81-92`、`src/components/history/CompareDialog.tsx:16`、`src/services/backend.ts:825`。

**问题**

- `CompareResult` 仍暴露 `env_added` / `env_removed` / `env_modified`，但恒为 `Vec::new()`。
- 前端文案写「将在 M3 提供」——M3/M4 已发布，v6.0.0 已发版。
- 这不是「未覆盖项」，而是**已发布版本里的空承诺**。

**建议**

- 要么在 v6.1 实现 env 对比（涉及掩码语义，需单独设计）；
- 要么删除三个字段，前端明确显示「本版本不支持环境变量对比」，并把 `CompareResult` 收窄为只含 PATH。后者更诚实。

---

### L-10【P1】双错误模型仍在：`String` 与 `CoreError` 并存

**位置**

- `core/src/reg_store.rs:30-58`：`EnvHiveStore` 的 `enum_names` / `get_raw` / `set_raw` / `delete_value` 返回 `Result<_, String>`，只有 `get_raw_kind` 返回 `CoreError`。
- `core/src/backup.rs:1383`：`backup_registry` 仍返回 `Result<String, String>`。
- `core/src/error.rs:110-115`：`impl From<String> for CoreError` 标注「迁移完成后应移除」，但仍在。
- `core/src/fs.rs`：`read_text_file` / `import_paths` / `export_path_entries` 全部返回 `Result<_, String>`。

**影响**

- 错误码契约（前端/CLI 只认 `code`）在这些路径上退化为字符串匹配。
- `From<String>` 把所有旧错误降级为 `Internal`，丢失了 `NameExists` / `PermissionDenied` 等可判定信息。
- 新增命令时，开发者会照抄最近的 `String` 签名，债务继续扩散。

**建议**

- 分两步：先把 `EnvHiveStore` 与 `backup_registry` 改成 `CoreError`（影响面可控）；再迁移 `fs` 的导入导出。
- 每改一处，补一条「错误码可判定」的测试，禁止再用 `message.contains(...)` 做控制流。
- 全量完成后删除 `From<String> for CoreError`，让编译器强制所有调用方处理结构化错误。

---

### L-11【P2】`EventSource` 被硬编码为 `Gui`

**位置**：`core/src/service.rs:337-339`。

```rust
source: EventSource::Gui,
```

**问题**

- `EventSource` 有 `Cli` 变体，`profile_ops.rs` 的 `profile_save` 也用了 `EventSource::Cli`；但 PATH 保存无论从哪来都记为 `Gui`。
- 现在 CLI 传 `NoopHistory` 所以看不出来；一旦 L-1 修复、CLI 开始写历史，所有 CLI 事件都会被标成 GUI。

**建议**

- `save_path_with_sidecar` 增加 `source: EventSource` 参数（或放进 `PathSaveContext`）。
- GUI 传 `Gui`，CLI 传 `Cli`，迁移/导入传对应值。

---

### L-12【P2】`disabled.rs` 的全局可变状态与测试固定路径

**位置**

- `core/src/disabled.rs:83-100`：`V5_WRITES_DISABLED` 进程级 `AtomicBool`。
- `core/src/disabled.rs:111-123`：`CURRENT_SNAPSHOT_LOADER` 进程级 `OnceLock`。
- `core/src/disabled.rs:20-36`：测试路径固定为 `temp_dir()/patheditor_test_disabled.json` 与 `.../pending_snapshot.json`。
- `core/src/service.rs:675-677` 自认：「pending / disabled.json 在测试下共享固定 temp 路径，存在并行竞态窗口」。
- `crate::persist::test_persist_lock` 被 26 处引用，用来串行化这些测试。

**影响**

- 测试必须靠全局锁避免互相踩踏；新增测试容易忘记加锁，产生偶发失败。
- 全局 loader 一旦设置就无法更换，测试只能验证一次注入。

**建议**

- L-1 的 `PathStateStore` 注入会自然消除这两个全局变量。
- 在完成之前，至少把测试路径改成 `TempDir` 参数注入，而不是固定文件名。
- 删除 `test_persist_lock`，让测试可以并行。

---

### L-13【P2】启动时全量重建派生表，数据量增长后线性变慢

**位置**：`gui/src/lib.rs:159-187`。

- `daily_stats` 每次启动全量重建。
- `var_references` 每次启动全量重建。
- `search_fts` 只在为空时重建（但 CLI 写入后不会增量更新，导致陈旧）。

**影响**

- 当前数据量小，毫秒级；一旦历史快照上千、引用图上万，启动会明显变慢。
- 注释里的「当前数据量下是毫秒级」是一个会过期的假设。

**建议**

- `daily_stats` 改为按 `changes` 增量更新，启动只补最近 N 天。
- `var_references` 改为监听 current 快照变更（`app_state.version`）重建，而不是每次启动。
- `search_fts` 在 CLI 写入后也更新，或至少在 `sync status`/GUI 启动时检测 `MAX(snapshot.id)` 与索引水位。

---

### L-14【P2】CLI 的 JSON 输出用 `unwrap()`

**位置**：`cli/src/env_ops.rs:330,589,643,682`、`cli/src/main.rs:385`、`cli/src/profile_ops.rs:23`、`cli/src/scan_ops.rs:15,49`。

**问题**

- `serde_json::to_string_pretty(...).unwrap()` 在理论上失败会 panic，违反「生产代码不用 `unwrap`」。
- 这些类型都是可序列化的，实际触发概率极低，但它是坏示范，且 8 处重复。

**建议**

- 抽 `cli/src/output.rs`：

```rust
pub(crate) fn print_json<T: serde::Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(s) => println!("{s}"),
        Err(e) => exit_err(&format!("JSON 序列化失败: {e}")),
    }
}
```

- 同时删掉 9 处重复的 `println!`。

---

### L-15【P2】GUI 应用配置计数走 O(n) 列表查找

**位置**：`gui/src/commands/service.rs:251-267`。

- 先 `list_profiles(db)`，再 `find(|p| p.name == name)`，再 `mark_profile_applied(uid)`。
- core 已经有 `mark_profile_applied_by_name`（`core/src/db/profiles.rs:520`），CLI 也在用。

**建议**

- 直接调用 `mark_profile_applied_by_name`，删掉 GUI 的列表查找。

---

### L-16【P2】README 测试/覆盖率徽章失真

**位置**：`README.md:13-14`。

- `tests-582 passed`：当前 Rust 350 + 前端 398 = 748，E2E 54。
- coverage 徽章指向 `branch/v5.1`：当前分支是 `main`、版本 6.0.0。

**建议**

- 更新徽章或改为动态徽章；如果不维护，删除比保留错误数字更诚实。

---

## 六、命名与契约一致性

| 现状                                                    | 问题                                   | 建议                                                        |
| ------------------------------------------------------- | -------------------------------------- | ----------------------------------------------------------- |
| `core/src/profiles.rs` 与 `core/src/db/profiles.rs`     | 同名，一个是 v5 文件，一个是 DB        | v5 改为 `legacy_v5::profiles`                               |
| `core/src/history.rs` 与 `core/src/db/history.rs`       | 同名，一个是端口，一个是实现           | trait 改为 `history/events.rs`，或在模块头写明「端口/实现」 |
| `core/src/backup.rs` 与 `core/src/db/backups.rs`        | 同名，一个是文件备份，一个是索引       | `backup/files.rs` 与 `db/backup_index.rs`                   |
| `core/src/env_var.rs` 与 `core/src/registry/env_var.rs` | 同名，一个是模型/安全判定，一个是 CRUD | 合并为 `env/` 目录                                          |
| `gui/src/commands/m2.rs`                                | 以里程碑命名                           | `analytics.rs` 或按域拆分                                   |
| `gui/src/commands/history.rs` 含 `profile_*`            | 模块名与职责不符                       | 拆出 `profiles.rs`                                          |
| `ProfilePathEntry`（`core/src/profiles.rs:71`）         | 只是 `PathEntry` 的别名                | 删除别名，统一用 `PathEntry`                                |
| `_pendingSys` / `_pendingUser`                          | 文档说已删除，代码仍在                 | 删除字段与全部引用                                          |
| `ErrorCode::Sync` 文档                                  | 说映射 1，实际 4                       | 修文档                                                      |
| `README.md` 说「19 条顶层命令」                         | 需要与 `main.rs` 实际枚举核对          | 发版前用脚本核对                                            |

---

## 七、测试与质量门

### 7.1 现有基线

| 层             | 规模            | 结果                              |
| -------------- | --------------- | --------------------------------- |
| Rust           | 57 个测试二进制 | 350 passed / 0 failed / 2 ignored |
| 前端 Vitest    | 33 个文件       | 398 passed                        |
| E2E Playwright | 54 条           | passed（本地，CI 不跑）           |

本轮只执行了格式门，结果：

```text
cargo fmt --all -- --check   → exit 0
npm run format:check         → exit 0（All matched files use Prettier code style）
```

### 7.2 测试盲区

| 盲区                     | 现状                                               | 建议补的测试                                                                                                                |
| ------------------------ | -------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| CLI 写入 → GUI/DB 可见   | 无                                                 | `core/tests/db_cli_write_visible.rs`：fake 注册表 IO + 临时 DB + `save_path_with_state`，断言 current snapshot 与 `changes` |
| 事件结果分类             | 只测保存成功                                       | `service_event_result.rs`：表驱动五种 hive 组合                                                                             |
| v5 归档后 CLI 读禁用状态 | `db_v5_read_from_db.rs` 只测注入 loader 的正向路径 | 负向：无 loader + 文件已归档 → 断言不会静默丢禁用项（或明确报错）                                                           |
| 死 IPC 命令              | 无                                                 | 前端契约测试：`invoke_handler` 注册表与 `backend.ts` 方法集合做差集，差集必须为空或显式 allowlist                           |
| 注释考古层               | 无                                                 | `scripts/check-comment-archaeology.mjs`，先软后硬                                                                           |
| 环境变量对比             | 无                                                 | 如果决定不支持，加一个「字段为空且 UI 显示不支持」的契约测试                                                                |

### 7.3 一文件一测试的副作用

`core/src/disabled.rs` 的固定 temp 路径与 `test_persist_lock` 是「一文件一测试」规则的衍生债务。建议在 L-1 重构时一起解决，而不是继续加锁。

---

## 八、整改路线图

> 原则：**先修行为，再清注释，最后搬目录**。三者不要混在一个 PR 里。

### Wave 5.0：逻辑正确性（P0 / P1）

**目标**：CLI 与 GUI 对「当前 PATH / 禁用状态」给出同一答案；事件结果正确；删除活陷阱。

| 任务  | 内容                                                                                  | 验收                                                        |
| ----- | ------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| 5.0.1 | 定义 `PathStateStore`，实现 `DbPathStateStore` / `V5FileStateStore`                   | 两个实现各有单测；不碰真实注册表                            |
| 5.0.2 | `core::service::save_path_with_state` 统一编排（写注册表 + 更新 current + 事件/变更） | GUI/CLI 都只调它；`gui/src/commands/service.rs` 降到 ~80 行 |
| 5.0.3 | CLI 启动时尝试打开 DB，成功则注入 DB store；失败退回文件                              | CLI 在归档后仍能读到禁用项                                  |
| 5.0.4 | 删除 `service::apply_profile` 与未使用 IPC；`apply` 统一走 DB                         | `grep apply_profile` 只剩 DB 版本                           |
| 5.0.5 | 修事件结果分类                                                                        | 表驱动测试全绿                                              |
| 5.0.6 | 启动 reconcile：current snapshot 与注册表不一致时重建                                 | 手工删 DB 当前快照后重启可自愈                              |
| 5.0.7 | 补 CLI→GUI 回归测试                                                                   | 新增测试在 CI 绿                                            |

**风险**：5.0.2 会触碰保存主链路，必须有「fake 注册表 IO + 临时 DB」的完整回归；不要用真实注册表验证。

### Wave 5.1：注释与死代码（低风险）

**目标**：生产代码零里程碑标记；死代码清零；超大文件拆分。

| 任务  | 内容                                                                                                   | 验收                                              |
| ----- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------- |
| 5.1.1 | 按 §2.4 规则清理注释，先修 §2.3 的过期注释                                                             | 370 + 61 → 0                                      |
| 5.1.2 | 删除 `gui/src/commands/profiles.rs`、`_pending*`、未使用 IPC                                           | `cargo clippy` 无 dead_code 警告                  |
| 5.1.3 | 拆分 `backup.rs`、`cli/src/main.rs`、`cli/src/env_ops.rs`、`gui/src/commands/history.rs`、`backend.ts` | 单文件生产代码 ≤ 400 行（`backup.rs` 目标 ≤ 500） |
| 5.1.4 | 合并 force/normal env 写路径；合并 `exit_*`；抽 `print_json`                                           | 行数下降 ≥ 200；行为不变                          |
| 5.1.5 | 加注释考古脚本，CI 软门禁                                                                              | 新注释不再引入编号                                |

**验收**：`npm run verify:all` 全绿；`git diff --stat` 只包含注释/文件移动/删除，不包含逻辑变化。

### Wave 5.2：目录迁移（独立 PR）

**目标**：`src/{core,cli,gui}`（或 `crates/` + `apps/`）。

| 步骤  | 内容                                                            | 验收                                              |
| ----- | --------------------------------------------------------------- | ------------------------------------------------- |
| 5.2.0 | spike：只移动 `gui`，验证 Tauri 配置发现与 `beforeBuildCommand` | `npx tauri info` / `npx tauri build --debug` 成功 |
| 5.2.1 | 移动前端到 `src/gui/web`                                        | `npm run build` / `npm test` 绿                   |
| 5.2.2 | 移动 Tauri Rust 到 `src/gui/src-tauri`                          | `cargo check -p patheditor` 绿                    |
| 5.2.3 | 移动 core/cli                                                   | `cargo metadata` / `cargo test --workspace` 绿    |
| 5.2.4 | 更新 CI/release/docs                                            | 发布流水线 dry-run 成功                           |

**禁止**：与 5.0/5.1 合并；在迁移 PR 中改任何业务逻辑。

### Wave 5.3：可选债务

- `EnvHiveStore` / `backup_registry` / `fs` 全量迁移到 `CoreError`，删除 `From<String>`。
- `daily_stats` / `var_references` 增量重建。
- Tauri 依赖升级（`npx tauri info` 显示 2.11.2 → 最新 2.12.5；升级需单独验证 WebView2/NSIS）。

---

## 九、合规亮点（应保留）

1. **`unsafe` 纪律良好**：28 个 `unsafe` 块，29 条 `SAFETY:` 注释；`grep` 未发现无注释的 `unsafe`。DPAPI/DACL 这类 Win32 调用是必要的，注释也解释了不变量。
2. **生产代码无 `panic!` / `todo!` / `unimplemented!`**（本轮脚本扫描结果为空）。少量 `expect` 都在已校验长度的同步包解析或启动 `run()` 中，属于可接受的不变量。
3. **注册表写入有唯一编排**：GUI/CLI 都经 `core::service`，没有各自直接 `RegSetValueEx`。这是 v5 重构最重要的成果，不要在 v6 重构中破坏。
4. **导入文件有 scope 校验**：`read_text_file_scoped` 用 `canonicalize` + 根目录前缀比较，能挡住 `..` 与符号链接逃逸。
5. **FTS 查询有正确转义**：`escape_query` 把用户输入包成短语并双写引号，避免 FTS5 语法注入。
6. **CSP 未放松**：`gui/tauri.conf.json` 的 `csp` 是具体白名单，不是 `null`。
7. **测试规模真实**：350 Rust + 398 前端 + 54 E2E，不是「只有 happy path」的装饰性测试。
8. **错误码契约基本贯彻**：`CoreError.code` 是主判定依据；`[E_CONFLICT]` 只作为展示文本保留。新增代码应继续遵守。

---

## 十、未覆盖项（如实列出）

1. **未运行 GUI/CLI 跨进程联调**。L-1/L-2/L-3 是基于代码路径的静态结论，证据链完整，但没有在「GUI 已迁移并归档 + CLI 随后写入」的真实进程组合下复现。建议 Wave 5.0 用集成测试补上。
2. **未运行完整质量门**。本轮只跑了 `cargo fmt --check` 与 `npm run format:check`；没有跑 `cargo test`、`clippy`、`npm test`、E2E。现有基线数字来自仓库文档与历史记录，不是本轮实测。
3. **未做依赖安全审计**。没有跑 `cargo audit` / `npm audit`；`npx tauri info` 只显示 Tauri 相关版本偏旧。
4. **未逐行审查全部 27k 行 Rust**。重点覆盖了 `service`、`disabled`、`db/{mod,history,profiles,maintenance,archive_v5}`、`registry/env_var`、`backup`、CLI runtime/profile/env、GUI commands、前端 store/backend。`db/sync/{bundle,merge,conflict}` 只做了接口级检查（M4 已专门审查过）。
5. **未验证 `src/` 目录迁移后的 Tauri 配置发现**。§3.5 的 Step 0 spike 是必须做的，不能凭本文推断。
6. **未评估 UI/UX 与视觉层**。本报告不涉及组件样式、i18n 文案质量、无障碍。
7. **未评估性能基准**。L-13 是复杂度判断，没有实测启动耗时随快照数的曲线。
8. **未检查 Git 历史中的敏感信息**。`docs/` 与测试夹具可能含本机路径，未做 secret 扫描。

---

## 十一、结论

1. **当前代码可以继续维护，但不适合在现有过渡层上继续加功能。** v6 的加密库、同步、检索、统计都建立在「注册表是唯一真相源、DB 是 sidecar」这个前提上；CLI 与 DB 的分裂正在破坏这个前提。先把 L-1/L-2/L-3 修掉，再谈新特性。
2. **注释清理可以立即开始，且应该先做。** 它不改行为、风险最低，却能显著降低后续重构的认知成本。先修 §2.3 的 8 处过期注释，再按文件清理 370 + 61 处标记。
3. **目录迁移放到最后。** 它是「搬家」，不是「治病」。在逻辑与注释都稳定之后再做，才能用「行为不变」作为验收标准。
4. **不要为了瘦身而瘦身。** 19,328 行 Rust 不算多；真正该减的是死代码（S-1/S-2/S-3）、重复写路径（S-7）、重复退出函数（S-13）和重复 JSON 打印（S-14）。预计可净减 300–500 行，而不是砍掉一半。
5. **保留合规亮点。** 唯一保存编排、`SAFETY` 纪律、错误码契约、导入 scope 校验是这座代码库真正的资产；重构时不要把它们一起「优化」掉。

**下一步建议**：先由用户确认 Wave 5.0 的范围与授权（是否允许改保存主链路、是否允许补测试、是否允许删除死代码），再写 spec/plan。在此之前，审核窗口不建议放行任何「先搬目录再顺手改逻辑」的混合改动。
