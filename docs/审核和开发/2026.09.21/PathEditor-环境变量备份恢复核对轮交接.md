# PathEditor 环境变量备份恢复 — 核对轮交接（开发窗口 → 审核窗口）

- **日期**：2026-09-21
- **提交窗口**：开发窗口
- **交付对象**：审核窗口
- **核对对象**：`docs/superpowers/specs/2026-09-21-env-backup-restore-design.md`、`docs/superpowers/plans/2026-09-21-env-backup-restore-implementation.md`（本轮更新版，未提交）
- **当前状态**：**第 1–3 轮已完成**。第 1 轮提 6 项（3 阻断）→ 审核窗口全部采纳 → 第 3 轮复核新增 5 项（1 阻断 N1、3 待裁断 N2/N3/N5、1 轻 N4）+ J5 无归属。**仍未收敛，暂不开工。**

> 本文件是「核心细节必须落盘」规则下的产物。审核窗口看不到开发窗口的会话，以下每条都自带 `file:line` 或实验输出，不依赖对话上下文即可复核。
>
> **章节导航**：§1–§2 第一轮（已采纳项核验 / E1-E4 复核）；§3–§5 第一轮（阻断项 B1–B3 / 判断意见 J1–J3 / 未覆盖项）；§6 独立紧急发现（v5.1.3 发布事故）；§7–§8 待裁断与授权；**文末「附：第三轮复核」为最新状态**（8 条裁断折入核验 + 新发现 N1–N5 + J5 归属 + 提交策略建议）。

---

## 1. 先核已采纳项（8 条，全部确认折入正文）

| #                 | 核对结果  | 落点证据                                                                                                                                                                                    |
| ----------------- | --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E3/C6             | ✅ 已折入 | spec §公开 API 签名（`:183-194`）；计划 Task 5 补两个包装（`:1409-1428`）；Task 7 调用已改（`:1929`）                                                                                       |
| E5                | ✅ 已折入 | spec `:132-134`、S6 `:335`；计划 Task 6 新增 `preview_includes_protected_names`（`:1569-1588`），`restore_force_does_not_bypass_protected_names` 改为断言「不中止其余恢复」（`:1551-1563`） |
| C5                | ✅ 已折入 | spec S2.1 `:309`、验收标准 `:401`、范围外 `:401`、遗留与风险 `:415` 四处同步                                                                                                                |
| C7                | ✅ 已折入 | spec §手工兜底出口 `:138-147`（含「不得做成自动调用」）；计划 Task 7 `:1945-1949`                                                                                                           |
| config.ini 方案 A | ✅ 已折入 | spec §配置文件 `:196-245`；计划 Task 2 Step 2b/3 `:389-531`；Global Constraints 补 P3 `:26`                                                                                                 |
| P1                | ✅ 已折入 | 计划 Task 5 `:1318-1347` 降级为行为契约，`current` 值类型修正为 `(原始名, revision)`，并显式写「不要引用不存在的函数」                                                                      |
| P2                | ✅ 已折入 | 计划 Task 7 `:1965`、Self-Review 缺口 2 `:2366`                                                                                                                                             |
| P3                | ✅ 已折入 | 计划 Global Constraints `:26`；spec `:245` 补「config.ini 不用 Versioned 信封」                                                                                                             |

## 2. 复核驳回项 E1/E4 —— 独立实测，**驳回成立**

我用独立最小 crate 复刻了 PathEditor 的模块结构（`rustc 1.96.0 (ac68faa20 2026-05-25)`，Windows）：

```rust
pub mod registry {
    mod access   { pub(crate) fn hive_location() -> u8 { 1 } }
    mod env_var  { pub fn read_env_var() -> u8 { 2 } }
    pub(crate) use access::hive_location;      // 与 core/src/registry.rs:30 同形
}
mod backup {
    pub fn a() -> u8 { crate::registry::hive_location() }         // 经 registry 根
    pub fn b() -> u8 { crate::registry::env_var::read_env_var() } // 经私有子模块全路径
}
```

实测输出：

```text
error[E0603]: module `env_var` is private
  --> lib.rs:17:41
   |
17 |     pub fn b() -> u8 { crate::registry::env_var::read_env_var() }
   |                                         ^^^^^^^  ------------ function `read_env_var` is not publicly re-exported
   |                                         |
   |                                         private module
```

`a()` 处无错误，`b()` 处报错。**结论：审核窗口的驳回正确，开发窗口此前对 E1/E4 的撤回是错的。** 计划 Task 1（`use crate::registry::{hive_location, read_env_var};`）与 Task 6 的修正已到位，不再提异议。

**另外补一条同源风险（审核窗口未覆盖）**：`registry.rs:12-15` 的四个 `mod` 全部是私有的，而 `golden_tests.rs:240,246` 走的是 `crate::registry::load_system_paths()`（经根 `pub use`）。新增的 `pub(crate) use` 必须落在 `registry.rs:30` 那一组，**不能**另起一处放在测试模块里——否则 `golden_tests` 的既有模式会被破坏。

---

## 3. 阻断项（3 条，开工前必须解决）

### B1【阻断】`collect_hive_vars_in_store` 用 `read_env_var` 采集，遇到任何 `Unsupported` 类型变量会让**整次备份失败**

**位置**：计划 Task 1 Step 3（`:197`）

```rust
let (vtype, value) = read_env_var(store, &name).map_err(|e| { ... })?;   // ← 这里
let kind = EnvValueKind::from_reg_type(vtype.clone());
if !kind.is_writable() { continue; }   // ← 永远到不了，上一行已 `?` 短路
```

