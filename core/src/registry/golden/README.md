# Golden 用例（F-10：C→Rust 行为等价基线）

> Wave 2 Task 7 产出。PathEditor 首版是 C/IUP（源码仅存于 git 历史），
> Rust 版与「旧 C 行为」之间无可执行的行为对照。本目录以**数据驱动**
> 的 golden 测试固定「输入快照 + 操作 → 期望结果」，防止后续重构
> 无意中改变这些语义。

## 格式

每用例一个 JSON：`name` / `input` / `op` / `expect`，可选 `note`
（语义说明，不参与断言）。`op` 字符串在测试代码中 match 到具体函数：

- `core/src/registry/golden_tests.rs`（挂 `registry/path.rs` 下，访问其私有 fn）
- `core/src/registry/golden/merge_golden_tests.rs`（挂 `crate::disabled` 下，访问 `merge_hive`）

## 六类覆盖矩阵

| #   | 行为类别                                                                            | 用例数 | 覆盖情况                                                                          |
| --- | ----------------------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------- |
| 1   | PATH 分割/空项/空白/重复（`split_path`/`join_path`/`clean_path_entries`）           | 8      | ✅ 完整覆盖，含空白项、重复项、null 字节、32767 UTF-16 上限                       |
| 2   | `REG_SZ`/`REG_EXPAND_SZ` 写回类型保持（`select_path_value_type`/`make_path_value`） | 4      | ✅ 覆盖类型判定与 UTF-16LE+NUL 编码契约                                           |
| 3   | 权限失败（`capabilities_for_with`，hive 写能力参数化注入）                          | 4      | ✅ 可写放行 / 不可写拒绝 / 保护名单 / 保留名 `Path`                               |
| 4   | 备份格式与恢复可用性（`backup.rs`）                                                 | 2      | ⚠️ 部分覆盖（见下方边界说明）                                                     |
| 5   | profile / 导入导出 / 禁用项兼容性（`persist` 信封、`fs`、`merge_hive`）             | 7      | ✅ 覆盖 legacy v1 读、future 版本拒绝、导入净化、CSV enabled 往返、孤儿禁用项恢复 |
| 6   | `WM_SETTINGCHANGE` 广播时机                                                         | 0      | ⏸️ **延后**（见下）                                                               |

合计 25 个 JSON 用例，每类 ≥2（第 6 类除外）。

### 第 4 类的边界

`backup_registry` 本体读写真实注册表，无法在纯函数级端到端验证。
已覆盖的部分：

