# 环境变量备份与恢复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让每一次通用环境变量写入（CLI 与 GUI 共 6 个入口）在写注册表前自动产生可恢复的 JSON 备份，并提供 `env restore` 把备份还原回注册表，形成备份—恢复闭环。

**Architecture:** 备份采集与恢复编排全部落在 `core::backup`，复用 `core::persist` 已有的 `Versioned` 信封与 `.bak` 轮换原语；备份挂载点在 `core::registry::env_var` 的公开写函数内部，写函数返回值从 `Result<(), CoreError>` 改为 `Result<WriteOutcome, CoreError>` 以诚实携带备份结果；CLI 与 GUI 只做参数转换与错误呈现。

**Tech Stack:** Rust（core / gui / cli 三 crate workspace，edition 2021）、winreg 0.52、chrono 0.4、dirs 5、serde / serde_json、Tauri 2.11 IPC、React 19 + TypeScript strict、Vitest、Playwright。

**Spec:** `docs/superpowers/specs/2026-09-21-env-backup-restore-design.md`

## Global Constraints

- **分支与提交**：在 `.claude/worktrees/` 下的独立 worktree 工作；每任务结束提交一次，Conventional Commits（`feat` / `fix` / `refactor` / `docs` / `test` / `chore` / `perf` / `ci` / `style` / `revert`）；commit body 每行 ≤ 100 字符。**不推送、不升版本号、不建 tag、不出 release**（本波不出 5.1.4 发布）。
- **删除文件的硬约束**：全局规则「未经书面同意不得删除任何文件」。本计划中**唯一被授权的自动删除**是 §S1 限定的备份轮换，且必须通过 S1 的四条限界测试。
- **架构边界**：`gui` / `cli` 只做参数转换、命令分派、错误呈现；保护名单、`Unsupported` 类型、hive 写权限、名称与值合法性**全部由 core 判定**，CLI 侧零安全判定逻辑。组件与 Store 不得直接 `invoke`，统一走 `src/services/backend.ts`。`src/core/` 保持零 React / Tauri 依赖。
- **错误契约**：判定只认 `CoreError.code`（serde camelCase）；不得匹配 `message` 文本，不得匹配 `[E_CONFLICT]` 前缀。新增错误一律用 `CoreError::new(code, operation, message)`。
- **文档注释**：所有 `pub fn` / `pub(crate) fn` 必须有 `///` 文档注释（CONTRIBUTING.md 硬性要求）。
- **代码风格**：UTF-8 / CRLF；TS 2 空格、Rust 与 TOML 4 空格；Prettier 单引号、尾逗号、100 列。所有 `unsafe` 块必须有 `// SAFETY:` 注释。
- **注释语言**：代码注释一律中文。
- **测试命令**：CLI 单测必须用 `cargo test -p patheditor-cli --bins`（bin-only crate，`--lib` 会报 `no library targets found`）；前端单测 `npx vitest run tests/unit/<file>`；E2E `npm run test:e2e`（生产构建 + **mock IPC，禁止写入真实注册表**）。
- **质量门**：`npm run verify:all`（Prettier → ESLint → 构建 → 覆盖率 80% 行门槛 → `cargo fmt` → Clippy `-D warnings` → Rust 测试 → Playwright）。
- **真实注册表**：本计划**全程不写真实注册表**。所有 Rust 测试通过 `MemoryHive` 注入或临时目录隔离；端到端注册表闭环需用户单独授权，不在本计划内。
- **文档双副本**：`CLAUDE.md` 与 `AGENTS.md` 必须**字节级一致**，改动后须同步复制。
- **`persist` 的可见性边界（核对轮 P3）**：`core/src/lib.rs:8` 是 `pub(crate) mod persist;`，且 `Versioned<T>` / `PERSIST_SCHEMA_VERSION` 均为 `pub(crate)`。**`persist` 的信封类型只在 core crate 内使用**——gui / cli **不得**构造或解构 `Versioned`，只能经 `core::backup` 的公开 API（返回 `EnvBackupPayload` / `RestorePreview` 等已解开的类型）交互。
- **模块路径写法（核对轮 E1/E4，已实证）**：`core/src/registry/env_var.rs` 所在模块是**私有**的 `mod env_var;`（`core/src/registry.rs:14`），因此**不得**写 `crate::registry::env_var::X` —— 会报 `error[E0603]: module env_var is private`。core 内部跨模块引用一律经 `crate::registry::X`（根 re-export），需要暴露新符号时在 `registry.rs` 加 `pub(crate) use`（该文件 :30 已有 `hive_location` 先例）。
- **每任务结束**：`cargo fmt` + `cargo clippy --workspace --all-targets -- -D warnings` 零警告。

---

## 文件结构

| 文件                                         | 职责                                                                                                                   |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `core/src/backup.rs`                         | 修改：在既有 PATH 备份旁新增 env 备份的采集、写文件、轮换、列表、差异计算、恢复（全部实现集中于此，PATH 备份代码不动） |
| `core/src/registry/env_var.rs`               | 修改：6 个公开写函数入口触发备份，返回值改为`WriteOutcome`                                                             |
| `core/src/registry.rs`                       | 修改：re-export 新类型                                                                                                 |
| `core/src/lib.rs`                            | 修改：导出`WriteOutcome` / `BackupOutcome`                                                                             |
| `cli/src/main.rs`                            | 修改：`EnvCmd` 新增 `Backup` / `Restore` / `Backups` 三个子命令                                                        |
| `cli/src/env_ops.rs`                         | 修改：三个子命令的处理函数                                                                                             |
| `cli/src/runtime.rs`                         | 修改：`apply_core_result` 适配 `WriteOutcome`                                                                          |
| `gui/src/commands/env_var.rs`                | 修改：返回类型适配`WriteOutcome`                                                                                       |
| `gui/src/commands/backup.rs`                 | 修改：新增 4 个 Tauri 命令                                                                                             |
| `gui/src/lib.rs`                             | 修改：注册新命令                                                                                                       |
| `src/services/backend.ts`                    | 修改：4 个前端方法 + 运行时形状校验                                                                                    |
| `src/core/env-backup.ts`                     | 创建：备份列表与差异摘要的纯展示逻辑                                                                                   |
| `src/components/dialogs/EnvBackupDialog.tsx` | 创建：备份与恢复对话框                                                                                                 |

---

### Task 1: 备份数据模型与采集

**Files:**

- Modify: `core/src/backup.rs`
- Test: `core/src/backup.rs`（同文件 `#[cfg(test)] mod tests`）

**Interfaces:**

- Consumes: `crate::reg_store::{EnvHiveStore, WinregHive}`、`crate::env_var::{EnvHive, EnvValueKind, is_reserved, revision_of}`、`crate::error::{CoreError, ErrorCode}`、**`crate::registry::hive_location`**（经 registry 根 re-export——`registry::env_var` 是私有模块，不可直接引用；见 Global Constraints）
- **不消费**：`read_env_var`（B1 裁断后采集改走 `store.get_raw`，不再需要它）
- Produces:
  - `pub struct EnvBackupVar { pub name: String, pub kind: EnvValueKind, pub value: String, pub revision: String }`
  - `pub struct EnvBackupPayload { pub captured_at: i64, pub hives: EnvBackupHives }`
  - `pub struct EnvBackupHives { pub system: Vec<EnvBackupVar>, pub user: Vec<EnvBackupVar> }`
  - `fn collect_hive_vars_in_store(store: &dyn EnvHiveStore, hive: EnvHive) -> Result<Vec<EnvBackupVar>, CoreError>`
  - `pub fn collect_env_backup() -> Result<EnvBackupPayload, CoreError>`

- [ ] **Step 1: 写失败测试**

在 `core/src/backup.rs` 的 `mod tests` 中追加（`use crate::reg_store::MemoryHive;` 与 `use winreg::enums::{REG_SZ, REG_EXPAND_SZ, REG_DWORD};` 加到测试模块头部）：

> **B1 修正（核对轮第二轮，已实证）**：采集**不得**用 `read_env_var`——它在 `core/src/registry/env_var.rs:29-35` 对 `Unsupported` 类型**直接返回 `Err`**，`?` 传播会让**整次备份失败**（任一 hive 有一个 `REG_DWORD` 就中招）。改用 `store.get_raw` + `from_reg_type` + `is_writable()` 跳过，与 `list_env_vars_in_store`（`env_var.rs:151-166`）**完全同形**。
>
> 因此测试必须**先断言 `Ok`**，再断言内容——只断言「列表里没有 SomeDword」的写法无法区分「跳过」与「报错」两种行为，是坏测试。

```rust
    /// 采集必须排除保留名（Path）与 Unsupported 类型，并携带明文与 revision。
    ///
    /// B1：**先断言 Ok** —— 遇到 Unsupported 必须跳过该变量而非让整次采集失败。
    #[test]
    fn collect_hive_vars_excludes_reserved_and_unsupported() {
        let hive = MemoryHive::new(true);
        hive.seed("JAVA_HOME", "C:\\jdk17", REG_SZ);
        hive.seed("API_TOKEN", "secret-value", REG_SZ);
        hive.seed("Path", "C:\\Windows", REG_EXPAND_SZ); // 保留名，须排除
        hive.seed("SomeDword", "1", REG_DWORD); // Unsupported，须跳过（不是报错）

        // 关键：必须 Ok。若实现走 read_env_var，这里会 Err 而失败。
        let vars = collect_hive_vars_in_store(&hive, EnvHive::User)
            .expect("遇到 Unsupported 类型必须跳过该变量，而不是整次采集失败");

        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"JAVA_HOME"), "普通变量必须被采集");
        assert!(names.contains(&"API_TOKEN"), "敏感变量也必须采集（备份要能还原）");
        assert!(!names.contains(&"Path"), "保留名必须排除");
        assert!(!names.contains(&"SomeDword"), "Unsupported 类型必须被跳过");

        let java = vars.iter().find(|v| v.name == "JAVA_HOME").unwrap();
        assert_eq!(java.value, "C:\\jdk17", "备份必须携带明文值");
        assert_eq!(java.kind, EnvValueKind::String);
        assert_eq!(java.revision.len(), 16, "revision 是 16 位十六进制摘要");
    }

    /// B1 回归：Unsupported 只在被跳过时出现，**不得**使同一 hive 的其他变量丢失。
    #[test]
    fn collect_hive_vars_keeps_others_when_unsupported_present() {
        let hive = MemoryHive::new(true);
        hive.seed("A_DWORD", "1", REG_DWORD);
        hive.seed("B_NORMAL", "b", REG_SZ);
        hive.seed("C_DWORD", "2", REG_DWORD);
        hive.seed("D_NORMAL", "d", REG_EXPAND_SZ);

        let vars = collect_hive_vars_in_store(&hive, EnvHive::User).expect("必须跳过而非失败");
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();

        assert_eq!(vars.len(), 2, "两个 Unsupported 被跳过，两个正常变量保留");
        assert!(names.contains(&"B_NORMAL"));
        assert!(names.contains(&"D_NORMAL"));
    }

    /// 采集的 revision 必须与 core 的 revision_of 同口径（恢复时据此判冲突）。
    #[test]
    fn collect_hive_vars_revision_matches_revision_of() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "value-1", REG_SZ);

        let vars = collect_hive_vars_in_store(&hive, EnvHive::User).expect("采集失败");
        let expected = revision_of("MY_VAR", REG_SZ, "value-1");

        assert_eq!(vars[0].revision, expected);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path-editor-core collect_hive_vars`
Expected: 编译失败 —— `cannot find function collect_hive_vars_in_store`、`cannot find type EnvBackupVar`。

- [ ] **Step 3: 写最小实现**

在 `core/src/backup.rs` 的 `use` 区补（**注意路径写法**：`registry::env_var` 是私有模块，必须经 `registry` 根的 re-export；见 Global Constraints 的模块路径条目）：

```rust
use crate::env_var::{is_reserved, revision_of, EnvHive, EnvValueKind};
use crate::error::{CoreError, ErrorCode};
use crate::reg_store::{EnvHiveStore, WinregHive};
// 经 registry 根 re-export —— 不能写 crate::registry::env_var::X（E0603）
use crate::registry::hive_location;
use serde::{Deserialize, Serialize};
```

> **B1 相关**：本任务**不需要** `read_env_var`（采集改走 `store.get_raw`），因此**不要**为它加 import 或改它的可见性。`String::from_reg_value` 与 `EnvValueKind::from_reg_type` 是本任务用到的解码路径。
>
> **需要一并做的改动**：`hive_location` 已在 `core/src/registry.rs:30` 的 `pub(crate) use` 组中，可直接经 `crate::registry::hive_location` 引用，**无需**新增 re-export。

在 `backup_registry` 之前插入：

