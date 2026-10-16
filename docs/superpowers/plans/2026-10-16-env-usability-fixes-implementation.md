# env 子命令可用性收口 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 5.1.4 `env` 子命令的 6 条已核实 issue（#33/#34/#35/#36/#37/#38）一次收口：修正 1 处真实的错误泄漏（#34）、3 处文案/语义不一致（#33/#35/#38）、补齐 3 个破坏性写命令的 dry-run（#36），并给 path 备份加保留策略（#37）。

**Architecture:** 全部改动落在 `core` 与 `cli` 两层，**不涉 GUI**。唯一有回归风险的是 #34 的 `reg_store` 错误类型重构（保留 `ErrorKind`）；其余为新增只读预览函数、新增轮换函数、改 help 文案。判定（NotFound / 保留名单 / 类型 / 权限 / 保留份数）全部留在 core，CLI 只做参数转换与渲染。

**Tech Stack:** Rust（core / gui / cli 三 crate workspace，edition 2021）、winreg 0.52、chrono 0.4、dirs 5、serde / serde_json、clap 4。

**Spec:** `docs/superpowers/specs/2026-10-16-env-usability-fixes-design.md`

## Global Constraints

- **分支与提交**：本波**不授权提交**（用户 2026-10-16 明确：不提交、不推送、不升版本号）。开发窗口完成代码与质量门即可，提交须用户单独授权。若后续获授权：Conventional Commits，commit body 每行 ≤ 100 字符。
- **删除文件的硬约束**：全局规则「未经书面同意不得删除任何文件」。本计划中**唯一被授权的自动删除**是 §S1 限定的 `path_backup_*.txt` 轮换，且必须通过对向对抗性测试。env 备份轮换为既有已授权行为，本计划不改其范围。
- **架构边界**：`gui` / `cli` 只做参数转换、命令分派、错误呈现；保护名单、`Unsupported` 类型、hive 写权限、名称与值合法性、revision 校验**全部由 core 判定**，CLI 侧零安全判定逻辑。本计划不涉前端，故 `src/` 与 `gui/` 均无改动。
- **错误契约**：判定只认 `CoreError.code`（serde camelCase）；**不得**匹配 `message` 文本，不得匹配 `[E_CONFLICT]` 前缀。这是 #34 必须改 `reg_store` 而非文本匹配的根本原因。
- **文档注释**：所有 `pub fn` / `pub(crate) fn` 必须有 `///` 文档注释（CONTRIBUTING.md 硬性要求）。
- **代码风格**：UTF-8 / CRLF；TS 2 空格、Rust 与 TOML 4 空格；Prettier 单引号、尾逗号、100 列。无新增 `unsafe`。
- **注释语言**：代码注释一律中文。
- **测试命令**：CLI 单测必须用 `cargo test -p patheditor-cli --bins`（bin-only crate，`--lib` 会报 `no library targets found`）；core 单测 `cargo test -p path-editor-core <name>`。
- **质量门**：`npm run verify:all`（Prettier → ESLint → 构建 → 覆盖率 80% 行门槛 → `cargo fmt` → Clippy `-D warnings` → Rust 测试 → Playwright）。
- **真实注册表**：本计划**全程不写真实注册表**。所有 Rust 测试通过 `MemoryHive` 注入或临时目录隔离。
- **模块路径写法（已实证）**：`core/src/registry/env_var.rs` 所在模块是**私有**的 `mod env_var;`（`core/src/registry.rs:14`），**不得**写 `crate::registry::env_var::X` —— 会报 `error[E0603]: module env_var is private`。core 内部跨模块引用一律经 `crate::registry::X`（根 re-export），需要暴露新符号时在 `registry.rs` 加 `pub(crate) use`。
- **`reg_store` 内存实现位置**：`MemoryHive` 定义在 `reg_store::memory`（`#[cfg(test)] pub(crate) mod`），**未经根 re-export**；测试中写 `use crate::reg_store::memory::MemoryHive;`（`crate::reg_store::MemoryHive` **不成立**）。
- **每任务结束**：`cargo fmt` + `cargo clippy --workspace --all-targets -- -D warnings` 零警告。

---

## 文件结构

| 文件                           | 职责                                                                                                                                                                             |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `core/src/reg_store.rs`        | 修改：#34 让 `get_raw` 的错误保留 `ErrorKind`（K1），trait + `WinregHive` + `MemoryHive` 三处同步改                                                                              |
| `core/src/registry/env_var.rs` | 修改：#34 `read_env_var` 按 kind 映射 `NotFound`；#36 新增只读预览所需的公开读接口                                                                                               |
| `core/src/backup.rs`           | 修改：#37 新增 `PATH_BACKUP_KEEP` / `path_backup_keep()` / `rotate_path_backups()`；重构 `read_env_backup_keep` 为通用 `read_backup_keep(path, key)`；`backup_registry` 写后轮换 |
| `cli/src/main.rs`              | 修改：#33/#35/#38 help 文案；#36 三个 `EnvCmd` 变体加 `dry_run` / `json` 字段                                                                                                    |
| `cli/src/env_ops.rs`           | 修改：#36 三个 `cmd_env_*` 增加 dry-run 分支与 JSON 渲染                                                                                                                         |
| `CLAUDE.md` / `AGENTS.md`      | 修改：同步命令表、错误语义、config 新键（两者字节级一致）                                                                                                                        |
| `README.md` / `CHANGELOG.md`   | 修改：用户文档与发布日志                                                                                                                                                         |

---

### Task 1: `reg_store` 错误保留 `ErrorKind`（#34 根因）

**Files:**

- Modify: `core/src/reg_store.rs`（trait 定义 + `WinregHive` 实现 + `MemoryHive` 实现）
- Modify: `core/src/registry/env_var.rs`（`read_env_var` 消费新错误类型）
- Test: `core/src/reg_store.rs`（同文件 `#[cfg(test)] mod`）

**Interfaces:**

- Consumes: `crate::error::{CoreError, ErrorCode}`（已存在）
- Produces:
  - `fn get_raw_kind(&self, name: &str) -> Result<RegValue, CoreError>`（trait **新增**方法，带默认实现；`get_raw` 等既有 4 个方法签名**不变**）
  - `fn get_raw(&self, name: &str) -> Result<RegValue, String>`（**既有方法，签名不动** —— W2-N2 端口边界裁断）
  - 新增 `fn field_not_found_error(operation: &str, name: &str) -> CoreError`（`reg_store.rs` 私有辅助，产出 `ErrorCode::NotFound`）

> **实施自由度（K1）**：spec 允许开发窗口用更小的方案。**唯一硬性不变量**是：(1) `ErrorKind::NotFound` 的结构信息必须到达 `read_env_var`；(2) 其余错误的 `message` 文本与 v5.1.4 **逐字节一致**。
>
> **⚠️ 核对轮裁断 N1（2026-10-16，开发窗口异议 §2 成立）**：初稿本 Task 要求把 `EnvHiveStore::get_raw` 的签名从 `Result<_, String>` 改为 `Result<_, CoreError>`，**这与既有架构裁断 W2-N2（`core/src/reg_store.rs:48`：「trait 的其他方法保持 `Result<_, String>`，Wave 0 端口边界不动」）正面冲突**，而初稿未提及该裁断的存在（它离被引的 `:100-104` 约 60 行）。
>
> 经核实：改 trait 签名会连带改动 `set_raw` / `delete_value` / `enum_names` 的调用点（`registry/env_var.rs:203/685/852`、`backup.rs:214/809`、`registry/env_var.rs:268/581`）与 `backup.rs` 的恢复路径。
>
> **口径说明（核对轮第 2 轮更正）**：初稿写的「7 处」与开发窗口实测的「26 处」**都对但指的不是同一集合** —— 26 是全部 `.get_raw(` 文本出现次数（含 22 处测试）；7 是「`set_raw`/`delete_value`/`enum_names` 的生产调用点」。而开发窗口进一步指出：**`get_raw` 本身的生产调用点只有 4 处**（`env_var.rs:158/284`、`backup.rs:226/821`），其中本 Task 只改 1 处（`:158`），其余 3 处**刻意保持**（见 Step 4 的「邻近改动禁区」）。改签名的真实代价 = 4 处 `get_raw` + 7 处其它方法调用点。
>
> **裁断：采纳开发窗口的旁路方案** —— trait 签名**一切不动**，新增带默认实现的 `get_raw_kind`，仅 `WinregHive` 与 `MemoryHive` 覆写。理由：既满足 K1 两条不变量，又不推翻已落盘的架构裁断，且改动面从「7 处调用点 + 恢复路径」缩小到「2 个 impl + 1 个调用点」。**本节以下步骤已按此重写。**

