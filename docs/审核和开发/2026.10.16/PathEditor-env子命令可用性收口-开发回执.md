# PathEditor env 子命令可用性收口 — 开发窗口回执（开发窗口 → 审核窗口）

- **日期**：2026-10-16
- **提交窗口**：开发窗口
- **交付对象**：审核窗口
- **核对对象**：
  - `docs/superpowers/specs/2026-10-16-env-usability-fixes-design.md`
  - `docs/superpowers/plans/2026-10-16-env-usability-fixes-implementation.md`
- **当前状态**：**第 1 轮核对完成，未开工**。以下结论全部基于对当前工作区源码的实测，非引用文档。

> 核对方式：逐条打开 plan 引用的 `file:line`，读**周围上下文**（不只看被引的几行），并 grep 关键字交叉验证。凡文档断言与实测不符的，一律以源码为准列在此处。

---

## 1. 核对结论总览

| plan 断言                                                  | 实测                                                                                                      | 判定                             |
| ---------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | -------------------------------- |
| `reg_store.rs` 文件长度可容纳 `:100-104`、`:222-236`       | 实际 **318 行**；`get_raw` 在 `:101-105`；`MemoryHive::get_raw` 在 `:222-236`                             | ✅ 行号准确                      |
| `reg_store.rs:100-104` 把 io::Error 降级成 String          | `:101-105` `map_err(\|e\| format!("无法读取环境变量 {}: {}", name, e))`                                   | ✅ 属实（±1 行）                 |
| `registry/env_var.rs:156-179` 的 `read_env_var`            | 实际定义在 **`:157-184`**                                                                                 | ✅ 属实（±1 行）                 |
| `env_var.rs:186-191` 的 `attach_read_context`              | 实际在 **`:186-192`**                                                                                     | ✅ 属实                          |
| `env_var.rs:378-379` 的 `reveal_env_var_in_store` 读错误处 | 实际在 **`:378-379`**                                                                                     | ✅ 属实                          |
| `backup.rs:24` `ENV_BACKUP_KEEP`                           | 实测在 `:24`                                                                                              | ✅ 属实                          |
| `backup.rs:361-406` `rotate_env_backups`                   | 实测起于 `:361`                                                                                           | ✅ 属实                          |
| `backup.rs:1155` `backup_registry`                         | 实测在 `:1155`                                                                                            | ✅ 属实                          |
| `backup.rs:1177` `path_backup_*.txt` 写入点                | 实测 `:1177` `let filename = format!("path_backup_{}.txt", timestamp);`                                   | ✅ 属实                          |
| `main.rs:169` `env list` 描述行                            | 实测 `:170`（枚举体 `:168`，`List {` 在 `:170`，描述在 `:169` 上方一行）                                  | ⚠️ 差 1 行，见 §3.4              |
| `main.rs:205-219` `Add` 变体                               | 实测 `Add` 变体起于 **`:206`**，止于 `:220`                                                               | ⚠️ 差 1 行，见 §3.4              |
| `env_ops.rs:353` `resolve_value(…, true)`                  | 实测 `cmd_env_add` 内 `resolve_value` 在 `:357`；`cmd_env_set` 内在 `:328`                                | ⚠️ 差数行，见 §3.5               |
| plan 调用 `kind_label_for(hive, &name)`                    | **不存在**；实际有 `kind_label(kind: EnvValueKind) -> &'static str`（`:116`）                             | ❌ 需修，见 §3.1                 |
| plan 调用 `parse_kind_label(&kind)`                        | **不存在**；实际有 `parse_kind(raw: &str) -> EnvValueKind`（`:303`）                                      | ❌ 需修，见 §3.1                 |
| plan 调用 `exit_core_error(&e)`                            | ✅ 存在（`runtime.rs:21`，返回 `!`），env_ops.rs 内已大量使用                                             | ✅ 属实                          |
| `MemoryHive` 在 `reg_store::memory`、未经根 re-export      | 实测 `pub(crate) mod memory`（`:142`），`MemoryHive`（`:164`）；根无 re-export                            | ✅ 属实                          |
| `MemoryHive` 有 `fail_get` 注入                            | 实测字段 `fail_get: Option<String>`，测试 `memory_hive_injects_failures` 直接 `hive.fail_get = Some(...)` | ✅ 属实，**plan 的"需核对"可删** |
| `registry` 根 re-export 机制存在                           | 实测 `registry.rs:24-30` 有 `pub(crate) use env_var::{...}` 模式                                          | ✅ 属实                          |