**实证**（`core/src/registry/env_var.rs:22-45`）：

- `read_env_var` 在 `:30-36` 先做类型判定，`Unsupported` 时**直接返回错误**：
  ```rust
  if !EnvValueKind::from_reg_type(raw.vtype.clone()).is_writable() {
      return Err(CoreError::new(ErrorCode::UnsupportedType, "read_env_var", ...));
  }
  ```
- 既有测试正是靠这个契约写的 —— `core/src/registry/env_var.rs:745` 附近：
  ```rust
  let revision = revision_of("MY_DWORD", stored.vtype, "");   // Unsupported 用空串算 revision
  let err = update_env_var_in_store(...).unwrap_err();
  assert_eq!(err.code, ErrorCode::UnsupportedType, "必须断言 code 而非文本（F-06）");
  ```

**后果**：`collect_env_backup()` 在 `:223-232` 对两个 hive 都 `?` 传播。只要用户机器上任一 hive 存在一个 `REG_DWORD` / `REG_BINARY` / `REG_MULTI_SZ` 变量，**每一次环境变量写入的写前备份都会失败** —— 备份文件永远不产生，而 `BackupOutcome::Failed` 只在 stderr 记一行，用户不会察觉。这条同时打掉 spec 验收标准 2、3、4。

**本机实测（重要限定）**：我按只读方式枚举了两个 hive 的**全部值名与注册表类型**，**没有发现任何 `Unsupported` 类型变量**：

| hive                                   | 变量数 | 出现的类型                |
| -------------------------------------- | ------ | ------------------------- |
| `HKCU\Environment`                     | 31     | `REG_SZ`、`REG_EXPAND_SZ` |
| `HKLM\...\Session Manager\Environment` | 25     | `REG_SZ`、`REG_EXPAND_SZ` |

所以在这台机器上，B1 的缺陷**不会**被触发。请据此校准严重度：它不是「用户的备份永远失败」，而是「**在一台有 `REG_DWORD` 变量的机器上，备份会整体失败**」。

**为什么仍然必须在开工前修**（三条，不是因为本机复现不了就降级）：

1. **计划 Task 1 Step 1 的测试是错的，而不是不充分**——它断言「采集结果的列表里没有 `SomeDword`」，而真实行为是「采集直接返回 `Err`」。照抄这份测试，实施者会写出一个**永远不成立的断言**，然后在「跑测试确认失败」与「跑测试确认通过」之间反复调试，最后很可能把测试改成迁就实现（`unwrap_err()`），把缺陷固化进代码。
2. **缺陷的性质是 fail-everything，不是 degrade-gracefully**。`collect_env_backup` 对两个 hive 都 `?`；只要任一 hive 命中一个不支持类型，整份备份就没了。而这恰好与本版的目标（「凡写入必有备份可回退」）正相反——**最需要备份的强约束用户，反而最可能拿不到备份**。
3. `REG_MULTI_SZ` 这类类型在 Windows 上**是被系统与安装程序实际使用的**（典型如 `PSModulePath` 在某些环境下、以及部分驱动/杀软写入项）。本机没有不代表目标用户没有——这正是「失败响亮」优于「悄悄没有」的场合。

**验收标准 3 的措辞也建议随之收紧**：现文（spec `:382`）是「`Unsupported` 类型变量与 `Path` 不出现在备份中」。在修好 B1 之前，这句话对 `Unsupported` 是**不可达的**——它描述的是一个当前架构下走不到的分支。建议改为「采集遇到 `Unsupported` 类型时跳过该变量并继续（不使整次备份失败）」，把「跳过而非失败」这个语义写进验收标准，才具备可判定的完成标准。

**计划 Task 1 Step 1 的测试为什么没拦住**：它用 `MemoryHive::seed("SomeDword", "1", REG_DWORD)`，`seed` 走 `set_raw` 直接存字节，**不经过 `read_env_var` 的类型判定**；测试断言的是「采集结果里没有 SomeDword」，而真实效果是「采集直接报错」。测试全绿但行为是错的。

**建议修法（供裁断，我倾向 A）**：

- **A（推荐）**：采集改用 `store.get_raw(&name)` + `EnvValueKind::from_reg_type` + `is_writable()` 判空，再用 `String::from_reg_value` 解码。行为与 `list_env_vars_in_store`（`core/src/registry/env_var.rs:154-166`）完全同形——先 `get_raw`，判 `Unsupported` 则跳过（`:158-165`），否则解码。**验收标准 3 的字面要求即为此。**
- **B**：保留 `read_env_var`，但显式忽略 `ErrorCode::UnsupportedType` 并 `continue`，其余错误照旧传播。可行，但要多写一层错误码分支，且不如 A 贴近既有写法。

选 A 或 B 都需要**改测试**：`collect_hive_vars_excludes_reserved_and_unsupported` 必须改成能真正区分两种行为的形态（例如断言返回 `Ok` 且不含 `SomeDword`，而不是仅断言列表内容）。

---

### B2【阻断】Task 3 的 `apply_core_result` 泛型化会破坏 `apply_concurrency` 的调用兼容

**位置**：计划 Task 3 Step 3（`:878-882`）只给了 `runtime.rs` 的新签名，**没有同步 `cli/src/env_ops.rs:120`**：

```rust
// runtime.rs —— 计划改后
pub(crate) fn apply_core_result<T>(result: Result<T, core::CoreError>) { ... }

// env_ops.rs:120 —— 计划未提，仍是具体类型
pub(crate) fn apply_concurrency(result: Result<(), CoreError>) {
    apply_core_result(result);   // ← 这里仍能编译（T 被推断为 ()）
}
```

