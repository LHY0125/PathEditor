# PathEditor v6.0.0 M4 开发回执

**提交窗口**: 开发窗口
**日期**: 2026-10-21
**交付对象**: 审核窗口
**分支**: `main`
**版本**: 6.0.0
**状态**: 开发完成，`npm run verify:all` exit 0，**未推送**

---

## 0. 事故与异常

| #   | 事件                                                                                                 | 影响                 | 处理                                                                                                                                                                       |
| --- | ---------------------------------------------------------------------------------------------------- | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A-1 | 一次测试统计出现 `failed=6`，但同一份代码随后连跑两次都是 0 failed                                   | 无（数据本身是假的） | 根因是**两个 `cargo test --workspace` 并发**：`disabled.rs` / `profiles.rs` 的单元测试用固定临时路径，两个进程互相踩。之后所有全量测试均串行执行，并改为把输出落文件再解析 |
| A-2 | CLI 冒烟时用 `cmd \| head` 取 `$?`，把 `head` 的 0 当成命令退出码，一度误以为「缺 `--yes` 也返回 0」 | 差点写出错误结论     | 去掉管道重测，实际退出码 **1**（符合 spec §6.5.3）；回执里所有退出码均来自无管道调用                                                                                       |
| A-3 | 归档 dry-run 的 CLI 文案把**目标路径**打印成「将归档 `<目标>`」，读起来像在归档一个 `.v5.bak` 文件   | 仅文案               | 改成「将归档到」；见 §3 偏离 D-9                                                                                                                                           |

没有发生数据损坏、没有触碰注册表、没有对真实 `~/.patheditor` 执行任何写操作。

---

## 1. 交付内容（逐 Task）

### Task 1 — profile 落库 + 依赖排序（`034ffbe`）

- `APPLICABLE_ENTITY_TYPES` 扩到全部 5 种已知类型。
- 新增 `entity_priority`：导入前按 `snapshot → profile → profile_revision → path_entry → env_value`
  **稳定排序**，包内顺序颠倒也能解析 `snapshotUid`。
- 新增 `latest_for_entity_tx`：`snapshot` / `profile` 走**实体级 LWW**，其余仍按 field。
- 新增 `profiles::import_profile_content_tx`：按远端 uid 落库，`snapshotUid` 解析成本地主键，
  悬空引用 → `SyncFormat` 整包回滚；`(profile_id, version)` upsert。
- 新增 `core/tests/db_sync_profile_order.rs`（profile 写在 snapshot 前面仍能应用）。
- 按核对轮裁断改写 `core/tests/db_sync_import_limits.rs`（见 §3 D-2）。
- 更正 `core/tests/db_sync_conflict.rs` 的过期注释。

### Task 2 — 同名自动改名 + `renamedProfiles`（`d1d853f`）

- `unique_name_tx`（私有）：`"{name} (来自 {device 前 8 位})"`，仍冲突则追加 ` 2`…；
  **按字符截取设备 id**（多字节输入不会在字符边界 panic）。
- `ImportReport.renamed_profiles`；`apply_entity_tx` 返回「是否改名」。
- 前端：`SyncImportReport.renamedProfiles` + `parseSyncImportReport` 校验 + `renderNotice` 新分支
  - 中英 i18n；CLI 在 stderr 明示改名条数；E2E mock 同步更新。
- 新增 `core/tests/db_sync_profile_name_conflict.rs`。

### Task 3 — 墓碑软删除（`2d6ba81`）

- `apply_tombstone_tx`：`content = null` **且** `field = "deleted_at"` → 软删除 `profiles` / `snapshots`；
  其它实体类型返回 `SyncFormat`（不静默忽略）。
- 新增 `core/tests/db_sync_profile_tombstone.rs`（含「名称被释放」断言）。

### Task 4 — 墓碑与旧内容时序（`0ef05b5`）

- 新增 `core/tests/db_sync_profile_resurrect.rs`。
- **反向验证**：临时把实体级 LWW 换回字段级，测试在 `assert_eq!(report.skipped, 1)` 处转红
  （实测 `left: 0, right: 1`），还原后转绿。`git diff core/src/db/sync/import.rs` 为空，确认还原彻底。

