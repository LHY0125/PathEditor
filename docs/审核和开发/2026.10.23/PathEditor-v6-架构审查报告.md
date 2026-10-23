# PathEditor v6 架构审查报告（core 边界专项）

- **审查日期**：2026-10-23
- **被审分支 / 提交**：`main` @ `56e5158`（v6.1.0 发布后）
- **范围**：`src/core`、`src/cli`、`src/gui/src-tauri`、`src/gui/web`
- **审查主题**：
  1. CLI 与 GUI 的核心组件是否都已放进 core；
  2. 分层架构、依赖方向、模块边界的复审。
- **审查方式**：静态阅读 + 依赖检查 + 质量门复跑；**本轮只审不改**。
- **判定**：**有条件通过**——核心业务约 85% 已在 core；仍有 3 项 P0、8 项 P1、8 项 P2 需要收口。

> 绿灯不等于边界正确。`cargo test` 与 `npm run verify` 全绿只能证明现有行为被覆盖，不能证明「业务规则只存在一份」。

---

## 一、验证证据

| 命令                                                    | 结果                                  |
| ------------------------------------------------------- | ------------------------------------- |
| `cargo test --workspace`                                | **320 passed / 0 failed / 2 ignored** |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0                                |
| `npm run verify`（发布提交上）                          | exit 0                                |
| `npm test -- --run`                                     | **34 files / 399 passed**             |
| `npm run test:e2e`                                      | **54 passed**                         |
| core 依赖检查（`src/core/Cargo.toml`）                  | 无 `tauri` / `clap` / 前端依赖        |
| `invoke(` 在 `services/backend` 之外的命中              | **0**                                 |

---

## 二、core 边界审计（诉求 1）

### 2.1 已经正确下沉的部分

| 能力                                               | core 位置                          | 外层只做                     |
| -------------------------------------------------- | ---------------------------------- | ---------------------------- |
| 注册表读写、校验、清理                             | `registry/{path,env_var,store}.rs` | 参数转换、错误呈现           |
| PATH 保存编排（备份/并发/顺序写/广播/sidecar）     | `service.rs`                       | 传 `PathHiveWrite`、映射文案 |
| 环境变量 CRUD、保留名/保护名单/revision            | `registry/env_var.rs`              | 展示、退出码                 |
| DB schema/历史/配置/备份索引/检索/统计/引用图/同步 | `db/*`                             | 取 `Db`、调函数              |
| 备份/恢复/轮换                                     | `backup/*`                         | 参数转发                     |
| 扫描与冲突                                         | `scanner.rs`                       | 参数转发                     |
| 文件导入导出格式                                   | `fs.rs`                            | 读文件、写输出               |

依赖方向正确：`core` 不依赖 `tauri` / `clap`；GUI/CLI 单向依赖 core。

### 2.2 仍在 CLI / GUI 的核心组件

#### A1（P0）配置应用流程：CLI 与 GUI 两套实现，且 GUI 不记账

| 实现 | 位置                                                            | 行为                                                        |
| ---- | --------------------------------------------------------------- | ----------------------------------------------------------- |
| CLI  | `src/cli/src/profile_ops.rs:85`                                 | 读配置 → `save_path_write` → `mark_profile_applied_by_name` |
| GUI  | `src/gui/web/src/components/dialogs/profile/use-profiles.ts:71` | 读配置 → `replaceBothPaths` → `savePaths()`，**不记账**     |

后果：

1. **功能不一致**：CLI 应用配置后「应用次数/最后应用时间」会更新，GUI 不会。同一个 `profiles` 表两套语义。
2. **死命令**：`profile_mark_applied` / `profile_mark_applied_by_name`（`commands/profiles.rs:32`、`:149`）的 IPC 注册与前端包装（`backend/index.ts:129`、`:253`）都没有业务调用者。IPC 契约测试只校验「注册 ↔ 包装」一一对应，**发现不了「包装无人调用」**。
3. **编排外泄**：`handleApply` 里的「读配置 → 写注册表 → 关窗」是业务流程，写在 React hook 里，CLI 无法复用。