- `backup_rejects_system_dir`：`C:\Windows\` / `C:\Program Files\` 目标拒绝
  （校验发生在读注册表之前，真实执行）。
- `backup_format_contract`：真实调用 `backup_registry` 并对**输出文件**
  断言格式契约（固定头 `PathEditor Backup - `、`[System PATH]` /
  `[User PATH]` 两个 section、`path_backup_*.txt` 文件名形态）。
  该用例**只读不写**注册表（HKLM/HKCU 读取对所有用户开放），与既有
  `backup_registry_with_custom_dir` 单测同级。

**未覆盖**：注册表 PATH 值损坏（如 PATH 值缺失）时备份的行为。

> **2026-09-21 更新**：本节曾写「本项目无独立『从备份恢复』入口，恢复靠手工
> 把备份内容粘贴回 PATH」。该声明**已不再成立**——v5.1.4 新增了独立恢复入口
> （CLI `patheditor env restore`、GUI 恢复命令 `restore_env_backup`），经
> `core/src/backup.rs` 的 `restore_env_backup_from` 实现，差异计算与写入判定
> 均在 core 一处（设计文档 §S6）。恢复本身**不产生新备份**（核对轮 C7 裁断：
> 每次恢复都新增文件会与保留策略互相吞噬），因此「恢复错了」没有自动回退，
> CLI 与 GUI 都只**提示**手工兜底出口、不自动执行。本条仍是**既有 golden 基线
> 的未覆盖项**：上述恢复路径的端到端契约由 `backup.rs` 内的单元测试覆盖
> （见 `restore_*` 系列用例），不在本 golden 用例集内。

### 第 6 类延后：`WM_SETTINGCHANGE` 广播时机

**广播端口尚未抽出**（F-08 只迁移了部分）。当前 `crate::system::broadcast_env_change()`
在以下调用点是**硬编码、不可注入**的：

- `core/src/registry/env_var.rs`：`update_env_var` / `create_env_var` /
  `delete_env_var` / `update_env_var_force` / `delete_env_var_force` 的
  公开 API 层（`*_in_store` 注入核心不含广播）。
- `core/src/service.rs:207`：`save_path_with_sidecar` 任一 hive 写成功后广播。

要按 brief 的替身方案断言「成功路径调用广播、失败/冲突路径不调用」，
需要把广播抽成可注入端口（trait 或闭包参数）后，用记录式替身验证
调用次数与时机。

> **2026-09-21 更新：广播的不变量已由「观察标志」测试守住（但端口仍未可注入）。**
>
> 本节曾写「**在端口可注入之前，本类不做测试**——不硬造」。该结论**已过时**，
> 且会误导读者以为这类**仍无任何测试**。实际做法**不是**把广播抽成端口，
> 而是加了一个全局观察标志：`core/src/registry/env_var.rs` 的
> `BROADCAST_OBSERVED`（写入成功路径置位，仅测试读取）+ 写入口共用的
> `broadcast_after_write()`（含 `cfg!(test)` 门控，测试构建跳过真实 Win32
> 广播调用、生产构建无条件执行）。表驱动测试
> `all_write_entrypoints_broadcast_on_success` 穷举 5 个写入口，逐一断言
> 「成功 → 已广播」；负向用例 `rejected_write_does_not_broadcast` 断言
> 「被拒绝 → 不广播」。这条测试正是 2026-09-21 复审发现「5 处广播调用被
> 整体丢失而无人察觉」（Critical）后补上的。
>
> **性质差别必须说清**：观察标志**不是**端口。它只记录「最近一次写有没有广播」
> 这一个布尔事实，**无法**断言调用**次数**、**时机**，也无法做记录式替身
> 的顺序验证；它靠一个全局可变状态在用例间复位（`reset_write_broadcast()`），
> 而非依赖注入。因此上面「建议的后续形态」**仍然成立**——抽出真端口后，
> 可以断言更强的契约（成功恰好 1 次、拒绝 0 次、顺序）。观察标志是**过渡期
> 补丁**，不是那个终态。golden 用例集内第 6 类**依然是 0 个 JSON 用例**
> （覆盖率矩阵的「延后」标注据此保留）；广播契约目前由上述 Rust 单元测试覆盖。

**建议的后续形态**（供 F-08 续期参考）：

1. 抽 `trait BroadcastPort: Fn() + Send + Sync` 或直接 `Box<dyn Fn()>` 注入；
2. `*_in_store` 系列收 `broadcast: Option<&dyn Fn()>` 参数；
3. golden 用例断言：成功写 → 恰好 1 次；revision 冲突 / 保护名单 /
   保留名 / 类型不支持 / 写入失败 → 0 次；
4. 用例文件命名建议 `broadcast_success_path.json` / `broadcast_conflict_no_broadcast.json`。

## 迁移记录（旧行为 / 新行为 / 改变原因）

PathEditor 的 C 首版行为细节在 git 历史里，`docs/审核和开发/` 无逐条
对照。以下以**当前实现与 Rust 版历史演进的差异**为准；无法确证的
地方如实标注，不虚构。

### PATH 分割语义（第 1 类）

- **旧行为**：C 行为无记录，按当前语义固定。
- **新行为**：`split_path` 逐段 trim、丢弃空段（含纯空白段）；不去重
  （去重是 `clean_path_entries` 的职责，按小写化键保留首次出现）。
- **改变原因**：Rust 版 v4 重写（Tauri）时确立，TS 端 `split_path`
  夹具与 Rust 语义对齐过；trim 语义已长期稳定，视为契约而非实现细节。

### `REG_EXPAND_SZ` 默认值（第 2 类）

- **旧行为**：C 版写回 PATH 的类型策略无记录。
- **新行为**：已存在的值是 `REG_SZ` 则保持 `REG_SZ`；值不存在或类型
  不是 `REG_SZ`（含 `REG_DWORD` 等非字符串类型）一律默认
  `REG_EXPAND_SZ`（issue #26 行为契约，见 `path.rs` 的 `issue26_tests`）。
- **改变原因**：修复「保存后 `%VAR%` 不再展开」的回归（issue #26）。
  `REG_EXPAND_SZ` 是 Windows 对 PATH 的惯例类型，默认它保证环境变量
  展开语义不丢。

### 备份格式（第 4 类）

- **旧行为**：C 行为无记录，按当前语义固定。
- **新行为**：固定头 `PathEditor Backup - <时间>` + `[System PATH]` /
  `[User PATH]` 两个 section + 条目逐行；文件名 `path_backup_YYYYMMDD_HHMMSS_mmm.txt`。
- **改变原因**：Rust 版沿用既有格式；拒绝写入系统目录
  （`C:\Windows\` / `C:\Program Files\`，大小写不敏感、`/` 与 `\` 等价）
  是 Rust 版新增的防呆。

### 禁用项兼容（第 5 类）

- **旧行为**：C 首版 `disabled.json` 只有禁用字符串列表
  （`system` / `user` 两个 `Vec<String>`），无完整快照、无 `enabled`
  概念、无 schemaVersion。
- **新行为**：`disabled.json` 携带 `systemSnapshot` / `userSnapshot`
  完整有序快照（`PathEntry { path, enabled }`）+ `schemaVersion` 信封；
  旧格式仍可读——孤儿禁用项按 `enabled=false` 恢复到列表末尾；
  `schemaVersion` 高于当前支持值时拒绝解析并提示升级（不隔离原文件）。
- **改变原因**：`enabled` 是跨层契约（CLAUDE.md 明确），禁用项必须
  在注册表移除后跨重启恢复且保持顺序；版本信封（F-11）防止旧代码
  误解新格式造成数据损坏。

### 导入导出（第 5 类）

- **旧行为**：C 行为无记录，按当前语义固定。
- **新行为**：导入统一净化（trim、去空、拒绝分号与 null 字节条目），
  `enabled` 缺省 true 且净化后原样保留；CSV 固定三列头
  `type,path,enabled`，含逗号字段 RFC 4180 引号转义。
- **改变原因**：分号是 PATH 分隔符，混入单条路径会静默改变语义；
  `enabled` 无损往返是「清理、禁用和启用操作必须保留 `PathEntry`」
  约束的导入侧对应。

## 用例清单

| 类  | 用例                            | 断言点                                         |
| --- | ------------------------------- | ---------------------------------------------- |
| 1   | split_trims_and_drops_empty     | 逐段 trim + 空段丢弃                           |
| 1   | split_empty_string              | 空 PATH 值 → 空列表（非 `[""]`）               |
| 1   | split_keeps_duplicates          | split 不去重                                   |
| 1   | join_trims_and_drops_empty      | join trim/丢弃空段，split∘join 往返稳定        |
| 1   | validate_rejects_null_byte      | null 字节拦截 + hive 标签前缀                  |
| 1   | validate_rejects_oversize       | 32767 UTF-16 上限（按码元计）                  |
| 1   | clean_dedupes_case_insensitive  | 大小写不敏感去重保首次、空项移除、enabled 保留 |
| 1   | clean_keeps_percent_paths       | `%` 路径保留（unknown 不误删）、缺失目录移除   |
| 2   | type_keeps_reg_sz               | 已有 REG_SZ 写回保持 REG_SZ                    |
| 2   | type_defaults_expand_sz         | 非字符串类型回退 REG_EXPAND_SZ                 |
| 2   | make_value_utf16_nul            | 值文本逐字节保留 + UTF-16LE + NUL 终止         |
| 2   | make_value_reg_sz               | REG_SZ 不被暗中改写                            |
| 3   | caps_writable_allows            | 可写 hive + 普通名 → 放行                      |
| 3   | caps_nonwritable_denies         | 不可写 hive → 拒绝（查看不受影响）             |
| 3   | caps_protected_denies           | 保护名单硬拒绝                                 |
| 3   | caps_reserved_denies            | 保留名 `Path` 硬拒绝                           |
| 4   | backup_format_contract          | 备份输出文件格式契约（只读注册表）             |
| 4   | backup_rejects_system_dir       | 系统目录拒绝                                   |
| 5   | profile_legacy_v1               | 无 schemaVersion 按 v1 读、enabled 往返        |
| 5   | profile_future_version_rejected | 高版本 Parse 拒绝 + 提示升级（按 code 判定）   |
| 5   | import_json_sanitizes           | 导入净化 + enabled 保留                        |
| 5   | export_csv_enabled              | CSV 三列头 + 引号转义 + enabled 往返           |
| 5   | merge_legacy_orphan             | 旧版字符串-only 的孤儿禁用项恢复               |
| 5   | merge_snapshot_order_wins       | 快照定序、外部删除不复活、新路径追加           |
| 5   | merge_legacy_marks_registry     | legacy 禁用串按小写键标记注册表路径            |