- [ ] **Step 1: 写失败测试**

在 `core/src/reg_store.rs` 的 `#[cfg(test)] mod memory_tests` 中追加（测试对象是**新增的 `get_raw_kind`**，不是既有 `get_raw`）：

```rust
    /// #34：不存在的变量必须产出 `NotFound` 码，而不是笼统的 `Io`。
    ///
    /// 这是 `env get` 能给出「该变量不存在」而非「os error 2」的前提。
    #[test]
    fn get_raw_kind_missing_name_yields_not_found_code() {
        let hive = MemoryHive::new(true);
        hive.seed("PRESENT", "v", REG_SZ);

        let err = hive.get_raw_kind("ABSENT").expect_err("不存在应返回 Err");
        assert_eq!(
            err.code,
            crate::error::ErrorCode::NotFound,
            "不存在的变量必须是 NotFound 码，实际: {:?}",
            err.code
        );
    }

    /// #34 不变量 2：其它读取错误的 message 文本必须保持原样（不因新增方法而改变用户可见文案）。
    ///
    /// N1 裁断后：`fail_get` 注入场景下 `get_raw_kind` 默认实现直接复用 `get_raw`
    /// 的错误字符串，故 message 与 `get_raw` 逐字节相同。
    #[test]
    fn get_raw_kind_error_message_text_matches_get_raw() {
        // 既有测试 `memory_hive_injects_failures` 直接 `hive.fail_get = Some(...)`；
        // 该字段在测试模块内可见，本用例沿用同一手法（plan 初稿的 seed_fail_get 假设已删）。
        // ⚠️ 故障注入需要**可变绑定** —— 必须是 `let mut hive`（开发窗口 Task 1 实测修正）。
        let mut hive = MemoryHive::new(true);
        hive.fail_get = Some("BROKEN".to_string());

        let err = hive.get_raw_kind("BROKEN").expect_err("注入失败应返回 Err");
        assert_eq!(
            err.message, "无法读取环境变量 BROKEN: 注入的读取失败",
            "message 文本不得因 #34 而改变"
        );
    }
```

> **核对轮裁断 N6（2026-10-16，开发窗口实施时实测）**：本 Step 代码块初稿误写为 `let hive = MemoryHive::new(true);` —— 对 `hive.fail_get = Some(...)` 这种字段赋值，**必须 `let mut hive`**。既有测试 `memory_hive_injects_failures`（`reg_store.rs:363-364`）本就带注释「brief 原文为 `let hive`，故障注入需要可变绑定，此处加 `mut`」，plan 抄写时丢了该 `mut`。**已就地修正为 `let mut hive`。**

> **删去初稿的 `seed_fail_get` 假设**：开发窗口实测 `MemoryHive` 已有 `fail_get: Option<String>` 字段（`reg_store.rs:180` 附近），既有测试 `memory_hive_injects_failures` 直接赋值，**无需新增测试辅助**。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path-editor-core get_raw_kind_`
Expected: 编译失败 —— `cannot find method get_raw_kind`。

- [ ] **Step 3: 新增 `get_raw_kind`（trait 默认实现 + 两个 impl 覆写）**

**⚠️ trait 既有方法签名一律不动**（W2-N2）。只**新增**一个方法。

`core/src/reg_store.rs`，在 trait 的 `get_raw` 声明（`:24-25`）之后新增：

```rust
    /// 读取值的原始字节与真实类型，并携带结构化错误分类（#34）。
    ///
    /// 与 [`get_raw`] 的**唯一区别**是错误类型：本方法把 winreg 的
    /// `io::ErrorKind::NotFound` 映射为 [`ErrorCode::NotFound`]，其余为 [`ErrorCode::Io`]。
    /// `get_raw` 保持 `Result<_, String>` 不动（W2-N2 端口边界裁断，`reg_store.rs:48`）。
    ///
    /// 默认实现退回 `get_raw`（给不关心分类的实现用）；`WinregHive` 与 `MemoryHive` 各自覆写。
    /// 判定只认 `code`，**不得**匹配 message 文本（Global Constraints）。
    fn get_raw_kind(&self, name: &str) -> Result<RegValue, CoreError> {
        self.get_raw(name)
            .map_err(|m| CoreError::new(ErrorCode::Io, "read_env_var", m))
    }
```

`WinregHive` impl 内新增覆写（与既有 `get_raw` 相邻，**不动** `get_raw` 本身）：

```rust
    fn get_raw_kind(&self, name: &str) -> Result<RegValue, CoreError> {
        self.key.get_raw_value(name).map_err(|e| {
            let code = if e.kind() == std::io::ErrorKind::NotFound {
                ErrorCode::NotFound
            } else {
                ErrorCode::Io
            };
            CoreError::new(code, "read_env_var", format!("无法读取环境变量 {}: {}", name, e))
        })
    }
```

`MemoryHive` impl 内新增覆写：

```rust
    fn get_raw_kind(&self, name: &str) -> Result<RegValue, CoreError> {
        if self.fail_get.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(name)) {
            return Err(CoreError::new(
                ErrorCode::Io,
                "read_env_var",
                format!("无法读取环境变量 {}: 注入的读取失败", name),
            ));
        }
        self.values
            .borrow()
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| dup_reg_value(v))
            .ok_or_else(|| {
                CoreError::new(
                    ErrorCode::NotFound,
                    "read_env_var",
                    format!("无法读取环境变量 {}: 找不到", name),
                )
            })
    }
```

> 注意：`WinregHive::get_raw_kind` 的 message 在**非 NotFound** 分支必须与既有 `get_raw` 的 `format!("无法读取环境变量 {}: {}", name, e)` 逐字节相同 —— 满足不变量 2（`get_raw` 的 message 不变，`get_raw_kind` 与之对齐）。

- [ ] **Step 4: 改 `read_env_var` 调用 `get_raw_kind`（保留 W2-B1 注释）**

`core/src/registry/env_var.rs:156-184`：**只改 `.map_err(...)` 那两行**：

```rust
    let raw = store.get_raw(name)
        .map_err(|m| CoreError::new(ErrorCode::Io, "read_env_var", m))?;
```

改为：

```rust
    // #34：改用 `get_raw_kind` —— 它已把 NotFound 与 Io 分类，此处**不得**再统一包成 Io，
    // 否则「变量不存在」在到达用户前被稀释成通用 IO 错误（正是 #34 的根因）。
    // N1 裁断：`get_raw` 保留原样（W2-N2 端口边界），本函数改调新方法。
    let raw = store.get_raw_kind(name)?;
```

**⚠️ 控制审查提示（开发窗口 §3.3）**：初稿的替换代码会把 `env_var.rs:152-155` 的 **W2-B1 设计注释整段删掉**。该注释是代码与设计文档的溯源链，**必须保留**。本 Step 只改那两行，注释原样留在函数文档块里。

**🔴 必须遵守的「邻近改动禁区」（核对轮裁断 N5，开发窗口 §3 新发现）**：

改完本 Step 后，**同一文件 `registry/env_var.rs` 内会并存两种读法**。这是**有意的**，不是漏改：

| 位置                             | 用途                    | 改后用             | 为什么                                                                                                                                                           |
| -------------------------------- | ----------------------- | ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `env_var.rs:158` `read_env_var`  | 单变量读取（`env get`） | **`get_raw_kind`** | 需要区分 `NotFound`（#34 的诉求）                                                                                                                                |
| `env_var.rs:284` `list_env_vars` | 列表采集                | **保持 `get_raw`** | 列表路径不需要逐变量分类 —— 某变量读不到就报**一致的**「读取X环境变量列表失败」文案，这是 `with_hive(ErrorCode::Io, "list_env_vars", …)`（`:286-291`）的刻意设计 |

**必须在本 Step 落一行注释**说明这一点（建议加在 `read_env_var` 的新注释里，或紧邻 `:284`），否则后人（与下一个 AI）看到同文件两套读法，最自然的反应就是「统一一下」——而那个统一正是**非预期回归**（会把 `list_env_vars` 的错误文案也改掉）。

同一规则适用于另外两处**路径**（均非本 Task 改动范围）：

- `backup.rs:226`（备份采集）：**保持 `get_raw`** + `map_err` 到 `Io`，理由同 `list_env_vars`（全文一致的采集失败文案）。
- `backup.rs:821`（恢复预览）：**保持 `get_raw`** —— 它用的是 `let Ok(raw) = store.get_raw(&name) else { continue }`，即**逐条读取失败就跳过**，连错误都不构造。同样不得改为 `get_raw_kind`。

> **注意 `:821` 与 `:226` 的机制不同**（开发窗口回执把它们归为一类是口径不精确，实施时以本表为准）：`:226` 是 `map_err` 成 `Io` 文案；`:821` 是 `else { continue }` 静默跳过。两者的共同点是**都不需要 `NotFound` 分类**，故都保持 `get_raw`。

> **其余部分**（`UnsupportedType` 分支、`Parse` 分支、`Ok((raw.vtype, value))`）**不动**。

> **附带发现（须回填 GitHub issue #34）**：`env_var.rs:153-155` 的 W2-B1 注释声称「三类错误在函数内部分类完成……不再整体包成 Io」，但 `:157-159` 的当前代码**恰恰在整体包 Io** —— 即该注释描述的行为**从未实现**（或被后续改动回退）。这说明 #34 的根因不只是「漏判 NotFound」，而是「注释与实现不符」。开发窗口完成本 Task 后，**在 issue #34 补一条评论**记录此发现（用 `gh issue comment 34`，GitHub MCP 本次连接失败）。

> **`attach_read_context`（`:186-191`）保持不动** —— 它给错误补 hive 标签与「读取{系统}环境变量 X 失败: 」前缀。这层前缀在 #34 改造后仍需要（见 Task 2 对最终文案的处理）。

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p path-editor-core reg_store`
Expected: PASS（含既有 `memory_tests` 全部用例，证明新增 `get_raw_kind` 未影响既有 `get_raw`）。