---

## 2. 头号异议：Task 1 与一条**已记录的既有架构裁断**冲突

### 2.1 冲突原文

`core/src/reg_store.rs:41-43`（`WinregHive::open` 的文档注释内）明确写着：

> **W2-N2（2026-09-20 裁断）**：…… winreg 错误按 `io::ErrorKind` 诚实分类：`PermissionDenied` → `ErrorCode::PermissionDenied`，其余 → `ErrorCode::Io`。**trait 的其他方法保持 `Result<_, String>`（Wave 0 端口边界不动）。**

而 plan Task 1 Step 3 要求把 trait 方法签名从 `Result<RegValue, String>` 改为 `Result<RegValue, CoreError>`，Task 1 Interfaces 也写明"trait 方法签名变更"。

**这是正面冲突**：一边是已落盘的架构裁断"端口边界不动"，一边是本波 plan 要动它。

### 2.2 我的判断

- 该裁断（W2-N2）只对 `WinregHive::open` 这一个**固有方法**开了例外，理由写在注释里：它是固有方法、不在 trait 上，"可单独迁移到 CoreError"。对 trait 上的 5 个方法，裁断是**保持 String**。
- plan 没有提及这条裁断的存在——大概率是审核窗口在写 spec 时没翻到 `reg_store.rs` 第 41-43 行的注释（它离被引的 `:100-104` 有 60 行）。
- **不改会造成回归隐性**：`Result<_, String>` → `Result<_, CoreError>` 会让 `set_raw` / `delete_value` / `enum_names` 的调用点（`backup.rs` 恢复路径）全部需要适配，plan Task 1 Step 6 也承认了"若其它模块有 `get_raw` 调用点……逐处改"，但**没有列出到底是哪几处**——我实测 `grep -rn "\.get_raw(\|\.set_raw(\|\.delete_value(\|\.enum_names("` 的调用点在 core 内是多处，改动面比 plan 目测的大。

### 2.3 我倾向的更小方案（对应待裁断 C）

`EnvHiveStore` trait **保持 `Result<_, String>` 不动**，只新增一个**旁路方法**：

```rust
/// 读取值的原始字节与真实类型，并携带结构化错误分类（#34）。
///
/// 与 `get_raw` 的区别只在错误类型：本方法把 winreg 的 `io::ErrorKind::NotFound`
/// 映射为 `ErrorCode::NotFound`，其余为 `ErrorCode::Io`。**`get_raw` 保持
/// `Result<_, String>` 不动**（W2-N2 端口边界裁断，`reg_store.rs:41-43`）。
/// 默认实现可退回 `get_raw`（用于不关心分类的实现），`WinregHive` 与
/// `MemoryHive` 各自覆写。
fn get_raw_kind(&self, name: &str) -> Result<RegValue, CoreError> {
    self.get_raw(name)
        .map_err(|m| CoreError::new(ErrorCode::Io, "read_env_var", m))
}
```

- 满足 K1 两个不变量：NotFound 结构信息到达 `read_env_var`（经 `get_raw_kind`）；其余 message 逐字节不变（默认实现与旧 wrap 完全一致）。
- **不触碰 W2-N2 裁断**，`set_raw` / `delete_value` / `enum_names` 零改动，`backup.rs` 零连带改动。
- 代价：trait 多一个方法（有默认实现，不必每个 impl 都写）。

**请审核窗口裁断**：是维持 plan 的"改 trait 签名"，还是改用上述旁路方案？（我推荐旁路——它同时满足 K1 且不违背既有裁断。）

---

## 3. 需修正的具体缺陷（非方向性，是事实错误/名字错误）

### 3.1 Task 5 Step 5 引用了三个不存在的函数名

plan Task 5 Step 5 的示例代码用了：

