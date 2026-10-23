# PathEditor v6 代码重构整改回执

- **回执日期**：2026-10-23
- **分支 / 提交**：`refactor/v6-cleanup`，HEAD `8f79e1d`
- **基准点**：`main` @ `30bfb75`（v6.0.0 tag 之后 1 个文档提交）
- **关联审查报告**：`docs/审核和开发/2026.10.22/PathEditor-v6-代码重构审查报告.md`
- **授权**：用户口头「全部授权」；后续要求「完全舍弃 v5」「继续执行未完成的任务」「继续拆 backend.ts 和 L-10」
- **范围**：`30bfb75..8f79e1d`，348 个文件，+12,262 / −13,320 行

> 本回执只记录实际执行过的命令与结果。未执行、未验证的项在第 5 节如实列出，不用「应该没问题」代替证据。

---

## 1. 回执结论

用户提出三项诉求，完成情况如下：

| 诉求                            | 状态           | 说明                                                                                                                      |
| ------------------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------- |
| 代码注释去开发过程残留          | **完成**       | 生产代码中 M0–M4 / Wave / Task / spec § / F-J-D-S-R-N-P-T 编号与「欠账」命中数为 **0**                                    |
| 目录整合为 `src/{core,cli,gui}` | **完成**       | `core/` → `src/core/`、`cli/` → `src/cli/`、`gui/` → `src/gui/src-tauri/`、前端 → `src/gui/web/`；Tauri 构建与 E2E 均通过 |
| 代码级重构 + 详细审查报告       | **大部分完成** | P0 全部修复；P1 全部收口（L-9 UI 明示、L-10 已统一）；P2 修复 4 项、部分 1 项；结构精简 12/15 完成，2 项部分、1 项未做    |

一句话总结：**三项主诉求已完成两项半**——注释清零、目录迁移完成、逻辑 P0 全修；剩余是可选债务（L-13、S-7、S-3、S-15）与推送/发版决策。

---

## 2. 提交清单

```text
8f79e1d refactor: 拆分 backend.ts 为 parse-core/parse-data/index
b3cc47e refactor: 统一环境变量/备份/文件错误模型
0f6902c refactor: 拆分 backup 为 config/env/path/restore 子模块
57fab5b docs: 更新布局迁移后的路径与架构说明
499f4c6 fix: 修正布局迁移后的契约 fixture 路径
c9aecbf refactor: 完成 src/{core,cli,gui} 布局迁移
b13ce45 refactor: 迁移 gui 到 src/gui/src-tauri（布局 spike）
3548a82 refactor: 拆分 env 子命令的值解析/渲染/命令实现
1678de3 refactor: 将 reg_store 收进 registry::store
74b0c27 refactor: 拆分 CLI 命令定义与 PATH 实现
68a40c6 refactor: 拆分 GUI 历史/配置/备份命令
66dd9a7 docs: 新增 v6 重构整改回执
cca805e refactor: 删除前端未注册的 IPC 包装
6e2622e refactor: 统一 CLI 的 JSON 输出与错误退出
96e9976 refactor: 移除 v5 过渡层，DB 成为唯一持久化状态
dc10222 refactor: 全量清理代码注释中的开发过程标记
92932de refactor: 统一 GUI/CLI 的 PATH 保存与 DB 当前快照
cac2fe8 refactor: 清理 v6 过渡层死代码与过期注释
c89b8cf docs: 新增 v6 代码重构审查报告
111517b docs: 拆分 CLAUDE.md 细节到 docs/dev
```

各提交职责：