- [ ] **Step 6: 跑 core 全量 + clippy**

Run: `cargo test -p path-editor-core && cargo clippy -p path-editor-core --all-targets -- -D warnings`
Expected: 全绿。**N1 裁断后无连带改动**：trait 既有方法签名未变，`backup.rs` 恢复路径、`env_var.rs` 写入路径的 `get_raw` / `set_raw` / `delete_value` / `enum_names` 调用点**均不需适配**。唯一的新调用点是 `read_env_var`（Task 1 Step 4）。

- [ ] **Step 7: 提交（**待用户授权**）**

本波默认不提交。若获授权：

```bash
git add core/src/reg_store.rs core/src/registry/env_var.rs
git commit -m "fix(core): 新增 get_raw_kind 保留 NotFound 错误码，修复 env get 泄漏 os error (#34)"
```

---

### Task 2: `env get` 错误文案收敛（#34 用户可见层）

**Files:**

- Modify: `core/src/registry/env_var.rs`（`reveal_env_var_in_store` 的读取错误处理）
- Test: `core/src/registry/env_var.rs` 的测试模块 —— **模块名是 `env_var_tests`**（非通用名 `tests`），新增用例加到该模块末尾

**Interfaces:**

- Consumes: Task 1 的 `read_env_var`（已产出 `ErrorCode::NotFound`）
- Produces: `reveal_env_var` 对不存在变量返回 `message == "{用户|系统}环境变量 {NAME} 不存在"`、`code == NotFound`

- [ ] **Step 1: 写失败测试**

在 `core/src/registry/env_var.rs` 的测试模块追加：

```rust
    /// #34：不存在的变量，message 必须是「{hive}环境变量 X 不存在」，
    /// 且**不含** `os error`、**不含**重复的变量名。
    #[test]
    fn reveal_missing_var_message_is_concise_and_named_once() {
        let hive = MemoryHive::new(true);
        let err = reveal_env_var_in_store(&hive, EnvHive::User, "THIS_VAR_ABSENT")
            .expect_err("不存在应返回 Err");

        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.message, "用户环境变量 THIS_VAR_ABSENT 不存在");
        assert!(!err.message.contains("os error"), "不得泄漏原生错误串: {}", err.message);
        assert_eq!(
            err.message.matches("THIS_VAR_ABSENT").count(),
            1,
            "变量名只应出现一次: {}",
            err.message
        );
    }

    /// #34 不变量：其它错误（如类型不支持）的 message 保持既有前缀，不因本次改动而变。
    #[test]
    fn reveal_unsupported_type_message_keeps_existing_prefix() {
        let hive = MemoryHive::new(true);
        hive.seed_raw("DWORD_VAR", {
            let mut v = "1".to_reg_value();
            v.vtype = winreg::enums::REG_DWORD;
            v
        });
        let err = reveal_env_var_in_store(&hive, EnvHive::User, "DWORD_VAR")
            .expect_err("Unsupported 应返回 Err");
        assert_eq!(err.code, ErrorCode::UnsupportedType);
        assert!(
            err.message.starts_with("读取用户环境变量 DWORD_VAR 失败:"),
            "非 NotFound 错误的前缀必须保持: {}",
            err.message
        );
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path-editor-core reveal_missing_var_message_is_concise`
Expected: FAIL —— 当前 message 是 `读取用户环境变量 THIS_VAR_ABSENT 失败: 无法读取环境变量 THIS_VAR_ABSENT: 找不到`，与期望不符。

- [ ] **Step 3: 在 `reveal_env_var_in_store` 的读取错误处加 NotFound 分支**

`core/src/registry/env_var.rs:378-379` 当前为：

```rust
    let (vtype, value) =
        read_env_var(store, name).map_err(|e| attach_read_context(e, hive, name))?;
```

改为：

```rust
    // #34：`NotFound` 走专用简洁文案（变量名只出现一次、不泄漏原生错误串）；
    // 其余错误保留 `attach_read_context` 的「读取X环境变量 Y 失败: 」前缀。
    let (vtype, value) = read_env_var(store, name).map_err(|e| {
        if e.code == ErrorCode::NotFound {
            CoreError::new(
                ErrorCode::NotFound,
                "reveal_env_var",
                format!("{}环境变量 {} 不存在", hive_location(hive).2, name),
            )
            .with_target(hive, name)
        } else {
            attach_read_context(e, hive, name)
        }
    })?;
```