**下沉方案**：

```rust
// core::service
pub fn apply_profile(
    db: &mut Db,
    name: &str,
    source: EventSource,
) -> Result<ApplyOutcome, CoreError>
```

内部：`profiles::profile_paths` → `save_path_with_db` → `mark_profile_applied_by_name`（失败只告警，与 CLI 现语义一致）。GUI 命令改为一行调用；前端 `handleApply` 只保留确认弹窗与文案。

---

#### A2（P0）CLI 侧存在安全判定：导出目标目录黑名单

- 位置：`src/cli/src/import_export.rs:34-38`
- 内容：`c:\windows\`、`c:\program files\` 前缀判定，命中则拒绝导出。
- 同一规则在 core 已有一份：`src/core/src/backup/path.rs:77-84`。

后果：

1. 违反仓库硬约束「CLI 侧零安全判定」。
2. 两份黑名单会漂移：core 改了校验、CLI 没改（或反之），用户看到的拒绝范围不一致。
3. CLI 用的是字符串替换 + 小写化，core 用的是同样的写法——**目前一致只是巧合，没有编译期约束**。

**下沉方案**：

```rust
// core::fs
pub fn write_export_file(path: &Path, content: &str) -> Result<(), CoreError>
```

把「系统目录黑名单 + 写文件」收进 core；`cmd_export` 只调它并把错误交给 `exit_core_error`。

---

#### A3（P0）GUI 启动序列在 GUI 层，且直接执行 SQL

- 位置：`src/gui/src-tauri/src/lib.rs:9-144`
- 内容：`setup_database`、`rebuild_derived_tables`、`ensure_current_snapshot_baseline`。
- 直接 SQL：
  - `lib.rs:70`：`SELECT count(*) FROM search_fts`
  - `lib.rs:99`：`SELECT count(*) FROM app_state a JOIN snapshots ...`

后果：

1. 违反「所有 DB 访问在 `src/core/src/db/`；GUI/CLI 不得直接打开 SQL」。
2. 启动引导是**应用核心流程**：迁移、pending 补写、基线快照、备份索引、保留策略、派生表重建。CLI 将来若需要同样的「首次使用基线」逻辑，无处复用。
3. 这段代码在 GUI crate 内无法被 core 的集成测试直接覆盖（只能靠 GUI 手工冒烟）。

**下沉方案**：

```rust
// core::bootstrap
pub struct BootstrapReport { /* 各步骤结果 */ }
pub fn initialize(db: &mut Db) -> Result<BootstrapReport, CoreError>
```

SQL 查询改为 `db::search::index_count` / `db::history::has_current_snapshot` 等 core 内部函数；GUI 的 `setup_database` 缩成「调用 + 打日志」。

---

#### A4（P1）同步口令非空校验：三处重复

- `src/gui/src-tauri/src/commands/sync.rs:51-57`（export）
- `src/gui/src-tauri/src/commands/sync.rs:70-76`（import）
- `src/cli/src/sync_ops.rs:32-35`

后果：这是**加密强度**相关判定（空口令等于不加密），三份实现任何一份漏掉都是安全缺陷。

**下沉方案**：`db::sync::export_bundle` / `import_bundle` 内部拒绝空口令（`InvalidValue`）；外层删除判定。core 已有错误类型与文案规范。

---

#### A5（P1）GUI 两处把 `CoreError` 降级为 `String`

- `src/gui/src-tauri/src/commands/disabled.rs:9`：`Result<PathSnapshot, String>`，且手写 `state.0.lock()` 与降级分支。
- `src/gui/src-tauri/src/commands/service.rs:22`：`Result<ApplyOutcome, String>`。

后果：

1. 与其余 DB 命令（`CoreError`）不一致；前端 `parseCoreError` 只能回落成 `internal`，丢失 `conflict` / `permissionDenied` 等判定。
2. `disabled.rs` 绕过了 `db_state::with_db` 的统一锁中毒/降级语义，多一份手写判定。

**下沉/修正方案**：

- 返回类型改 `Result<_, CoreError>`。
- 读取逻辑改为 `core::service::load_path_snapshot_any(db: Option<&Db>)`（见 A6）。
- 保存逻辑直接 `with_db_mut` + `save_path_with_db`，DB 不可用时 core 内的 `save_path_registry_only` 分支保留。

---

#### A6（P1）`load_path_snapshot` 的降级编排在命令层

- 位置：`src/gui/src-tauri/src/commands/disabled.rs:10-16`
- 内容：锁 → `Some(db)` 读 DB 快照，`None` 读注册表。

**下沉方案**：

```rust
// core::service
pub fn load_path_snapshot_any(db: Option<&Db>) -> Result<PathSnapshot, CoreError>
```

CLI 的 `runtime.rs:64` 已经有一份同构逻辑（`open_db_optional` + 两个分支），下沉后两边共用。

---

#### A7（P1）`profile_paths` 视图组合在命令层

- 位置：`src/gui/src-tauri/src/commands/profiles.rs:88-109`
- 内容：`get_profile_by_name` + `profile_paths` 两次查询，再拼 `ProfilePathsView`。

**下沉方案**：core 提供 `profiles::get_profile_paths_view(db, name) -> Result<Option<ProfilePathsView>, CoreError>`；视图类型移入 core（`Serialize + camelCase`），Tauri 命令只转发。

---

#### A8（P1）`backup_list` 的索引刷新编排在命令层

- 位置：`src/gui/src-tauri/src/commands/backup.rs:76-85`
- 内容：先 `index_backups`（失败只告警），再 `list_backups`。

**下沉方案**：`db::backups::refresh_and_list(db, dir) -> Result<Vec<BackupView>, CoreError>`，告警策略在 core 内统一。

---

#### A9（P1）`history_restore_snapshot` 的编排在命令层

- 位置：`src/gui/src-tauri/src/commands/history.rs:124-151`
- 内容：读快照 → 构造两个 `PathHiveWrite` → `save_path_with_db`。

**下沉方案**：

```rust
// core::service
pub fn restore_snapshot(db: &mut Db, uid: &str, source: EventSource)
    -> Result<ApplyOutcome, CoreError>