```rust
/// 备份中的单个环境变量条目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvBackupVar {
    /// 注册表返回的原始大小写变量名
    pub name: String,
    /// 注册表类型；备份中只出现 String / ExpandString
    pub kind: EnvValueKind,
    /// 完整明文值（设计文档 D2：备份的价值在于能还原，故不打码）
    pub value: String,
    /// 采集时的并发校验摘要（FNV-1a 64，16 位十六进制）
    pub revision: String,
}

/// 按 hive 分组的备份内容。
///
/// 加 `rename_all = "camelCase"` 与 `EnvVarSnapshot`（`core/src/env_var.rs:113`）
/// 及本文件的 `EnvBackupPayload` 保持风格一致；`system` / `user` 两个单字字段
/// 在此宏下不变形，无功能影响（核对轮一致性观察）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvBackupHives {
    #[serde(default)]
    pub system: Vec<EnvBackupVar>,
    #[serde(default)]
    pub user: Vec<EnvBackupVar>,
}

/// env 备份文件的载荷（由 `persist::Versioned` 信封包裹后落盘）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvBackupPayload {
    /// 采集时刻（Unix 毫秒），口径同 `EnvVarSnapshot.capturedAt`
    pub captured_at: i64,
    pub hives: EnvBackupHives,
}

/// 采集单个 hive 中全部可恢复的环境变量。
///
/// 排除两类变量：保留名（`Path` 由专用通路管理）与 `Unsupported` 类型
/// （本就不可写，备份了也无法通过通用通路恢复）。
///
/// **B1：`Unsupported` 必须「跳过该变量」，不得让整次采集失败。** 因此本函数
/// 用 `store.get_raw` + `from_reg_type` 自己判类型，而**不能**用 `read_env_var`
/// ——后者（`core/src/registry/env_var.rs:29-35`）对 `Unsupported` 直接返回
/// `Err`，配上调用方的 `?` 会让「机器上存在一个 `REG_DWORD` 变量」变成
/// 「每次写前备份都失败」。写法与 `list_env_vars_in_store`
/// （`core/src/registry/env_var.rs:151-166`）保持一致。
///
/// # Returns
/// - `Ok(Vec<EnvBackupVar>)` — 该 hive 的可恢复变量，含明文与 revision
/// - `Err(CoreError)` — 枚举失败（`Io`）或读取/解码失败（`Io`/`Parse`）；**不含** `UnsupportedType`
pub(crate) fn collect_hive_vars_in_store(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
) -> Result<Vec<EnvBackupVar>, CoreError> {
    let (_, _, label) = hive_location(hive);
    let with_hive = |code: ErrorCode, msg: String| {
        let mut e = CoreError::new(code, "collect_env_backup", msg);
        e.hive = Some(hive);
        e
    };

    let names = store.enum_names().map_err(|e| {
        with_hive(ErrorCode::Io, format!("读取{}环境变量列表失败: {}", label, e))
    })?;

    let mut vars = Vec::new();
    for name in names {
        if is_reserved(&name) {
            continue;
        }
        let raw = store.get_raw(&name).map_err(|e| {
            with_hive(
                ErrorCode::Io,
                format!("读取{}环境变量 {} 失败: {}", label, name, e),
            )
            .with_target(hive, &name)
        })?;
        let kind = EnvValueKind::from_reg_type(raw.vtype.clone());
        if !kind.is_writable() {
            continue; // Unsupported：跳过该变量，**不**使整次采集失败（B1）
        }
        // 先算 revision 再移动 raw.vtype；`RegType` 非 Copy（winreg 0.52）。
        let value = String::from_reg_value(&raw).map_err(|e| {
            with_hive(
                ErrorCode::Parse,
                format!("无法解码{}环境变量 {}: {}", label, name, e),
            )
            .with_target(hive, &name)
        })?;
        vars.push(EnvBackupVar {
            revision: revision_of(&name, raw.vtype, &value),
            name,
            kind,
            value,
        });
    }
    Ok(vars)
}

/// 采集两个 hive 的完整备份载荷。
///
/// # Returns
/// - `Ok(EnvBackupPayload)` — system 与 user 两个 hive 的可恢复变量
/// - `Err(CoreError)` — 任一 hive 采集失败即整体失败（沿用 F-04 的 fail-fast：不产出「成功但不完整」的备份）
pub fn collect_env_backup() -> Result<EnvBackupPayload, CoreError> {
    let sys_store = WinregHive::open(EnvHive::System, false)?;
    let usr_store = WinregHive::open(EnvHive::User, false)?;
    let system = collect_hive_vars_in_store(&sys_store, EnvHive::System)?;
    let user = collect_hive_vars_in_store(&usr_store, EnvHive::User)?;
    Ok(EnvBackupPayload {
        captured_at: chrono::Local::now().timestamp_millis(),
        hives: EnvBackupHives { system, user },
    })
}
```