> `hive_location(hive).2` 即「用户」/「系统」标签，与 `attach_read_context` 同源（`env_var.rs:187`）。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p path-editor-core reveal_`
Expected: PASS（两条新用例 + 既有 reveal 用例）。

- [ ] **Step 5: 验证 CLI 端到端文案**

构造一个确定不存在的变量名跑真实二进制（只读，不写注册表）：

Run: `cargo run -p patheditor-cli -- env get PATHEDITOR_ABSENT_PROBE_9F3`
Expected: stderr 为 `错误: 用户环境变量 PATHEDITOR_ABSENT_PROBE_9F3 不存在`，退出码 1，**无** `os error`。

> 该命令只读，不触碰注册表；即使变量恰好存在也只是打印它的值，无副作用。

- [ ] **Step 6: 提交（待授权）**

```bash
git add core/src/registry/env_var.rs
git commit -m "fix(core): env get 报「变量不存在」而非原生 os error (#34)"
```

---

### Task 3: `env list` help 文案（#33）

**Files:**

- Modify: `cli/src/main.rs:169`（`EnvCmd::List` 变体上方的文档注释；实测描述行在 `:169`、`List {` 在 `:170` —— 定位时找含「列出环境变量元数据」的那一行）

**Interfaces:** 无（纯文档注释）

- [ ] **Step 1: 改 help 文案**

`cli/src/main.rs:169`：

```rust
    /// 列出环境变量元数据（不含明文）
```

改为：

```rust
    /// 列出环境变量元数据（敏感变量不含明文，非敏感变量给出可能截断的 preview）
```

> **不要动** `sanitize_preview`（`core/src/env_var.rs:197-211`）—— 它**已实现截断**（超 `PREVIEW_MAX_CHARS` 加 `…`）。#33 的「方向 2 加截断」在核实中被否定（已实现），本 Task 只改文案。

- [ ] **Step 2: 验证 help 输出**

Run: `cargo run -p patheditor-cli -- env list --help`
Expected: 描述行包含「敏感变量不含明文，非敏感变量给出可能截断的 preview」。

- [ ] **Step 3: 提交（待授权）**

```bash
git add cli/src/main.rs
git commit -m "docs(cli): env list help 不再声称「不含明文」 (#33)"
```

---

### Task 4: `env add` help 补语义（#35 / #38）

**Files:**

- Modify: `cli/src/main.rs:206-220`（`EnvCmd::Add` 变体；实测起于 `:206`、止于 `:220`）
- Test: `cli/src/main.rs`（**新增** `#[cfg(test)] mod help_tests`，help 文案回归）

> **⚠️ 实施修正（开发窗口，Task 4 执行时发现）**：初稿写测试放 `cli/src/env_ops.rs`。实测**不可行** —— `struct Cli`（`main.rs:23`）**未 `pub`**，bin-only crate 的 root 对子模块私有，`env_ops.rs` 里写 `crate::Cli` 会因可见性失败。已按 Step 2 预留的退路把测试放到 `cli/src/main.rs` 的 root 级 `#[cfg(test)] mod help_tests`。同模块内另有 `env list` help 的回归测试（#33）。

**Interfaces:** 无（纯文档注释 + 一条文本断言测试）

- [ ] **Step 1: 改写 `Add` 变体文档注释与字段注释**

`cli/src/main.rs:206-220` 改为：

```rust
    /// 新建变量
    ///
    /// **纯新建语义**：同名变量会被拒绝（`NameExists`），不会覆盖 —— 因此无需
    /// revision 守卫（新建没有「旧值」可被并发覆盖）。注意检查与写入是两步，
    /// 存在毫秒级竞态窗口。
    ///
    /// 值必须经三通道之一提供；都不给会**立即报错**（不进入交互提示）。
    Add {
        name: String,
        /// 值。省略且不给 --stdin/--value-file 时报错（不进入交互提示）；
        /// 敏感值建议改用 --stdin 或 --value-file
        value: Option<String>,
        /// 从标准输入读取值（读到 EOF）
        #[arg(long, conflicts_with_all = ["value", "value_file"])]
        stdin: bool,
        /// 从文件读取值
        #[arg(long, conflicts_with_all = ["value", "stdin"])]
        value_file: Option<String>,
        /// 注册表类型：string (REG_SZ) 或 expand (REG_EXPAND_SZ)
        #[arg(long, default_value = "string", value_parser = ["string", "expand"])]
        kind: String,
        #[arg(short, long)]
        system: bool,
    },
```

> #38 核实：`resolve_value(…, required=true)`（`env_ops.rs:39-44`）零通道时**立即 `exit_err`**，不读 stdin，**不阻塞**。#38 **无代码改动**，本 Task 只在 help 里写明该行为，并由第 2 步的测试钉住。

- [ ] **Step 2: 写 help 文案回归测试**

在 `cli/src/env_ops.rs` 测试模块追加（用 clap 的 `render_long_help` 断言文案已落位）：

```rust
    /// #35 / #38：`env add --help` 必须写明「纯新建语义」与「省略值会报错」。
    ///
    /// 可证伪性：删掉 `Add` 变体的文档注释或字段注释，对应片段消失，断言失败。
    /// 这是把「豁免理由」从源码注释提升为**用户可见契约**的回归测试。
    #[test]
    fn env_add_help_documents_pure_create_and_no_prompt() {
        use clap::CommandFactory;
        let mut cmd = crate::Cli::command();
        let env = cmd
            .find_subcommand_mut("env")
            .expect("env 子命令必须存在");
        let add = env.find_subcommand_mut("add").expect("add 子命令必须存在");
        let help = add.render_long_help().to_string();

        assert!(
            help.contains("纯新建"),
            "add --help 必须写明纯新建语义（同名会被拒绝、不会覆盖）: {help}"
        );
        assert!(
            help.contains("不进入交互提示"),
            "add --help 必须写明省略值会报错、不交互: {help}"
        );
    }
```

> 若 `crate::Cli` 的 `CommandFactory` 在 bin-only crate 中不可直接取用，开发窗口可在 `main.rs` 用 `Cli::command()` 生成 help 后断言 —— 保留「断言 help 文本包含关键短语」这一可证伪目标即可，不要退化成只断言「解析成功」。

- [ ] **Step 3: 跑测试确认通过**

Run: `cargo test -p patheditor-cli --bins env_add_help_documents`
Expected: PASS。

- [ ] **Step 4: 人工核对 help**

Run: `cargo run -p patheditor-cli -- env add --help`
Expected: 出现「纯新建语义」「不进入交互提示」「不会覆盖」。

- [ ] **Step 5: 提交（待授权）**

```bash
git add cli/src/main.rs cli/src/env_ops.rs
git commit -m "docs(cli): env add help 补纯新建语义与不交互说明 (#35, #38)"
```

---

### Task 5: `env set/remove/add` 的 `--dry-run` 与 `--json`（#36）

**Files:**

- Modify: `cli/src/main.rs`（三个 `EnvCmd` 变体加 `dry_run` / `json` 字段）
- Modify: `cli/src/env_ops.rs`（三个 `cmd_env_*` 加 dry-run 分支）
- Test: `cli/src/env_ops.rs`（`#[cfg(test)] mod`）

**Interfaces:**

- Consumes: `crate::env_ops::{select_hive, hive_label, resolve_value, read_value, kind_label, parse_kind}`（**均已存在**）、`crate::runtime::exit_core_error`（`runtime.rs:21`，存在）
- Produces:
  - `core::registry::preview_env_change(hive: EnvHive, name: &str) -> Result<EnvChangePreview, CoreError>`（**core 新增**，见 Step 3b）
  - `pub struct EnvChangePreview { pub old_value: Option<String>, pub kind: String, pub writable: bool }`（core，camelCase 序列化）
  - `fn build_change_preview(op: &str, hive: EnvHive, name: &str, old: Option<String>, new: Option<String>, kind: Option<String>, writable: bool) -> serde_json::Value`
  - `fn render_change_preview_text(preview: &serde_json::Value) -> String`

> **⚠️ 核对轮裁断 N2（2026-10-16，开发窗口 §3.1/§3.2 成立）**：初稿 Task 5 的示例代码引用了两个**不存在**的函数（`kind_label_for(hive, &name)`、`parse_kind_label(&kind)`），且把「core 加预览函数」列为*推荐*而非*必选*。核实：
>
> - `kind_label_for` 全仓库不存在；`kind_label(kind: EnvValueKind) -> &'static str` 存在于 `env_ops.rs:116`。
> - `parse_kind_label` 不存在；`parse_kind(raw: &str) -> EnvValueKind` 存在于 `env_ops.rs:303`，**返回枚举非字符串**。
> - `set` 的 `kind` 是**注册表现有类型**（`set` 不改类型），必须经 core 读；若 CLI 自己读就违反 S3（CLI 零判定）。
>
> **裁断**：① `preview_env_change` 提为**必选** core 接口（新增 Step 3b）；② 修正两处函数名；③ `add` 的 kind 标签用 `kind_label(parse_kind(&kind))`（枚举→标签）。

- [ ] **Step 1: 写失败测试**

在 `cli/src/env_ops.rs` 测试模块追加：

```rust
    /// #36：预览 JSON 的字段与类型契约（spec §命令规格 4）。
    ///
    /// 字段名错一个就会破坏脚本消费，故逐字段断言，而非只断言「是合法 JSON」。
    #[test]
    fn change_preview_json_has_stable_contract() {
        let v = build_change_preview(
            "set",
            EnvHive::User,
            "JAVA_HOME",
            Some("C:\\jdk-17".into()),
            Some("C:\\jdk-21".into()),
            Some("string".into()),
            true,
        );
        assert_eq!(v["operation"], "set");
        assert_eq!(v["hive"], "user");
        assert_eq!(v["name"], "JAVA_HOME");
        assert_eq!(v["oldValue"], "C:\\jdk-17");
        assert_eq!(v["newValue"], "C:\\jdk-21");
        assert_eq!(v["kind"], "string");
        assert_eq!(v["writable"], true);
    }

    /// #36：`add` 的 oldValue 为 null、`remove` 的 newValue 为 null（spec 字段表）。
    #[test]
    fn change_preview_nulls_are_correct_per_operation() {
        let add = build_change_preview("add", EnvHive::User, "X", None, Some("v".into()), Some("string".into()), true);
        assert!(add["oldValue"].is_null(), "add 的 oldValue 必须为 null");

        let rm = build_change_preview("remove", EnvHive::User, "X", Some("v".into()), None, None, true);
        assert!(rm["newValue"].is_null(), "remove 的 newValue 必须为 null");
    }

    /// #36：文本预览必须点明「未写入任何内容」，且带 hive 标签。
    #[test]
    fn change_preview_text_mentions_no_write() {
        let v = build_change_preview("set", EnvHive::System, "JAVA_HOME", Some("a".into()), Some("b".into()), Some("string".into()), false);
        let text = render_change_preview_text(&v);
        assert!(text.contains("未写入任何内容"), "必须点明未写入: {text}");
        assert!(text.contains("系统"), "必须带 hive 标签: {text}");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p patheditor-cli --bins change_preview`
Expected: 编译失败 —— `cannot find function build_change_preview`。

- [ ] **Step 3: 在 `cli/src/env_ops.rs` 加两个预览辅助函数**

```rust
/// 构造 dry-run 的变更摘要（spec §命令规格 4 的统一 JSON 契约）。
///
/// 字段固定为 `operation / hive / name / oldValue / newValue / kind / writable`；
/// 无值一侧一律 `null`（`add` 无 oldValue，`remove` 无 newValue）。**判定不在本函数**——
/// `writable` 由调用方从 core 结果传入，本函数只做形状组装。
pub(crate) fn build_change_preview(
    op: &str,
    hive: EnvHive,
    name: &str,
    old: Option<String>,
    new: Option<String>,
    kind: Option<String>,
    writable: bool,
) -> serde_json::Value {
    serde_json::json!({
        "operation": op,
        "hive": match hive { EnvHive::System => "system", EnvHive::User => "user" },
        "name": name,
        "oldValue": old,
        "newValue": new,
        "kind": kind,
        "writable": writable,
    })
}

/// 把变更摘要渲染成人类可读的一行（spec §命令规格 4 的文本契约）。
///
/// 三种操作各自成句；一律以「（未写入任何内容）」收尾，避免用户误以为已生效。
pub(crate) fn render_change_preview_text(v: &serde_json::Value) -> String {
    let op = v["operation"].as_str().unwrap_or("");
    let hive = if v["hive"] == "system" { "系统" } else { "用户" };
    let name = v["name"].as_str().unwrap_or("");
    let old = v["oldValue"].as_str();
    let new = v["newValue"].as_str();
    let kind = v["kind"].as_str().unwrap_or("");
    let body = match op {
        "set" => format!("将更新{hive}变量 {name}：{} → {}", old.unwrap_or("-"), new.unwrap_or("-")),
        "add" => format!("将新建{hive}变量 {name}：{}（类型 {kind}）", new.unwrap_or("-")),
        "remove" => format!("将删除{hive}变量 {name}（当前值：{}）", old.unwrap_or("-")),
        _ => format!("将变更{hive}变量 {name}"),
    };
    format!("{body}（未写入任何内容）")
}
```

- [ ] **Step 3b: 在 core 新增 `preview_env_change`（N2 裁断：必选）**

**为什么必须放 core**：`set` 的 `kind` 是注册表现有类型、`writable` 是 hive 写权限 —— 两者都是判定，按 S3/Global Constraints **只能在 core**。CLI 若自己读注册表类型或调 `check_admin`，就是复制判定规则。

`core/src/registry/env_var.rs` 新增（紧邻 `reveal_env_var`）：

```rust
/// 预览一次环境变量变更所需的只读信息（#36）。
///
/// **只读**：不写注册表、不产生备份、不广播。判定（类型、写权限）在此完成，
/// CLI 只消费结果（S3：CLI 零判定）。
///
/// # Returns
/// - `Ok(EnvChangePreview)` — 变量存在时的当前值、注册表类型标签、该 hive 可写性
/// - `Err(CoreError)` — 变量不存在（`NotFound`）/ 保留名 / 读取失败
pub fn preview_env_change(hive: EnvHive, name: &str) -> Result<EnvChangePreview, CoreError> {
    let revealed = reveal_env_var(hive, name)?;
    // 类型标签与写权限的判定源都在 core：类型经 read 路径，权限经 capabilities。
    let kind = registry_kind_label(hive, name)?;
    let writable = capabilities_for(hive, name, /* kind 由上面算出 */ ...);
    Ok(EnvChangePreview { old_value: Some(revealed.value), kind, writable })
}

/// 变更预览的返回结构（camelCase 与前端/CLI 契约一致）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvChangePreview {
    /// 当前值；变量不存在时本函数已返回 `Err`，故此字段恒为 `Some`（保留 `Option` 便于将来扩展）
    pub old_value: Option<String>,
    /// 注册表类型标签：`"string"` / `"expand"`
    pub kind: String,
    /// 该 hive 对当前用户是否可写
    pub writable: bool,
}
```

> **实施结果（开发窗口，Task 5 执行时落定）**：初稿下方的示例代码有三处**示意性占位**（plan 自己也标注了"示意名"），实际实现如下（已成为源码，非示意）：

```rust
/// 预览一次环境变量变更所需的只读信息（#36）。
///
/// **只读**：不写注册表、不产生备份、不广播。判定（类型、写权限）在此完成，
/// CLI 只消费结果（S3：CLI 零判定）。
///
/// `writable` 是**该 hive 对当前用户的写权限**（hive 级，不含保护名单/类型判定）——
/// 与 spec §命令规格 4 的字段表一致，且满足验收标准 9「预览层不拒绝」：
/// 保护变量照常展示 `writable: false`，真正的拒绝留给执行阶段。
pub fn preview_env_change(hive: EnvHive, name: &str) -> Result<EnvChangePreview, CoreError> {
    let store = WinregHive::open(hive, false)?;
    let revealed = reveal_env_var_in_store(&store, hive, name)?;
    let kind = env_kind_label(&store, name)?;
    let writable = env_hive_writable(hive);
    Ok(EnvChangePreview {
        old_value: Some(revealed.value),
        kind,
        writable,
    })
}

/// 读取某变量的注册表类型标签（`"string"` / `"expand"`），供预览用。
fn env_kind_label(store: &dyn EnvHiveStore, name: &str) -> Result<String, CoreError> {
    let raw = store.get_raw_kind(name)?;
    Ok(match EnvValueKind::from_reg_type(raw.vtype) {
        EnvValueKind::ExpandString => "expand",
        _ => "string",
    }
    .to_string())
}

/// 该 hive 对当前用户是否可写（hive 级，只读探测）。
///
/// 与 `reg_store::hive_writable` 同源（`check_admin` / `can_write_user`）。
/// **必须 `pub`**：CLI 的 `env add --dry-run` 也需要它（该变量可能尚不存在，
/// 不能走 `preview_env_change`，后者对不存在变量返回 `Err`）。
pub fn env_hive_writable(hive: EnvHive) -> bool {
    match hive {
        EnvHive::System => crate::system::check_admin(),
        EnvHive::User => crate::registry::can_write_user(),
    }
}

/// 变更预览的返回结构（camelCase 与前端/CLI 契约一致）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvChangePreview {
    /// 当前值；变量不存在时本函数已返回 `Err`，故此字段恒为 `Some`
    pub old_value: Option<String>,
    /// 注册表类型标签：`"string"` / `"expand"`
    pub kind: String,
    /// 该 hive 对当前用户是否可写
    pub writable: bool,
}
```

> **三处对初稿的修正（都已落地）**：① `registry_kind_label(hive, name)?` → 实为私有辅助 `env_kind_label(&store, name)`（复用 `get_raw_kind`，与 Task 1 同源）；② `capabilities_for(...)` **不用** —— 它会掺入保护名单判定（对 `windir` 返回 `false`），而 spec 要的 `writable` 是**纯 hive 写权限**（验收标准 9：预览层不拒绝）；③ 新增 `pub fn env_hive_writable`（plan 授权项），并在 `core/src/registry.rs` 的 `pub use env_var::{...}` 组里补 `preview_env_change, env_hive_writable, EnvChangePreview`。

- [ ] **Step 4: 给三个 `EnvCmd` 变体加参数**

`cli/src/main.rs` 的 `Set` / `Add` / `Remove` 三个变体各加：

```rust
        /// 只显示将要发生的变更，不写注册表
        #[arg(long)]
        dry_run: bool,
        /// 以 JSON 输出变更摘要（需与 --dry-run 同用）
        #[arg(long, requires = "dry_run")]
        json: bool,
```

> **核对轮裁断 N3（2026-10-16，开发窗口 §4-A 补充成立）**：`--json` 用 clap 的 `requires = "dry_run"` 表达「必须与 --dry-run 同用」。这样单独 `--json` 由 clap 拒绝，退出码 **2**（参数解析失败），与 `CLAUDE.md` 退出码表一致；若改为在 `cmd_env_*` 里手写 `exit_err`，退出码会是 1，与「参数用法错误」语义冲突。**采纳开发窗口的补充。**

并在 `main()` 的 match 分发里把新字段透传给 `env_ops::cmd_env_set/add/remove`（函数签名相应加 `dry_run: bool, json_out: bool`）。

- [ ] **Step 5: 在三个 `cmd_env_*` 加 dry-run 分支**

`cmd_env_set`（在 `let new_value = read_value(&src);` 之后、`match mode` 之前插入；**并把 `resolve_concurrency` 移到本分支之后**）：

```rust
    if dry_run {
        // 预览是只读的：经 core 的 preview_env_change 取当前值/类型/写权限。
        // 变量不存在时 core 返回 NotFound，照常报错 —— 预览与执行判定一致（验收标准 8）。
        let preview = core::registry::preview_env_change(hive, &name)
            .unwrap_or_else(|e| exit_core_error(&e));
        let json = build_change_preview(
            "set", hive, &name,
            preview.old_value, Some(new_value), Some(preview.kind), preview.writable,
        );
        emit_preview(&json, json_out);
        return;
    }
```

`cmd_env_add`（`kind` 是命令行给出的待建类型，用 `kind_label(parse_kind(&kind))` 转标签）：

```rust
    if dry_run {
        // add 的 kind 来自 --kind（默认 string），非注册表读取；writable 经 core 判定。
        let json = build_change_preview(
            "add", hive, &name, None, Some(new_value.clone()),
            Some(kind_label(parse_kind(&kind)).to_string()), true,
        );
        emit_preview(&json, json_out);
        return;
    }
```

> `add` 的 `writable` 可经 core 的 capabilities（`core::env_var::capabilities_for` 或经 `preview_env_change` 的同类接口）取；开发窗口若发现无现成只读入口，可在 core 补一个轻量的 `env_hive_writable(hive) -> bool`，**不得**在 CLI 调 `check_admin`。

`cmd_env_remove`：

```rust
    if dry_run {
        let preview = core::registry::preview_env_change(hive, &name)
            .unwrap_or_else(|e| exit_core_error(&e));
        let json = build_change_preview("remove", hive, &name, preview.old_value, None, None, preview.writable);
        emit_preview(&json, json_out);
        return;
    }
```

新增共用出口：

```rust
/// dry-run 的统一出口：`--json` 打 JSON，否则打文本。**不写注册表、不备份、不广播。**
fn emit_preview(preview: &serde_json::Value, json_out: bool) {
    if json_out {
        println!("{}", serde_json::to_string_pretty(preview).unwrap());
    } else {
        println!("{}", render_change_preview_text(preview));
    }
}
```

> **关键**：dry-run 分支必须在 `resolve_concurrency` 调用**之前**（`cmd_env_set`/`cmd_env_remove` 当前第一行就是 `resolve_concurrency`）—— 预览**不要求** `--revision`/`--force`（spec D9 / 验收标准 7）。因此把 `resolve_concurrency` 移到 dry-run 判断之后。
>
> **N2 相关**：不要在 CLI 侧调用 `reveal_env_var` 后再自算类型/权限 —— 直接用 `preview_env_change` 一次拿到 `{old_value, kind, writable}`。

- [ ] **Step 6: 跑测试确认通过**

Run: `cargo test -p patheditor-cli --bins change_preview`
Expected: PASS。

- [ ] **Step 7: 端到端验证 dry-run 不写注册表**

> **⚠️ 命令修正（开发窗口，Task 5 执行时发现）**：初稿的 `env set PATHEDITOR_PROBE_9F3 v --dry-run` **有误** —— `set` 的值通道是 `--value <V>`，不是位置参数（只有 `add` 才接受位置参数 `[VALUE]`）。按初稿形式跑会得到 clap 参数错误（退出码 2），**测不到** notFound。正确形式见下。

Run: `cargo run -p patheditor-cli -- env set PATHEDITOR_PROBE_9F3 --value v --dry-run`
Expected: 因该变量不存在 → `错误: 用户环境变量 PATHEDITOR_PROBE_9F3 不存在`，退出码 1（验收标准 8）。

Run: `cargo run -p patheditor-cli -- env remove PATHEDITOR_PROBE_9F3 --dry-run`
Expected: 同上（notFound、退出码 1）。

Run: `cargo run -p patheditor-cli -- env add PATHEDITOR_PROBE_9F3 v --dry-run --json`
Expected: 打印 JSON，`oldValue: null`、`newValue: "v"`、`writable: <bool>`，**不写注册表**（用 PowerShell `Get-ItemProperty -Path 'HKCU:\Environment' -Name PATHEDITOR_PROBE_9F3` 确认返回 `$null`；**不要用 Git Bash 的 `reg query`**，反斜杠会被 shell 吃掉导致语法错误）。

Run: `cargo run -p patheditor-cli -- env add NAME v --json`（单独 `--json`，不带 `--dry-run`）
Expected: clap 拒绝（N3 裁断），提示 `--dry-run` 未提供，退出码 **2**。

- [ ] **Step 8: 提交（待授权）**

```bash
git add cli/src/main.rs cli/src/env_ops.rs
git commit -m "feat(cli): env set/remove/add 支持 --dry-run 与 --json 变更预览 (#36)"
```

---

### Task 6: `path_backup_*` 保留策略（#37）

**Files:**

- Modify: `core/src/backup.rs`（新增常量/函数、重构 INI 读取、`backup_registry` 写后轮换）
- Test: `core/src/backup.rs`（同文件 `#[cfg(test)] mod tests`）

**Interfaces:**

- Consumes: 既有 `read_env_backup_keep`、`config_file_path`、`backup_base_dir`、`should_warn_on_read_error`
- Produces:
  - `pub const PATH_BACKUP_KEEP: usize = 20`
  - `fn read_backup_keep(path: &Path, key: &str) -> usize`（通用化，两个调用者共用）
  - `fn path_backup_keep() -> usize`
  - `fn rotate_path_backups(dir: &Path, keep: usize) -> Result<Vec<PathBuf>, CoreError>`

- [ ] **Step 1: 写失败测试**

在 `core/src/backup.rs` 测试模块追加（复用既有的 `with_temp_backup_dir` 与 `path_backup_*.txt` 夹具命名，与 `rotate_env_backups_never_deletes_foreign_files` 同形）：

```rust
    /// #37：path 轮换只删 `path_backup_*.txt`，保留最近 keep 份。
    #[test]
    fn rotate_path_backups_keeps_exactly_keep_files() {
        with_temp_backup_dir(|dir| {
            for i in 0..25 {
                std::fs::write(
                    dir.join(format!("path_backup_202609{:02}_120000_000.txt", i)),
                    "PATH",
                )
                .unwrap();
            }
            let removed = rotate_path_backups(dir, PATH_BACKUP_KEEP).expect("轮换失败");
            let remaining = std::fs::read_dir(dir).unwrap().count();
            assert_eq!(remaining, PATH_BACKUP_KEEP, "必须恰好保留 keep 份");
            assert_eq!(removed.len(), 5);
        });
    }

    /// #37 对抗性测试（spec §S1）：轮换**不得**删除 env 备份或任何非 path_backup 文件。
    ///
    /// 这是对向断言：env 侧既有测试证明「env 轮换不删 path 文件」，
    /// 本用例证明「path 轮换不删 env 文件」——两个方向都必须成立。
    #[test]
    fn rotate_path_backups_never_deletes_foreign_files() {
        with_temp_backup_dir(|dir| {
            for i in 0..25 {
                std::fs::write(dir.join(format!("path_backup_202609{:02}_120000_000.txt", i)), "PATH").unwrap();
            }
            // 以下全部不得被删
            std::fs::write(dir.join("env_backup_20260901_120000_000.json"), "{}").unwrap();
            std::fs::write(dir.join("path_backup_20260901_120000_000.txt.bak"), "PATH").unwrap();
            std::fs::write(dir.join("path_backup_20260901_120000_000.txt.corrupt-123"), "x").unwrap();
            std::fs::write(dir.join("notes.txt"), "user file").unwrap();

            rotate_path_backups(dir, PATH_BACKUP_KEEP).expect("轮换失败");

            assert!(dir.join("env_backup_20260901_120000_000.json").exists(), "env 备份必须保留");
            assert!(dir.join("path_backup_20260901_120000_000.txt.bak").exists(), ".bak 必须保留");
            assert!(dir.join("path_backup_20260901_120000_000.txt.corrupt-123").exists(), ".corrupt-* 必须保留");
            assert!(dir.join("notes.txt").exists(), "无关文件必须保留");
        });
    }

    /// #37：`path_backup_keep` 的四种回落与 env 同口径。
    #[test]
    fn read_path_backup_keep_falls_back_like_env() {
        let missing = std::env::temp_dir().join("patheditor_no_such_cfg_9f3.ini");
        let _ = std::fs::remove_file(&missing);
        assert_eq!(read_backup_keep(&missing, "path_backup_keep"), PATH_BACKUP_KEEP);

        for bad in ["path_backup_keep = abc\n", "path_backup_keep = -5\n", "path_backup_keep =\n"] {
            let cfg = std::env::temp_dir().join(format!("patheditor_cfg_{}.ini", bad.len()));
            std::fs::write(&cfg, bad).unwrap();
            assert_eq!(read_backup_keep(&cfg, "path_backup_keep"), PATH_BACKUP_KEEP, "非法值应回落: {bad}");
            let _ = std::fs::remove_file(&cfg);
        }
    }

    /// #37：`env_backup_keep` 的既有解析经 `read_backup_keep` 通用化后逐字不回归。
    #[test]
    fn read_env_backup_keep_generic_refactor_preserves_behavior() {
        let cfg = std::env::temp_dir().join("patheditor_cfg_env_9f3.ini");
        std::fs::write(&cfg, "; c\n# c2\nenv_backup_keep   =   7   \n").unwrap();
        assert_eq!(read_backup_keep(&cfg, "env_backup_keep"), 7);
        let _ = std::fs::remove_file(&cfg);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path-editor-core rotate_path_backups`
Expected: 编译失败 —— `cannot find function rotate_path_backups`、`cannot find value PATH_BACKUP_KEEP`。

- [ ] **Step 3: 加常量与通用 INI 解析**

`core/src/backup.rs`，在 `ENV_BACKUP_KEEP`（`:24`）旁：

```rust
/// path 备份默认保留份数（spec D7，与 [`ENV_BACKUP_KEEP`] 对称）。
///
/// `path_backup_*.txt` 单文件约 2.3 KB，历史上无轮换，实测已累积约 250 份。
pub const PATH_BACKUP_KEEP: usize = 20;
```

把 `read_env_backup_keep(path)` 通用化为 `read_backup_keep(path, key, fallback)`：把函数体内 `key.trim() != "env_backup_keep"` 改为 `k.trim() != key`（新参数），warn 文案里的键名也用参数。原 `read_env_backup_keep` 改为薄包装：

```rust
/// 读取 env 备份保留份数（[`read_backup_keep`] 的 env 专用入口）。
fn read_env_backup_keep(path: &Path) -> usize {
    read_backup_keep(path, "env_backup_keep", ENV_BACKUP_KEEP)
}

/// 读取 path 备份保留份数。
fn path_backup_keep() -> usize {
    read_backup_keep(&config_file_path(), "path_backup_keep", PATH_BACKUP_KEEP)
}
```

> **⚠️ 签名修正（开发窗口，Task 6 执行时发现）**：初稿的 `read_backup_keep(path, key)` 只有 **2 个参数**，把"回落值"隐式假定为可由 key 推出。实际两处调用者的回落值虽然当前都是 20，但**语义上独立**（`ENV_BACKUP_KEEP` / `PATH_BACKUP_KEEP`），且旧 `read_env_backup_keep` 的回落是硬编码 `ENV_BACKUP_KEEP`。若用 `if key == "path_backup_keep" { PATH_BACKUP_KEEP } else { ENV_BACKUP_KEEP }` 硬编码判断，等于把"哪个 key 对应哪个回落值"的知识塞进通用函数——**不是通用化，是伪装**。
> **已改为 3 参数 `read_backup_keep(path, key, fallback)`**：函数只管"解析 key"，回落值由调用者（知道语义的一方）传入。行为不变（两处回落值当前均为 20），但契约更诚实。既有 `read_env_backup_keep` 薄包装签名不变，故其既有测试零改动。

> **重构要求**：`read_backup_keep(path, "env_backup_keep")` 的行为必须与旧 `read_env_backup_keep(path)` **逐用例一致** —— 第 1 步的 `read_env_backup_keep_generic_refactor_preserves_behavior` + 既有 `read_env_backup_keep_*` 用例共同保证。

- [ ] **Step 4: 加 `rotate_path_backups`**

在 `rotate_env_backups`（`:361-406`）之后新增，与其同构，仅匹配规则不同：

```rust
/// path 备份保留策略轮换：只删除本功能生成的 `path_backup_*.txt`，保留最近 `keep` 份。
///
/// 授权范围（spec §S1，用户 2026-10-16 明确授权的例外），与 [`rotate_env_backups`] 同构：
/// 1. 只匹配文件名 `path_backup_*.txt`；
/// 2. 只在给定目录内操作（自定义备份目录不参与轮换）；
/// 3. 只删最旧的、超出 `keep` 之外的文件；
/// 4. 绝不删除 `env_backup_*.json`、`.bak`、`.corrupt-*` 及目录中任何其它文件。
///
/// # Returns
/// - `Ok(Vec<PathBuf>)` — 被删除的文件路径
/// - `Err(CoreError)` — 目录枚举失败（code=`Io`）；单个文件删除失败只记 warn
fn rotate_path_backups(dir: &Path, keep: usize) -> Result<Vec<PathBuf>, CoreError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| {
        CoreError::new(ErrorCode::Io, "rotate_path_backups", format!("枚举备份目录 {} 失败: {}", dir.display(), e))
    })?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // 只收 path_backup_*.txt；`.txt.bak` / `.txt.corrupt-*` 因后缀不是 `.txt` 天然排除。
        if !name.starts_with("path_backup_") || !name.ends_with(".txt") {
            continue;
        }
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        candidates.push((name, path));
    }
    if candidates.len() <= keep {
        return Ok(Vec::new());
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    let mut removed = Vec::new();
    for (_, path) in candidates.into_iter().skip(keep) {
        match std::fs::remove_file(&path) {
            Ok(()) => { log::info!("已轮换删除旧 path 备份: {}", path.display()); removed.push(path); }
            Err(e) => log::warn!("删除旧 path 备份 {} 失败: {}", path.display(), e),
        }
    }
    Ok(removed)
}
```

- [ ] **Step 5: `backup_registry` 写后调用轮换**

`core/src/backup.rs:1194`（`std::fs::write(&filepath, &content)` 之后、`let result = ...` 之前）插入：

```rust
    // #37：与 env 备份对称，写后轮换。只作用于 path_backup_*.txt，且 custom_dir 不轮换
    // （spec §S1 / 核对轮裁断 N4：用户指定目录可能混有他自己的文件，轮换风险高于收益）。
    //
    // 注意：轮换失败只 log::warn!，而 CLI 未初始化 logger（见 runtime.rs:28），
    // 因此该警告在 CLI 下**不可见**。这是**有意的**，与 env 侧 rotate_env_backups 对称
    // （它同样只有 log::warn!）。**不要**据此误以为用户会看到提示 —— 若要统一解决，
    // 应作为独立 observability 议题（CLI 初始化 logger 或轮换失败计入退出口），不在本波。
    if custom_dir.is_none() {
        if let Err(e) = rotate_path_backups(&backup_dir, path_backup_keep()) {
            log::warn!("path 备份轮换失败（备份已落盘）: {}", e.message);
        }
    }
```

> **核对轮裁断 N4（2026-10-16）**：
>
> - **待裁断 B 裁断为 (a)**：只在默认目录轮换。开发窗口实测 `custom_dir` 确在 `backup_registry(custom_dir: Option<String>)` 作用域内（`backup.rs:1155`），`is_none()` 可直接写 —— ✅ 无需额外改动。
> - **轮换失败静默**：开发窗口 §5 的回应（接受现状、不加 CLI 提示、要求注释写明）**采纳**。上方注释已按要求写明「CLI 下不可见」这一事实，避免后人误解。

- [ ] **Step 6: 跑测试确认通过**

Run: `cargo test -p path-editor-core backup`
Expected: PASS（4 条新用例 + 全部既有 backup 用例，证明重构与轮换均未回归）。

- [ ] **Step 7: 对抗性对向验证**

在临时目录里手工放一份 `env_backup_x.json` 与 25 份 `path_backup_*.txt`，触发一次 path 写入（或直接调 `rotate_path_backups`），确认 `env_backup_x.json` 仍在。

Run: `cargo test -p path-editor-core rotate_path_backups_never_deletes_foreign_files`
Expected: PASS（spec 验收标准 11）。

> **⚠️ 补充（开发窗口，Task 6 执行时新增）**：初稿的 Step 7 只验证了 `rotate_path_backups` **函数本身**，未验证它**被 `backup_registry` 接线**（若 Step 5 的调用漏写或条件写反，函数测试照样全绿）。已新增接线测试 `backup_registry_triggers_path_rotation`（预置 25 份旧备份 → 调 `backup_registry(None)` → 断言收敛到 keep 份），并**经变异测试验证其有效性**（临时禁用 Step 5 接线 → 该测试转红，`left:26 / right:20`）。
> **该测试的已知局限**（如实记录）：无权限读注册表时 `backup_registry` 返回 `Err`，断言被跳过——测试会"静默通过但未验证"。这与既有 `backup_registry_with_custom_dir` 同手法（项目约定）。更彻底的方案是让 `backup_registry` 接受注入的 store，属超出本波的重构，**未做**。本机实测该分支返回 `Ok`（变异测试已证明断言真实执行）。

- [ ] **Step 8: 提交（待授权）**

```bash
git add core/src/backup.rs
git commit -m "feat(core): path 备份加入保留策略，默认 20 份 (#37)"
```

---

### Task 7: 文档同步与质量门

**Files:**

- Modify: `CLAUDE.md`、`AGENTS.md`（字节级一致）
- Modify: `README.md`、`CHANGELOG.md`

**Interfaces:** 无

- [ ] **Step 1: 同步 CLAUDE.md / AGENTS.md**

- CLI 命令表（`CLAUDE.md:131-149`）：`env set` / `env add` / `env remove` 各补 `[--dry-run] [--json]`。
- `env` 子命令说明段（`:153`）：补一句 dry-run 语义（只读、不需要 revision、不含保护名单判定）。
- 数据与事务段（`:163`）：`config.ini` 说明补 `path_backup_keep` 键。
- 错误语义：补 `env get` 的 NotFound 文案约定（`{用户|系统}环境变量 X 不存在`）。

改完后**同步复制**，保证 `diff CLAUDE.md AGENTS.md` 为空。

> **⚠️ 执行记录（开发窗口，Task 7 执行时）**：执行前发现 `CLAUDE.md` 与 `AGENTS.md` **已经不一致** —— `CLAUDE.md` 被某个**外部 Markdown 工具**（非项目 Prettier）格式化过（表格分隔行加长 + 表格前加空行），而 `AGENTS.md` 保持原样。该外部格式**不符合项目 Prettier 配置**（`npx prettier --check` 判其不合规）。
>
> **处置（按裁断 Q_A「接受 Prettier」）**：用项目 Prettier 规范化 `AGENTS.md`，再复制给 `CLAUDE.md`，使两者回到 **Prettier 标准格式并字节一致**。净效果是 4 处内容改动 + 一批 Prettier 格式规范化。
>
> **教训**：该外部工具与项目 Prettier 标准不一致，会让 `CLAUDE.md`/`AGENTS.md` 持续漂移且违反二者"必须一致"的约束。若你的编辑器装了 Markdown 表格美化插件，建议对这两个文件关闭自动格式化，或统一用项目 Prettier。

- [ ] **Step 2: 更新 README 与 CHANGELOG**

- `README.md`：`env` 子命令章节补 `--dry-run` 示例；备份章节补 path 备份保留策略与「含明文路径」提示。
- `CHANGELOG.md`：顶部写 5.1.5 段落，中文小节标题沿用（`### 新增` / `### 变更` / `### 修复` / `### 说明`）。

> **注意**：本波**不升版本号**（Global Constraints）。CHANGELOG 段落可先写「5.1.5（未发布）」，版本号四处同步等用户明确要求发版时再做。

- [ ] **Step 3: 质量门全绿**

Run: `npm run verify:all`
Expected: 全绿（Prettier → ESLint → 构建 → 覆盖率 ≥ 80% → cargo fmt → Clippy → Rust 测试 → Playwright）。

- [ ] **Step 4: 核对两文档字节级一致**

Run: `diff CLAUDE.md AGENTS.md`（或在 Windows 用 `fc /b CLAUDE.md AGENTS.md`）
Expected: 无差异输出。

- [ ] **Step 5: 提交（待授权）**

```bash
git add CLAUDE.md AGENTS.md README.md CHANGELOG.md
git commit -m "docs: 同步 env dry-run、错误语义与 path 备份保留策略"
```

---

## Self-Review

### 规格覆盖

| spec 验收标准                   | 落在                                 |
| ------------------------------- | ------------------------------------ |
| 1（`env get` 新文案）           | Task 2                               |
| 2（其它错误文本逐字节不变）     | Task 1 Step 4 + Task 2 Step 1 第二条 |
| 3（`env list` help）            | Task 3                               |
| 4（`env add` help）             | Task 4                               |
| 5（dry-run 不写/不备份/不广播） | Task 5 Step 7                        |
| 6（`--dry-run --json` 契约）    | Task 5 Step 1                        |
| 7（dry-run 不要 revision）      | Task 5 Step 5 提示                   |
| 8（不存在仍 notFound）          | Task 5 Step 7                        |
| 9（`add` 预览不拒绝）           | Task 5 Step 5                        |
| 10（path 轮换保留 N）           | Task 6 Step 1                        |
| 11（对抗性零删除）              | Task 6 Step 1 第二条 + Step 7        |
| 12（`path_backup_keep` 回落）   | Task 6 Step 1 第三条                 |
| 13（env 既有不回归）            | Task 6 Step 1 第四条 + Step 6        |
| 14（两文档一致）                | Task 7 Step 1/4                      |
| 15（质量门）                    | Task 7 Step 3                        |
| 16（文档注释/无 unsafe）        | 各 Task 内约束                       |

**待裁断 A / B** 已在 Task 5 / Task 6 内就地标注推荐项 —— **均已于核对轮第 1 轮裁断**（N3 采纳 a + clap `requires`；N4 裁断为 a）。

### 占位符扫描

无「TBD」「TODO」「适当处理」类占位。Task 5 Step 3b 的 `preview_env_change` 内部实现（`kind`/`writable` 的具体取法）留了实现自由度，但**明确给出了必须满足的两条约束**（全在 core、不写注册表），非占位。

### 类型一致性

- `build_change_preview` 的参数顺序在 Task 5 的定义与三条测试、三个调用点一致（`op, hive, name, old, new, kind, writable`）。
- `preview_env_change(hive, name) -> Result<EnvChangePreview, CoreError>` 在 Task 5 的 Interfaces、Step 3b 定义、三个调用点一致；`EnvChangePreview { old_value, kind, writable }` 字段名在 Step 3b 与 Step 5 一致。
- `get_raw_kind(&self, name) -> Result<RegValue, CoreError>` 在 Task 1 的 trait 声明、两个 impl 覆写、`read_env_var` 调用点一致。
- `rotate_path_backups(dir, keep) -> Result<Vec<PathBuf>, CoreError>` 与 `rotate_env_backups` 同签名。
- `read_backup_keep(path, key)` 在 Task 6 的定义与三处调用一致。

---

## Execution Notes

（留给开发窗口回填：偏离记录、实际行号、核对轮异议与裁断结果。）

**第 1 轮核对（2026-10-16）已回填的裁断**：

- **N1**：Task 1 由「改 `get_raw` 签名」改为「新增 `get_raw_kind` 旁路」（不违背 W2-N2）。本节全节已重写。
- **N2**：Task 5 新增 Step 3b（core `preview_env_change`），修正两处不存在的函数名。
- **N3**：`--json` 用 clap `requires = "dry_run"`，退出码 2。
- **N4**：path 轮换只作用于默认目录；Task 6 Step 5 注释写明「轮换失败 CLI 不可见」。
- **行号勘误**：`main.rs` `Add` 为 `:206-220`；`env_ops.rs` `resolve_value` 在 `:357`。
- **待回填**：issue #34 补「W2-B1 注释与实现不符」的发现。

**第 3 轮（Task 1/2 实施，2026-10-16，开发窗口实测）已回填的裁断**：

- **N6**：Task 1 Step 1 测试代码误写 `let hive`，故障注入需 `let mut hive`。**已就地修正**（见 Step 1 的 N6 注）。
- **实施确认（非缺陷）**：Task 2 测试的 `let mut v = "1".to_reg_value(); v.vtype = …` 是 `RegValue` 局部可变，**无需**给 `hive` 加 `mut`（`seed_raw` 方法内部 `&self`）。开发窗口已核实并保持原样。
- **补充：测试模块名** —— Task 2 的测试模块实为 `env_var_tests`（非通用 `tests`）。已补进 Task 2 的 Files 清单。
- **N5 落实位置**：开发窗口把「邻近改动禁区」注释落在 `read_env_var` 的**文档注释块**（plan 允许的位置之一），已确认落位。
- **实施状态**：Task 1 + Task 2 完成。审核窗口独立复验：`cargo test -p path-editor-core` = **214 passed / 0 failed**（+2 新用例）、`clippy --all-targets -D warnings` 零警告、`cargo fmt --check` 干净。改动仅 `core/src/reg_store.rs`（+96）、`core/src/registry/env_var.rs`（+71/-5）。