调用点 `cli/src/env_ops.rs:339`、`:377` 传入的将是 `Result<WriteOutcome, CoreError>`：

```rust
apply_concurrency(core::registry::update_env_var(hive, &name, &new_value, &r));  // :339
```

→ `apply_concurrency` 形参是 `Result<(), CoreError>`，实参是 `Result<WriteOutcome, CoreError>` → **类型不匹配，编译失败**。

**实际波及点（我逐处查过，比计划写的多）**：

| 位置                     | 现状                                            | 需改                                                                                         |
| ------------------------ | ----------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `cli/src/runtime.rs:27`  | `apply_core_result(result: Result<(), _>)`      | 改泛型（计划已给）                                                                           |
| `cli/src/env_ops.rs:120` | `apply_concurrency(result: Result<(), _>)`      | **改泛型或直接转发**（计划未给）                                                             |
| `cli/src/env_ops.rs:339` | `apply_concurrency(update_env_var(...))`        | 传参类型变了                                                                                 |
| `cli/src/env_ops.rs:377` | `apply_concurrency(delete_env_var(...))`        | 同上                                                                                         |
| `cli/src/env_ops.rs:342` | `update_env_var_force(...).unwrap_or_else(...)` | 返回值变成 `WriteOutcome`，当前丢弃后 `println!` 仍可编译，但**备份结果被静默吞掉**（见 B3） |
| `cli/src/env_ops.rs:363` | `create_env_var(...).unwrap_or_else(...)`       | 同上                                                                                         |
| `cli/src/env_ops.rs:380` | `delete_env_var_force(...).unwrap_or_else(...)` | 同上                                                                                         |

**建议**：把 `apply_concurrency` 也改成泛型：

```rust
pub(crate) fn apply_concurrency<T>(result: Result<T, CoreError>) {
    apply_core_result(result);
}
```

并在 Task 3 Step 4 的验证命令里补一条 `cargo clippy --workspace --all-targets -- -D warnings`——计划已写，但按现状跑一定红，实施者会误以为是自己的问题。

---

### B3【阻断】计划 Task 7 的 CLI 测试用了从 `cli` crate 不可达的 API

**位置**：计划 Task 7 Step 1（`:1818`）

```rust
let _guard = path_editor_core::persist::test_persist_lock();
```

**实证**：`core/src/lib.rs:8` 是 `pub(crate) mod persist;`，`core/src/persist.rs:20` 的 `test_persist_lock` 也是 `pub(crate)`。从 `patheditor-cli` crate 引用 → `error[E0603]: module persist is private`。

（`path-editor-core` 作为外部 crate 看，`pub(crate)` 等价于私有。这条与 P3 是同一个可见性事实，但 P3 只覆盖了 gui/cli 不能用 `Versioned`，没覆盖测试辅助函数。）

**建议修法（供裁断）**：

- **A**：Task 7 的 `backups_list_json_shape_is_stable` 不取锁，改为**不依赖锁**——它只调 `list_env_backups()`（纯目录枚举），设置 `PATHEDITOR_BACKUP_DIR` 后立即读，冲突窗口极小。风险：与其他改同一环境变量的测试并行时可能串扰。
- **B（推荐）**：`cli` 侧单测本就在**独立的测试二进制**里跑（`cargo test -p patheditor-cli --bins` 与 core 的测试是两个进程），进程级环境变量不跨进程共享，**根本不需要那把锁**。删掉取锁行即可，同时在计划里写明「core 的 `test_persist_lock` 只用于 core 内部同进程测试」。
- **C**：把 `test_persist_lock` 提升为 `pub`（不推荐——为测试扩大公开面）。

另附：`path_editor_core::persist::test_persist_lock` 这个写法还依赖 `persist` 是 `pub`，即使把函数改成 `pub` 也仍缺 `pub mod persist`。B 是唯一零代价的修法。

---

## 4. 判断性意见（3 条，不阻断但需在计划里写明）

### J1 `backupFailed` i18n 键不存在；且 K2 的「CLI stderr 警告」在 CLI 侧**实际不会出现**

**a) i18n 键**：spec K2（`:265`）写「新增 i18n 键 `status.backupFailed`，PATH 通路已有同义键，复用」。实测 `src/i18n/locales/{zh-CN,en}.json` 的 `status` 命名空间里**没有** `backupFailed`；既有同义键叫 **`status.saved_without_backup`**（zh 文案「保存成功（备份失败）」，en 同结构），另有一个 `status.warning_backup`（「备份创建失败，保存将继续但不生成备份」）。PATH 通路的用法在 `src/services/path-session.ts:211`：

```ts
statusMessage: backupFailed ? i18n.t('status.saved_without_backup') : i18n.t('status.saved'),
```

**建议**：spec K2 把键名改成 `status.backupFailed` → 实际使用 `status.saved_without_backup`（或新增一个语义更准的键，但那样要同步两个 locale 文件）。

**b) CLI 侧警告不会出现**：K2 要求「CLI 必须在 stderr 打印警告」。实现路径是 `core/src/registry/env_var.rs` 的 `backup_before_write()` 里 `log::warn!(...)`，但**`cli` crate 没有任何 logger 初始化**——`cli/Cargo.toml` 无 `env_logger`/`log` 依赖，`cli/src/main.rs` 无 `log::` 调用（全仓 `rg "log::" cli/src` 零命中）。因此 `log::warn!` 在 CLI 下被静默丢弃。