同时在 `core/src/lib.rs` 确认 `pub mod reg_store;` 已导出（若未导出需补）。`hive_location` 已在 `core/src/registry.rs:30` 的 `pub(crate) use` 组中，无需改动。**不要**在 backup.rs 里写 `crate::registry::env_var::hive_location`——模块私有，会报 E0603。**本任务不需要 `read_env_var`**（B1 裁断后采集走 `store.get_raw`），不要为它加 re-export。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p path-editor-core collect_hive_vars`
Expected: 3 passed（excludes / keeps_others / revision_matches）。

- [ ] **Step 5: 提交**

```bash
git add core/src/backup.rs core/src/lib.rs core/src/registry.rs
git commit -m "feat(core): 环境变量备份数据模型与采集实现"
```

---

### Task 2: 备份落盘、轮换与保留策略

**Files:**

- Modify: `core/src/backup.rs`
- Test: `core/src/backup.rs`

**Interfaces:**

- Consumes: Task 1 的 `EnvBackupPayload`、`collect_env_backup`；`crate::persist::{Versioned, PERSIST_SCHEMA_VERSION, rotate_backup}`
- Produces:
  - `pub const ENV_BACKUP_KEEP: usize = 20`
  - `fn config_file_path() -> PathBuf`
  - `fn read_env_backup_keep(path: &Path) -> usize`（任何异常回落默认值）
  - `fn env_backup_keep() -> usize`
  - `pub fn env_backup_dir() -> PathBuf`
  - `pub fn write_env_backup_to(dir: &Path, payload: &EnvBackupPayload) -> Result<PathBuf, CoreError>`
  - `pub fn backup_env_vars() -> Result<PathBuf, CoreError>`
  - `fn rotate_env_backups(dir: &Path, keep: usize) -> Result<Vec<PathBuf>, CoreError>`

- [ ] **Step 1: 写失败测试**

```rust
    /// 备份目录可用环境变量重定向，供测试隔离；未设置时回落 ~/.patheditor/backups。
    fn with_temp_backup_dir<F: FnOnce(&std::path::Path)>(f: F) {
        // 环境变量是进程级的，与其他触碰它的测试互斥
        let _guard = crate::persist::test_persist_lock();
        let dir = std::env::temp_dir().join(format!(
            "patheditor_test_env_backup_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("PATHEDITOR_BACKUP_DIR", &dir);
        f(&dir);
        std::env::remove_var("PATHEDITOR_BACKUP_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn sample_payload() -> EnvBackupPayload {
        EnvBackupPayload {
            captured_at: 1_758_400_000_000,
            hives: EnvBackupHives {
                system: vec![],
                user: vec![EnvBackupVar {
                    name: "JAVA_HOME".into(),
                    kind: EnvValueKind::String,
                    value: "C:\\jdk17".into(),
                    revision: "a1b2c3d4e5f60718".into(),
                }],
            },
        }
    }

    /// 落盘文件必须带 schemaVersion 信封，且能被读回（往返一致）。
    #[test]
    fn write_env_backup_round_trips_through_versioned_envelope() {
        with_temp_backup_dir(|dir| {
            let path = write_env_backup_to(dir, &sample_payload()).expect("写备份失败");
            assert!(path.exists());
            assert!(
                path.file_name().unwrap().to_string_lossy().starts_with("env_backup_"),
                "文件名必须以 env_backup_ 开头"
            );
            assert_eq!(path.extension().unwrap(), "json");

            let content = std::fs::read_to_string(&path).unwrap();
            assert!(
                content.contains("\"schemaVersion\""),
                "必须带 schemaVersion 信封（复用 persist::Versioned）"
            );
            assert!(content.contains("JAVA_HOME"));

            let back: Versioned<EnvBackupPayload> = serde_json::from_str(&content).unwrap();
            assert_eq!(back.inner, sample_payload());
        });
    }

    /// S1 对抗性测试：轮换只删自己生成的 env_backup_*.json，其他文件一律不动。
    #[test]
    fn rotate_env_backups_never_deletes_foreign_files() {
        with_temp_backup_dir(|dir| {
            // 25 份正常备份（超过保留数 20）
            for i in 0..25 {
                let name = format!("env_backup_2026010{}_120000_000.json", (i % 9) + 1);
                std::fs::write(dir.join(&name), format!("{{\"n\":{i}}}")).unwrap();
            }
            // 干扰文件：PATH 备份、.bak、损坏隔离、用户自放文件
            std::fs::write(dir.join("path_backup_20260920_152446_043.txt"), "PATH").unwrap();
            std::fs::write(dir.join("env_backup_old.json.bak"), "BAK").unwrap();
            std::fs::write(dir.join("disabled.json.corrupt-20260920-120000000"), "C").unwrap();
            std::fs::write(dir.join("我的笔记.txt"), "note").unwrap();
            std::fs::write(dir.join("env_backup_note.md"), "md").unwrap();

            rotate_env_backups(dir, ENV_BACKUP_KEEP).expect("轮换失败");

            assert!(dir.join("path_backup_20260920_152446_043.txt").exists(), "PATH 备份不得删除");
            assert!(dir.join("env_backup_old.json.bak").exists(), ".bak 不得删除");
            assert!(
                dir.join("disabled.json.corrupt-20260920-120000000").exists(),
                "损坏隔离文件不得删除"
            );
            assert!(dir.join("我的笔记.txt").exists(), "无关文件不得删除");
            assert!(dir.join("env_backup_note.md").exists(), "非 .json 的同前缀文件不得删除");
        });
    }

    /// 轮换后 env_backup_*.json 恰好保留 keep 份。
    #[test]
    fn rotate_env_backups_keeps_exactly_keep_files() {
        with_temp_backup_dir(|dir| {
            for i in 0..25 {
                std::fs::write(
                    dir.join(format!("env_backup_202601{:02}_120000_000.json", i)),
                    "{}",
                )
                .unwrap();
            }
            rotate_env_backups(dir, ENV_BACKUP_KEEP).expect("轮换失败");

            let remaining = std::fs::read_dir(dir)
                .unwrap()
                .flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.starts_with("env_backup_") && n.ends_with(".json")
                })
                .count();
            assert_eq!(remaining, ENV_BACKUP_KEEP);
        });
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path-editor-core rotate_env_backups`
Expected: 编译失败 —— `cannot find function rotate_env_backups`、`cannot find value ENV_BACKUP_KEEP`。

- [ ] **Step 2b: 先写 config.ini 的失败测试**

`config.ini` 的保留数覆盖（设计文档 §配置文件，核对轮方案 A）。在 `mod tests` 追加：

```rust
    /// 配置文件缺失 → 回落默认值，且**不创建文件**。
    #[test]
    fn config_missing_returns_default_and_creates_nothing() {
        let _guard = crate::persist::test_persist_lock();
        let dir = std::env::temp_dir().join(format!("patheditor_cfg_missing_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("config.ini");

        assert_eq!(read_env_backup_keep(&cfg), ENV_BACKUP_KEEP);
        assert!(!cfg.exists(), "读配置不得有副作用");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 四种回落情形（键缺失 / 非整数 / 负数 / 空值）都返回默认值而不是报错。
    #[test]
    fn config_invalid_values_fall_back_to_default() {
        let dir = std::env::temp_dir().join(format!("patheditor_cfg_invalid_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("config.ini");

        for body in [
            "; 只有注释\n",                       // 键缺失
            "env_backup_keep = abc\n",            // 非整数
            "env_backup_keep = -5\n",             // 负数
            "env_backup_keep =\n",                // 空值
            "other_key = 1\n",                    // 未知键
        ] {
            std::fs::write(&cfg, body).unwrap();
            assert_eq!(read_env_backup_keep(&cfg), ENV_BACKUP_KEEP, "回落失败: {body:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 合法值被采纳；`;` 与 `#` 注释、行内两侧空格都能正确解析。
    #[test]
    fn config_reads_valid_value_with_comments_and_spaces() {
        let dir = std::env::temp_dir().join(format!("patheditor_cfg_valid_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("config.ini");

        std::fs::write(&cfg, "; 注释行\n# 另一种注释\nenv_backup_keep   =   7   \n").unwrap();
        assert_eq!(read_env_backup_keep(&cfg), 7);

        let _ = std::fs::remove_dir_all(&dir);
    }
```

Run: `cargo test -p path-editor-core config_`
Expected: 编译失败 —— `cannot find function read_env_backup_keep`。

- [ ] **Step 3: 写最小实现**

在 `core/src/backup.rs` 的 `backup_base_dir` 上方插入常量，并修改 `backup_base_dir` 支持重定向：

```rust
/// env 备份默认保留份数（设计文档 D6）。
///
/// 全量快照单文件约几十 KB；20 份在正常使用下是几百 KB 量级。
/// 写操作密集的用户可调大——注意 README 已提示备份含明文敏感值，
/// 保留份数越大暴露面越大。
pub const ENV_BACKUP_KEEP: usize = 20;

/// 配置文件路径：`~/.patheditor/config.ini`。
///
/// 放用户目录而非 exe 同目录（设计文档 §配置文件）：CLI/GUI 是两份独立 exe，
/// scoop 升级换目录、NSIS 装 Program Files 需提权——只有用户目录能让双 exe
/// 共享、升级不丢、免提权。
fn config_file_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("config.ini")
}

/// 读取 env 备份保留份数；任何异常一律回落 [`ENV_BACKUP_KEEP`]。
///
/// 回落情形（设计文档 §解析与回落行为）：文件不存在 / 键不存在 / 值非整数 /
/// 值为负数 / 值空 / 文件不可读。**不报错、不中止备份**，非默认值时记一次 warn。
/// 读取**无副作用**：文件不存在时不创建。
///
/// 手写极简 INI 解析（`key = value`，`;` 或 `#` 起始为注释），不引入新依赖——
/// 单键配置不值得拉一个 crate，与项目「手写 FNV-1a 而不引 sha2」的先例一致。
fn read_env_backup_keep(path: &Path) -> usize {
    let fallback = ENV_BACKUP_KEEP;
    let Ok(content) = std::fs::read_to_string(path) else {
        return fallback; // 不存在或不可读，无副作用地回落
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.trim() != "env_backup_keep" {
            continue;
        }
        match value.trim().parse::<usize>() {
            Ok(n) => return n,
            Err(_) => {
                log::warn!(
                    "config.ini 的 env_backup_keep 值非法（{}），回落默认 {}",
                    value.trim(),
                    fallback
                );
                return fallback;
            }
        }
    }
    fallback
}

/// 从真实配置文件读取保留份数。
fn env_backup_keep() -> usize {
    read_env_backup_keep(&config_file_path())
}

/// 备份根目录。`PATHEDITOR_BACKUP_DIR` 存在时用它（测试隔离用），
/// 否则回落 `~/.patheditor/backups/`。
fn backup_base_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("PATHEDITOR_BACKUP_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".patheditor")
        .join("backups")
}

/// env 备份目录的公开访问器。
pub fn env_backup_dir() -> PathBuf {
    backup_base_dir()
}
```

在 `collect_env_backup` 之后插入：

```rust
/// 把备份载荷写入目录，并在写入后执行保留策略轮换。
///
/// 文件名为 `env_backup_<YYYYMMDD>_<HHMMSS>_<毫秒3位>.json`，时间戳格式与
/// PATH 备份一致，便于用户在同一目录中找到全部备份。
///
/// # Returns
/// - `Ok(PathBuf)` — 写入的备份文件绝对路径
/// - `Err(CoreError)` — 目录创建、轮换或写文件失败（code=`Io`）
pub fn write_env_backup_to(dir: &Path, payload: &EnvBackupPayload) -> Result<PathBuf, CoreError> {
    std::fs::create_dir_all(dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "backup_env_vars",
            format!("无法创建备份目录 {}: {}", dir.display(), e),
        )
    })?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%3f");
    let filepath = dir.join(format!("env_backup_{}.json", timestamp));

    let versioned = Versioned {
        schema_version: PERSIST_SCHEMA_VERSION,
        inner: payload.clone(),
    };
    let json = serde_json::to_string_pretty(&versioned).map_err(|e| {
        CoreError::new(
            ErrorCode::Internal,
            "backup_env_vars",
            format!("序列化备份失败: {e}"),
        )
    })?;

    // 轮换在写新文件之前执行：新文件必然保留，不会被自己轮换掉。
    // 保留份数来自 config.ini（缺省 ENV_BACKUP_KEEP），每次读一次不缓存。
    rotate_env_backups(dir, env_backup_keep())?;

    std::fs::write(&filepath, json).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "backup_env_vars",
            format!("无法写入备份文件 {}: {}", filepath.display(), e),
        )
    })?;

    log::info!("env 备份已保存到: {}", filepath.display());
    Ok(filepath)
}

/// 保留策略轮换：只删除**本功能生成**的旧备份，保留最近 `keep` 份。
///
/// 授权范围（设计文档 §S1，用户 2026-09-21 明确授权的例外）：
/// 1. 只匹配文件名 `env_backup_*.json`；
/// 2. 只在给定目录内操作（自定义备份目录不参与轮换）；
/// 3. 只删最旧的、超出 `keep` 之外的文件；
/// 4. 绝不删除 `.txt` PATH 备份、`.bak`、`.corrupt-*` 及目录中任何其他文件。
///
/// # Returns
/// - `Ok(Vec<PathBuf>)` — 被删除的文件路径（供测试与日志核对）
/// - `Err(CoreError)` — 目录枚举失败（code=`Io`）；单个文件删除失败只记 warn，不影响其他
fn rotate_env_backups(dir: &Path, keep: usize) -> Result<Vec<PathBuf>, CoreError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    // 只收集本功能生成的备份：前缀 env_backup_ + 后缀 .json，
    // `.bak` / `.corrupt-*` 因后缀不同天然被排除。
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "rotate_env_backups",
            format!("枚举备份目录 {} 失败: {}", dir.display(), e),
        )
    })?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("env_backup_") || !name.ends_with(".json") {
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

    // 文件名内嵌时间戳，字典序即时间序；降序排列后丢弃最旧的。
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    let mut removed = Vec::new();
    for (_, path) in candidates.into_iter().skip(keep) {
        match std::fs::remove_file(&path) {
            Ok(()) => {
                log::info!("已轮换删除旧备份: {}", path.display());
                removed.push(path);
            }
            // 删除失败不中止轮换：留着旧文件无害，报错反而阻断后续写入。
            Err(e) => log::warn!("删除旧备份 {} 失败: {}", path.display(), e),
        }
    }
    Ok(removed)
}

/// 采集两个 hive 并落盘一份 env 备份（公开入口，供 CLI `env backup` 与写前自动备份使用）。
///
/// # Returns
/// - `Ok(PathBuf)` — 备份文件绝对路径
/// - `Err(CoreError)` — 采集或落盘失败
pub fn backup_env_vars() -> Result<PathBuf, CoreError> {
    let payload = collect_env_backup()?;
    let dir = env_backup_dir();
    write_env_backup_to(&dir, &payload)
}
```

在文件顶部 `use` 区补：

```rust
use crate::persist::{Versioned, PERSIST_SCHEMA_VERSION};
use std::path::Path;
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p path-editor-core env_backup` 与 `cargo test -p path-editor-core config_`
Expected: 3 + 3 passed（round_trip / never_deletes_foreign / keeps_exactly_keep；config 的 missing / invalid / valid）。

- [ ] **Step 5: 提交**

```bash
git add core/src/backup.rs
git commit -m "feat(core): env 备份落盘、Versioned 信封与保留策略轮换"
```

---

### Task 3: 挂载到 6 个写入口（WriteOutcome 改造）

**Files:**

- Modify: `core/src/registry/env_var.rs`、`core/src/registry.rs`、`core/src/lib.rs`
- Modify: `cli/src/env_ops.rs`、`cli/src/runtime.rs`、`gui/src/commands/env_var.rs`
- Test: `core/src/registry/env_var.rs`

**Interfaces:**

- Consumes: Task 2 的 `backup_env_vars()`
- Produces:
  - `pub enum BackupOutcome { Created(PathBuf), Skipped, Failed(String) }`
  - `pub struct WriteOutcome { pub backup: BackupOutcome }`
  - 6 个写函数返回值由 `Result<(), CoreError>` 改为 `Result<WriteOutcome, CoreError>`：
    `update_env_var`、`create_env_var`、`delete_env_var`、`update_env_var_force`、`delete_env_var_force`

> **注**：6 个「写入口」指 CLI 3 个命令 ×（revision/force 两条路径）与 GUI 3 个命令，其映射到 5 个 core 函数（`create_env_var` 无 force 变体）。

- [ ] **Step 1: 写失败测试**

在 `core/src/registry/env_var.rs` 的 `mod tests` 中追加：

```rust
    /// 写成功后必须返回备份结果（Created 或 Failed），不得静默丢弃。
    #[test]
    fn update_env_var_reports_backup_outcome() {
        let _guard = crate::persist::test_persist_lock();
        let dir = std::env::temp_dir().join(format!("patheditor_t3_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("PATHEDITOR_BACKUP_DIR", &dir);

        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let revision = revision_of("MY_VAR", vtype, &value);

        let outcome = update_env_var_in_store(&hive, EnvHive::User, "MY_VAR", "new", &revision)
            .expect("写入必须成功");

        // 备份是 best-effort：本测试在真实目录写入，成功则 Created，失败则 Failed。
        // 两者都证明结果被诚实返回，而非被吞掉。
        assert!(
            matches!(
                outcome.backup,
                BackupOutcome::Created(_) | BackupOutcome::Skipped | BackupOutcome::Failed(_)
            ),
            "备份结果必须被返回"
        );
        assert_eq!(
            String::from_reg_value(&hive.get_raw("MY_VAR").unwrap()).unwrap(),
            "new",
            "备份结果不影响写入本身"
        );

        std::env::remove_var("PATHEDITOR_BACKUP_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 备份失败（目录不可写）时写入仍必须成功（K2：不阻断）。
    #[test]
    fn update_env_var_succeeds_even_when_backup_fails() {
        let _guard = crate::persist::test_persist_lock();
        // 指向一个不可能创建目录的路径：Windows 上以文件占位再当目录用
        let blocker = std::env::temp_dir().join(format!("patheditor_t3_block_{}", std::process::id()));
        let _ = std::fs::remove_file(&blocker);
        std::fs::write(&blocker, b"x").unwrap();
        std::env::set_var("PATHEDITOR_BACKUP_DIR", blocker.join("sub"));

        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "original", REG_SZ);
        let (vtype, value) = read_env_var(&hive, "MY_VAR").expect("读取失败");
        let revision = revision_of("MY_VAR", vtype, &value);

        let outcome = update_env_var_in_store(&hive, EnvHive::User, "MY_VAR", "new", &revision)
            .expect("备份失败不得使写入失败");
        assert!(
            matches!(outcome.backup, BackupOutcome::Failed(_)),
            "备份失败必须被如实报告为 Failed"
        );
        assert_eq!(
            String::from_reg_value(&hive.get_raw("MY_VAR").unwrap()).unwrap(),
            "new"
        );

        std::env::remove_var("PATHEDITOR_BACKUP_DIR");
        let _ = std::fs::remove_file(&blocker);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path-editor-core reports_backup_outcome`
Expected: 编译失败 —— `cannot find type BackupOutcome`、`no field backup on type ()`。

- [ ] **Step 3: 写最小实现**

在 `core/src/backup.rs` 追加：

```rust
/// 一次写操作前的备份结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupOutcome {
    /// 备份已写入，携带文件路径
    Created(PathBuf),
    /// 本次操作无需备份
    Skipped,
    /// 备份失败（不阻断写入），携带原因
    Failed(String),
}
```

在 `core/src/registry/env_var.rs` 顶部补 `use crate::backup::{backup_env_vars, BackupOutcome};`，并在该文件内新增：

```rust
/// 一次环境变量写操作的结果。
///
/// 写入本身的成败仍由外层 `Result` 表达；本结构只承载**备份**这一附带结果——
/// 备份失败不使写入失败（设计文档 K2），但必须被调用方看见。
#[derive(Debug, Clone, PartialEq)]
pub struct WriteOutcome {
    /// 写前备份的结果
    pub backup: BackupOutcome,
}

/// 写前尽力备份：失败只记警告并如实返回，绝不阻断写入。
fn backup_before_write() -> BackupOutcome {
    match backup_env_vars() {
        Ok(path) => BackupOutcome::Created(path),
        Err(e) => {
            log::warn!("环境变量写前备份失败（写入继续）: {}", e.message);
            BackupOutcome::Failed(e.message)
        }
    }
}
```

**J3 裁断（核对轮第二轮，采纳开发窗口推荐的「甲」方案）：不抽 `validate_write`，只抽备份**

> 计划此前给的 `validate_write(store, hive)` 骨架**是一个不存在的函数，且抽象不成立**。开发窗口逐入口数过校验项，5 个写函数互不相同：
>
> | 写函数                 | 名称 | reserved | protected | revision |  类型可写  | 值  | 同名 |
> | ---------------------- | :--: | :------: | :-------: | :------: | :--------: | :-: | :--: |
> | `update_env_var`       |  ✅  |    ✅    |    ✅     |    ✅    |     ✅     | ✅  |  —   |
> | `create_env_var`       |  ✅  |    ✅    |    ✅     |    —     |     ✅     | ✅  |  ✅  |
> | `delete_env_var`       |  ✅  |    ✅    |    ✅     |    ✅    | ✅（代劳） |  —  |  —   |
> | `update_env_var_force` |  ✅  |    ✅    |    ✅     |    —     |     ✅     | ✅  |  —   |
> | `delete_env_var_force` |  ✅  |    ✅    |    ✅     |    —     | ✅（代劳） |  —  |  —   |
>
> 单参数签名表达不了这张表，强行抽象会退化成一个大 `match`。**采纳「甲」**：接受校验在公开包装与 `*_in_store` 各写一份——`*_in_store` 的校验是**防御性兜底**（真实写入路径必经），判定源仍是 `is_reserved` / `is_protected` / `EnvValueKind::is_writable` 这三个 core 函数，**不构成判定源分裂**。这是本设计里唯一接受的「双份规则」，理由是「先校验后备份」的顺序要求优先。

**统一形态**：5 个公开包装 =「校验（内联，复用 `*_in_store` 里的判定）→ `backup_before_write()` → 调 `*_in_store`」。以 `update_env_var` 为例（`prepare_and_backup` 作废）：

```rust
/// 写入已有变量。类型从注册表读取，不由前端决定。
///
/// 写入前尽力备份当前全部环境变量（设计文档 K2：备份失败不阻断写入，
/// 结果经 [`WriteOutcome::backup`] 如实返回）。
///
/// # Returns
/// - `Ok(WriteOutcome)` — 写入成功；`backup` 字段说明备份结果
/// - `Err(CoreError)` — 校验失败、修订冲突或类型不受支持（此时**不发生写入，也不发生备份**）
pub fn update_env_var(
    hive: EnvHive,
    name: &str,
    value: &str,
    expected_revision: &str,
) -> Result<WriteOutcome, CoreError> {
    let store = WinregHive::open(hive, true)?;
    // 1) 校验（内联，与 *_in_store 中的判定同源）：失败时不产生无意义的备份文件
    validate_env_name(name).map_err(|m| {
        CoreError::new(ErrorCode::InvalidName, "update_env_var", m).with_target(hive, name)
    })?;
    if is_reserved(name) { /* 返回 ReservedName ... */ }
    if is_protected(name) { /* 返回 Protected ... */ }
    // 2) 校验通过 → 写前备份（失败不阻断）
    let backup = backup_before_write();
    // 3) 实际写入（*_in_store 内部会再判一次 —— 防御性兜底，见 J3 裁断）
    update_env_var_in_store(&store, hive, name, value, expected_revision)?;
    crate::system::broadcast_env_change();
    Ok(WriteOutcome { backup })
}
```

> **校验内联到什么程度**：至少覆盖「保留名 / 保护名单」这两项**便宜且无副作用**的判定；revision 与类型校验需要读注册表，可在 `*_in_store` 内完成——此时备份会先于这些失败发生，属可接受代价（备份文件无害）。**硬性要求只有一条：`backup_before_write()` 必须在所有「便宜校验」之后、`*_in_store` 调用之前。**

备份辅助 `backup_before_write()` 的定义见本任务 Step 3 开头（无 store / hive 参数，只有「尽力备份并如实返回」一件事）。

对 `create_env_var` / `delete_env_var` / `update_env_var_force` / `delete_env_var_force` 做同样改造，返回 `Result<WriteOutcome, CoreError>`。各 `*_in_store` 私有函数**保持不变**（仍返回 `Result<(), CoreError>`），只有公开包装函数改签名。

`core/src/registry.rs` 的 re-export 补 `WriteOutcome`（写函数是公开 API，走 `pub use`）；`core/src/lib.rs` 补 `pub use backup::BackupOutcome;` 与 `pub use registry::WriteOutcome;`。

CLI 侧 `cli/src/runtime.rs` 的 `apply_core_result` 改为泛型：

```rust
/// 统一处理写操作结果：冲突 3，其余 1（由 CoreError 决定）。
///
/// 泛型以同时接受 `Result<(), CoreError>` 与 `Result<WriteOutcome, CoreError>`。
/// 备份结果不在此处判定——它不改变退出码（设计文档 K2）。
pub(crate) fn apply_core_result<T>(result: Result<T, core::CoreError>) {
    if let Err(e) = result {
        exit_core_error(&e);
    }
}
```

**B2（阻断，核对轮第二轮）：`apply_concurrency` 必须一并泛型化。** 计划此前只给了 `runtime.rs:27`，漏了 `cli/src/env_ops.rs:120`：

```rust
// cli/src/env_ops.rs:120 —— 原形参是具体类型，实参变成 WriteOutcome 后会编译失败
pub(crate) fn apply_concurrency<T>(result: Result<T, CoreError>) {
    apply_core_result(result);
}
```

**本任务在 `cli/src/env_ops.rs` 的完整改动清单（逐处核对过，共 6 处）**：

| 行     | 现状                                               | 改动                                                   |
| ------ | -------------------------------------------------- | ------------------------------------------------------ |
| `:120` | `apply_concurrency(result: Result<(), CoreError>)` | 改成泛型 `<T>`（**计划此前遗漏**）                     |
| `:339` | `apply_concurrency(update_env_var(...))`           | 实参类型变化，泛型化后自动适配                         |
| `:377` | `apply_concurrency(delete_env_var(...))`           | 同上                                                   |
| `:342` | `update_env_var_force(...).unwrap_or_else(...)`    | 返回值变 `WriteOutcome`，须接住并检查 backup（见 J1b） |
| `:363` | `create_env_var(...).unwrap_or_else(...)`          | 同上                                                   |
| `:380` | `delete_env_var_force(...).unwrap_or_else(...)`    | 同上                                                   |

**J1b（核对轮裁断）：CLI 侧的备份失败警告必须由返回值显式 `eprintln!`，不能依赖 core 的 `log::warn!`。** 实证：`cli/Cargo.toml` 无 `log` 依赖、`cli/src/` 中 `log::` 零命中——**CLI 没有初始化任何 logger**，core 里的 `log::warn!` 在 CLI 下被静默丢弃。

统一加一个 CLI 侧辅助（放在 `cli/src/runtime.rs`）：

```rust
/// 备份失败时在 stderr 提示，**不改变退出码**（设计文档 K2）。
///
/// CLI 未初始化 logger，core 的 `log::warn!` 在此被丢弃 —— 必须经返回值显式打印。
pub(crate) fn warn_if_backup_failed(outcome: &core::backup::BackupOutcome) {
    if let core::backup::BackupOutcome::Failed(reason) = outcome {
        eprintln!("警告: 环境变量写前备份失败（写入已完成）: {reason}");
    }
}
```

三个 force/add 分支改为：

```rust
        Concurrency::Force => {
            let outcome = core::registry::update_env_var_force(hive, &name, &new_value)
                .unwrap_or_else(|e| exit_core_error(&e));
            warn_if_backup_failed(&outcome.backup);
        }
```

（`:363` 的 `create_env_var`、`:380` 的 `delete_env_var_force` 同样处理。）

两个 revision 分支（`:339`/`:377`）经 `apply_concurrency` 泛型化后返回值被丢弃——**这是可接受的**：`env set --revision` 的失败语义是冲突退出码 3，而备份失败不改变退出码；但为保持与 force 分支一致的可观测性，建议把 `apply_concurrency` 也改为返回 `Option<BackupOutcome>` 或直接在调用点接住后 `warn_if_backup_failed`。**二选一即可，实施时择一并在回执中说明。**

`gui/src/commands/env_var.rs` 的 5 个命令返回类型跟着改（`Result<WriteOutcome, CoreError>`），文档注释补 `# Returns` 说明 `backup` 字段：

```rust
/// 写入已有变量；类型从注册表读取，revision 不匹配则拒绝（code=`Conflict`）。
///
/// 写前尽力备份；备份失败不阻断写入，结果经 `WriteOutcome.backup` 如实返回。
///
/// # Returns
/// - `Ok(WriteOutcome)` — 写入成功；`backup` 说明备份结果
/// - `Err(CoreError)` — 校验失败、修订冲突或类型不受支持
#[tauri::command]
pub fn update_env_var(
    hive: EnvHive,
    name: String,
    value: String,
    expected_revision: String,
) -> Result<WriteOutcome, CoreError> {
    registry::update_env_var(hive, &name, &value, &expected_revision)
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test --workspace`
Expected: 全绿。既有测试用 `.expect()` / `.unwrap()` / `.is_err()`，对 `Result<WriteOutcome, _>` 仍然适用；若出现 `Ok(())` 精确匹配则改为 `Ok(_)`。

再跑 `cargo clippy --workspace --all-targets -- -D warnings`，排查未使用变量与签名漂移。
最后 `cargo test -p patheditor-cli --bins` 确认 CLI 侧未破坏。

- [ ] **Step 5: 提交**

```bash
git add core/src/backup.rs core/src/registry/env_var.rs core/src/registry.rs core/src/lib.rs \
        cli/src/env_ops.rs cli/src/runtime.rs gui/src/commands/env_var.rs
git commit -m "feat(core): 5 个 env 写入口挂载写前备份，返回 WriteOutcome"
```

---

### Task 4: 备份列表与路径校验

**Files:**

- Modify: `core/src/backup.rs`
- Test: `core/src/backup.rs`

**Interfaces:**

- Produces:
  - `pub struct EnvBackupInfo { pub file: String, pub path: String, pub timestamp: String, pub size_bytes: u64, pub variable_count: u64 }`
  - `pub fn list_env_backups_in(dir: &Path) -> Result<Vec<EnvBackupInfo>, CoreError>`
  - `pub fn list_env_backups() -> Result<Vec<EnvBackupInfo>, CoreError>`
  - `pub fn validate_backup_path(path: &str) -> Result<PathBuf, CoreError>`

- [ ] **Step 1: 写失败测试**

```rust
    /// 列表只枚举目录、不解析内容：损坏文件不得让列表整体失败（S5）。
    #[test]
    fn list_env_backups_survives_corrupt_file() {
        with_temp_backup_dir(|dir| {
            std::fs::write(dir.join("env_backup_20260901_120000_000.json"), "not json at all").unwrap();
            std::fs::write(dir.join("env_backup_20260902_120000_000.json"), "{\"broken\":").unwrap();
            // 干扰文件不得出现在列表里
            std::fs::write(dir.join("path_backup_20260920_152446_043.txt"), "PATH").unwrap();
            std::fs::write(dir.join("env_backup_x.json.bak"), "BAK").unwrap();

            let list = list_env_backups_in(dir).expect("损坏文件不得使列表失败");

            assert_eq!(list.len(), 2, "只有两个 env_backup_*.json");
            assert!(list.iter().all(|i| i.file.starts_with("env_backup_")));
        });
    }

    /// 列表按时间倒序（最新在前）。
    #[test]
    fn list_env_backups_sorted_newest_first() {
        with_temp_backup_dir(|dir| {
            for ts in ["20260901_120000_000", "20260903_120000_000", "20260902_120000_000"] {
                std::fs::write(dir.join(format!("env_backup_{ts}.json")), "{}").unwrap();
            }
            let list = list_env_backups_in(dir).unwrap();
            assert_eq!(list[0].file, "env_backup_20260903_120000_000.json");
            assert_eq!(list[2].file, "env_backup_20260901_120000_000.json");
        });
    }

    /// S4：路径校验拒绝非 .json、非备份目录、超大文件。
    #[test]
    fn validate_backup_path_rejects_invalid() {
        with_temp_backup_dir(|dir| {
            assert!(validate_backup_path("C:\\Windows\\System32\\config\\SAM").is_err(), "非 .json 必须拒绝");
            assert!(validate_backup_path("C:\\some\\other\\file.json").is_err(), "既不在备份目录也不带 env_backup_ 前缀必须拒绝");

            // 带前缀但超大 → 拒绝
            let big = dir.join("env_backup_huge.json");
            std::fs::write(&big, vec![b'x'; 1024 * 1024 + 1]).unwrap();
            assert!(validate_backup_path(&big.to_string_lossy()).is_err(), "超过 1 MiB 必须拒绝");

            // 正常文件 → 通过
            let ok = dir.join("env_backup_20260901_120000_000.json");
            std::fs::write(&ok, "{}").unwrap();
            assert!(validate_backup_path(&ok.to_string_lossy()).is_ok());
        });
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path-editor-core validate_backup_path`
Expected: 编译失败 —— `cannot find function validate_backup_path`、`cannot find function list_env_backups_in`。

- [ ] **Step 3: 写最小实现**

```rust
/// 备份文件列表项（不含内容，见设计文档 §S5）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvBackupInfo {
    /// 文件名
    pub file: String,
    /// 绝对路径
    pub path: String,
    /// 文件名中的时间戳部分（`YYYYMMDD_HHMMSS_mmm`）
    pub timestamp: String,
    /// 文件字节数
    pub size_bytes: u64,
    /// 备份中的变量条数；只读目录时为 0（不解析内容）
    pub variable_count: u64,
}

/// 备份文件大小上限：正常备份几十 KB，超过 1 MiB 说明不是本工具产物。
const MAX_BACKUP_FILE_BYTES: u64 = 1024 * 1024;

/// 列出目录中的 env 备份，按时间倒序。**只枚举目录与 stat，不解析内容**，
/// 因此单个损坏文件不会让列表整体失败。
///
/// # Returns
/// - `Ok(Vec<EnvBackupInfo>)` — 备份列表，最新在前
/// - `Err(CoreError)` — 目录枚举失败（code=`Io`）；目录不存在时返回空列表
pub fn list_env_backups_in(dir: &Path) -> Result<Vec<EnvBackupInfo>, CoreError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "list_env_backups",
            format!("枚举备份目录 {} 失败: {}", dir.display(), e),
        )
    })?;

    let mut infos = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("env_backup_") || !name.ends_with(".json") {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let timestamp = name
            .trim_start_matches("env_backup_")
            .trim_end_matches(".json")
            .to_string();
        infos.push(EnvBackupInfo {
            file: name,
            path: path.to_string_lossy().into_owned(),
            timestamp,
            size_bytes: meta.len(),
            variable_count: 0, // 不解析内容：见 S5
        });
    }

    infos.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(infos)
}

/// 列出默认备份目录中的 env 备份。
///
/// # Returns
/// - `Ok(Vec<EnvBackupInfo>)` — 备份列表，最新在前
/// - `Err(CoreError)` — 目录枚举失败
pub fn list_env_backups() -> Result<Vec<EnvBackupInfo>, CoreError> {
    list_env_backups_in(&env_backup_dir())
}

/// 校验用户给定的备��文件路径（设计文档 §S4）。
///
/// 规则：扩展名必须为 `.json`；必须位于默认备份目录之内，**或**文件名以
/// `env_backup_` 开头；文件必须存在且不超过 1 MiB。
///
/// # Returns
/// - `Ok(PathBuf)` — 校验通过的绝对路径
/// - `Err(CoreError)` — 路径非法（code=`InvalidValue`）或文件不存在/过大（code=`NotFound`/`InvalidValue`）
pub fn validate_backup_path(path: &str) -> Result<PathBuf, CoreError> {
    let p = PathBuf::from(path);
    let file_name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| {
            CoreError::new(
                ErrorCode::InvalidValue,
                "restore_env_backup",
                format!("备份路径非法: {path}"),
            )
        })?;

    if p.extension().map(|e| e != "json").unwrap_or(true) {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "restore_env_backup",
            format!("备份文件必须是 .json: {path}"),
        ));
    }

    let in_backup_dir = p
        .parent()
        .map(|parent| parent == env_backup_dir())
        .unwrap_or(false);
    if !in_backup_dir && !file_name.starts_with("env_backup_") {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "restore_env_backup",
            format!("备份文件必须位于备份目录内或以 env_backup_ 开头: {path}"),
        ));
    }

    let meta = std::fs::metadata(&p).map_err(|e| {
        CoreError::new(
            ErrorCode::NotFound,
            "restore_env_backup",
            format!("无法读取备份文件 {path}: {e}"),
        )
    })?;
    if meta.len() > MAX_BACKUP_FILE_BYTES {
        return Err(CoreError::new(
            ErrorCode::InvalidValue,
            "restore_env_backup",
            format!(
                "备份文件过大（{} 字节，上限 {}），不是本工具生成的备份",
                meta.len(),
                MAX_BACKUP_FILE_BYTES
            ),
        ));
    }
    Ok(p)
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p path-editor-core env_backup`
Expected: 全部通过（含 Task 2 的 3 个与本任务 3 个）。

- [ ] **Step 5: 提交**

```bash
git add core/src/backup.rs
git commit -m "feat(core): env 备份列表与恢复路径校验"
```

---

### Task 5: 差异计算（preview）

**Files:**

- Modify: `core/src/backup.rs`
- Test: `core/src/backup.rs`

**Interfaces:**

- Produces:
  - `pub enum RestoreChangeKind { Added, Modified, Removed, Conflict }`
  - `pub struct RestoreChange { pub hive: EnvHive, pub name: String, pub kind: RestoreChangeKind }`
  - `pub struct RestorePreview { pub changes: Vec<RestoreChange>, pub added: usize, pub modified: usize, pub removed: usize, pub conflicts: usize }`
  - `pub fn read_env_backup(path: &Path) -> Result<EnvBackupPayload, CoreError>`
  - `pub fn preview_restore_in_stores(sys: &dyn EnvHiveStore, usr: &dyn EnvHiveStore, payload: &EnvBackupPayload) -> Result<RestorePreview, CoreError>`

- [ ] **Step 1: 写失败测试**

```rust
    fn payload_with(user: Vec<EnvBackupVar>) -> EnvBackupPayload {
        EnvBackupPayload {
            captured_at: 0,
            hives: EnvBackupHives { system: vec![], user },
        }
    }

    /// 三类差异都要被识别：备份有而注册表无 → Added；
    /// 值不同 → Modified；注册表有而备份无 → Removed。
    #[test]
    fn preview_classifies_added_modified_removed() {
        let hive = MemoryHive::new(true);
        hive.seed("SAME", "v", REG_SZ); // 与备份一致，无变化
        hive.seed("CHANGED", "new-value", REG_SZ); // 备份里是旧值 → Modified
        hive.seed("EXTRA", "x", REG_SZ); // 备份里没有 → Removed

        let payload = payload_with(vec![
            EnvBackupVar { name: "SAME".into(), kind: EnvValueKind::String, value: "v".into(), revision: revision_of("SAME", REG_SZ, "v") },
            EnvBackupVar { name: "CHANGED".into(), kind: EnvValueKind::String, value: "old-value".into(), revision: revision_of("CHANGED", REG_SZ, "old-value") },
            EnvBackupVar { name: "BRAND_NEW".into(), kind: EnvValueKind::String, value: "n".into(), revision: revision_of("BRAND_NEW", REG_SZ, "n") },
        ]);

        let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();

        assert_eq!(preview.added, 1, "BRAND_NEW 应记为新增");
        assert_eq!(preview.modified, 1, "CHANGED 应记为修改");
        assert_eq!(preview.removed, 1, "EXTRA 应记为删除");
        assert_eq!(preview.conflicts, 0);
        assert!(preview.changes.iter().any(|c| c.name == "SAME") == false, "无变化的变量不进差异");
    }

    /// K3：备份中的 revision 与当前不一致时记为 Conflict。
    #[test]
    fn preview_marks_conflict_when_revision_differs() {
        let hive = MemoryHive::new(true);
        hive.seed("MY_VAR", "changed-by-other-tool", REG_SZ);

        let payload = payload_with(vec![EnvBackupVar {
            name: "MY_VAR".into(),
            kind: EnvValueKind::String,
            value: "backed-up-value".into(),
            // 故意用一个与当前值不匹配的 revision
            revision: revision_of("MY_VAR", REG_SZ, "backed-up-value"),
        }]);

        let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();
        assert_eq!(preview.conflicts, 1, "外部改动必须被识别为冲突");
        assert_eq!(preview.changes[0].kind, RestoreChangeKind::Conflict);
    }

    /// Unsupported 类型的当前值不得被恢复流程删除（它们不在备份里，但不是差异）。
    #[test]
    fn preview_ignores_unsupported_current_vars() {
        let hive = MemoryHive::new(true);
        hive.seed("SomeDword", "1", REG_DWORD); // 备份不可能含它

        let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload_with(vec![])).unwrap();
        assert_eq!(preview.removed, 0, "Unsupported 变量的存在不构成「删除」差异");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path-editor-core preview_`
Expected: 编译失败 —— `cannot find function preview_restore_in_stores`、`cannot find type RestoreChangeKind`。

- [ ] **Step 3: 写最小实现**

````rust
/// 恢复差异的类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RestoreChangeKind {
    /// 备份中有、注册表中没有 → 将新建
    Added,
    /// 两边都有但值不同 → 将覆盖
    Modified,
    /// 注册表中有、备份中没有 → 将删除（最不可逆）
    Removed,
    /// 备份中的 revision 与注册表当前值不符 → 备份后被外部修改
    Conflict,
}

/// 单条恢复差异。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreChange {
    pub hive: EnvHive,
    pub name: String,
    pub kind: RestoreChangeKind,
}

/// 恢复差异预览（供 CLI `--dry-run` 与 GUI 确认弹窗消费）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub changes: Vec<RestoreChange>,
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
    pub conflicts: usize,
}

/// 读取并校验备份文件。
///
/// # Returns
/// - `Ok(EnvBackupPayload)` — 校验通过的内容
/// - `Err(CoreError)` — 路径非法（`InvalidValue`）、读取失败（`Io`）、
///   解析失败（`Parse`，损坏文件会被隔离）、版本过高（`Parse`）
pub fn read_env_backup(path: &Path) -> Result<EnvBackupPayload, CoreError> {
    let versioned = crate::persist::read_versioned_file::<EnvBackupPayload>(path, "env 备份")?;
    crate::persist::migrate(versioned, "env 备份")
}

/// 计算备份相对当前注册表的差异（不写入任何内容）。**存储可注入版本，仅 core 内部用。**
///
/// `Revision` 比对口径与写通路一致：备份记录的 revision 与注册表当前值的
/// revision 不同即说明备份之后被改动过（设计文档 K3）。
///
/// # Returns
/// - `Ok(RestorePreview)` — 差异摘要
/// - `Err(CoreError)` — 读取注册表失败
pub(crate) fn preview_restore_in_stores(
    sys: &dyn EnvHiveStore,
    usr: &dyn EnvHiveStore,
    payload: &EnvBackupPayload,
) -> Result<RestorePreview, CoreError> {
    let mut changes = Vec::new();
    diff_one_hive(usr, EnvHive::User, &payload.hives.user, &mut changes)?;
    diff_one_hive(sys, EnvHive::System, &payload.hives.system, &mut changes)?;

    let added = changes.iter().filter(|c| c.kind == RestoreChangeKind::Added).count();
    let modified = changes.iter().filter(|c| c.kind == RestoreChangeKind::Modified).count();
    let removed = changes.iter().filter(|c| c.kind == RestoreChangeKind::Removed).count();
    let conflicts = changes.iter().filter(|c| c.kind == RestoreChangeKind::Conflict).count();

    Ok(RestorePreview { changes, added, modified, removed, conflicts })
}

/// 计算单个 hive 的差异，追加到 `out`。
///
/// **行为契约（必须全部成立，实现方式自定）**：
/// 1. 当前注册表中**保留名**（`Path`）与 **`Unsupported` 类型**不参与差异计算——
///    它们既不产生 `Removed`，也不产生其他任何变体；
/// 2. 备份中有、当前无 → `Added`；
/// 3. 备份 revision 与当前 revision 相同 → **不进差异**（无变化）；
/// 4. 备份 revision 与当前不同 → `Conflict`；
/// 5. 当前有、备份无 → `Removed`，且 **`Removed` 条目必须携带注册表返回的原始大小写名字**
///    （不能是小写化的比较键）；
/// 6. 变量名比较**忽略大小写**（Windows 注册表语义），但输出保留原始大小写。
///
/// **实现提示（不要照抄下面的骨架）**：下表结构表达上述契约，但 `current` 的
/// 值类型需同时携带「注册表原始名」与「revision」两项，末段的删除循环才能拿到
/// 原始名。早期版本的计划草稿在这里写了一个不存在的 `raw_name_of(store, key)`
/// 辅助函数，且 `HashMap<String, (RegValue, String)>` 与 `for (key, (raw, _))`
/// 的解构不匹配——**请自行实现，不要引用不存在的函数**。
///
/// ```text
/// current: HashMap<小写名, (原始名: String, revision: String)>
/// for var in backup_vars:
///     小写键 = var.name.to_lowercase()
///     记入 seen
///     match current.get(小写键):
///         None            → push Added（用 var.name）
///         Some((_, rev)) if rev == var.revision → 无变化
///         Some(_)         → push Conflict（用 var.name）
/// for (小写键, (原始名, _)) in current where 小写键 ∉ seen:
///     push Removed（用原始名，不是小写键）
/// ```
fn diff_one_hive(
    store: &dyn EnvHiveStore,
    hive: EnvHive,
    backup_vars: &[EnvBackupVar],
    out: &mut Vec<RestoreChange>,
) -> Result<(), CoreError> {
    let (_, _, label) = hive_location(hive);
    // 当前可写变量表：小写名 → (注册表原始名, revision)
    // 原始名用于 Removed 差异的展示（Windows 注册表名大小写不保证稳定）。
    let mut current: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    let names = store.enum_names().map_err(|e| {
        CoreError::new(
            ErrorCode::Io,
            "preview_restore",
            format!("读取{}环境变量列表失败: {}", label, e),
        )
    })?;
    for name in names {
        if is_reserved(&name) {
            continue;
        }
        let Ok(raw) = store.get_raw(&name) else { continue };
        let kind = EnvValueKind::from_reg_type(raw.vtype.clone());
        if !kind.is_writable() {
            continue; // Unsupported 不参与恢复，也不构成删除差异
        }
        let Ok(value) = String::from_reg_value(&raw) else { continue };
        let rev = revision_of(&name, raw.vtype.clone(), &value);
        current.insert(name.to_ascii_lowercase(), (name, rev));
    }

    let mut seen = std::collections::HashSet::new();
    for var in backup_vars {
        let key = var.name.to_ascii_lowercase();
        seen.insert(key.clone());
        match current.get(&key) {
            None => out.push(RestoreChange { hive, name: var.name.clone(), kind: RestoreChangeKind::Added }),
            Some((_, current_rev)) if current_rev == &var.revision => {
                // revision 相同即值相同，无差异
            }
            Some(_) => out.push(RestoreChange { hive, name: var.name.clone(), kind: RestoreChangeKind::Conflict }),
        }
    }

    // 注册表有、备份无 → 删除（用注册表原始名展示，不是小写比较键）
    for (key, (original_name, _)) in &current {
        if seen.contains(key) {
            continue;
        }
        out.push(RestoreChange {
            hive,
            name: original_name.clone(),
            kind: RestoreChangeKind::Removed,
        });
    }

    Ok(())
}
````

> **签名的完整形态**：`preview_restore_in_stores` 必须是 `pub(crate)`（core 内部测试用）；
> 另需两个公开包装：
>
> ```rust
> /// 计算备份相对当前注册表的差异（不写入任何内容）。
> ///
> /// **不含 force 参数**——force 只在执行层影响「冲突是否中止」，
> /// 差异计算本身与 force 无关（核对轮 E3）。
> pub fn preview_restore(payload: &EnvBackupPayload) -> Result<RestorePreview, CoreError> {
>     let sys = WinregHive::open(EnvHive::System, false)?;
>     let usr = WinregHive::open(EnvHive::User, false)?;
>     preview_restore_in_stores(&sys, &usr, payload)
> }
>
> /// 从文件读取备份后计算差异（GUI 确认弹窗用）。
> pub fn preview_restore_file(path: &Path) -> Result<RestorePreview, CoreError> {
>     let payload = read_env_backup(path)?;
>     preview_restore(&payload)
> }
> ```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p path-editor-core preview_`
Expected: 3 passed。

- [ ] **Step 5: 提交**

```bash
git add core/src/backup.rs
git commit -m "feat(core): env 备份差异计算（新增/修改/删除/冲突）"
```

---

### Task 6: 恢复执行

**Files:**

- Modify: `core/src/backup.rs`
- Test: `core/src/backup.rs`

**Interfaces:**

- Consumes: Task 5 的 `preview_restore_in_stores`、`read_env_backup`；**`crate::registry::{create_env_var_in_store, update_env_var_force_in_store, delete_env_var_force_in_store}`**（Task 6 需在 `registry.rs` 加 `pub(crate) use` 才能这样引用）
- Produces:
  - `pub struct RestoreOutcome { pub applied: usize, pub skipped: usize, pub failures: Vec<String> }`
  - `pub fn restore_env_backup_from(path: &Path, force: bool) -> Result<RestoreOutcome, CoreError>`

- [ ] **Step 1: 写失败测试**

```rust
    /// 默认模式遇到冲突必须中止，且**不做任何写入**（注册表零改动）。
    #[test]
    fn restore_aborts_on_conflict_without_writing() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("MY_VAR", "changed-by-other-tool", REG_SZ);
            hive.seed("UNTOUCHED", "keep-me", REG_SZ);

            let payload = EnvBackupPayload {
                captured_at: 0,
                hives: EnvBackupHives {
                    system: vec![],
                    user: vec![
                        EnvBackupVar { name: "MY_VAR".into(), kind: EnvValueKind::String, value: "backed-up".into(), revision: revision_of("MY_VAR", REG_SZ, "backed-up") },
                        EnvBackupVar { name: "NEW_ONE".into(), kind: EnvValueKind::String, value: "n".into(), revision: revision_of("NEW_ONE", REG_SZ, "n") },
                    ],
                },
            };
            let path = write_env_backup_to(dir, &payload).unwrap();

            let err = restore_in_stores(&MemoryHive::new(true), &hive, &path, false)
                .expect_err("冲突必须中止");
            assert_eq!(err.code, crate::error::ErrorCode::Conflict);

            assert_eq!(String::from_reg_value(&hive.get_raw("MY_VAR").unwrap()).unwrap(), "changed-by-other-tool", "冲突时不得写入");
            assert!(!hive.contains("NEW_ONE"), "中止时不得部分写入：NEW_ONE 也不应被创建");
        });
    }

    /// --force 模式下冲突被覆盖，写入成功。
    #[test]
    fn restore_force_overwrites_conflict() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("MY_VAR", "changed-by-other-tool", REG_SZ);

            let payload = EnvBackupPayload {
                captured_at: 0,
                hives: EnvBackupHives {
                    system: vec![],
                    user: vec![EnvBackupVar {
                        name: "MY_VAR".into(), kind: EnvValueKind::String,
                        value: "backed-up".into(),
                        revision: revision_of("MY_VAR", REG_SZ, "backed-up"),
                    }],
                },
            };
            let path = write_env_backup_to(dir, &payload).unwrap();

            let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true).expect("force 恢复必须成功");
            assert_eq!(outcome.applied, 1);
            assert_eq!(String::from_reg_value(&hive.get_raw("MY_VAR").unwrap()).unwrap(), "backed-up");
        });
    }

    /// S6：--force 不豁免保护名单——保护名单变量出现在差异中，
    /// 但写入阶段被 core 拒绝并记入 failures，且**不中止其余变量的恢复**。
    ///
    /// 核对轮 E5 裁断：preview 不为保护名单新增枚举变体、不预排除；
    /// 拒绝发生在写入阶段（core 写函数内是保护名单的唯一真相源）。
    #[test]
    fn restore_force_does_not_bypass_protected_names() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("windir", "C:\\Windows", REG_SZ);

            let payload = EnvBackupPayload {
                captured_at: 0,
                hives: EnvBackupHives {
                    system: vec![],
                    user: vec![
                        EnvBackupVar {
                            name: "windir".into(), kind: EnvValueKind::String,
                            value: "C:\\evil".into(),
                            revision: revision_of("windir", REG_SZ, "attacker-value"),
                        },
                        // 同批还有一个正常变量：用来证明保护名单失败**不中止**其余恢复
                        EnvBackupVar {
                            name: "NORMAL_ONE".into(), kind: EnvValueKind::String,
                            value: "ok".into(),
                            revision: revision_of("NORMAL_ONE", REG_SZ, "ok"),
                        },
                    ],
                },
            };
            let path = write_env_backup_to(dir, &payload).unwrap();

            let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true)
                .expect("保护名单被拒不得使整个恢复失败");

            assert_eq!(outcome.applied, 1, "只有 NORMAL_ONE 应写入成功");
            assert_eq!(outcome.failures.len(), 1, "windir 必须被记为失败");
            assert!(
                outcome.failures[0].contains("windir"),
                "失败原因必须点名 windir，实际: {}",
                outcome.failures[0]
            );
            assert_eq!(
                String::from_reg_value(&hive.get_raw("windir").unwrap()).unwrap(),
                "C:\\Windows",
                "保护名单变量的值必须保持原样"
            );
            assert!(hive.contains("NORMAL_ONE"), "其余变量必须照常恢复");
        });
    }

    /// E5 的对应断言：保护名单变量**必须出现在 preview 差异里**（不预排除）。
    #[test]
    fn preview_includes_protected_names() {
        let hive = MemoryHive::new(true);
        hive.seed("windir", "C:\\Windows", REG_SZ);

        let payload = EnvBackupPayload {
            captured_at: 0,
            hives: EnvBackupHives {
                system: vec![],
                user: vec![EnvBackupVar {
                    name: "windir".into(), kind: EnvValueKind::String,
                    value: "C:\\evil".into(),
                    revision: revision_of("windir", REG_SZ, "attacker-value"),
                }],
            },
        };

        let preview = preview_restore_in_stores(&MemoryHive::new(true), &hive, &payload).unwrap();
        assert_eq!(preview.conflicts, 1, "保护名单变量必须出现在差异中，不被预先排除");
        assert!(preview.changes.iter().any(|c| c.name == "windir"));
    }

    /// 恢复新增与删除两类差异。
    #[test]
    fn restore_creates_and_removes() {
        with_temp_backup_dir(|dir| {
            let hive = MemoryHive::new(true);
            hive.seed("WILL_BE_REMOVED", "x", REG_SZ);

            let payload = EnvBackupPayload {
                captured_at: 0,
                hives: EnvBackupHives {
                    system: vec![],
                    user: vec![EnvBackupVar {
                        name: "WILL_BE_CREATED".into(), kind: EnvValueKind::String,
                        value: "created".into(),
                        revision: revision_of("WILL_BE_CREATED", REG_SZ, "created"),
                    }],
                },
            };
            let path = write_env_backup_to(dir, &payload).unwrap();

            let outcome = restore_in_stores(&MemoryHive::new(true), &hive, &path, true).unwrap();
            assert_eq!(outcome.applied, 2, "一个新增 + 一个删除");
            assert!(hive.contains("WILL_BE_CREATED"));
            assert!(!hive.contains("WILL_BE_REMOVED"));
        });
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p path-editor-core restore_`
Expected: 编译失败 —— `cannot find function restore_in_stores`、`cannot find type RestoreOutcome`。

- [ ] **Step 3: 写最小实现**

```rust
/// 恢复执行结果。单变量失败不中止整体（best-effort），但必须逐条记录。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    /// 成功写入的变量数（新增 + 修改 + 删除）
    pub applied: usize,
    /// 因冲突被跳过的变量数（仅 `--force` 之外的场景可能出现）
    pub skipped: usize,
    /// 失败原因（含变量名与错误信息），空表示全部成功
    pub failures: Vec<String>,
}