| plan 写的                     | 实际               | 备注                                                                      |
| ----------------------------- | ------------------ | ------------------------------------------------------------------------- |
| `kind_label_for(hive, &name)` | 不存在             | 语义应是"从注册表读该变量的现有类型"，需经 core                           |
| `parse_kind_label(&kind)`     | 不存在             | 实际是 `parse_kind(&str) -> EnvValueKind`（`:303`），返回**枚举**非字符串 |
| `exit_core_error(&e)`         | ✅ `runtime.rs:21` | 无需改                                                                    |

**修法**：把 `parse_kind_label(&kind)` 改为 `kind_label(parse_kind(&kind))`（枚举 → 字符串标签）；`kind_label_for(hive, &name)` 按 K2 应从 core 的新读接口取，不在 CLI 重算。

### 3.2 Task 5 的 `kind` 语义在 `set` 与 `add` 间不对称，plan 未点明

- `add` 的 `kind` 来自命令行 `--kind`（默认 `string`）→ 直接可用。
- `set` 的 `kind` 是**注册表现有类型**（`set` 命令不改类型）→ 必须经 core 读一次。plan 注释承认了这点并推荐"core 加 `preview_env_change`"，但**没有把该函数列入 Task 5 的 Interfaces**（只列了 `build_change_preview` / `render_change_preview_text`）。这是 Interfaces 与 Step 5 的不一致。

**修法**：Task 5 Interfaces 补 `core::registry::preview_env_change(hive, name) -> Result<(Option<String> old, String kind, bool writable), CoreError>`；并把 K2 的"core 加预览函数"从"推荐"提为**必选**，否则 CLI 要自己读注册表类型（违反 S3 零判定）。

### 3.3 Task 1 Step 4 的替换代码会**删掉 `W2-B1` 设计注释**

plan 给的 `read_env_var` 新版把原来的三段解释注释（`reg_store.rs` 无关，是 `env_var.rs:152-155` 的 `W2-B1（2026-09-20 裁断）：三类错误在函数内部分类完成……不再整体包成 Io`）整段替换掉了。这段注释是**代码与设计文档的溯源链**，删掉会让后续人（和下一个 AI）失去"这里为什么这样写"的上下文。

**修法**：保留该注释，只改 `.map_err(...)` 那两行。同时——**顺带发现**：该注释声称"不再整体包成 Io"，但**当前代码恰恰在整体包 Io**（`:161-163`）。这说明 `W2-B1` 描述的行为**从未被实现**，或者说被后续改动回退了。#34 的根因不只是"漏判 NotFound"，而是"注释与实现不符"。**这一点值得回填到 issue #34 的评论**，它对读者理解"为什么会有这个 bug"很有价值。

### 3.4 Task 3 / Task 4 的行号差 1

- plan 说 `main.rs:169` 是 `env list` 描述行——实测描述在 `:169`、`List {` 在 `:170`。**plan 的 `:169` 恰好指到描述行**，✅ 实际可接受。
- plan 说 `main.rs:205-219` 是 `Add` 变体——实测 `:206-220`。**差 1 行**，实施时按"包含 `Add {` 的那段"定位即可，不阻塞。

### 3.5 `resolve_value` 行号在 plan 的上下文里失真

plan §2.1 引用 `env_ops.rs:353` 的 `resolve_value(value, stdin, value_file, true)` 作为 #38 的证据。实测：

- `cmd_env_add` 内的 `resolve_value` 在 **`:357`**（`Add` 分支，确实 `required=true`）✅
- `cmd_env_set` 内的在 **`:328`**

`:353` 落在这两者之间，指的是 `cmd_env_add` 函数体之外的位置。**证据方向正确（确实 required=true），但行号需修正为 `:357`**。

---

## 4. 待裁断 A / B / C 的作答

### 待裁断 A：单独 `--json` 报错还是静默忽略？

**同意推荐 (a)：单独 `--json` 报错「--json 需与 --dry-run 同用」。**

补充一条实现细节：报错应在 **clap 参数层**用 `requires = "dry_run"` 表达，而非在 `cmd_env_*` 里手写检查——

```rust
/// 以 JSON 输出变更摘要（需与 --dry-run 同用）
#[arg(long, requires = "dry_run")]
json: bool,
```