### Task 5 — 幂等回归（`74d2d51`）

- 新增 `core/tests/db_sync_profile_idempotent.rs`：第二次导入 `applied=0`、`skipped=2`、
  `profiles` 1 行、`profile_revisions` 1 行。

### Task 6 — 保留策略清理（`9121322`）

- 新增 `core/src/db/maintenance.rs`：`prune` / `prune_plan` / `prune_now`，
  谓词 `exported_at IS NOT NULL AND applied_at > 0 AND applied_at < ?`（待导出队列永不删）。
- GUI 启动序列 ④b 调用一次，失败只告警。
- CLI `maintenance prune [--dry-run]`（用 `open_default + migrate_to_current`，见 §3 D-6）。
- 新增 `core/tests/db_maintenance_prune.rs`。

### Task 7 — `.v5.bak` 归档（`1d604cf`）

- 新增 `core/src/db/archive_v5.rs`：只改名、目标已存在则跳过、绝不覆盖；
  `dry-run` 纯读；候选按路径排序（输出去可复现）；`read_dir` 命中目录时用 `is_file()` 排除。
- GUI：**放在 `setup_database` 末尾**（见 §3 D-7，核对轮裁断 S-2）。
- CLI `maintenance archive-v5 [--dry-run]`。
- 新增 `core/tests/db_archive_v5.rs`。

### Task 8 — 遗留明文备份清理（`26dcc4d`）

- `cleanup_legacy_env_backups(dir, material, dry_run)`：解析 → 写 `.pebak` → **回读校验** → 删 `.json`；
  任一失败保留原文件。
- 解析不复用 `backup::read_env_backup`（它会隔离/改名文件，dry-run 就不再纯读）。
- 新增 `scan_legacy_env_backups(dir)`：CLI 的 dry-run 入口，**不需要密钥**（见 §3 D-8）。
- 严格文件名匹配 `env_backup_YYYYMMDD_HHMMSS_mmm.json`，`env_backup_x.json` 不碰。
- CLI `maintenance cleanup-legacy-backups [--dry-run] [--yes]`；无 `--yes` 打印提示并以 **1** 退出。
- 测试内联在 `core/src/db/maintenance.rs`（DPAPI，不建 SQLCipher），该文件只有 1 个 `#[test]`。

### Task 9 — v5 配置导入（`5f18dbc`）

- 新增 `core/src/db/v5_import.rs`：`import_profile_v5(db, path) -> String`，复用
  `profiles::load_profile_from` 的 `Versioned` 信封语义。
- 名称冲突返回 `NameExists`（**不自动改名**：用户指定了文件，改名会让「导入了什么」不可预期）。
- CLI `profile import <FILE>`。
- 新增 `core/tests/db_v5_import.rs`。

### Task 10 — 文档与 CHANGELOG（`0aa6214`）

- `CHANGELOG.md`：把「未发布（v6.0.0 里程碑）」改为 `## 6.0.0 (2026-10-21)`，
  新增 M4 小节（新增 / 变更 / 修复），M1/M2/M3 的「（开发中）」去掉。
  这样 `release.yml` 的版本段落正则可以命中。
- `README.md`：CLI 命令表补 `profile import` 与三条 `maintenance`；
  同步章节补配置同步/墓碑/整实体 LWW；修掉「v6.0.0 尚未交付」等过时表述。

### Task 11 — 版本号与验收（`ecf300e` + 本回执）

- 四处版本号 → 6.0.0（`package.json` / `Cargo.toml` / `gui/tauri.conf.json` / `README.md`），
  `package-lock.json` 与 `Cargo.lock` 同步。
- `gui/tauri.conf.json` 的窗口标题 `PathEditor v5.1.7` → `v6.0.0`。
- `npm run verify:all` **exit 0**（详见 §4）。

---

## 2. 设计决策（选了什么 + 为什么）