/// 从备份恢复单个 hive；存储与文件路径可注入，供测试。
///
/// 恢复**不使用** `EnvHiveStore::set_raw` 直接写裸值 —— 必须逐变量走
/// `create_env_var_in_store` / `update_env_var_force_in_store` /
/// `delete_env_var_force_in_store`，否则会绕过保护名单与类型判定
/// （设计文档 §S6）。
///
/// # Returns
/// - `Ok(RestoreOutcome)` — 恢复结果（可能含逐条失败）
/// - `Err(CoreError)` — 读取备份失败，或**默认模式下检测到冲突**（code=`Conflict`，注册表零改动）
fn restore_in_stores(
    sys: &dyn EnvHiveStore,
    usr: &dyn EnvHiveStore,
    path: &Path,
    force: bool,
) -> Result<RestoreOutcome, CoreError> {
    let payload = read_env_backup(path)?;
    let preview = preview_restore_in_stores(sys, usr, &payload)?;

    // K3：默认模式下任何冲突都中止，且不做任何写入。
    if !force && preview.conflicts > 0 {
        let names: Vec<&str> = preview
            .changes
            .iter()
            .filter(|c| c.kind == RestoreChangeKind::Conflict)
            .map(|c| c.name.as_str())
            .collect();
        return Err(CoreError::new(
            ErrorCode::Conflict,
            "restore_env_backup",
            format!(
                "备份后有 {} 个变量被外部修改，恢复已中止（未做任何改动）: {}。确认要覆盖请加 --force",
                names.len(),
                names.join(", ")
            ),
        ));
    }

    let mut outcome = RestoreOutcome { applied: 0, skipped: 0, failures: Vec::new() };

    for change in &preview.changes {
        let store = match change.hive {
            EnvHive::System => sys,
            EnvHive::User => usr,
        };
        // Conflict 在 force 模式下按 Modified 处理；非 force 模式已在上面中止。
        let result = match change.kind {
            RestoreChangeKind::Added => {
                let var = find_backup_var(&payload, &change.hive, &change.name);
                match var {
                    Some(v) => create_env_var_in_store(store, change.hive, &v.name, &v.value, v.kind),
                    None => continue,
                }
            }
            RestoreChangeKind::Modified | RestoreChangeKind::Conflict => {
                let var = find_backup_var(&payload, &change.hive, &change.name);
                match var {
                    Some(v) => update_env_var_force_in_store(store, change.hive, &v.name, &v.value),
                    None => continue,
                }
            }
            RestoreChangeKind::Removed => {
                delete_env_var_force_in_store(store, change.hive, &change.name)
            }
        };

        match result {
            Ok(()) => outcome.applied += 1,
            Err(e) => outcome.failures.push(format!(
                "[{}] {}: {}",
                match change.hive {
                    EnvHive::System => "系统",
                    EnvHive::User => "用户",
                },
                change.name,
                e.message
            )),
        }
    }

    Ok(outcome)
}