**建议**：K2 的 CLI 半边要靠 `WriteOutcome.backup` 的返回值在 `cli/src/env_ops.rs` 里显式 `eprintln!` 实现，而不能依赖 core 的 `log::warn!`。计划 Task 3 的 3 个 force/add 分支（`:342`、`:363`、`:380`）目前把返回值丢掉了，需要在 Task 7 里补上「拿到 `WriteOutcome` → 若 `BackupOutcome::Failed` 则 `eprintln!` 且不改退出码」。

### J2 验收标准 5 的 GUI 半边没有任务承载

spec 验收标准 5（`:384`）：「备份失败时写入照常成功，CLI stderr 有警告且退出码不受影响；**GUI 状态栏显示警告**」。

计划 Task 8 只改了 `backend.ts` 的 4 个新方法（`:2192-2204`），**没有**改 `updateEnvVar` / `createEnvVar` / `deleteEnvVar` 的返回类型，也没有改 `src/store/env-store.ts`。实测现状：`src/services/backend.ts:303-308` 三个方法都是 `invoke<void>`，`src/store/env-store.ts:159/186/202` 用 `await` 丢弃返回值并直接设 `statusMessage: i18n.t('status.saved')`。

→ **`WriteOutcome.backup` 产生了但前端永不消费**，GUI 状态栏不会显示任何备份失败警告。验收标准 5 的 GUI 部分不达标。

**建议**：Task 8 增加一步——`backend.ts` 三个 env 写方法返回类型改为 `WriteOutcome`（`{ backup: BackupOutcome }`，Rust 侧 `BackupOutcome` 是外部标签枚举，序列化形如 `{"created":"C:\\..."}` / `"skipped"` / `{"failed":"原因"}`，前端需按此形状做运行时校验）；`env-store.ts` 在 `await` 后查 `backup`，命中 `failed` 时把 `statusMessage` 换成 `status.saved_without_backup`。**这属于验收标准范围，不是 scope creep。**

### J3 Task 3 的 `validate_write(store, hive)` 是一个不存在的函数；计划已给「允许内联」的出口，但需在 Interfaces 里写明

计划 Task 3 Step 3（`:851-862`）给出 `prepare_and_backup` 骨架，内部调用 `validate_write(store, hive)?`，随后在提示（`:865`）说明「`validate_write` 需按 5 个写入口各自的校验需求实现……允许把校验逻辑内联在各处」。全文除这两处外无 `validate_write` 定义（`:833`、`:858` 是唯一命中）。

我在 5 个写入口里各数了一遍校验项，**互不相同**：

| 写函数                           | 名称校验 | reserved | protected | revision |          类型可写           | 值校验 | 同名检查 |
| -------------------------------- | :------: | :------: | :-------: | :------: | :-------------------------: | :----: | :------: |
| `update_env_var`（`:251`）       |    ✅    |    ✅    |    ✅     |    ✅    |             ✅              |   ✅   |    —     |
| `create_env_var`（`:323`）       |    ✅    |    ✅    |    ✅     |    —     |             ✅              |   ✅   |    ✅    |
| `delete_env_var`（`:407`）       |    ✅    |    ✅    |    ✅     |    ✅    | —（由 `read_env_var` 代劳） |   —    |    —     |
| `update_env_var_force`（`:472`） |    ✅    |    ✅    |    ✅     |    —     |             ✅              |   ✅   |    —     |
| `delete_env_var_force`（`:526`） |    ✅    |    ✅    |    ✅     |    —     |          —（同上）          |   —    |    —     |

**单参数 `validate_write(store, hive)` 无法表达这张表**——它至少要接「操作类型 + 名称 + 值 + revision」。强行抽象必然退化成一个大 `match`，反而比内联更难读。

**建议**：采纳计划已给的出口，把 `prepare_and_backup` 的契约收紧为**只有顺序、没有校验**：

```rust
/// 备份是写前尽力行为：调用方先自行完成全部校验，校验通过后再调用本函数。
/// 备份失败不阻断写入（设计文档 K2），结果经返回值如实交给调用方。
fn backup_before_write() -> BackupOutcome { ... }   // 计划 :803-811 已给，无需 store/hive 参数
```

即 5 个公开包装各写成「校验（内联，复用各 `*_in_store` 里已有的判定）→ `backup_before_write()` → 调 `*_in_store`」。注意这会造成**校验逻辑在两处各写一份**（公开包装内联一份、`*_in_store` 内一份）——这是本次设计里第二处「双份规则」，与 spec S6 反对的「preview 里复制保护名单」同性质。

**待裁断**：先给出两个选项，请审核窗口择一：

- **甲（我推荐）**：接受双份校验（公开包装内联 + `*_in_store` 保留），理由是「先校验后备份」这个顺序要求优先，且 `*_in_store` 的校验是**防御性兜底**（真实写入路径必经），不构成判定源分裂——判定源仍是 `is_reserved` / `is_protected` / `EnvValueKind::is_writable` 这三个 core 函数。
- **乙**：把 5 个 `*_in_store` 拆分出「校验 + 写」两个阶段（如 `validate_update(store, hive, name, value, rev)?` + `write_...`），公开包装与 `*_in_store` 都调同一个 `validate_*`。**零双份**，但要动 5 个既有私有函数的结构，改动面比本波其他任务都大。

---

## 5. 未覆盖项与其余观察