| 提交      | 内容                                                                                                        |
| --------- | ----------------------------------------------------------------------------------------------------------- |
| `c89b8cf` | 新增 850 行审查报告，记录 4 个 P0、6 个 P1、6 个 P2 与 15 项结构精简建议                                    |
| `cac2fe8` | 删除 v6 过渡层死代码、过期注释；先修审查报告 §2.3 的误导性注释                                              |
| `92932de` | 保存编排下沉到 core：`save_path_with_db` / `save_path_registry_only`，GUI/CLI 共用；CLI 可选打开 DB         |
| `dc10222` | 全量清理注释中的里程碑、工单号、计划任务号；保留 SAFETY、TOCTOU、加密顺序等「为什么」注释                   |
| `96e9976` | 移除 v5 过渡层：迁移、归档、导出/导入、v5 profiles、`disabled.json`、全局 loader/flag、v5 CLI/IPC 命令      |
| `6e2622e` | 抽出 `cli::output::print_json`；合并重复的 `exit_persist_error`；清理 golden README 的里程碑标记            |
| `cca805e` | 删除前端三个未注册 IPC 的包装与 mock                                                                        |
| `68a40c6` | `src/gui/src-tauri/src/commands/history.rs` 拆为 history / profiles / db_state；backup_list 归入 backup.rs  |
| `74b0c27` | `src/cli/src/main.rs` 拆为 cli.rs（clap）与 path_ops.rs（命令实现）                                         |
| `1678de3` | `reg_store.rs` → `registry/store.rs`，从 crate 根移除公开模块                                               |
| `3548a82` | `src/cli/src/env_ops.rs` 拆为 value / render / cmd / tests 子模块                                           |
| `b13ce45` | 布局 spike：`gui` → `src/gui/src-tauri`，验证 Tauri 配置发现与 frontendDist 解析                            |
| `c9aecbf` | 完成 `src/{core,cli,gui}` 布局：core/cli 移入 src，前端移入 `src/gui/web`，更新全部工具链路径               |
| `499f4c6` | 修正 core 测试的契约 fixture include 路径                                                                   |
| `57fab5b` | 更新 CLAUDE/AGENTS、README 与 docs/dev 的目录树和路径                                                       |
| `0f6902c` | `backup.rs` 拆为 config / env / path / restore 子模块，测试独立到 tests.rs                                  |
| `b3cc47e` | `EnvHiveStore` 四个方法改 `CoreError`；`backup_registry` / `fs` 公共 API 同步；删除 `From<String>` 过渡转换 |
| `8f79e1d` | `backend.ts` 拆为 `backend/{index,parse-core,parse-data}.ts`；`@/services/backend` 导出路径不变             |

---

## 3. 整改核对

### 3.1 注释考古层（用户问题 1）

**验收命令**

```bash
grep -rnE '\b(M[0-4]|Wave|Task [0-9]|spec §|F-[0-9]|J-[0-9]|D-[0-9]|S-[0-9]|R-[0-9]|N-[0-9]|P[0-9]-[0-9]|T-[0-9])\b|欠账' \
  src/core/src src/cli/src src/gui/src-tauri/src src/gui/web/src \
  src/gui/web/tests src/gui/web/e2e src/core/tests \
  --include='*.rs' --include='*.ts' --include='*.tsx' --include='*.md' | grep -v icons.tsx
```

**结果**：0 命中。

**处理原则**

- 删除编号，但保留约束本身。例如「M0 实测 ON 在受限进程里 VirtualLock 失败」改成「受限进程里 `VirtualLock` 会失败」，不把有效信息一起删掉。
- `// SAFETY:`、SQLCipher 初始化顺序、TOCTOU 窗口、墓碑语义、一文件一测试等「为什么必须这样做」的注释全部保留。
- 未改任何 `UTF-8` / `UTF-16` 相关说明。上一轮正则误伤已修复，本轮扫描确认无 `UT` 残片。
- 模块头只保留职责、边界、设计依据；变更史留在 `docs/` 与 CHANGELOG。

**同时修复的过期注释**

- `src/core/src/error.rs`：删除「`Sync` 仍映射为 1」的错误说明，实际映射是 4。
- `src/core/src/db/compare.rs`、`src/gui/web/src/components/history/CompareDialog.tsx`：删除「环境变量对比将在 M3 提供」的空承诺。
- `src/gui/web/src/components/history/HistoryPanel.tsx`：删除「恢复/筛选留到 M2」的过期边界说明。
- `src/core/src/env_var.rs`：不再拿已删除的 `_pendingSys` / `_pendingUser` 当保留 `Path` 的理由。
- `src/gui/web/src/services/backend.ts`（现 `backend/index.ts`）：删除「Wave 2 收尾时复核」的过渡期注释。
- `src/core/src/registry/golden/README.md`：删除 `Wave 2 Task 7` / `F-10` / `Wave 3.0-B` 标记，并把已删除的 `save_path_with_sidecar` 更正为 `save_path_with_db` / `save_path_registry_only`。