/// 在备份载荷中按 hive 与变量名查找条目（忽略大小写）。
fn find_backup_var<'a>(
    payload: &'a EnvBackupPayload,
    hive: &EnvHive,
    name: &str,
) -> Option<&'a EnvBackupVar> {
    let list = match hive {
        EnvHive::System => &payload.hives.system,
        EnvHive::User => &payload.hives.user,
    };
    list.iter().find(|v| v.name.eq_ignore_ascii_case(name))
}

/// 从备份文件恢复环境变量（公开入口）。
///
/// 恢复本身**不产生新备份**——否则每次恢复都会新增文件，与保留策略互相吞噬。
///
/// # Returns
/// - `Ok(RestoreOutcome)` — 恢复结果（含逐条失败）
/// - `Err(CoreError)` — 路径非法、读取/解析失败，或默认模式下检测到冲突
pub fn restore_env_backup_from(path: &Path, force: bool) -> Result<RestoreOutcome, CoreError> {
    let verified = validate_backup_path(&path.to_string_lossy())?;
    let sys = WinregHive::open(EnvHive::System, true)?;
    let usr = WinregHive::open(EnvHive::User, true)?;
    let outcome = restore_in_stores(&sys, &usr, &verified, force)?;
    if outcome.applied > 0 {
        crate::system::broadcast_env_change();
    }
    Ok(outcome)
}
```

公开包装函数（供 `restore_env_backup_from` 之外的测试与高级用法）：把三个 `*_in_store` 函数由私有改为 `pub(crate)`，**并在 `core/src/registry.rs` 加 `pub(crate) use`**：

```rust
// core/src/registry/env_var.rs —— 原 `fn` 改为 `pub(crate) fn`
pub(crate) fn create_env_var_in_store(store: &dyn EnvHiveStore, hive: EnvHive, name: &str, value: &str, kind: EnvValueKind) -> Result<(), CoreError>
pub(crate) fn update_env_var_force_in_store(store: &dyn EnvHiveStore, hive: EnvHive, name: &str, value: &str) -> Result<(), CoreError>
pub(crate) fn delete_env_var_force_in_store(store: &dyn EnvHiveStore, hive: EnvHive, name: &str) -> Result<(), CoreError>