| 项                                            | 说明                                                                                                                                                                                                                                                                                                    |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `EnvBackupVar` 的差异计算口径                 | 计划 Task 5 用 `store.get_raw`（`:1370-1376`）而非 `read_env_var`，对 `Unsupported` 用 `continue` 跳过。这与 Task 1 的采集口径**不一致**（Task 1 走 `read_env_var`，会失败；Task 5 走 `get_raw`，会跳过）。B1 按 A 方案修完后两者自然统一，因此不单列为阻断项，但**修 B1 时务必确认两处口径一致**。     |
| `EnvBackupPayload` 无 `#[serde(rename_all)]`  | 计划 Task 1（`:161-167`）给 `EnvBackupPayload` 加了 `#[serde(rename_all = "camelCase")]`，而 `EnvBackupHives`（`:152-158`）没加。`system`/`user` 小写本就一致，无功能影响；但两个结构体风格不统一，建议都加或都不加（`EnvVarSnapshot` 在 `core/src/env_var.rs:113` 是加的）。                           |
| `list_env_backups` 的 `variable_count` 恒为 0 | spec 命令规格（`:161`）要求 `env backups` 打印「文件名、时间、大小、**包含变量数**」，而 S5（`:327-329`）又要求「不解析 JSON 内容」。两条直接冲突。计划 Task 4 选择守 S5、`variable_count` 填 0（`:1066`）。**这是正确的取舍，但 spec 的两处表述需要对齐**——否则验收标准 10（`:389`）与命令规格对不上。 |
| 计划的验证命令 crate 名                       | Task 1 Step 2（`:113`）写 `cargo test -p path-editor-core`，与 `core/Cargo.toml` 的 `name = "path-editor-core"` 一致 ✅。此处仅记录已核实，无需改。                                                                                                                                                     |
| `&String → impl Into<String>`                 | 计划 Task 1 的 `with_target(hive, &name)` 传入 `&String`，而 `CoreError::with_target`（`core/src/error.rs:69`）是 `impl Into<String>`。已用最小 crate 实测 `&String` 满足 `impl Into<String>`（`&&str` 与 `&str` 两条 `From` 链均存在），**编译无问题**，不构成异议。                                   |
| 真实注册表恢复闭环                            | 与计划一致：需用户单独授权，不在本波。                                                                                                                                                                                                                                                                  |

---

## 6. 【独立于本波，但紧急】已发布的 v5.1.3 CLI 安装包装的是 GUI 二进制

> **发现路径**：本轮核对时我按仓库规则调用 `patheditor env list` 想查本机环境变量类型，结果命令无法执行，顺藤摸瓜查到了这个问题。**它与本波特性无关，但影响的是已经发出去的 v5.1.3。**

### 实测证据（全部只读）

```text
D:\settings\settings\Scoop\apps\patheditor-cli\current\patheditor.exe   20,790,849 B
D:\settings\settings\Scoop\apps\patheditor-gui\current\patheditor.exe   20,790,849 B
```

| 检验           | CLI 位置的文件                                                     | 结论                                         |
| -------------- | ------------------------------------------------------------------ | -------------------------------------------- |
| sha256         | `0bdc814a02356fd4e32585b50183f8e4920329215c5ea6e68de2c0b16ba83603` | 与 `patheditor-gui` 位置的文件**逐字节相同** |
| 该 hash 的来源 | = `bucket/patheditor-cli.json` 的 `architecture.64bit.hash`        | 即 **CLI 的发布资产本身就是 GUI 二进制**     |
| PE 子系统      | `0x20B` / **GUI (WINDOWS)**                                        | 不是控制台程序                               |
| 字符串特征     | `clap_builder` 命中 **0** 次，`tauri` 命中 **9787** 次             | 含 Tauri 运行时，无 clap 命令行解析器        |
| 运行结果       | `0xC0000135`（DLL_NOT_FOUND）/ Git Bash 下 `127`                   | 无法执行                                     |

对照：GUI 清单 `patheditor-gui.json` 声明的 hash 是 `c8a1c568...`（portable zip），**与实际装出来的 `0bdc814a...` 不符**。

### 根因（v5.1.3 的 workflow 原文）

```yaml
# git show v5.1.3:.github/workflows/release.yml
- name: 构建 CLI
  run: cargo build --release -p patheditor-cli # ← 与 GUI 共用 target 目录
- name: 整理发布产物
  run: |
    $cli = 'target\release\patheditor.exe'           # ← 与 GUI 本体同一目录条目
    Copy-Item -LiteralPath $cli -Destination (... "patheditor-cli_${env:VERSION}_x64.exe")
```

也就是 v5.1.3 发布的当时**没有** `--target-dir target/cli` 隔离、也**没有** GUI 的 portable zip 步骤。NTFS 大小写不敏感使 `target\release\PathEditor.exe` 与 `patheditor.exe` 是**同一条目录项**，CLI 链接产物覆盖了它，于是 `Copy-Item` 把 GUI 二进制当 CLI 发出去。

**这两点已在 gui-fix 波次（`17777a8`）修好**：现 HEAD 的 `release.yml` 有 `--target-dir target/cli`（`:112`）与 portable zip 步骤（`:144`）。所以**5.1.4 不会再犯**——本条不是要求改方案，是要求 **5.1.4 发布后立刻修复存量**。

### 影响面（比单看一个 bucket 更大）