| #   | 决策                                                                     | 为什么                                                                                                                                  |
| --- | ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| D1  | 依赖排序用 `sort_by_key` 稳定排序，放在事务外                            | 稳定排序保证「同类型内部维持包内顺序」，同一实体多次变更时最后的 `content` 才是当前值；放事务外避免持锁做纯内存工作                     |
| D2  | `latest_for_entity_tx` 与 `latest_for_tx` 共用 `map_change_row`          | `ChangeRow` 有 13 个列位，两处各写一份、将来加列只改一半就会静默错位                                                                    |
| D3  | 已知类型全部可落库后**保留** `skipped_unsupported` 分支与字段            | 将来新增实体类型但未实现落库时，这条 `continue` 是防止 uid 被永久占住的唯一闸门（R-1 的第二层陷阱）                                     |
| D4  | 墓碑只认 `content = null` **且** `field = "deleted_at"`                  | 只有这两个条件同时成立才是删除意图；其它 `content = null` 是「内容随父实体」的普通变更                                                  |
| D5  | 改名后缀按 `chars().take(8)`                                             | `device_id` 来自外部包，按字节切片会在多字节字符中间 panic                                                                              |
| D6  | 维护 CLI 用 `open_default + migrate_to_current`，不用 `open_and_migrate` | 后者会顺带执行 v5 文件导入——`--dry-run` 不该以写入作为副作用                                                                            |
| D7  | 归档移到 `setup_database` **末尾**                                       | 归档会重命名 `disabled.json`，而 `load_path_snapshot()` 在降级模式/无当前快照时仍回落读它；放最后保证中途失败都保留兜底（核对轮 S-2）   |
| D8  | 新增 `scan_legacy_env_backups`，dry-run 不构造密钥                       | 若为了满足 `&KeyMaterial` 而造一个全零密钥，将来代码顺序一改就会用零密钥真的加密；`Option<&KeyMaterial>` 让「忘了传密钥却去写」不可表达 |
| D9  | 归档 CLI 文案「将归档**到**」                                            | `archived` 存的是目标路径；原文案读起来像在归档 `.v5.bak` 文件（冒烟时实测发现）                                                        |
| D10 | `import_profile_v5` 名称冲突**不**自动改名                               | 与同步导入有意不同：同步是远端数据，改名是唯一可行的合并；用户手工指定文件时改名会让结果不可预期                                        |

---

## 3. 偏离记录（计划原文 / 实际 / 处理）

| #    | 计划原文                                                 | 实际                                                                                              | 处理                                                                             |
| ---- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| D-1  | `cargo test -p path_editor_core ...`                     | 包名是 **`path-editor-core`**（连字符），计划里的下划线写法报 `did not match any packages`        | 全程用连字符                                                                     |
| D-2  | Task 1 Step 6「既有同步测试全部 passed」                 | `db_sync_import_limits.rs` 断言的是 M3 的旧行为，Task 1 后必红                                    | 按核对轮裁断改写：守「content=null 只记变更」+「未知类型整包拒绝」（核对轮 S-1） |
| D-3  | `latest_for_entity_tx` 复制一份 mapper                   | 抽出 `map_change_row` 共用                                                                        | 见 §2 D2                                                                         |
| D-4  | `unique_name_tx` 标 `pub(crate)`                         | 只在 `profiles.rs` 内调用，改为私有                                                               | 核对轮 C-3                                                                       |
| D-5  | Task 4「Expected: PASS」                                 | 该测试天然没有 RED 阶段                                                                           | 补一次反向验证（换回字段级 LWW → 转红），核对轮 C-4                              |
| D-6  | Task 6 CLI 用 `open_and_migrate()`                       | 改用 `open_default() + migrate_to_current()`                                                      | 核对轮 C-1                                                                       |
| D-7  | Task 7 归档放在 ③b 之后                                  | 移到 `setup_database` 末尾（⑤/⑥ 之后）                                                            | 核对轮 S-2                                                                       |
| D-8  | Task 8 CLI 先 `load_or_create_key` 再判 dry-run          | dry-run 走新增的 `scan_legacy_env_backups`，完全不加载密钥                                        | 核对轮 C-2                                                                       |
| D-9  | Task 7 CLI 打印「将归档 {dst}」                          | 改为「将归档**到** {dst}」                                                                        | 冒烟发现歧义                                                                     |
| D-10 | Task 7 `archive_v5_sources` 直接收集 `extension == json` | 增加 `is_file()`（排除名为 `xxx.json` 的目录）与 `candidates.sort()`（输出去可复现）              | 防御性改进，行为不变                                                             |
| D-11 | Task 10「在 CHANGELOG 顶部加 `## 6.0.0`」                | 直接把已有的「未发布（v6.0.0 里程碑）」标题改成 `## 6.0.0 (2026-10-21)`，避免两段并行说明同一版本 | 语义等价，且 release.yml 的正则能命中                                            |
| D-12 | `db_sync_conflict.rs:26` 注释                            | 原文「profile 属于格式合法但当前版本不落库」在 M4 后不成立                                        | 只改注释，**不动断言**（核对轮 C-5）                                             |
| D-13 | Task 8 `cleanup_legacy_env_backups` 的签名               | 保持 spec §6.5.3 的形状不变；dry-run 由 `material: None` 的内部实现表达                           | 见 §2 D8                                                                         |