// core/src/registry.rs —— 加在既有 `pub(crate) use access::hive_location;` 那一组里
pub(crate) use env_var::{
    create_env_var_in_store, delete_env_var_force_in_store, update_env_var_force_in_store,
};
```

> **关键（核对轮 E1/E4）**：**只有**在 `registry.rs` 里加了 `pub(crate) use`，`backup.rs` 才能经 `crate::registry::X` 引用它们。模块 `mod env_var;` 是私有的（`registry.rs:14`），写 `crate::registry::env_var::X` 会报 `error[E0603]: module env_var is private`——已用最小实验实证。新增的 `pub(crate) use` **必须落在 `registry.rs:30` 那一组**（crate 内部共享区），**不能**另起一处放在测试模块里——否则 `golden_tests.rs:240,246` 依赖的既有模式会被破坏（它们同样走 `crate::registry::load_system_paths()` 根路径）。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p path-editor-core restore_`
Expected: 4 passed。

- [ ] **Step 5: 提交**

```bash
git add core/src/backup.rs core/src/registry/env_var.rs core/src/registry.rs
git commit -m "feat(core): env 备份恢复执行与冲突中止语义"
```

---

### Task 7: CLI 三个子命令

**Files:**

- Modify: `cli/src/main.rs`、`cli/src/env_ops.rs`
- Test: `cli/src/env_ops.rs`（`mod tests`）

**Interfaces:**

- Consumes: `core::backup::{backup_env_vars, list_env_backups, restore_env_backup_from, validate_backup_path}`
- Produces: `cmd_env_backup(json: bool)`、`cmd_env_backups(json: bool)`、`cmd_env_restore(file: String, dry_run: bool, force: bool, json: bool)`

- [ ] **Step 1: 写失败测试**

在 `cli/src/env_ops.rs` 的 `mod tests` 追加：

```rust
    /// 恢复命令的参数解析：--dry-run 与 --force 可共存（先看差异再覆盖）。
    #[test]
    fn restore_flags_are_independent() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "patheditor", "env", "restore", "env_backup_x.json", "--dry-run", "--force",
        ])
        .expect("--dry-run 与 --force 必须可共存");
        let crate::Command::Env(crate::EnvCmd::Restore { dry_run, force, .. }) = cli.command else {
            panic!("应解析为 env restore");
        };
        assert!(dry_run);
        assert!(force);
    }

    /// 备份列表在空目录下返回空数组而非报错（JSON 输出形状稳定）。
    ///
    /// B3（核对轮第二轮）：**不取 `core::persist::test_persist_lock`** —— 该函数是
    /// `pub(crate)`（`core/src/lib.rs:8` 的 `pub(crate) mod persist;`），从 cli crate
    /// 引用会报 `error[E0603]: module persist is private`。而且也不需要：`--bins`
    /// 测试跑在**独立进程**里，与 core 的测试不共享进程级环境变量，那把进程内的锁
    /// 跨进程无意义。core 的 `test_persist_lock` 只用于 core crate 内部的同进程测试。
    #[test]
    fn backups_list_json_shape_is_stable() {
        let dir = std::env::temp_dir().join(format!("patheditor_t7_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("PATHEDITOR_BACKUP_DIR", &dir);

        let list = path_editor_core::backup::list_env_backups().expect("空目录不得报错");
        let json = serde_json::to_string(&list).unwrap();
        assert_eq!(json, "[]");

        std::env::remove_var("PATHEDITOR_BACKUP_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p patheditor-cli --bins restore_flags_are_independent`
Expected: 编译失败 —— `no variant named Restore found for enum EnvCmd`。

- [ ] **Step 3: 写最小实现**

`cli/src/main.rs` 的 `enum EnvCmd` 末尾追加：

```rust
    /// 立即创建一份环境变量备份
    Backup {
        #[arg(long)]
        json: bool,
    },
    /// 列出已有的环境变量备份（按时间倒序）
    Backups {
        #[arg(long)]
        json: bool,
    },
    /// 从备份文件恢复环境变量
    Restore {
        /// 备份文件路径
        file: String,
        /// 只显示将要发生的变更，不写注册表
        #[arg(long)]
        dry_run: bool,
        /// 跳过 revision 校验直接覆盖（不豁免保护名单与类型判定）
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
```

命令分派处追加：

```rust
            EnvCmd::Backup { json } => env_ops::cmd_env_backup(json),
            EnvCmd::Backups { json } => env_ops::cmd_env_backups(json),
            EnvCmd::Restore { file, dry_run, force, json } => {
                env_ops::cmd_env_restore(file, dry_run, force, json)
            }
```

`cli/src/env_ops.rs` 追加：

```rust
/// `env backup` —— 立即创建一份环境变量备份。
///
/// 与写前自动备份共用同一实现；可在「改之前想手动留个还原点」时使用。
pub(crate) fn cmd_env_backup(json: bool) {
    match core::backup::backup_env_vars() {
        Ok(path) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "path": path.to_string_lossy() })
                );
            } else {
                println!("备份已保存到: {}", path.display());
            }
        }
        Err(e) => exit_core_error(&e),
    }
}

/// `env backups` —— 列出已有的环境变量备份（按时间倒序）。
///
/// 只枚举目录与 stat，不解析内容：单个损坏的备份不会让列表失败。
pub(crate) fn cmd_env_backups(json: bool) {
    let list = core::backup::list_env_backups().unwrap_or_else(|e| exit_core_error(&e));
    if json {
        println!("{}", serde_json::to_string_pretty(&list).unwrap_or_else(|_| "[]".into()));
        return;
    }
    if list.is_empty() {
        println!("暂无环境变量备份（目录: {}）", core::backup::env_backup_dir().display());
        return;
    }
    println!("{:<40} {:>12}  {}", "文件", "大小(字节)", "路径");
    for info in &list {
        println!("{:<40} {:>12}  {}", info.file, info.size_bytes, info.path);
    }
}

/// `env restore` —— 从备份文件恢复环境变量。
///
/// 默认模式在检测到 revision 冲突时中止且不做任何写入（退出码 3）；
/// `--force` 跳过 revision 校验直接覆盖，但**不豁免**保护名单与类型判定。
pub(crate) fn cmd_env_restore(file: String, dry_run: bool, force: bool, json: bool) {
    let path = core::backup::validate_backup_path(&file).unwrap_or_else(|e| exit_core_error(&e));

    if dry_run {
        let payload = core::backup::read_env_backup(&path).unwrap_or_else(|e| exit_core_error(&e));
        // preview 不带 force：差异计算与 force 无关（核对轮 E3 裁断）
        let preview = core::backup::preview_restore(&payload)
            .unwrap_or_else(|e| exit_core_error(&e));
        if json {
            println!("{}", serde_json::to_string_pretty(&preview).unwrap_or_else(|_| "{}".into()));
            return;
        }
        println!(
            "将新增 {} 个、修改 {} 个、删除 {} 个；冲突 {} 个（未写入任何内容）",
            preview.added, preview.modified, preview.removed, preview.conflicts
        );
        for change in &preview.changes {
            println!("  [{:?}] {} ({:?})", change.hive, change.name, change.kind);
        }
        return;
    }

    // 手工兜底出口（核对轮 C7）：恢复自己不产生备份，先把两条命令告诉用户。
    // 只打印、不自动执行——自动备份会与保留策略互相吞噬。
    eprintln!("提示: 恢复不会自动备份当前状态。如需留还原点，请先执行:");
    eprintln!("  patheditor backup       # 备份当前 PATH");
    eprintln!("  patheditor env backup   # 备份当前全部环境变量");

    let outcome = core::backup::restore_env_backup_from(&path, force)
        .unwrap_or_else(|e| exit_core_error(&e));

    if json {
        println!("{}", serde_json::to_string_pretty(&outcome).unwrap_or_else(|_| "{}".into()));
    } else {
        println!("恢复完成: 成功 {} 个", outcome.applied);
    }
    for failure in &outcome.failures {
        eprintln!("警告: {failure}");
    }
}
```

> **注**：`preview_restore(&payload)` **不含 force 参数**（核对轮 E3 裁断：差异计算与 force 无关），由 Task 5 一并产出；GUI 侧用 `preview_restore_file(&path)`。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p patheditor-cli --bins`
Expected: 全绿（含 2 个新用例）。

再做一次手工冒烟（**只读路径，不写注册表**）：

```powershell
cargo run -p patheditor-cli -- env backups
cargo run -p patheditor-cli -- env backups --json
cargo run -p patheditor-cli -- env restore "不存在的文件.json"   # 期望退出码 1 + 明确报错
```

- [ ] **Step 5: 提交**

```bash
git add cli/src/main.rs cli/src/env_ops.rs
git commit -m "feat(cli): 新增 env backup / backups / restore 三个子命令"
```

---

### Task 8: GUI 命令与前端接线

**Files:**

- Modify: `gui/src/commands/backup.rs`、`gui/src/lib.rs`、`src/services/backend.ts`
- Create: `src/core/env-backup.ts`、`src/components/dialogs/EnvBackupDialog.tsx`
- Test: `tests/unit/env-backup.test.ts`

**Interfaces:**

- Consumes: Task 4/5/6 的 core API
- Produces（Tauri 命令）：
  - `backup_env_vars() -> Result<String, CoreError>`
  - `list_env_backups() -> Result<Vec<EnvBackupInfo>, CoreError>`
  - `preview_env_backup(file: String) -> Result<RestorePreview, CoreError>`
  - `restore_env_backup(file: String, force: bool) -> Result<RestoreOutcome, CoreError>`

- [ ] **Step 1: 写失败测试**

创建 `tests/unit/env-backup.test.ts`：

```ts
import { describe, it, expect } from 'vitest';
import { summarizePreview, type RestorePreview } from '@/core/env-backup';

function preview(over: Partial<RestorePreview> = {}): RestorePreview {
  return { changes: [], added: 0, modified: 0, removed: 0, conflicts: 0, ...over };
}