```

「恢复快照 = 读快照再保存」是产品语义，不是 IPC 参数转换。

---

#### A10（P1）`sync_status` 的组合在命令层

- 位置：`src/gui/src-tauri/src/commands/sync.rs:30-39`
- 内容：`pending_export().len()` + `unresolved_count()` + 构造 `SyncStatusView`。

**下沉方案**：`db::sync::status(db) -> SyncStatus`；视图类型放 core，CLI 的 `sync_status` 也改为调它（CLI 目前分别调两个函数，`sync_ops.rs:101-108`）。

---

#### A11（P1）`scan_history_record` 让 core 接收未校验的 JSON 字符串

- IPC：`src/gui/src-tauri/src/commands/analytics.rs:137-146`
- core：`db::scan_history::record_run(db, &str, &str)`
- 注释理由：「让命令对未来 `scan_paths` 形状变化免疫」。

**反对理由**：

1. core 是**唯一可信边界**，接收裸 JSON 意味着把「序列化正确性」交给前端。前端一旦传错形状，坏数据直接落库，core 无法拒绝。
2. CLI 也会走同一入口（注释自己承认），意味着 CLI 要自己序列化——又一处契约双写。
3. 「形状变化免疫」应该通过版本化契约解决，不是放弃校验。

**下沉方案**：参数改为结构化 `scanner::ScanResult`（core 已定义），由 core 负责序列化落库；若担心形状演进，用 `#[serde(default)]` 或显式版本字段。

---

#### A12（P2）`open_db` 的重复包装

- `src/cli/src/profile_ops.rs:11-13`
- `src/cli/src/sync_ops.rs:18-20`
- `src/cli/src/maintenance_ops.rs:12-17`
- `src/cli/src/runtime.rs:43-61`（`open_db_optional`，语义不同：不创建）