**未做**

- 没有新增 `scripts/check-comment-archaeology.mjs` 与 CI 软门禁（审查报告 S-5.1.5）。当前零命中靠人工扫描，后续仍可能回归。

### 3.2 结构整合（用户问题 2）

**已完成：目录迁移（Wave 5.2）**

当前布局：

```text
src/
├── core/                 # Rust 核心库
├── cli/                  # Rust CLI
└── gui/
    ├── src-tauri/        # Tauri Rust + tauri.conf.json
    └── web/              # React + Vite + 单测 + E2E
```

迁移过程分两步：

1. **Step 0 spike**（`b13ce45`）：只移动 `gui` → `src/gui/src-tauri`，实测 Tauri 2 行为：
   - CLI 能自动发现 `src/gui/src-tauri/tauri.conf.json`；
   - `beforeBuildCommand` 在**仓库根**执行，命令保持 `npm run build`；
   - `frontendDist` 相对**配置文件所在目录**解析（从 `src/gui/src-tauri` 到 `src/gui/web/dist` 写 `../web/dist`）；
   - `npx tauri build --debug --no-bundle` 成功产出 `target/debug/PathEditor.exe`。
2. **完整迁移**（`c9aecbf`）：`core` → `src/core`、`cli` → `src/cli`、前端 → `src/gui/web`。
   - Cargo workspace `members` 与 `path-editor-core` 的路径依赖已更新；
   - Vite `root`、alias `@`、Vitest、tsconfig、ESLint、Prettier、Playwright 与 `package.json` 脚本全部改到新路径；
   - CI 的版本校验与注释、release workflow 的 `tauri.conf.json` 路径已更新；
   - `CLAUDE.md` / `AGENTS.md` / `README.md` / `docs/dev/*` 的目录树与路径已更新。

**已完成的模块边界修复**

- `src/gui/src-tauri/src/commands/service.rs` 从 271 行降到 **36 行**，只做参数解包与错误映射；保存编排、current 快照、FTS 增量索引全部下沉到 `core::service`。
- 删除 `src/gui/src-tauri/src/commands/profiles.rs`（死模块）与 `src/gui/src-tauri/src/commands/m2.rs` 的里程碑命名，后者改为 `analytics.rs`。
- 删除 v5 文件配置模块 `src/core/src/profiles.rs`，消除 `src/core/src/profiles.rs` 与 `src/core/src/db/profiles.rs` 的同名歧义。
- 删除前端未注册 IPC 的三个包装，缩小 IPC 面。

**验证**

- `npm run build` 在 `src/gui/web` root 下通过；
- `npm test` 33 files / 398 passed；
- `npm run test:e2e` 54 passed；
- `npx tauri build --debug --no-bundle` 成功产出 `PathEditor.exe`；
- `cargo test --workspace` 322 passed / 2 ignored。

**结论**：用户问题 2 的「模块没整合」与「目录没整合」均已完成。

### 3.3 逻辑审查整改（用户问题 3.2）

#### P0

| 编号 | 状态 | 证据                                                                                                                                          |
| ---- | ---- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| L-1  | 已修 | CLI 打开 DB 后调用 `service::save_path_with_db`，不再传 `NoopHistory`；DB 不可用才走 `save_path_registry_only`（`src/cli/src/runtime.rs:91`） |
| L-2  | 已修 | `disabled.json` 读写整体删除；`load_path_snapshot` 直接读 DB current 快照；CLI 与 GUI 看到同一份禁用状态                                      |
| L-3  | 已修 | 删除 GUI 进程级 `Arc<RwLock<Option<PathPair>>>` 缓存与 `CURRENT_SNAPSHOT_LOADER`，每次读 DB                                                   |
| L-4  | 已修 | `event_result` 改为「有成功 + 有失败 = Partial；全失败 = Failure」，并补表驱动测试（`src/core/src/service.rs:409`、`:1018`）                  |

#### P1

