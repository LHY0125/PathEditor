# PathEditor v6 代码重构整改回执

- **回执日期**：2026-10-22
- **分支 / 提交**：`refactor/v6-cleanup`，HEAD `cca805e`
- **基准点**：`main` @ `30bfb75`（v6.0.0 tag 之后 1 个文档提交）
- **关联审查报告**：`docs/审核和开发/2026.10.22/PathEditor-v6-代码重构审查报告.md`
- **授权**：用户口头「全部授权」；后续明确要求「完全舍弃 v5，不为项目保留 v5 过渡模块」
- **范围**：`30bfb75..cca805e`，187 个文件，+3,076 / −4,600 行

> 本回执只记录实际执行过的命令与结果。未执行、未验证的项在第 5 节如实列出，不用「应该没问题」代替证据。

---

## 1. 回执结论

用户提出三项诉求，完成情况如下：

| 诉求                            | 状态         | 说明                                                                                   |
| ------------------------------- | ------------ | -------------------------------------------------------------------------------------- |
| 代码注释去开发过程残留          | **完成**     | 生产代码中 M0–M4 / Wave / Task / spec § / F-J-D-S-R-N-P-T 编号与「欠账」命中数为 **0** |
| 目录整合为 `src/{core,cli,gui}` | **未执行**   | 只修了模块边界，没有搬目录；按审查报告建议，目录迁移应单独立项并先做 Tauri spike       |
| 代码级重构 + 详细审查报告       | **部分完成** | P0 全部修复；P1 修复 4/6；P2 修复 4 项、部分 1 项；结构拆分与目录迁移留待后续          |

一句话总结：**行为层的 v5 过渡债务已经清掉，目录层的「搬家」还没做。** 这不是遗漏，而是审查报告明确建议把两者分开：先修行为，再清注释，最后搬目录。

---

## 2. 提交清单

```text
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

| 提交      | 内容                                                                                                         |
| --------- | ------------------------------------------------------------------------------------------------------------ |
| `c89b8cf` | 新增 850 行审查报告，记录 4 个 P0、6 个 P1、6 个 P2 与 15 项结构精简建议                                     |
| `cac2fe8` | 删除 v6 过渡层死代码、过期注释；先修审查报告 §2.3 的误导性注释                                               |
| `92932de` | 保存编排下沉到 core：`save_path_with_db` / `save_path_registry_only`，GUI/CLI 共用；CLI 可选打开 DB          |
| `dc10222` | 全量清理注释中的里程碑、工单号、计划任务号；保留 SAFETY、TOCTOU、加密顺序等「为什么」注释                    |
| `96e9976` | 移除 v5 过渡层：迁移、归档、导出/导入、v5 profiles、`disabled.json`、全局 loader/flag、v5 CLI/IPC 命令       |
| `6e2622e` | 抽出 `cli::output::print_json`；合并重复的 `exit_persist_error`；清理 golden README 的里程碑标记             |
| `cca805e` | 删除前端三个未注册 IPC 的包装（`load_disabled_state` / `save_disabled_state` / `save_path_snapshot`）与 mock |

---

## 3. 整改核对

### 3.1 注释考古层（用户问题 1）

**验收命令**

```bash
grep -rnE '\b(M[0-4]|Wave|Task [0-9]|spec §|F-[0-9]|J-[0-9]|D-[0-9]|S-[0-9]|R-[0-9]|N-[0-9]|P[0-9]-[0-9]|T-[0-9])\b|欠账' \
  core/src gui/src cli/src src tests e2e core/tests \
  --include='*.rs' --include='*.ts' --include='*.tsx' --include='*.md' | grep -v icons.tsx
```

**结果**：0 命中。

**处理原则**

- 删除编号，但保留约束本身。例如「M0 实测 ON 在受限进程里 VirtualLock 失败」改成「受限进程里 `VirtualLock` 会失败」，不把有效信息一起删掉。
- `// SAFETY:`、SQLCipher 初始化顺序、TOCTOU 窗口、墓碑语义、一文件一测试等「为什么必须这样做」的注释全部保留。
- 未改任何 `UTF-8` / `UTF-16` 相关说明。上一轮正则误伤已修复，本轮扫描确认无 `UT` 残片。
- 模块头只保留职责、边界、设计依据；变更史留在 `docs/` 与 CHANGELOG。

**同时修复的过期注释**