**下沉方案**：core 增加 `db::open_existing() -> Result<Option<Db>, CoreError>`（不存在返回 `Ok(None)`，不创建密钥/空库）；三个 `open_db` 改为调 `open_and_migrate`（已有）或新函数。

---

#### A13（P2）前端权限预检 `canWriteTarget` 的回退分支

- 位置：`src/gui/web/src/core/path-capabilities.ts:31-45`
- 内容：capabilities 全空时回退 `isAdmin`；否则按 `canWriteSystem` / `canWriteUser`。

后果：「哪个 hive 可写」的判定在前端有一份；Rust 侧 `capabilities::detect()` 是权威。回退分支会让「capabilities 加载失败」时的行为与 Rust 判定不一致。

**建议**：capabilities 只由 Rust 下发，前端删除 `isAdmin` 回退（或把回退标注为「仅测试/降级」并加单测钉住）。

---

#### A14（P2）组合长度软提醒在前端

- 位置：`src/gui/web/src/services/path-session.ts:102-116`
- 规则：`system + user` 启用路径拼接长度 > `appConfig.path.maxCombinedLength`。

**判断**：这条是**产品 UX 规则**（软提醒、可 force 放行），不是安全边界；core 的单 hive 32767 校验仍在 `registry/path.rs`。保留在前端可以接受，但应：

1. 在 `docs/dev/data-and-security.md` 明确「组合长度是 UX 规则，不是安全校验」；
2. 数值来源单一（`appConfig`），不要与 core 的 32767 混淆。

---

## 三、架构设计审计（诉求 2）

### 3.1 依赖方向

| 检查                  | 结果                              |
| --------------------- | --------------------------------- |
| core → tauri          | 无                                |
| core → clap           | 无                                |
| core → 前端           | 无                                |
| gui → core            | 单向                              |
| cli → core            | 单向                              |
| 前端组件直接 `invoke` | 0 处（全部经 `services/backend`） |

**结论：依赖方向正确**。唯一越界是 GUI 启动序列的 SQL（A3）。

### 3.2 模块边界与命名

- **`core::env_var` 与 `core::registry::env_var` 同名**：前者是契约/安全判定（`EnvHive`、`EnvVarMeta`、`RESERVED_NAMES`），后者是注册表 CRUD。公开 API 里两个名字都出现，调用方需要看路径才能区分。建议把前者改名 `env_var_meta` 或 `env_contract`，或把契约类型统一从 `registry::env_var` 重导出。
- **`core::service.rs` 1034 行**（含测试），是最大的非 db 模块。职责包括保存编排、快照读取、pending 补写、事件结果分类。按仓库「文件 <800 行」标准，建议拆 `service/{mod,save,snapshot,retry}.rs`。
- **`core::db::mod.rs` 257 行**，含 `Db` 句柄与 `open_default` / `open_and_migrate`；与 `db::crypto` 的职责划分清晰，可保留。

### 3.3 重复实现清单

| 规则                   | 位置 A                                  | 位置 B                   | 风险               |
| ---------------------- | --------------------------------------- | ------------------------ | ------------------ |
| 配置应用流程           | CLI `profile_ops.rs:85`                 | GUI `use-profiles.ts:71` | 行为漂移（A1）     |
| 同步口令非空           | GUI `sync.rs:51`、`:70`                 | CLI `sync_ops.rs:32`     | 安全规则三份（A4） |
| 导出系统目录黑名单     | CLI `import_export.rs:34`               | core `backup/path.rs:77` | 安全规则双份（A2） |
| 打开 DB                | CLI 三处 `open_db` + `open_db_optional` | core `open_and_migrate`  | 维护成本（A12）    |
| 读取快照的 DB/降级分支 | GUI `disabled.rs:10`                    | CLI `runtime.rs:64`      | 行为漂移（A6）     |

### 3.4 错误模型一致性

已统一为 `CoreError` 的：注册表、扫描、env 校验、备份、DB 命令。

仍降级为 `String` 的：

- `commands/disabled.rs:9`
- `commands/service.rs:22`

前端 `parseCoreError` 对两种形状都兼容，但降级会丢失 `code`，属于**已知不一致**。