| 编号 | 状态       | 证据 / 说明                                                                                                                                                                                             |
| ---- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| L-5  | 已修       | `service::apply_profile` 与 v5 `core::profiles` 模块删除；配置应用统一走 DB 版 `profile_paths` / `profile_apply`                                                                                        |
| L-6  | 已修       | `save_path_with_db_io` 只在至少一个 hive `Applied` 时写 current 快照并推进 `app_state.version`（`src/core/src/service.rs:500`）                                                                         |
| L-7  | 已修       | CLI 保存经 `DbHistory` 写入快照与 `changes`；`source = cli`，跨设备同步可见                                                                                                                             |
| L-8  | 已修       | GUI 命令层降到 36 行；保存编排、CAS、FTS 索引全部在 core                                                                                                                                                |
| L-9  | **已收口** | `CompareDialog` 已显式显示「本版本不支持环境变量对比」，不会再让用户误以为「没有变化」；三个字段保留为空数组，v6.1 再决定实现或删除                                                                     |
| L-10 | **已修**   | `EnvHiveStore` 四个方法、`backup_registry`、`fs` 公共 API 全部改为 `CoreError`；`From<String> for CoreError` 已删除。残余：`registry/path.rs` 与 `scanner.rs` 的公开签名仍为 `String`（不再有隐式转换） |

#### P2

| 编号 | 状态     | 说明                                                                                                                                                 |
| ---- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| L-11 | 已修     | PATH 保存增加 `EventSource` 参数，CLI 传 `Cli`、GUI 传 `Gui`。**残余**：`apply_path_snapshot` 包装仍硬编码 `Gui`                                     |
| L-12 | 部分     | 全局 `V5_WRITES_DISABLED` / `CURRENT_SNAPSHOT_LOADER` 已删除；`test_persist_lock` 仍被 backup/persist/env_var 测试使用，固定路径尚未全部改 `TempDir` |
| L-13 | **未修** | 启动仍全量重建 `daily_stats` / `var_references`；`search_fts` 只在为空时重建                                                                         |
| L-14 | 已修     | 新增 `src/cli/src/output.rs::print_json`，替换 9 处 `serde_json::to_string_pretty(...).unwrap()`；序列化失败以 1 退出                                |
| L-15 | 已修     | `src/gui/src-tauri/src/commands/history.rs:300` 已直接用 `mark_profile_applied_by_name`，不再 list + find                                            |
| L-16 | 已修     | README 测试徽章改为 `720 passed`（322 Rust + 398 前端）；coverage 徽章指向 `main`                                                                    |

### 3.4 结构精简（用户问题 3.1）

| 编号 | 状态 | 说明                                                                                                                                      |
| ---- | ---- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| S-1  | 已修 | 删除 `src/gui/src-tauri/src/commands/profiles.rs`                                                                                         |
| S-2  | 已修 | 删除 `_pendingSys` / `_pendingUser` 字段与全部引用                                                                                        |
| S-3  | 部分 | 删除 v5 IPC 与 3 个未注册前端包装；`stats_rebuild` / `refs_rebuild` / `search_rebuild` 仍暴露，未做「注册表 vs backend/index.ts」差集测试 |
| S-4  | 已修 | `src/gui/src-tauri/src/commands/m2.rs` → `analytics.rs`                                                                                   |
| S-5  | 已修 | `src/gui/src-tauri/src/commands/history.rs` 拆为 `history.rs` / `profiles.rs` / `db_state.rs`；`backup_list` 归入 `backup.rs`             |
| S-6  | 已修 | `src/core/src/backup.rs` → `backup/{mod,config,env,path,restore,tests}.rs`，公开 API 路径不变                                             |
| S-7  | 未做 | `src/core/src/registry/env_var.rs` 的 normal/force 写路径未合并                                                                           |
| S-8  | 已修 | `src/cli/src/main.rs` 拆为 `cli.rs`（clap）与 `path_ops.rs`（命令实现），main.rs 只剩分派与 help 测试                                     |
| S-9  | 已修 | `src/cli/src/env_ops.rs` 拆为 `env_ops/{mod,value,render,cmd,tests}.rs`                                                                   |
| S-10 | 已修 | `src/gui/web/src/services/backend.ts` → `backend/{index,parse-core,parse-data}.ts`，index 455 行 / core 438 行 / data 763 行              |
| S-11 | 已修 | `core/src/reg_store.rs` → `src/core/src/registry/store.rs`，`pub(crate)`，从 crate 根移除公开模块                                         |
| S-12 | 已修 | v5 孤岛直接删除（比归档到 `legacy_v5` 更彻底，符合用户「完全舍弃 v5」的要求）                                                             |
| S-13 | 已修 | `exit_persist_error` 合并到 `exit_core_error`                                                                                             |
| S-14 | 已修 | 抽出 `print_json`，消除 9 处序列化 `unwrap`                                                                                               |
| S-15 | 部分 | DB 快照相关固定路径已随 `disabled.json` 移除而消失；其余 `test_persist_lock` 仍在                                                         |