| 受影响者                      | 现状                                                                                                                                      | 需要做什么                                                                   |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `lhy/patheditor-cli`（5.1.3） | 装出来的 exe 是 GUI，且无法运行                                                                                                           | 5.1.4 发布后 `scoop update patheditor-cli`                                   |
| `lhy/patheditor-gui`（5.1.3） | `shortcuts` 指向 `Patheditor.exe`/`patheditor.exe`，而**已安装目录里的名字仍是 `patheditor.exe`**；快捷方式能打开窗口，但 hash 与清单不符 | 5.1.4 发布后 `scoop update patheditor-gui`（新 zip 内才是 `PathEditor.exe`） |
| 所有 5.1.3 的 CLI 用户        | 拿不到能用的命令行工具                                                                                                                    | 只能等 5.1.4                                                                 |

**这与 README 现有内容冲突**：`README.md:307` 把 `scoop install lhy/patheditor-cli` 列为 CLI 的推荐安装方式（`:173-183` 同）。按上面的实测，这条路径在 5.1.3 下**装不出能用的 CLI**。建议在 README 的 v5.1.3 已知问题段补一条，或等 5.1.4 发布后一并修正——**这一条需要审核窗口与用户裁断口径**。

### 状态

**未做任何处置**（未改 bucket、未改 README、未提交任何东西）。bucket 的提交时机仍按此前约定：与 5.1.4 的 version/hash 一起提交。**在 5.1.4 发布前请不要跑 `scoop update patheditor-cli` 或 `scoop update patheditor-gui`**——现 bucket 里的 `patheditor-gui.json` 仍写着 5.1.3 的 URL 与 hash，但本地工作区已有未提交的 `shortcuts` 改动，此刻更新会装到一个连名字都对不上的包。

---

## 7. 待裁断汇总（请审核窗口逐条作答）

| #       | 问题                                                                                                                                                                                                                                                                                                                              | 我的推荐                                                                                                                            |
| ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| **B1**  | 采集用 `get_raw`（同 `list_env_vars_in_store` 写法）还是保留 `read_env_var` 并分支忽略 `UnsupportedType`？                                                                                                                                                                                                                        | 用 `get_raw`（方案 A），并改测试                                                                                                    |
| **B2**  | `apply_concurrency` 是否一并泛型化？                                                                                                                                                                                                                                                                                              | 是，且 Task 3 的 Step 4 应显式列出 `env_ops.rs` 的 6 处改动点                                                                       |
| **B3**  | Task 7 测试的取锁改为「直接不取锁（跨进程无需锁）」还是「提升 `test_persist_lock` 可见性」？                                                                                                                                                                                                                                      | 直接不取锁（方案 B），并在计划里说明理由                                                                                            |
| **J1a** | K2 的 i18n 键名                                                                                                                                                                                                                                                                                                                   | 复用既有 `status.saved_without_backup`                                                                                              |
| **J1b** | K2 的 CLI 警告实现路径                                                                                                                                                                                                                                                                                                            | 由 `WriteOutcome.backup` 在 `env_ops.rs` 显式 `eprintln!`，不依赖 core 的 `log::warn!`                                              |
| **J2**  | 是否把验收标准 5 的 GUI 半边补进 Task 8（改 `backend.ts` 三个方法 + `env-store.ts` 消费 `backup`）？                                                                                                                                                                                                                              | 是，属验收标准范围                                                                                                                  |
| **J3**  | 校验与备份的顺序如何落地：接受双份校验（甲）还是拆分 `*_in_store`（乙）？                                                                                                                                                                                                                                                         | 甲                                                                                                                                  |
| **J4**  | spec 命令规格「包含变量数」与 S5「不解析内容」冲突，如何对齐？                                                                                                                                                                                                                                                                    | 命令规格删去「变量数」，保留 S5                                                                                                     |
| **J5**  | §6 的 v5.1.3 事故：README 的 CLI scoop 安装说明（`README.md:173-183`、`:307`）现在就与事实不符，是**立即补已知问题提示**还是**等 5.1.4 发布后一并改**？                                                                                                                                                                           | 立即补一行提示（用户按 README 装 CLI 会拿到跑不起来的 GUI，属误导）                                                                 |
| **J6**  | 已安装的 GUI 快捷方式 `%APPDATA%\...\PathEditor.lnk` 指向 `apps\patheditor-gui\current\patheditor.exe`——旧 hash、22.5MB 的 GUI-子系统的包。它**能打开窗口**，但 5.1.3 的关窗缺陷（`core:window:allow-destroy` 缺失，gui-fix 才修）让它**大概率关不掉**。是否提示用户改用本地已构建的 `target\release\PathEditor.exe` 做手工验证？ | 提示，避免用户被旧包误导（当前 `target\release\PathEditor.exe` = 3,102,348 B / `0x20B CONSOLE`，**是 CLI 产物**，不是 GUI——详见下） |

> **关于本地 `target\release\PathEditor.exe`（第三条裁断需要的事实）**：实测它是 **3,102,348 字节、PE 子系统 `CONSOLE`、`clap_builder` 命中 655 次、`tauri` 命中 0 次**——**是 CLI 二进制，不是 GUI**。原因是 NTFS 大小写不敏感：本地曾用共享 target 跑过 CLI 构建，`patheditor.exe` 覆盖了 `PathEditor.exe` 这个同一条目（`WebView2Loader.dll` 仍在同目录，是唯一残留的 GUI 痕迹）。**所以此刻不要拿它做 GUI 手工冒烟**，会得到「双击无窗口」的错误结论。要手工验证 GUI 需先 `npx tauri build`（会重新生成 GUI 本体）。

---

## 8. 本轮授权确认（按轮次重新确认）