### 3.5 死代码与死命令

| 项                                 | 位置                         | 说明                               |
| ---------------------------------- | ---------------------------- | ---------------------------------- |
| `disabled_paths`                   | `src/cli/src/runtime.rs:167` | `#[allow(dead_code)]`，无调用者    |
| `profile_mark_applied` IPC         | `commands/profiles.rs:32`    | 前端包装存在，无业务调用           |
| `profile_mark_applied_by_name` IPC | `commands/profiles.rs:149`   | 同上                               |
| 过期注释                           | `commands/analytics.rs:62`   | 「环境变量对比将在提供」无对应计划 |

> IPC 契约测试（`tests/unit/ipc-contract.test.ts`）只校验「注册表 ↔ 包装」一致，**不校验包装是否被业务调用**。死命令因此不会被它发现。

### 3.6 前端边界

- `services/backend/*` 只做 IPC + 运行时形状校验，符合约定。
- `services/path-session.ts` 保留的三件事（权限预检、组合长度、文案映射）中，前两件见 A13/A14。
- `store/app-store.ts` 的 `savePaths` 只做状态管理，未复制 core 编排，合规。

---

## 四、问题清单与优先级

### P0（必须修，影响正确性/安全边界）

| 编号 | 问题                                | 证据                                      |
| ---- | ----------------------------------- | ----------------------------------------- |
| A1   | GUI 应用配置不记账 + CLI/GUI 双实现 | `use-profiles.ts:71`、`profile_ops.rs:85` |
| A2   | CLI 侧导出目标安全判定              | `import_export.rs:34`                     |
| A3   | GUI 启动序列含 SQL 且未下沉         | `lib.rs:9-144`、`:70`、`:99`              |

### P1（应修，属于边界/契约问题）

| 编号 | 问题                                    |
| ---- | --------------------------------------- |
| A4   | 同步口令校验三处重复                    |
| A5   | GUI 两处错误降级为 String               |
| A6   | 快照读取降级编排在命令层                |
| A7   | `profile_paths` 视图组合在命令层        |
| A8   | `backup_list` 索引编排在命令层          |
| A9   | `history_restore_snapshot` 编排在命令层 |
| A10  | `sync_status` 组合在命令层              |
| A11  | `scan_history_record` 接收未校验 JSON   |

### P2（可延后）

A12 `open_db` 重复、A13 前端权限回退、A14 组合长度归属、A15 模块同名、A16 `service.rs` 体积、A17 死代码、A18 过期注释、A19 IPC 死命令、A20 IPC 测试盲区。

---

## 五、建议整改顺序

1. **A3**：下沉启动引导到 `core::bootstrap`——它决定 GUI/CLI 能否共享初始化语义。
2. **A1**：新增 `service::apply_profile`，GUI/CLI 共用；顺带删除两个死 IPC（或接线）。
3. **A2 + A4**：把安全判定收回 core（导出目标、空口令），外层删除复制。
4. **A5 + A6**：统一 GUI 快照读取的错误模型与降级分支。
5. **A7–A11**：逐个把「两次查询 + 视图构造」下沉到 core 的对应模块。
6. **A12–A20**：清理与拆分。

每步都应保持 `npm run verify` 与 IPC 契约测试全绿；A1/A9 需要补 core 层测试（GUI 侧的 React 测试无法证明记账）。

---

## 六、总结

**「核心组件都在 core」的判断：基本成立，但尚未收口。**

- 保存、注册表、环境变量、DB、同步、备份、扫描这些**主干**已经全部在 core，且依赖方向正确。
- 仍在外层的不是「实现细节」，而是**业务流程**：启动引导、配置应用、安全判定、口令校验、快照读取降级。
- 最值得优先处理的是 **A1**：它同时是架构问题（双实现）与功能缺陷（GUI 不记账），且暴露了 IPC 契约测试的盲区。

本报告只审不改。**整改已于同日完成**，逐项处置与提交见
`docs/审核和开发/2026.10.23/PathEditor-v6-架构审查整改回执.md`。
