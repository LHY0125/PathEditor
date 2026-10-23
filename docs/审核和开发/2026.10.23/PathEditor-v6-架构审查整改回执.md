# PathEditor v6 架构审查整改回执

- **回执日期**：2026-10-23
- **分支 / 提交**：`main`，HEAD `4a45c52`（架构整改最后一项）
- **基准点**：`56e5158`（v6.1.0 发布提交）
- **关联审查报告**：`docs/审核和开发/2026.10.23/PathEditor-v6-架构审查报告.md`
- **授权**：用户「按报告第五节的顺序开始整改」
- **范围**：11 个提交，覆盖审查报告的 3 项 P0、8 项 P1、8 项 P2

> 本回执只记录实际执行过的命令与结果。未做的项在第四节如实列出。

---

## 一、整改清单

### P0

| 编号 | 处置 | 提交      | 说明                                                                                                                                                                             |
| ---- | ---- | --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A1   | 已修 | `55bb812` | `core::service::apply_profile` 统一「读配置 → 只写变化的 hive → 记账」；GUI 应用配置现在会更新 `apply_count` / `last_applied_at`；删除两个无调用者的 `profile_mark_applied*` IPC |
| A2   | 已修 | `bd01736` | `core::fs::ensure_not_system_dir` 与 `write_export_file`；CLI 删除系统目录黑名单复制                                                                                             |
| A3   | 已修 | `54ca43a` | `core::bootstrap::initialize` 承接迁移/pending/基线/索引/保留策略/派生表；GUI `lib.rs` 删除两处裸 SQL                                                                            |

### P1

| 编号 | 处置 | 提交      | 说明                                                                                                        |
| ---- | ---- | --------- | ----------------------------------------------------------------------------------------------------------- | --- | ----------- |
| A4   | 已修 | `bd01736` | `export_bundle` / `import_bundle` 内部拒绝空口令；删除 GUI 两处判定，CLI 保留交互预检并注明 core 是权威     |
| A5   | 已修 | `23325e6` | `load_path_snapshot` / `save_path_with_sidecar` 返回 `CoreError`，删除两处 `.map_err(                       | e   | e.message)` |
| A6   | 已修 | `23325e6` | `core::service::load_path_snapshot_any(Option<&Db>)`；GUI/CLI 共用；`db_state::with_optional_db` 统一锁语义 |
| A7   | 已修 | `453f42d` | `profiles::get_profile_paths_view` + `ProfilePathsView` 移入 core                                           |
| A8   | 已修 | `480c12b` | `db::backups::refresh_and_list` / `_at`；接线测试改测生产路径                                               |
| A9   | 已修 | `518bb85` | `service::restore_snapshot` / `_io`；GUI 命令一行转发                                                       |
| A10  | 已修 | `8e96e74` | `db::sync::status` + `SyncStatus`；GUI/CLI 共用                                                             |
| A11  | 已修 | `dd344e6` | `record_run` 接收 `&[String]` 内部序列化；删除三个无调用者的 scan history IPC                               |

### P2

| 编号 | 处置             | 提交                              | 说明                                                                                                        |
| ---- | ---------------- | --------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| A12  | 已修             | `b64b77a`                         | `db::open_existing` + `runtime::open_db_or_exit`；CLI 三处 `open_db` 合并                                   |
| A13  | 已修（保留降级） | `b64b77a`                         | `canWriteTarget` 保留 `isAdmin` 回退但注释明确「UI 降级，不是安全判定」，新增单测钉住两条语义               |
| A14  | 已修             | `b64b77a`                         | `docs/dev/data-and-security.md` 明确「组合长度是 UX 规则，硬上限在 core」                                   |
| A15  | 已修             | `4a45c52`                         | `registry::env_var` → `registry::env_edit`，与契约模块 `core::env_var` 区分                                 |
| A16  | 已修             | `4a45c52`                         | `service.rs` 1162 → 641 行；拆出 `service/{profile,restore,tests}`，公共路径经 re-export 不变               |
| A17  | 已修             | `b64b77a`                         | 删除 `runtime::disabled_paths`（`#[allow(dead_code)]`）                                                     |
| A18  | 已修             | `b64b77a`                         | 修正「环境变量对比将在提供」为「本版不支持」                                                                |
| A19  | 已修             | `55bb812` / `dd344e6` / `b64b77a` | 共删除 13 个无业务调用者的 IPC 命令（2 个 mark_applied、3 个 scan history、10 个死包装）及其前端包装/parser |
| A20  | 已修             | `b64b77a`                         | IPC 契约测试新增「backend 方法必须有调用者」检查，修复链式调用正则                                          |

---

## 二、验证证据

| 命令                                                    | 结果                                                         |
| ------------------------------------------------------- | ------------------------------------------------------------ |
| `npm run verify`                                        | **exit 0**（Prettier/ESLint/build/coverage/fmt/clippy/test） |
| `cargo test --workspace`                                | **324 passed / 0 failed / 2 ignored**                        |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0                                                       |
| `npm test -- --run`                                     | **35 files / 397 passed**                                    |
| `npm run test:e2e`                                      | **54 passed**                                                |
| IPC 契约测试                                            | 注册表 ↔ 包装一致，且每个包装都有调用者                      |

> 前端用例从 399 降到 397：删除 5 个 scan parser 测试、新增 2 个 `canWriteTarget` 测试、新增 1 个 IPC 调用者检查（净 −2 文件内用例）。

---

## 三、行为变化

1. **GUI 应用配置会记账**：`apply_count` / `last_applied_at` 与 CLI 一致。这是修复，不是兼容性破坏。
2. **IPC 面从 55 条收窄到 42 条**：删除的 13 条均为无 UI 调用者的命令；core 能力保留（`db::backups::refresh_and_list`、`scan_history` 等），CLI 或未来 UI 可直接调用。
3. **`save_path_with_sidecar` / `load_path_snapshot` 的错误形状**：从 `String` 变为 `CoreError` 对象；前端已接 `parseCoreError`，状态栏不再出现 `[object Object]`。
4. **同步错误 operation 名**：空口令错误由 `sync_export` / `sync_import` 改为 `export_bundle` / `import_bundle`；`code` 与 `message` 不变。
5. **crate 内部路径**：`registry::env_var` → `registry::env_edit`；`core::service` 子模块化。跨 crate 的公开路径（`path_editor_core::service::save_path_with_db` 等）经 re-export 保持不变。

---

## 四、未覆盖项（如实列出）

1. **`refresh_and_list` 暂无生产调用者**：`backup_list` IPC 删除后，该函数只被 core 测试覆盖；保留为 CLI/未来 UI 的入口。启动引导仍用 `index_backups`。
2. **A20 的静态分析有边界**：只识别 `backend.<method>`（含链式换行），不覆盖把 `backend` 赋给别名或动态 `backend[name]` 的写法；当前仓库无此类写法。
3. **真实 GUI + 注册表闭环**：本轮未启动 GUI 人工走查，也未写真实注册表。
4. **未推送**：11 个提交都在本地 `main`，未推送远端。

---

## 五、判定

审查报告列出的 **3 项 P0、8 项 P1、8 项 P2 全部处置完毕**，质量门全绿。

建议下一步：人工走查「应用配置会记账」「删除死 IPC 后界面无缺失入口」两条，然后决定是否推送。