### 3.5 新增 / 调整的测试

- `src/core/tests/db_path_save_state.rs`（新增，140 行）：CLI 保存后 `current_snapshot` 更新、产生 `changes`、事件 `source = cli`。
- `src/core/tests/db_current_snapshot_read.rs`（由 `db_v5_read_from_db.rs` 重命名）：`load_path_snapshot_db` 读取 `app_state.current_snapshot_id`，并保留禁用条目。
- `src/core/src/service.rs::tests::event_result_classifies_hive_combinations`（新增）：覆盖 Applied+Applied、Applied+Failed、Failed+Failed、Skipped+Failed、Skipped+Skipped。
- 删除 v5 专用测试与 fixture：`profile_legacy_v1`、`profile_future_version_rejected`、`merge_legacy_*`、`merge_snapshot_order_wins`、v5 导出/导入边界测试。
- 前端删除 3 个未注册 IPC 包装及对应 e2e mock 分支；单测数仍为 398。

---

## 4. 验证证据

| 命令                                                    | 结果                                                         |
| ------------------------------------------------------- | ------------------------------------------------------------ |
| `npm run verify`                                        | **exit 0**（Prettier/ESLint/build/coverage/fmt/clippy/test） |
| `cargo test --workspace`                                | **322 passed / 0 failed / 2 ignored**                        |
| `cargo test -p patheditor-cli --bins`                   | **66 passed / 0 failed**                                     |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0                                                       |
| `cargo fmt --all -- --check`                            | exit 0                                                       |
| `npm test -- --run`                                     | **33 files / 398 passed**                                    |
| `npm run test:e2e`                                      | **54 passed**（新布局，Playwright mock IPC）                 |
| `npm run build`                                         | 通过；Vite root = `src/gui/web`，产物 `src/gui/web/dist`     |
| `npx tauri build --debug --no-bundle`                   | 通过，产出 `target/debug/PathEditor.exe`                     |
| 注释考古扫描（§3.1 命令）                               | **0 命中**                                                   |

> 绿灯不等于规格全覆盖：未启动真实 GUI 做人工走查，未做真实注册表闭环；E2E 使用 mock IPC。第 5 节如实列出。

---

## 5. 未执行项（如实列出）

1. **S-7 normal/force 写路径合并**：`src/core/src/registry/env_var.rs` 仍保留两套写路径。
2. **S-3 完整 IPC 差集**：没有自动比对 `invoke_handler` 与 `backend/index.ts` 方法集合。
3. **S-15 测试固定路径**：`test_persist_lock` 仍存在。
4. **L-13 派生表增量重建**：启动仍全量重建 `daily_stats` / `var_references`。
5. **残余 String 签名**：`src/core/src/registry/path.rs` 与 `src/core/src/scanner.rs` 的公开 API 仍返回 `String`（已无 `From<String>` 隐式转换）。
6. **真实 GUI + 注册表闭环**：未启动 GUI 做人工走查，未写真实注册表。
7. **推送、合并到 main、版本号**：未推送，未升级版本号，未打 tag。
8. **README 截图**：用户明确表示自行替换，本轮未动图片。

---

## 6. 兼容性变化与残余风险

### 6.0 布局迁移