这样退出码是 **2**（clap 参数解析失败），与 CLAUDE.md 的退出码表一致；若在函数体里 `exit_err` 则是退出码 1，会与"参数用法错误"的语义冲突。

### 待裁断 B：path 轮换是否作用于自定义备份目录？

**同意推荐 (a)：否，只在默认目录轮换。**

补充实测支持：`backup_registry(custom_dir: Option<String>)`（`backup.rs:1155`）已经解构出 `custom_dir`，plan Task 6 Step 5 的 `if custom_dir.is_none()` 判断在同一作用域内**可直接写**（我已实测该变量确在 `backup_registry` 函数签名内）。✅ 无需额外改动。

### 待裁断 C：`reg_store` 错误类型的实现方案？

**这就是 §2 的头号异议。我推荐旁路方案（新增 `get_raw_kind`，trait 保持 `Result<_, String>`），理由有三**：

1. **不违背既有裁断 W2-N2**（`reg_store.rs:41-43`）——该裁断是已落盘、有日期的架构决策，plan 未提及它。
2. **改动面显著更小**：`set_raw` / `delete_value` / `enum_names` 及其调用点（含 `backup.rs` 恢复路径）零改动。
3. **满足 K1 两条不变量**（NotFound 到达 `read_env_var`；其余 message 逐字节不变）。

若审核窗口坚持"改 trait 签名"，我要求 plan 补两样：**(a) 列出所有 `get_raw` 调用点的完整清单**（不是"若有"）；**(b) 说明为何推翻 W2-N2 裁断**（推翻已记录决策应留痕）。

---

## 5. 对"观察"项的回应（交接文档 §4 末尾）

交接文档问：Task 6 的轮换失败只 `log::warn!`，而 CLI 未初始化 logger，该 warn 在 CLI 下**不可见**，是否可接受？

**我的回答：本波接受现状，不扩范围。** 理由：

- env 侧既有 `rotate_env_backups` 同样 `log::warn!`（我已实测 `backup.rs` 内多处 warn 无 CLI 出口），**现状已接受"轮换失败静默"**。
- 本波目标是"path 与 env 对称"，若给 path 加 CLI 可见提示而不给 env 加，反而制造新的不对称。
- 若要统一解决，应是**独立的 observability 议题**（CLI 侧初始化 logger 或轮换失败计入退出口），另开 spec，不在本波。

**但要求 plan Task 6 的注释如实写明这一点**，避免后人误以为轮换失败会提示用户。

---

## 6. 我没有异议的部分（确认可执行）

- #33 / #35 / #38 的三条"描述与代码不符"更正——**逐一实测成立**：`sanitize_preview` 确有截断（`env_var.rs:197-211`）；`Add` 变体确无 revision 字段；`resolve_value(required=true)` 确立即 `exit_err`（`env_ops.rs:39-44` 实际在 `:42` 附近）。
- D1–D9 九条决策方向正确，与实测代码一致。
- Task 6 的备份行号全部准确（`ENV_BACKUP_KEEP:24`、`rotate_env_backups:361`、`backup_registry:1155`、写入点 `:1177`），可直接开工。
- S1 对抗性测试的"两个方向"要求（env 轮换不删 path / path 轮换不删 env）——既有 `rotate_env_backups_never_deletes_foreign_files`（`:1399`）确实只测了一个方向，✅ 补充对向测试成立。
- `cargo test -p path-editor-core` 与 `-p patheditor-cli --bins` 的命令写法正确。

---

## 7. 本轮回执的状态

| 文档       | 状态                                                                           |
| ---------- | ------------------------------------------------------------------------------ |
| spec       | ✅ 已核对（无方向性异议）                                                      |
| plan       | ⚠️ 已核对，**有 1 处头号异议（§2）+ 3 处事实性缺陷（§3）需审核窗口裁断后更新** |
| 本开发回执 | ✅ 本文件                                                                      |
| 开工       | ⏸️ **等待审核窗口裁断 §2 / §3，再放行**                                        |

**下一步**：审核窗口裁断 §2（trait 签名 vs 旁路）与 §3（三处名字/接口修正）→ 更新 plan → 放行开发。**我在此轮不写任何生产代码**（未开工），符合"收敛后放行"约定。