---

## 4. 测试与质量门

### 4.1 每个 Task 都跑过的门（未攒到最后）

```text
cargo fmt --all -- --check                              → FMT_OK（每个 Task）
cargo clippy --workspace --all-targets -- -D warnings   → 0 error / 0 warning（每个 Task）
cargo test --workspace                                  → 0 failed（每个 Task）
npx tsc --noEmit                                        → 无输出（每个 Task）
npx vitest run                                          → 全绿（每个 Task）
```

### 4.2 测试数变化（无一次下降）

| 节点   | Rust binaries / passed              | 前端 files / tests | 说明                         |
| ------ | ----------------------------------- | ------------------ | ---------------------------- |
| 基线   | 48 / 340 / 0 failed                 | 33 / 397           | 开工提示词 §二 声明值        |
| Task 3 | 51 / 343                            | 33 / 398           | T1 +1、T2 +1（含 1 条前端）  |
| Task 4 | 52 / 344                            | 33 / 398           | +1（墓碑时序）               |
| Task 5 | 53 / 345                            | 33 / 398           | +1（幂等）                   |
| Task 6 | 54 / 346                            | 33 / 398           | +1（保留策略）               |
| Task 7 | 55 / 347                            | 33 / 398           | +1（归档）                   |
| Task 8 | 55 / 348                            | 33 / 398           | +1（内联测试，不新增二进制） |
| Task 9 | 56 / 349                            | 33 / 398           | +1（v5 导入）                |
| 最终   | **56 / 349 / 0 failed / 2 ignored** | **33 / 398**       | `verify:all` 实测            |

> Task 1 / 2 之间没有单独统计：两者的测试文件在同一个编译窗口里创建，
> 单独统计会得到「编译失败」而非测试数。以 Task 3 的实测值反推（+3 = 1+1+1）是诚实的做法。

### 4.3 `npm run verify:all`（Task 11）

```text
EXIT = 0

Prettier      All matched files use Prettier code style!
ESLint        ✖ 3 problems (0 errors, 3 warnings)   ← 均为既有的 react-hooks/incompatible-library
构建          通过（vite build）
覆盖率        Statements 81.5% (652/800)
              Branches   72.33% (319/441)
              Functions  84.76% (128/151)
              Lines      82.7% (574/694)            ← 门槛 80%，通过
cargo fmt     FMT_OK
cargo clippy  0 error / 0 warning
cargo test    56 binaries / 349 passed / 0 failed / 2 ignored
Playwright    54 passed (1.5m)
```

### 4.4 一文件一测试核对

```text
db_sync_profile_order.rs           #[test]=1
db_sync_profile_name_conflict.rs   #[test]=1
db_sync_profile_tombstone.rs       #[test]=1
db_sync_profile_resurrect.rs       #[test]=1
db_sync_profile_idempotent.rs      #[test]=1
db_maintenance_prune.rs            #[test]=1
db_archive_v5.rs                   #[test]=1
db_v5_import.rs                    #[test]=1
maintenance.rs（内联）             #[test]=1
```

### 4.5 S-1 自查（每个新增 `pub fn` 都有生产调用者）

先用已知有调用者的 `dashboard(` 验证 grep 模式有效，再逐个查：