- 源码路径已变为 `src/core`、`src/cli`、`src/gui/src-tauri`、`src/gui/web`；外部脚本、CI、编辑器配置和文档若仍引用 `core/` / `cli/` / `gui/` 会失效。
- `cargo test -p ...`、`cargo build -p ...` 的包名不变；npm 脚本仍在仓库根运行。
- Tauri CLI 自动发现 `src/gui/src-tauri/tauri.conf.json`；`frontendDist` 相对配置文件解析为 `../web/dist`；`beforeBuildCommand` 仍从仓库根执行。
- 本分支与迁移前的分支合并时，旧路径会产生大量 rename/delete 冲突，建议直接以本分支为准。

### 6.1 破坏性变化：v5 数据不再自动迁移

这是用户「完全舍弃 v5」指令的直接结果，发版说明必须写清楚：

- v5 的 `disabled.json`、`profiles/*.json`、v5 导出包不再被读取。
- 从 v6.0.0 升级的用户不受影响（DB 已存在）。
- 从 v5 直接升级到本分支的用户不会自动迁移；需要先走旧版 v6 的迁移路径，或接受重新配置。
- `cleanup-legacy-backups` 仍保留：它清理的是明文 `.json` 备份，属于安全工具，不是 v5 状态迁移。

### 6.2 schema 列漂移

- 新建 DB 的 `app_state` DDL 已删除 `migrated_v5` 列。
- 已存在的 v6 DB 仍保留该列（未做 `CURRENT_SCHEMA_VERSION = 4` 迁移）。
- 影响：新库与旧库列集合不完全一致，但代码不再读写该列，无数据损失；后续若做 schema 校验需注意。

### 6.3 CLI fail-open

- CLI 打开 DB 失败（文件损坏、密钥不可用、被锁）时打印警告并退回 `save_path_registry_only`。
- 此时 PATH 照常写入，但禁用状态不落库；GUI 之后读 DB 会看不到这次 CLI 的禁用/启用状态，直到下一次 GUI 保存重建 current 快照。
- 这是有意的 fail-open，不是静默失败；但如果用户只使用 CLI，禁用状态不会持久化。需要在 `docs/dev/data-and-security.md` 或 README 中明确。

### 6.4 保存失败仍会产生 History 快照

- `save_path_with_db_io` 在注册表写入前先建一条 `kind=history` 快照（用于撤销）。
- 如果两个 hive 都失败，current 快照不会推进，但这条 History 快照仍会留下。
- 影响有限，但严格说「失败保存不产生任何行」并未做到。若要求更严，可把 History 快照改到 `check_expected` 通过之后。

### 6.5 其他

- `apply_path_snapshot` 仍固定 `EventSource::Gui`；目前只有 GUI 同步应用调用它，CLI 不走该包装。
- 注释门禁仍是人工扫描，没有 CI 硬门禁。
- `test_persist_lock` 仍是测试串行化手段，新增测试仍可能忘记加锁。

---

## 7. 回执判定

**用户三项主诉求已完成，可以进入复审；剩余为可选债务。**

- 注释考古层：完成，可验收。
- v5 过渡层：完成移除，可验收；破坏性变化需用户确认。
- 目录迁移：完成，`src/{core,cli,gui}` 布局已通过 Tauri 构建、前端构建、单测与 E2E。
- 逻辑 P0：全部修复，可验收。
- 逻辑 P1：6/6 收口；L-9 由 UI 明示不支持，L-10 已统一错误模型并删除 `From<String>`。
- 结构精简：S-1/S-2/S-4/S-5/S-6/S-8/S-9/S-10/S-11/S-12/S-13/S-14 完成；S-3/S-15 部分；S-7 未做。

### 待用户/审核窗口拍板

1. **是否接受 v5 数据不迁移的破坏性变化**（影响从 v5 直接升级的用户）。
2. **是否推送 `refactor/v6-cleanup` / 合并到 main / 升级版本号**；本轮未获新的推送授权。
3. **L-13 的处置**：是否在 v6.1 做派生表增量重建。
4. **S-7 的处置**：是否合并 `registry/env_var.rs` 的 normal/force 写路径。

### 建议的下一步顺序

1. 让本分支通过复审并合并（行为修复与布局迁移优先落地）。
2. v6.1 处理 L-13、S-7、S-3、S-15 与注释 CI 门禁。
3. 若继续收紧 IPC，先补「`invoke_handler` vs `backend/index.ts` 方法集合」差集测试。