- 不出 release、不打 tag、不推送、不升级版本号。
- 全程不写真实注册表；所有测试用 `MemoryHive` 注入或临时目录隔离。
- 本文件为**未提交**工作产物，是否提交由用户决定。

**结论：计划尚未收敛，暂不开工。** 待审核窗口对上述 8 条作出裁断、更新 spec 与计划后，开发窗口复核一轮即可进入实施。

---

# 附：第三轮复核（2026-09-21，开发窗口）

审核窗口对第一轮 8 条待裁断项**全部采纳**，我已逐处核验折入情况，并复核了这轮扩写本身的**自洽性**。

## A. 8 条裁断的折入核验（全部真实折入，非文字敷衍）

| #   | 核验结果 | 证据                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| --- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| B1  | ✅       | 计划 `:75` 写了修正理由并点明"改用 `store.get_raw` + `from_reg_type` + `is_writable()` 跳过，与 `list_env_vars_in_store`（`env_var.rs:151-166`）完全同形"；`:234` 实现已改 `get_raw`；`:155`/`:280` 明确"**不需要** `read_env_var`，不要为它加 import 或改可见性"；测试改为 `:57` 新增 `collect_hive_vars_keeps_others_when_unsupported_present`，`:91` 注释"关键：必须 Ok。若实现走 read_env_var，这里会 Err 而失败"；`:233` 期望数 3 |
| B2  | ✅       | `:929-945` 新增「完整改动清单（逐处核对过，共 6 处）」表格，把 `:120`/`:339`/`:377`/`:342`/`:363`/`:380` 全列                                                                                                                                                                                                                                                                                                                          |
| B3  | ✅       | `:1901-1905` 测试注释写明"不取 `core::persist::test_persist_lock`——该函数是 `pub(crate)`，跨 crate 不可达；`--bins` 独立进程，进程内锁跨进程无意义"                                                                                                                                                                                                                                                                                    |
| J1a | ✅       | spec `:271-272` 改为复用 `status.saved_without_backup` 并**不新增键**，附 `zh-CN.json:97` 与 `path-session.ts:211` 双证；计划 `:2343` 同步                                                                                                                                                                                                                                                                                             |
| J1b | ✅       | 计划 `:949` 写明实证，`:957` 新增 `warn_if_backup_failed` 辅助（`cli/src/runtime.rs`），`:963` 三个 force/add 分支改为接住后调用                                                                                                                                                                                                                                                                                                       |
| J2  | ✅       | 计划 `:2299-2345` 补完整前端接线：`src/core/env-var.ts` 的 `BackupOutcome`/`WriteOutcome` 类型 + `backupFailed()`、`backend.ts` 三方法返回类型、`env-store.ts` 消费并换文案；并写明"形状校验要求（CLAUDE.md：不能只依赖 TS 断言）"                                                                                                                                                                                                     |
| J3  | ✅       | 计划 `:860-874` 采纳方案甲，删除 `prepare_and_backup`，明确"这是本设计里唯一接受的『双份规则』"并给出理由                                                                                                                                                                                                                                                                                                                              |
| J4  | ✅       | spec `:165` 采纳"删去变量数、保留 S5"，并给了两个实现选项                                                                                                                                                                                                                                                                                                                                                                              |
| J5  | ⚠️       | **未折入计划任何位置**（`rg "J5                                                                                                                                                                                                                                                                                                                                                                                                        | 已知问题"` 在计划中零命中）——见下 C 节 |
| J6  | ✅       | 已记入待裁断表                                                                                                                                                                                                                                                                                                                                                                                                                         |

## B. 第三轮新发现（这轮扩写自身引入的问题）

### N1【阻断】`WriteOutcome` 缺 `Serialize` / `Deserialize`

**位置**：计划 `:841`

```rust
#[derive(Debug, Clone, PartialEq)]      // ← 没有 Serialize / Deserialize
pub struct WriteOutcome {
    pub backup: BackupOutcome,
}
```

**为什么这是阻断项**：Task 3 把 `gui/src/commands/env_var.rs` 的 **5 个 `#[tauri::command]`** 返回类型改成 `Result<WriteOutcome, CoreError>`（计划 `:994` 给了完整签名）。Tauri 的 command 宏要求返回值实现 `Serialize`。`WriteOutcome` 不实现 → **gui crate 编译失败**。Task 8 补的前端形状校验（要求确认返回值含 `backup` 字段）也失去对象——没有序列化就没有 IPC 形状可校验。