| 函数                         | 生产调用者                       |
| ---------------------------- | -------------------------------- |
| `prune`                      | `prune_now` + CLI                |
| `prune_plan`                 | CLI                              |
| `prune_now`                  | GUI 启动序列                     |
| `archive_v5_sources`         | GUI 启动序列 + CLI               |
| `cleanup_legacy_env_backups` | CLI                              |
| `scan_legacy_env_backups`    | CLI（dry-run）                   |
| `import_profile_v5`          | CLI `profile import`             |
| `import_profile_content_tx`  | `import.rs` 的 `apply_entity_tx` |

### 4.6 命令行冒烟（真实环境，**只做纯读**）

```text
patheditor maintenance archive-v5 --dry-run
  → 将归档到 C:\Users\33644\.patheditor\disabled.json.v5.bak
  → 将归档到 C:\Users\33644\.patheditor\profiles\2026-09-27.json.v5.bak
  → （dry-run）共 2 个，跳过 0 个          [未改名]

patheditor maintenance cleanup-legacy-backups --dry-run
  → 9 个 env_backup_*.json 将转换           [未写、未删]

patheditor maintenance cleanup-legacy-backups
  → 错误: 这会删除明文备份。…              [退出码 1（无管道实测）]

patheditor maintenance --help / prune --help / profile import --help
  → 子命令与参数正确注册
```

---

## 5. 未覆盖项（如实列出）

| #   | 未覆盖                                                            | 原因                                                                                    |
| --- | ----------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| U-1 | **`maintenance prune --dry-run` 没有对真实 `~/.patheditor` 执行** | 它会 `open_default()`，在库不存在时会**创建**真实数据库文件。集成测试已覆盖同一逻辑     |
| U-2 | **整个 GUI 启动序列没有端到端运行**（④b prune、⑦ 归档）           | 开工提示词 §三.6 明令禁止启动真实 GUI：`setup_database` 会在真实 `~/.patheditor` 上归档 |
| U-3 | **`cleanup-legacy-backups --yes` 的实跑没有执行**                 | 它会删除用户机器上 9 个真实明文备份；dry-run 已确认候选与解析路径正确                   |
| U-4 | 跨设备同步没有真机双端验证                                        | 用 `TempDir` 双库集成测试替代；无第二台机器                                             |
| U-5 | MSVC 工具链未验证                                                 | 本机是 GNU；仓库 CI 不跑测试（只有 tag 触发的发布流水线）                               |
| U-6 | E2E 不覆盖 `maintenance` 子命令                                   | CLI 无 E2E 框架；E2E 只覆盖前端流程                                                     |
| U-7 | `gui/tauri.conf.json` 的窗口标题没有目视确认                      | 未启动 GUI                                                                              |
| U-8 | 敏感值掩码（M3 的 R-2）未变                                       | 不在 M4 范围                                                                            |

---

## 6. 残余风险

| #   | 风险                                                                                                    | 状态/缓解                                                                           |
| --- | ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| R-1 | 保留策略每次启动执行 `DELETE`，`changes` 无 `applied_at` 索引 → 数据量大时全表扫描                      | M4 接受（数据量小）；若耗时增长需 schema v4 加索引                                  |
| R-2 | 归档在**每次**启动执行（幂等，但 `profiles/*.json` 目录里用户手工放入的 JSON 也会被改名）               | spec §7.4 的明确要求；只改名不删除，`.v5.bak` 可随时改回                            |
| R-3 | 用户机器上当前有 **2 个 v5 原件 + 9 个遗留明文备份**，首次启动 GUI 会归档；明文清理仍需用户手工 `--yes` | 设计如此；本轮回执已记录现状                                                        |
| R-4 | 配置合并是**整实体 LWW**：两台设备改同一配置的不同字段会互相覆盖                                        | spec §6.5.5 明确不在 M4 范围；README 已写明「已知限制」                             |
| R-5 | 自动改名把中文 `(来自 …)` 写进 `profiles.name`（持久数据）                                              | spec §6.5.1 的规定；英文界面下也会出现中文名。将来要本地化需要数据迁移              |
| R-6 | i18n 键 `sync.noticeImportedUnsupported` 在 M4 后不可达                                                 | 有意保留（将来新增类型时复用）；不删                                                |
| R-7 | `CLAUDE.md` / `AGENTS.md` 与 spec §十二「字节级一致」冲突                                               | 当前 `CLAUDE.md` 自述「不要求逐字节一致」；**本轮未修改任何 CLAUDE.md**，由用户裁断 |
| R-8 | `import_profile_v5` 复用 `load_profile_from`，后者对损坏文件会**隔离改名**                              | 用户显式导入时的合理行为；与 sync 导入的「不隔离」不同                              |
| R-9 | Argon2 参数写死、口令强度无校验（M3 的 R-4 / R-5）                                                      | 未变                                                                                |