- `core/src/error.rs`：删除「`Sync` 仍映射为 1」的错误说明，实际映射是 4。
- `core/src/db/compare.rs`、`src/components/history/CompareDialog.tsx`：删除「环境变量对比将在 M3 提供」的空承诺。
- `src/components/history/HistoryPanel.tsx`：删除「恢复/筛选留到 M2」的过期边界说明。
- `core/src/env_var.rs`：不再拿已删除的 `_pendingSys` / `_pendingUser` 当保留 `Path` 的理由。
- `src/services/backend.ts`：删除「Wave 2 收尾时复核」的过渡期注释。
- `core/src/registry/golden/README.md`：删除 `Wave 2 Task 7` / `F-10` / `Wave 3.0-B` 标记，并把已删除的 `save_path_with_sidecar` 更正为 `save_path_with_db` / `save_path_registry_only`。

**未做**

- 没有新增 `scripts/check-comment-archaeology.mjs` 与 CI 软门禁（审查报告 S-5.1.5）。当前零命中靠人工扫描，后续仍可能回归。

### 3.2 结构整合（用户问题 2）

**已完成的模块边界修复**

- `gui/src/commands/service.rs` 从 271 行降到 **36 行**，只做参数解包与错误映射；保存编排、current 快照、FTS 增量索引全部下沉到 `core::service`。
- 删除 `gui/src/commands/profiles.rs`（死模块）与 `gui/src/commands/m2.rs` 的里程碑命名，后者改为 `analytics.rs`。
- 删除 v5 文件配置模块 `core/src/profiles.rs`，消除 `core/src/profiles.rs` 与 `core/src/db/profiles.rs` 的同名歧义。
- 删除前端未注册 IPC 的三个包装，缩小 IPC 面。

**未执行：目录迁移（Wave 5.2）**

当前仍是：

```text
core/      # Rust 库
gui/       # Tauri Rust + tauri.conf.json
cli/       # Rust CLI
src/       # React 前端
tests/     # 前端单测
e2e/       # Playwright
```

目标形态：

```text
src/
├── core/
├── cli/
└── gui/
    ├── src-tauri/
    └── web/
```

未执行的理由：

1. 审查报告 §3.5 要求先做 Step 0 spike（只移动 `gui`，验证 `npx tauri info`、`npm run build`、`npx tauri build --debug`），本轮没有做。
2. 迁移要同时改 Cargo workspace、Tauri 配置发现、Vite root、tsconfig、ESLint/Prettier/Vitest glob、CI/release 路径、137 份文档引用；任何一处漏改都可能让发布流水线失败。
3. 用户本轮的两个即时指令是「清注释」和「完全舍弃 v5」；目录迁移没有新的明确指令，按仓库规则「越界即停」处理。

**结论**：用户问题 2 的「模块没整合」已修；「目录没整合」仍是未完成项，需要单独授权与 spike。

### 3.3 逻辑审查整改（用户问题 3.2）

#### P0

| 编号 | 状态 | 证据                                                                                                                                      |
| ---- | ---- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| L-1  | 已修 | CLI 打开 DB 后调用 `service::save_path_with_db`，不再传 `NoopHistory`；DB 不可用才走 `save_path_registry_only`（`cli/src/runtime.rs:91`） |
| L-2  | 已修 | `disabled.json` 读写整体删除；`load_path_snapshot` 直接读 DB current 快照；CLI 与 GUI 看到同一份禁用状态                                  |
| L-3  | 已修 | 删除 GUI 进程级 `Arc<RwLock<Option<PathPair>>>` 缓存与 `CURRENT_SNAPSHOT_LOADER`，每次读 DB                                               |
| L-4  | 已修 | `event_result` 改为「有成功 + 有失败 = Partial；全失败 = Failure」，并补表驱动测试（`core/src/service.rs:409`、`:1018`）                  |

#### P1

| 编号 | 状态     | 证据 / 说明                                                                                                                                    |
| ---- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| L-5  | 已修     | `service::apply_profile` 与 v5 `core::profiles` 模块删除；配置应用统一走 DB 版 `profile_paths` / `profile_apply`                               |
| L-6  | 已修     | `save_path_with_db_io` 只在至少一个 hive `Applied` 时写 current 快照并推进 `app_state.version`（`core/src/service.rs:500`）                    |
| L-7  | 已修     | CLI 保存经 `DbHistory` 写入快照与 `changes`；`source = cli`，跨设备同步可见                                                                    |
| L-8  | 已修     | GUI 命令层降到 36 行；保存编排、CAS、FTS 索引全部在 core                                                                                       |
| L-9  | **未修** | `CompareResult` 的 `env_added` / `env_removed` / `env_modified` 仍恒为空；只清理了「将在 M3 提供」的注释。需要 v6.1 决定「实现」还是「删字段」 |
| L-10 | **未修** | `EnvHiveStore` / `backup_registry` / `fs` 仍返回 `Result<_, String>`，`From<String> for CoreError` 仍在                                        |