**修法**（与同文件其它类型对齐）：

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteOutcome {
    pub backup: BackupOutcome,
}
```

**自洽性检查（已做，无冲突）**：Task 3 Step 1 的测试用

```rust
matches!(outcome.backup, BackupOutcome::Created(_) | BackupOutcome::Skipped | BackupOutcome::Failed(_))
```

这是用 `backup` 字段做匹配，不需要 `WriteOutcome: PartialEq`，因此加 serde 派生不破坏任何既有测试。

### N2【需裁断】`backupFailed()` 是新增的前端纯函数，但计划里**没有它的测试**

**位置**：计划 `:2317-2323` 定义 `backupFailed()`；但 Task 8 Step 4 的期望仍是 `Expected: 4 passed`（`:276`），而那 4 个用例全部属于 `summarizePreview`，`import` 行也仍只从 `@/core/env-backup` 导入。Step 5 的 `git add` 列表（`:2352-2354`）**不含** `src/core/env-var.ts`。

**为什么重要**：这与本轮 B1 是同一类问题——**测试数量对上了，但没覆盖新增的那半个行为**。B1 的教训是「测试不覆盖关键分支 → 缺陷被固化」，这里如果照抄，`backupFailed` 的形状判定（`{"failed":"原因"}` vs `{"created":...}` vs `'skipped'` vs 未知形状）就没有任何回归保护，而它恰好是 J2 那条前端链路上唯一的判定点（且是 core 判定/前端展示边界上唯一的新逻辑）。

**修法**：Task 8 Step 1 补一个用例（至少覆盖 `{failed:'原因'}` 与 `'skipped'`/未知形状两条分支），Step 4 期望改 `5 passed`，Step 5 的 `git add` 补 `src/core/env-var.ts`。

### N3【需裁断】CLI revision 分支的警告落地方式被留成"二选一"

**位置**：计划 `:976`

> 两个 revision 分支（`:339`/`:377`）经 `apply_concurrency` 泛型化后返回值被丢弃——**这是可接受的**：……但为保持与 force 分支一致的可观测性，建议把 `apply_concurrency` 也改为返回 `Option<BackupOutcome>` 或直接在调用点接住后 `warn_if_backup_failed`。**二选一即可，实施时择一并在回执中说明。**

**问题**：可运行，但把 K2 / 验收标准 5 的 CLI 半边（"CLI stderr 有警告"）交给实施者临场决定，与计划自己的 Self-Review「占位符扫描：无『适当处理错误』类占位」的声明冲突。

**建议修法**（比二选一都简单）：**删掉 `apply_concurrency` 这层间接**，两个 revision 分支写成与 force 分支同形：

```rust
Concurrency::Revision(r) => {
    let outcome = core::registry::update_env_var(hive, &name, &new_value, &r)
        .unwrap_or_else(|e| exit_core_error(&e));
    warn_if_backup_failed(&outcome.backup);
}
```

这样 `apply_concurrency` 不再需要（`cli/src/env_ops.rs:120` 一并删除），B2 的「6 处改动清单」缩为 5 处 + 1 处删除，且**5 个写入口的备份警告形态完全一致**，无需实施者选择。

### N4【轻】`BackupOutcome::Skipped` 是死变体

`backup_before_write()`（计划 `:848-854`）只产出 `Created` 或 `Failed`，**从无路径构造 `Skipped`**。公开枚举里的死变体没有编译器告警，但会被后来者误读为「有场景不备份」。建议二选一：删掉变体，或在计划里写一行「`Skipped` 预留：供未来『本次操作无需备份』的场景（如 dry-run）」并说明当前不可达。

### N5【轻】配置路径缺少与备份目录对称的测试重定向

`backup_base_dir()` 支持 `PATHEDITOR_BACKUP_DIR` 环境变量重定向（计划 `:513-525`），但 `config_file_path()`（`:464-469`）**只走真实的 `dirs::home_dir()/.patheditor/config.ini`**。结果是 `write_env_backup_to` 的测试（内部会调 `env_backup_keep()` → `config_file_path()`）**会读开发者真实的 config.ini**。

当前**不造成污染**——因为 `rotate_env_backups_keeps_exactly_keep_files` 传的是显式 `keep`，而 `write_env_backup_round_trips...` 在空目录上运行。但它是隐藏依赖（测试结果依赖开发者本机状态），且与既有做法不对称。建议给 `config_file_path()` 也加一个 `PATHEDITOR_CONFIG_FILE` 重定向，或在 `write_env_backup_to` 上多加一个 `keep: usize` 参数。

## C. J5 没有归属（已确认）

`rg "J5|已知问题"` 在计划中**零命中**。审核窗口裁了「立即补 README 提示」，但该动作**既不在计划任务里，也不属于 5.1.4 的开发内容**，因此当前没有执行者。

需要裁断的是「谁做、什么时候做」，不是「做不做」：

| 选项 | 说明                                                                                               |
| ---- | -------------------------------------------------------------------------------------------------- |
| 甲   | 用户单独授权一次 docs 提交，只改 README 的 v5.1.3 已知问题段（不碰计划、不碰 spec），与 5.1.4 解耦 |
| 乙   | 登记为独立待办，等 5.1.4 发布时与版本号、CHANGELOG、bucket 一起改                                  |

**参考事实**（供裁断）：`README.md:301-315` 的「方式一：Scoop」把 `scoop install lhy/patheditor-cli` 列为 CLI 首选安装方式，而实测该路径在 5.1.3 下装出的是**无法运行的 GUI 二进制**。GUI 侧的 `README.md:289` 已有 v5.1.3 已知问题段，但只提关窗缺陷，未提 CLI 这个。

## D. 操作层面的一条建议（与提交策略相关）

计划 Global Constraints 要求在 `.claude/worktrees/` 下开工，而 worktree 从 **HEAD** 检出。当前 **spec / plan / 交接三份均未提交**：

```
?? docs/superpowers/plans/2026-09-21-env-backup-restore-implementation.md
?? docs/superpowers/specs/2026-09-21-env-backup-restore-design.md
?? docs/审核和开发/2026.09.21/PathEditor-环境变量备份恢复核对轮交接.md
```

→ 建 worktree 后**这三份都看不到**，每个任务派出的 subagent 也看不到计划。技能文件 3.2 已写明这个坑（「计划或 spec 在主目录尚未提交，需要先 `cp` 进去」），但逐轮手工 `cp` 易漏。

**建议**：三份一起提交到 main 后**再**建 worktree。这样计划、spec、交接文档在 worktree 里天然可见，`progress.md` 账本也随 worktree 走，不依赖手工拷贝。是否授权提交请用户确认（本轮尚未授权提交）。