---

## 7. 需要审核窗口重点关注的项

1. **S-1 的测试改写口径**（§3 D-2）：`db_sync_import_limits.rs` 现在守的是
   「content = null 的已知类型只记变更、不造实体」+「未知类型整包拒绝」。
   若你希望它守别的，请给出具体断言。
2. **S-2 的归档位置**（§3 D-7）：已挪到 `setup_database` 末尾。请复核
   「⑤ 之前任何失败都保留 `disabled.json` 兜底」这条推理是否成立。
3. **D8 的 `scan_legacy_env_backups`**：新增了一个公开函数。若你认为公共面不该多这一项，
   可以改成 `cleanup_legacy_env_backups` 收 `Option<&KeyMaterial>`（但那会改动 spec §6.5.3 的形状）。
4. **Task 4 的反向验证证据**（§1 Task 4）：`left: 0, right: 1` 是「字段级 LWW 会复活配置」的直接证据。
5. **§5 U-1 / U-3**：两条真实环境操作我**没有执行**，请确认这符合你期望的边界。

---

## 8. 交付物位置索引

| 交付物           | 路径                                                                                                                                                   |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 核对轮交接       | `docs/审核和开发/2026.10.21/PathEditor-v6-M4-核对轮交接.md`                                                                                            |
| 开发回执（本文） | `docs/审核和开发/2026.10.21/PathEditor-v6-M4-开发回执.md`                                                                                              |
| 配置同步落库     | `core/src/db/sync/import.rs`、`core/src/db/profiles.rs`                                                                                                |
| 保留策略         | `core/src/db/maintenance.rs`                                                                                                                           |
| v5 归档          | `core/src/db/archive_v5.rs`                                                                                                                            |
| v5 导入          | `core/src/db/v5_import.rs`                                                                                                                             |
| CLI 维护命令     | `cli/src/maintenance_ops.rs`                                                                                                                           |
| 新增测试（8 个） | `core/tests/db_sync_profile_{order,name_conflict,tombstone,resurrect,idempotent}.rs`、`db_maintenance_prune.rs`、`db_archive_v5.rs`、`db_v5_import.rs` |
| 文档             | `README.md`、`CHANGELOG.md`                                                                                                                            |

---

## 9. 提交清单与状态

```text
ecf300e chore(v6): 版本号升至 6.0.0
0aa6214 docs(v6): M4 功能与 6.0.0 发布日志
5f18dbc feat(v6): 支持从 v5 profiles/*.json 导入配置
26dcc4d feat(v6): 遗留明文 env 备份先转 .pebak 再删除
1d604cf feat(v6): v5 原件归档为 .v5.bak（位置改到启动序列末尾）
9121322 feat(v6): 保留策略清理（changes 90 天 / 冲突 30 天）
74d2d51 test(v6): 配置同步重复导入保持幂等
0ef05b5 test(v6): 墓碑不会被低 lamport 的旧内容复活
2d6ba81 feat(v6): 同步导入支持配置墓碑软删除
d1d853f feat(v6): 配置同步遇到同名时自动改名并在导入提示中计数
034ffbe feat(v6): M4 配置同步落库与依赖排序
1c0c294 docs(v6): M4 核对轮交接（S-1/S-2 已裁断，C-1~C-5 按建议执行）
```

- 分支 `main`，共 **12 个提交**（11 个 Task 提交 + 1 个核对轮交接文档）。
- **未打 tag、未创建 Release、未推送**（推送由用户确认后执行）。
- 工作区在写完本回执后应保持干净（回执本身单独提交）。

---

**签章**：本文中每条「已通过」都对应 §4 里一条实际执行过的命令与输出；
未执行的在 §5，未实现的与已知限制在 §6。没有推测性结论。