#### P2

| 编号 | 状态     | 说明                                                                                                                                                 |
| ---- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| L-11 | 已修     | PATH 保存增加 `EventSource` 参数，CLI 传 `Cli`、GUI 传 `Gui`。**残余**：`apply_path_snapshot` 包装仍硬编码 `Gui`                                     |
| L-12 | 部分     | 全局 `V5_WRITES_DISABLED` / `CURRENT_SNAPSHOT_LOADER` 已删除；`test_persist_lock` 仍被 backup/persist/env_var 测试使用，固定路径尚未全部改 `TempDir` |
| L-13 | **未修** | 启动仍全量重建 `daily_stats` / `var_references`；`search_fts` 只在为空时重建                                                                         |
| L-14 | 已修     | 新增 `cli/src/output.rs::print_json`，替换 9 处 `serde_json::to_string_pretty(...).unwrap()`；序列化失败以 1 退出                                    |
| L-15 | 已修     | `gui/src/commands/history.rs:300` 已直接用 `mark_profile_applied_by_name`，不再 list + find                                                          |
| L-16 | 已修     | README 测试徽章改为 `720 passed`（322 Rust + 398 前端）；coverage 徽章指向 `main`                                                                    |

### 3.4 结构精简（用户问题 3.1）

| 编号 | 状态 | 说明                                                                                                                                |
| ---- | ---- | ----------------------------------------------------------------------------------------------------------------------------------- |
| S-1  | 已修 | 删除 `gui/src/commands/profiles.rs`                                                                                                 |
| S-2  | 已修 | 删除 `_pendingSys` / `_pendingUser` 字段与全部引用                                                                                  |
| S-3  | 部分 | 删除 v5 IPC 与 3 个未注册前端包装；`stats_rebuild` / `refs_rebuild` / `search_rebuild` 仍暴露，未做「注册表 vs backend.ts」差集测试 |
| S-4  | 已修 | `gui/src/commands/m2.rs` → `analytics.rs`                                                                                           |
| S-5  | 未做 | `gui/src/commands/history.rs` 仍含 profile/backup 命令                                                                              |
| S-6  | 未做 | `core/src/backup.rs` 未拆分                                                                                                         |
| S-7  | 未做 | `core/src/registry/env_var.rs` 的 normal/force 写路径未合并                                                                         |
| S-8  | 未做 | `cli/src/main.rs` 未拆 clap 定义与命令实现                                                                                          |
| S-9  | 未做 | `cli/src/env_ops.rs` 未按子命令拆分                                                                                                 |
| S-10 | 未做 | `src/services/backend.ts` 未拆分                                                                                                    |
| S-11 | 未做 | `core/src/reg_store.rs` 仍公开                                                                                                      |
| S-12 | 已修 | v5 孤岛直接删除（比归档到 `legacy_v5` 更彻底，符合用户「完全舍弃 v5」的要求）                                                       |
| S-13 | 已修 | `exit_persist_error` 合并到 `exit_core_error`                                                                                       |
| S-14 | 已修 | 抽出 `print_json`，消除 9 处序列化 `unwrap`                                                                                         |
| S-15 | 部分 | DB 快照相关固定路径已随 `disabled.json` 移除而消失；其余 `test_persist_lock` 仍在                                                   |

### 3.5 新增 / 调整的测试

- `core/tests/db_path_save_state.rs`（新增，140 行）：CLI 保存后 `current_snapshot` 更新、产生 `changes`、事件 `source = cli`。
- `core/tests/db_current_snapshot_read.rs`（由 `db_v5_read_from_db.rs` 重命名）：`load_path_snapshot_db` 读取 `app_state.current_snapshot_id`，并保留禁用条目。
- `core/src/service.rs::tests::event_result_classifies_hive_combinations`（新增）：覆盖 Applied+Applied、Applied+Failed、Failed+Failed、Skipped+Failed、Skipped+Skipped。
- 删除 v5 专用测试与 fixture：`profile_legacy_v1`、`profile_future_version_rejected`、`merge_legacy_*`、`merge_snapshot_order_wins`、v5 导出/导入边界测试。
- 前端删除 3 个未注册 IPC 包装及对应 e2e mock 分支；单测数仍为 398。