describe('备份差异摘要', () => {
  it('无差异时返回「无变化」文案', () => {
    const s = summarizePreview(preview());
    expect(s.hasChanges).toBe(false);
    expect(s.text).toContain('无');
  });

  it('按新增/修改/删除分别计数并拼接文案', () => {
    const s = summarizePreview(preview({ added: 2, modified: 1, removed: 3 }));
    expect(s.hasChanges).toBe(true);
    expect(s.text).toContain('新增 2');
    expect(s.text).toContain('修改 1');
    expect(s.text).toContain('删除 3');
  });

  it('有冲突时给出显式提示（默认模式会中止）', () => {
    const s = summarizePreview(preview({ conflicts: 1 }));
    expect(s.hasConflicts).toBe(true);
    expect(s.text).toContain('冲突');
  });

  it('列出将被删除的变量名——这是最不可逆的部分', () => {
    const s = summarizePreview(
      preview({
        removed: 1,
        changes: [{ hive: 'user', name: 'OLD_VAR', kind: 'removed' }],
      }),
    );
    expect(s.removedNames).toEqual(['OLD_VAR']);
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

Run: `npx vitest run tests/unit/env-backup.test.ts`
Expected: FAIL —— `Failed to resolve import "@/core/env-backup"`。

- [ ] **Step 3: 写最小实现**

创建 `src/core/env-backup.ts`：

```ts
/**
 * 环境变量备份的纯展示逻辑（零 React / Tauri 依赖）。
 *
 * 差异计算在 Rust 侧完成；本模块只把结果转成用户可读的摘要，
 * 不重复实现任何判定。
 */

export type RestoreChangeKind = 'added' | 'modified' | 'removed' | 'conflict';
export type EnvHive = 'system' | 'user';

export interface RestoreChange {
  hive: EnvHive;
  name: string;
  kind: RestoreChangeKind;
}

export interface RestorePreview {
  changes: RestoreChange[];
  added: number;
  modified: number;
  removed: number;
  conflicts: number;
}

export interface EnvBackupInfo {
  file: string;
  path: string;
  timestamp: string;
  sizeBytes: number;
  variableCount: number;
}

export interface BackupSummary {
  hasChanges: boolean;
  hasConflicts: boolean;
  /** 将被删除的变量名——删除最不可逆，确认弹窗必须单独高亮 */
  removedNames: string[];
  text: string;
}

/** 把差异预览转成摘要；无变化时给出「无变化」而非空字符串。 */
export function summarizePreview(preview: RestorePreview): BackupSummary {
  const parts: string[] = [];
  if (preview.added > 0) parts.push(`新增 ${preview.added}`);
  if (preview.modified > 0) parts.push(`修改 ${preview.modified}`);
  if (preview.removed > 0) parts.push(`删除 ${preview.removed}`);
  if (preview.conflicts > 0) parts.push(`冲突 ${preview.conflicts}`);

  const hasChanges = preview.added + preview.modified + preview.removed > 0;
  const hasConflicts = preview.conflicts > 0;

  let text: string;
  if (parts.length === 0) {
    text = '与当前环境变量无变化';
  } else if (hasConflicts) {
    text = `${parts.join('、')}；其中冲突项在默认模式下会中止恢复`;
  } else {
    text = parts.join('、');
  }

  return {
    hasChanges,
    hasConflicts,
    removedNames: preview.changes.filter((c) => c.kind === 'removed').map((c) => c.name),
    text,
  };
}

/** 人类可读的文件大小。 */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
```

`gui/src/commands/backup.rs` 追加 4 个命令：

```rust
/// 立即创建一份环境变量备份，返回备份文件路径。
///
/// # Returns
/// - `Ok(String)` — 备份文件绝对路径
/// - `Err(CoreError)` — 采集或落盘失败
#[tauri::command]
pub fn backup_env_vars() -> Result<String, CoreError> {
    backup::backup_env_vars().map(|p| p.to_string_lossy().into_owned())
}

/// 列出已有的环境变量备份（按时间倒序，不解析内容）。
///
/// # Returns
/// - `Ok(Vec<EnvBackupInfo>)` — 备份列表
/// - `Err(CoreError)` — 目录枚举失败
#[tauri::command]
pub fn list_env_backups() -> Result<Vec<EnvBackupInfo>, CoreError> {
    backup::list_env_backups()
}

/// 计算备份相对当前注册表的差异（不写任何内容）。
///
/// # Returns
/// - `Ok(RestorePreview)` — 差异摘要
/// - `Err(CoreError)` — 路径非法、读取/解析失败
#[tauri::command]
pub fn preview_env_backup(file: String) -> Result<RestorePreview, CoreError> {
    let path = backup::validate_backup_path(&file)?;
    backup::preview_restore_file(&path)
}

/// 从备份文件恢复环境变量。
///
/// # Returns
/// - `Ok(RestoreOutcome)` — 恢复结果（含逐条失败）
/// - `Err(CoreError)` — 路径非法、读取失败，或默认模式下检测到冲突（code=`Conflict`）
#[tauri::command]
pub fn restore_env_backup(file: String, force: bool) -> Result<RestoreOutcome, CoreError> {
    let path = backup::validate_backup_path(&file)?;
    backup::restore_env_backup_from(&path, force)
}
```

`gui/src/lib.rs` 的 `generate_handler!` 列表补：

```rust
            commands::backup::backup_env_vars,
            commands::backup::list_env_backups,
            commands::backup::preview_env_backup,
            commands::backup::restore_env_backup,
```

`src/services/backend.ts` 追加（含运行时形状校验，遵循既有 `parsePathCapabilities` 的做法）：

```ts
  /** 立即创建一份环境变量备份，返回备份文件路径。 */
  backupEnvVars: () => invoke<string>('backup_env_vars'),
  /** 列出已有的环境变量备份（按时间倒序）。 */
  listEnvBackups: () => invoke<EnvBackupInfo[]>('list_env_backups'),
  /** 计算备份差异，用于恢复确认弹窗。 */
  previewEnvBackup: (file: string) => invoke<RestorePreview>('preview_env_backup', { file }),
  /** 执行恢复。force=false 时遇冲突返回 code='conflict'。 */
  restoreEnvBackup: (file: string, force: boolean) =>
    invoke<RestoreOutcome>('restore_env_backup', { file, force }),
```

创建 `src/components/dialogs/EnvBackupDialog.tsx`：展示 `listEnvBackups()` 结果；点击某行 → `previewEnvBackup(file)` → 用 `summarizePreview` 渲染摘要，**单独列出将被删除的变量名**；确认走 `backend.confirmDialog`（**异步对话框，不用 `window.confirm`**，与本波次已完成的关窗修复保持一致）；确认后 `restoreEnvBackup(file, false)`，遇 `code === 'conflict'` 时提示「备份后有外部修改」并提供「强制覆盖」二次确认（`force = true`）。恢复确认弹窗还须展示**手工兜底提示**（`patheditor backup` / `patheditor env backup` 两条命令，spec §手工兜底出口）。

在「全部变量」视图工具栏加「备份与恢复」入口按钮。

**J2（核对轮裁断，属验收标准 5 范围）：`WriteOutcome.backup` 的前端消费链路必须一并接线。** 现状实证：

- `src/services/backend.ts:303-308` 三个 env 写方法都是 `invoke<void>`；
- `src/store/env-store.ts:159/186/202` 用 `await` 丢弃返回值，直接设 `statusMessage: i18n.t('status.saved')`。

→ 不改这三处，`WriteOutcome.backup` 产生但**永不消费**，GUI 状态栏不会显示任何备份失败警告。

改动：

```ts
// src/core/env-var.ts —— 与 Rust backup::BackupOutcome 的 serde 形状对齐
// （外部标签枚举：Created → {"created":"路径"}；Skipped → "skipped"；Failed → {"failed":"原因"}）
export type BackupOutcome = { created: string } | 'skipped' | { failed: string };

export interface WriteOutcome {
  backup: BackupOutcome;
}

/** 备份是否失败；未知形状（未来新增变体）按「非失败」处理，不影响写入成功语义。 */
export function backupFailed(backup: BackupOutcome): string | null {
  return typeof backup === 'object' && 'failed' in backup ? backup.failed : null;
}
```

```ts
// src/services/backend.ts —— 三个方法返回类型由 void 改为 WriteOutcome
updateEnvVar: (hive, name, value, expectedRevision) =>
  parseCoreError(invoke<WriteOutcome>('update_env_var', { hive, name, value, expectedRevision })),
// createEnvVar / deleteEnvVar 同样
```

> **形状校验要求**（CLAUDE.md：`backend.ts` 不能只依赖 TS 断言）：`parseCoreError` 之后须加一层运行时校验，确认返回值是对象且含 `backup` 字段，否则抛「IPC 返回形状非法」。参照既有 `parsePathCapabilities` 的做法。

```ts
// src/store/env-store.ts —— 接住返回值，备份失败时换状态文案
const { backup } = await backend.updateEnvVar(meta.hive, meta.name, value, meta.revision);
const failed = backupFailed(backup);
set({
  draft,
  isSaving: false,
  statusMessage: failed
    ? i18n.t('status.saved_without_backup') // 复用既有键，不新增（J1a）
    : i18n.t('status.saved'),
});
```

`create` / `remove` 两处同样处理。**注意**：备份失败不改变操作成功的判定——`return true` 照旧。

- [ ] **Step 4: 跑测试确认通过**

Run: `npx vitest run tests/unit/env-backup.test.ts`
Expected: 4 passed。

再跑 `npm run build` 确认 tsc 通过；`npx eslint src/core/env-backup.ts src/components/dialogs/EnvBackupDialog.tsx` 零错误。

- [ ] **Step 5: 提交**

```bash
git add gui/src/commands/backup.rs gui/src/lib.rs src/services/backend.ts \
        src/core/env-backup.ts src/components/dialogs/EnvBackupDialog.tsx \
        tests/unit/env-backup.test.ts
git commit -m "feat(gui): 备份与恢复界面，含差异预览与冲突二次确认"
```

---

### Task 9: 文档同步与收口

**Files:**

- Modify: `CLAUDE.md`、`AGENTS.md`、`README.md`、`CHANGELOG.md`
- Modify: `docs/审核和开发/2026.09.19/PathEditor-备份体系未覆盖环境变量登记.md`（关闭登记）
- Create: `docs/审核和开发/2026.09.21/PathEditor-环境变量备份恢复开发回执.md`

**Interfaces:**

- Consumes: 前 8 个任务的全部产出
- Produces: 开发回执（供审核窗口复审）

- [ ] **Step 1: 同步 CLAUDE.md / AGENTS.md**

在「CLI 命令」一节补：

```text
patheditor env backup  [--json]
patheditor env backups [--json]
patheditor env restore <FILE> [--dry-run] [--force] [--json]
```

在「Tauri IPC 接口」表补 4 行（`backup_env_vars` / `list_env_backups` / `preview_env_backup` / `restore_env_backup`）。

在「数据、保存与事务」一节补一段：

```markdown
- 通用环境变量写入（CLI 3 命令、GUI 3 命令）前自动产生一份 JSON 备份，落在 `~/.patheditor/backups/env_backup_<时间戳>.json`，含两个 hive 全部可写变量的**明文值**与 revision。备份失败**不阻断写入**，只记警告并如实返回（`WriteOutcome.backup`）。默认保留最近 20 份（`backup::ENV_BACKUP_KEEP`），轮换只删除本功能生成的 `env_backup_*.json`。恢复默认在 revision 冲突时中止（退出码 3）且零写入；`--force` 跳过 revision 校验但**不豁免**保护名单与类型判定。
```

- [ ] **Step 2: 校验双文档字节一致**

Run:

```powershell
if ((Get-FileHash CLAUDE.md).Hash -ne (Get-FileHash AGENTS.md).Hash) { throw "CLAUDE.md 与 AGENTS.md 不一致" }
```

Expected: 无输出（一致）。

- [ ] **Step 3: 更新 README 与 CHANGELOG**

`README.md` 的备份章节补 env 备份说明，**必须包含敏感值提示**（设计文档 §S2.2）：

> ⚠️ `~/.patheditor/backups/` 下的 `env_backup_*.json` 含环境变量的**明文值**，可能包括 API key、token 等敏感信息。请勿将该目录同步到云端或提交到版本库。

`CHANGELOG.md` 顶部加 5.1.4 段落（沿用既有中文小节标题风格）：

```markdown
## 5.1.4

### 新增

- 环境变量备份：CLI 与 GUI 的每一次环境变量写入前自动备份到 `~/.patheditor/backups/env_backup_<时间戳>.json`，含两个 hive 的全部可写变量与注册表类型。
- 环境变量恢复：新增 `patheditor env restore <FILE>` 与 GUI 备份恢复界面，支持差异预览（新增 / 修改 / 删除）与冲突保护。

### 变更

- 环境变量写入口（CLI `env set/add/remove`、GUI 编辑/新建/删除）返回值携带备份结果；备份失败不阻断写入，仅在 stderr 或状态栏提示。

### 修复

- GUI 关窗死锁：关窗确认从阻塞式 `window.confirm` 改为 Tauri 异步对话框，并补齐 `core:window:allow-destroy` 权限（v5.1.3 中缺失导致无草稿关窗挂起）。
- gui/cli 产物同名冲突：GUI 二进制改名 `PathEditor.exe`，CI 用独立 target 目录构建 CLI，避免 NTFS 大小写不敏感导致 portable zip 装到 CLI。

### 说明

- `~/.patheditor/backups/` 下的 env 备份含敏感值**明文**，请勿同步到云端或提交到版本库。
- 备份默认保留最近 20 份，旧的自动轮换删除；只删除本工具生成的 `env_backup_*.json`。
```

- [ ] **Step 4: 关闭缺口登记**

修改 `docs/审核和开发/2026.09.19/PathEditor-备份体系未覆盖环境变量登记.md`，在文件顶部状态行与两处缺口条目上标注：

```markdown
> **状态更新（2026-09-21）**：两项缺口已由 5.1.4 的「环境变量备份与恢复」特性关闭。
> 本文件保留为历史记录，不再作为待办跟踪。
```

- [ ] **Step 5: 跑全量质量门**

Run: `npm run verify:all`
Expected: 全绿（Prettier / ESLint / tsc / vite build / 覆盖率 ≥80% / `cargo fmt` / Clippy 零警告 / Rust 测试 / Playwright）。记录精确数字（**Vitest 用例数、Rust passed/ignored、E2E passed**）——回执中的数字必须与命令输出逐项对应，注明口径与时间点（连续三波出现过回执计数与实测不符）。

额外确认：`cargo test -p patheditor-cli --bins` 独立通过。

- [ ] **Step 6: 写开发回执**

创建 `docs/审核和开发/2026.09.21/PathEditor-环境变量备份恢复开发回执.md`，按既有回执结构（参照 `docs/审核和开发/2026.09.20/PathEditor-Wave2架构收口开发回执.md`）包含：

- 任务映射（T1–T9 → commit）
- 质量门终态数字（含口径声明：worktree 路径、工具链、时间点）
- **Execution Notes**：计划原文 / 实际 / 处理 三列表
- **未覆盖项**（如实列明，不得美化）：至少应包含——真实注册表恢复闭环未做（需用户授权）、备份目录 ACL 收紧是否落地、GUI 手工冒烟未执行、CI 环境验证待发版
- **需要审核窗口重点关注的项**

- [ ] **Step 7: 提交**

```bash
git add CLAUDE.md AGENTS.md README.md CHANGELOG.md \
        "docs/审核和开发/2026.09.19/PathEditor-备份体系未覆盖环境变量登记.md" \
        "docs/审核和开发/2026.09.21/PathEditor-环境变量备份恢复开发回执.md"
git commit -m "docs: 同步 env 备份恢复的命令与 IPC 文档，落盘开发回执"
```

---

## Self-Review

### 规格覆盖

| Spec 章节                                                 | 落地任务                                                         |
| --------------------------------------------------------- | ---------------------------------------------------------------- |
| 文件格式规格（文件名 / 结构 / 字段）                      | Task 1（payload）、Task 2（文件名与信封）                        |
| 不备份的内容（Unsupported / Path）                        | Task 1 Step 1 测试                                               |
| CLI`env backup`                                           | Task 7                                                           |
| CLI`env restore`（含 `--dry-run`/`--force`/退出码 0/1/3） | Task 6 + Task 7                                                  |
| CLI`env backups`                                          | Task 4 + Task 7                                                  |
| GUI 4 个 Tauri 命令                                       | Task 8                                                           |
| K1 备份挂在 core 写函数内部                               | Task 3                                                           |
| K2 备份失败不阻断                                         | Task 3 Step 1 第二个测试                                         |
| K3 恢复默认走 revision 校验                               | Task 6 Step 1 第一个测试                                         |
| K4 复用 Versioned 信封                                    | Task 2 Step 1 第一个测试                                         |
| K5 轮换只删自己的文件                                     | Task 2 Step 1 对抗性测试                                         |
| S1 自动删除的授权与限界                                   | Task 2 对抗性测试（4 类干扰文件）                                |
| S2 明文暴露面缓解                                         | Task 9 Step 3（README 提示）；**ACL 收紧未指派任务**——见下方缺口 |
| S3 判定仍在 core                                          | Task 8（GUI 命令只做参数转换）；Task 7（CLI 只做透传）           |
| S4 恢复路径校验                                           | Task 4 Step 1 第三个测试                                         |
| S5 列表不读内容                                           | Task 4 Step 1 第一个测试                                         |
| S6 恢复走 core API 不用 set_raw                           | Task 6 Step 1 第三个测��（保护名单）                             |
| 验收标准 1–15                                             | 逐条对应上述任务；第 14 条（双文档一致）在 Task 9 Step 2         |
| 测试策略（MemoryHive / 目录重定向 / CLI`--bins`）         | 各任务的`with_temp_backup_dir` 与 Step 4 命令                    |

**发现缺口**：spec §S2.1 的「备份目录权限收紧（ACL）」没有对应任务。这是有意的——ACL 收紧在 Windows 上需要 `icacls` 或 Win32 API，实现复杂度未评估，spec 已允许「若实现复杂度过高，至少在 spec 范围外明确登记」。**处理**：Task 9 Step 6 的回执「未覆盖项」必须显式列出该条，不得静默略过。

**发现缺口 2**：spec 原先未规定 `preview_restore` 的公开包装签名，Task 7 的 CLI 代码依赖它。**核对轮 E3 已裁断**（spec §公开 API 签名已补）：`pub fn preview_restore(payload: &EnvBackupPayload) -> Result<RestorePreview, CoreError>`（**无 force 参数**）与 `pub fn preview_restore_file(path: &Path) -> Result<RestorePreview, CoreError>` 两个包装由 **Task 5 一并产出**；`preview_restore_in_stores` 改为 `pub(crate)`。

**发现缺口 3（核对轮新增）**：`config.ini` 的保留数覆盖已折入 **Task 2 Step 2b/3**，spec 新增 §配置文件 一节。注意 `config.ini` **不用** `persist::Versioned` 信封（面向用户手工编辑，加版本头破坏可读性）。

**发现缺口 4（核对轮新增）**：spec §S2.1 的 ACL 收紧按 C5 裁断**登记为未覆盖项**，Task 9 Step 6 的回执必须显式列出。

### 占位符扫描

无「TBD」「TODO」「适当处理错误」「参考 Task N」类占位。四处标注「实现提示」的段落给出了明确的行为要求与判定标准，允许开发窗口按实际代码结构裁剪写法，但**行为不可裁剪**（每处都写明了必须成立的性质）。

### 类型一致性

| 符号                                                                  | 定义处 | 使用处                                                             | 一致 |
| --------------------------------------------------------------------- | ------ | ------------------------------------------------------------------ | ---- |
| `EnvBackupVar { name, kind, value, revision }`                        | Task 1 | Task 2 / 5 / 6                                                     | ✅   |
| `EnvBackupPayload { captured_at, hives }`                             | Task 1 | Task 2 / 5 / 6                                                     | ✅   |
| `BackupOutcome { Created, Skipped, Failed }`                          | Task 3 | Task 3 / 7 / 8                                                     | ✅   |
| `WriteOutcome { backup }`                                             | Task 3 | Task 3 / 7 / 8                                                     | ✅   |
| `EnvBackupInfo { file, path, timestamp, size_bytes, variable_count }` | Task 4 | Task 7 / 8（TS 侧`sizeBytes` / `variableCount` 为 camelCase 镜像） | ✅   |
| `RestoreChangeKind { Added, Modified, Removed, Conflict }`            | Task 5 | Task 5 / 6 / 7 / 8                                                 | ✅   |
| `RestorePreview { changes, added, modified, removed, conflicts }`     | Task 5 | Task 6 / 7 / 8                                                     | ✅   |
| `RestoreOutcome { applied, skipped, failures }`                       | Task 6 | Task 7 / 8                                                         | ✅   |
| `ENV_BACKUP_KEEP`                                                     | Task 2 | Task 2                                                             | ✅   |
| `validate_backup_path(path: &str)`                                    | Task 4 | Task 6 / 7 / 8                                                     | ✅   |

**注意**：TS 侧 `RestoreChange.hive` 用 `'system' | 'user'`（与 `src/core/env-var.ts` 的 `EnvHive` 一致），Rust 侧 `EnvHive` serde 为 camelCase → `"system"` / `"user"`，两侧对齐。

## Execution Notes

（留给开发窗口回填：计划原文 / 实际实现 / 处理方式 三列）

---

## 核对轮裁断（2026-09-21，已落盘）

开发窗口对本计划的异议已逐条核实并折入正文。**下方是裁断记录，不是待办**——计划已收敛，可以开工。

### 采纳并已折入

| #     | 异议                                                            | 裁断与落点                                                                                                                                                                                                                           |
| ----- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| C1    | 5 个写函数返回 `Result<WriteOutcome, CoreError>`                | **维持原设计**（开发窗口未反对，仅提问波及面）。执行时须排查 `Ok(())` 精确匹配：用 `grep -rn "Ok(())" cli/src gui/src core/src` 逐处确认                                                                                             |
| C2    | 备份在「校验通过后、写入前」                                    | **维持**。顺序不可颠倒：校验失败时产生备份文件是无意义的磁盘与暴露面浪费。`prepare_and_backup` 若与实现结构冲突，允许把校验内联各处，但**顺序不可改**                                                                                |
| C3    | `PATHEDITOR_BACKUP_DIR` 环境变量重定向测试                      | **维持**：仓库既有做法（`scanner.rs` 用临时目录 + `persist::test_persist_lock()`）与此一致，无更优先例                                                                                                                               |
| C4    | Task 5 `diff_one_hive` 的行为契约式表述                         | **采纳**，并已按 P1 修正为「显式说明必须自写」                                                                                                                                                                                       |
| C5    | ACL 收紧登记为未覆盖项                                          | **成立**，不扩任务。spec S2.1 已标注，验收标准与范围外已同步                                                                                                                                                                         |
| C6/E3 | `preview_restore` 签名去掉 `force`                              | **成立**：差异计算与 force 无关。spec 已补 §公开 API 签名；Task 5 补两个公开包装；Task 7 调用已改                                                                                                                                    |
| C7    | 恢复不产生新备份 + 手工兜底提示                                 | **成立并补强**：提示**不得**做成自动调用（自动备份会与保留策略互相吞噬）。Task 7 已加 eprintln 指引；spec 新增 §手工兜底出口                                                                                                         |
| E5    | 保护名单不预排除，走「写入被拒 + 记入 failures」                | **成立**：在 preview 里复制保护名单判定等于双份规则，违反「判定只在 core 一处」。Task 6 测试已按真实行为重写（新增 `preview_includes_protected_names`，并让 `restore_force_does_not_bypass_protected_names` 断言「不中止其余恢复」） |
| P1    | `raw_name_of(store, key)` 全库不存在 + `current` 解构类型不匹配 | **成立**：计划代码的实质缺陷。Task 5 已改为「行为契约 + 显式禁止照抄」，并把 `current` 值类型修正为 `(原始名, revision)`                                                                                                             |
| P2    | `preview_restore` 调用与产出的签名冲突、两处表述不一致          | **成立**（与 E3 同源）。Task 7 与 Self-Review 缺口 2 已改齐                                                                                                                                                                          |
| P3    | `persist` 的信封类型是 `pub(crate)`，gui/cli 不可用             | **成立**：`core/src/lib.rs:8` 实证。已加进 Global Constraints                                                                                                                                                                        |
| —     | 配置文件落 `~/.patheditor/config.ini`（方案 A）                 | **成立**（用户已定）：双 exe 共享、升级不丢、免提权。spec 新增 §配置文件；Task 2 新增 Step 2b + 实现                                                                                                                                 |

### 驳回（附实证）

| #     | 异议                                                                                | 裁断                               | 实证                                                                                                                                                                                                                                                                                                                                                                         |
| ----- | ----------------------------------------------------------------------------------- | ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E1/E4 | 开发窗口**自行撤回**，改称「`crate::registry::env_var::X` 全路径在同 crate 内合法」 | **驳回撤回——原异议成立，撤回错误** | `core/src/registry.rs:14` 是 `mod env_var;`（**私有**）。独立 crate 最小实验复刻同形结构后报 `error[E0603]: module env_var is private`；改经 `registry` 根 re-export 则编译通过。计划 Task 1 原写的 `use crate::registry::env_var::{hive_location, read_env_var};` **会编译失败**，已修正为 `use crate::registry::{hive_location, read_env_var};` 并补 `pub(crate) use` 要求 |

### E1/E4 的实验记录（决定性证据）

```rust
// 复刻 PathEditor 的模块结构
pub mod registry {
    mod env_var;                                                 // 与 registry.rs:14 同形（私有）
    pub(crate) use access::hive_location;
    pub use env_var::pub_fn;
}
mod backup {
    pub fn a() { crate::registry::hive_location(); }             // 经 registry 根：✅
    pub fn b() { crate::registry::env_var::read_env_var(); }     // 经私有子模块：❌ E0603
}
```

```text
error[E0603]: module `env_var` is private
  --> src\lib.rs:14:43
   |
14 |     pub fn b() { crate::registry::env_var::read_env_var(); }
   |                                   ^^^^^^^ private module
```

**结论**：即使 `*_in_store` 函数改成 `pub(crate)`，**也必须在 `registry.rs` 加 `pub(crate) use`**，否则因模块私有而不可达。Task 1 与 Task 6 已双双写明。

### 撤回确认无误者

E6（`EnvValueKind` serde 形状）——`#[serde(rename_all = "camelCase")]`（`core/src/env_var.rs:50-59`）确实产出 `"string"` / `"expandString"`，与 spec 一致，撤回正确。

---

## 核对轮裁断（第二轮，2026-09-21）

开发窗口第二轮提出 3 阻断 + 3 判断意见（交接文档 `docs/审核和开发/2026.09.21/PathEditor-环境变量备份恢复核对轮交接.md`）。**逐条独立核实后全部成立**——其中 B1、B2、J1a、J3 直接指正了审核窗口的计划错误。裁断已全部折入上方正文。

### 阻断项（3）

| #      | 异议                                                         | 计划落点                                                                                                                                                                                                        |
| ------ | ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **B1** | Task 1 采集用 `read_env_var` → 遇 `Unsupported` 整次备份失败 | Task 1 Step 1 测试改为**先断言 `Ok`** + 新增 `keeps_others_when_unsupported_present`；Step 3 实现改用 `store.get_raw` + `is_writable()` 跳过（与 `list_env_vars_in_store` 同形）；`use` 区不再引 `read_env_var` |
| **B2** | 泛型化漏了 `apply_concurrency`（`cli/src/env_ops.rs:120`）   | Task 3 Step 3 补「6 处改动清单」表格                                                                                                                                                                            |
| **B3** | Task 7 测试用了跨 crate 不可达的 `test_persist_lock`         | Task 7 Step 1 删取锁行，注明「`--bins` 是独立进程，进程内锁跨进程无意义」                                                                                                                                       |

### 判断意见（3）

| #           | 异议                                      | 计划落点                                                                                                                                                                                    |
| ----------- | ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **J1a/J1b** | i18n 键名错；CLI 无 logger 导致警告不出现 | 统一用 `status.saved_without_backup`（不新增键）；新增 `warn_if_backup_failed` 辅助，由返回值显式 `eprintln!`                                                                               |
| **J2**      | 验收标准 5 的 GUI 半边无任务承载          | Task 8 补前端接线：`src/core/env-var.ts` 加 `BackupOutcome`/`WriteOutcome` 类型 + `backupFailed()`；`backend.ts` 三方法返回 `WriteOutcome`（含运行时形状校验）；`env-store.ts` 消费并换文案 |
| **J3**      | `validate_write` 不存在且抽象不成立       | 删除 `prepare_and_backup`，改为「只抽备份（`backup_before_write`）、不抽校验」；采方案甲，接受公开包装与 `*_in_store` 各写一份校验，`*_in_store` 那份是防御性兜底                           |

### 一致性观察（已采纳）

- `EnvBackupHives` 补 `#[serde(rename_all = "camelCase")]`，与 `EnvVarSnapshot`、`EnvBackupPayload` 风格统一。
- 修 B1 后，Task 5 的 `diff_one_hive` 与 Task 1 的采集口径自然统一（都走 `get_raw` + 跳过 `Unsupported`）——**修 B1 时须确认两处一致**。

### 独立于本波：v5.1.3 CLI 事故

开发窗口发现的「已发布的 v5.1.3 CLI 资产实为 GUI 二进制」，审核窗口已独立复核证实（两处文件 sha256 相同 = `0bdc814a…`，PE 子系统 `0x0002` GUI）。**根因已在 gui-fix 波次（`17777a8`）修好，5.1.4 不会再犯**；处置口径与发布后清单见 spec §「紧急发现」与「发布后处置清单」。**不阻塞本波开发。**

### 状态

**计划已收敛，可以开工。** 执行前请再确认一次：本波不出 release、不推送、不升版本号、不提交 bucket；全程不写真实注册表。