---

## 4. 验证证据

| 命令                                                    | 结果                                                                                      |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `cargo test --workspace`                                | **322 passed / 0 failed / 2 ignored**（57 个测试二进制汇总）                              |
| `cargo test -p patheditor-cli --bins`                   | **66 passed / 0 failed**                                                                  |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0                                                                                    |
| `cargo fmt --all -- --check`                            | exit 0                                                                                    |
| `npm test -- --run`                                     | **33 files / 398 passed**                                                                 |
| `npx tsc --noEmit`                                      | exit 0                                                                                    |
| `npx prettier --check <changed files>`                  | All matched files use Prettier code style                                                 |
| 注释考古扫描（§3.1 命令）                               | **0 命中**                                                                                |
| `npm run build`                                         | 在 `96e9976` 后通过；`cca805e` 的前端改动只跑了 `tsc --noEmit` + Vitest，未重跑 Vite 构建 |
| `npm run test:e2e`                                      | **未运行**                                                                                |
| `npm run verify:all`                                    | **未运行**                                                                                |

> 绿灯不等于规格全覆盖：E2E、真实 GUI + 注册表闭环、Tauri 打包均未在本轮验证。第 5 节如实列出。

---

## 5. 未执行项（如实列出）

1. **目录迁移 Wave 5.2**：`src/{core,cli,gui}` 未开始；Tauri 配置发现 spike 未做。
2. **文件拆分 S-5–S-11**：`backup.rs`、`cli/src/main.rs`、`cli/src/env_ops.rs`、`gui/src/commands/history.rs`、`src/services/backend.ts`、`reg_store.rs` 均保持原状。
3. **L-9 环境变量对比**：字段仍恒为空，UI 尚未明确显示「本版本不支持」。
4. **L-10 双错误模型**：`String` 错误仍在 `EnvHiveStore` / `backup_registry` / `fs`。
5. **L-13 派生表增量重建**：启动仍全量重建 `daily_stats` / `var_references`。
6. **S-3 完整 IPC 差集**：没有自动比对 `invoke_handler` 与 `backend.ts` 方法集合。
7. **S-15 测试固定路径**：`test_persist_lock` 仍存在。
8. **E2E、真实 GUI + 注册表闭环、`npx tauri build --debug`**：均未运行。
9. **推送、合并、版本号**：未推送，未升级版本号，未打 tag。
10. **README 截图**：用户明确表示自行替换，本轮未动图片。

---

## 6. 兼容性变化与残余风险

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

**本轮可以进入复审，但不能声称「用户三项诉求全部完成」。**

- 注释考古层：完成，可验收。
- v5 过渡层：完成移除，可验收；破坏性变化需用户确认。
- 逻辑 P0：全部修复，可验收。
- 逻辑 P1：4/6 修复；L-9、L-10 仍未修。
- 结构精简：S-1/S-2/S-4/S-12/S-13/S-14 完成；文件拆分 S-5–S-11 未做。
- 目录迁移：未做，是用户问题 2 的唯一未完成项。

### 待用户/审核窗口拍板

1. **是否接受 v5 数据不迁移的破坏性变化**（影响从 v5 直接升级的用户）。
2. **是否现在做 Wave 5.2 目录迁移**；若做，先批准 Step 0 spike（只移动 `gui`，跑 `npx tauri info` / `npx tauri build --debug`）。
3. **是否推送 `refactor/v6-cleanup` / 合并 / 升级版本号**；本轮未获新的推送授权。
4. **L-9 的处置**：v6.1 实现环境变量对比，还是删除三个恒空字段并在 UI 明示不支持。

### 建议的下一步顺序

1. 先让本分支通过复审并合并（行为修复优先落地）。
2. 单独开 `refactor/v6-layout-spike` 做目录迁移 Step 0；通过后再整体搬迁。
3. 目录稳定后，按 S-6 → S-8/S-9 → S-10 的顺序拆文件（先 Rust 后前端）。
4. v6.1 处理 L-9、L-10、L-13 与注释 CI 门禁。
