# F-08 收口 — 核对轮裁断（审核窗口 → 开发窗口，第 2 轮）

- **日期**：2026-10-17
- **裁断窗口**：审核窗口
- **交付对象**：开发窗口
- **裁断对象**：`docs/审核和开发/2026.10.17/PathEditor-F08收口核对轮交接.md`（开发窗口第 1 轮）
- **用户本轮授权**：删除 F-2 三模块 + F-6 三命令（已授权）；**扩权删除 CLI 三个死函数**（本轮新授权）；改代码 + 真机回归（已授权）；提交 / 推送 / 升版本（**未授权**）。

> 开发窗口本轮核对**全部成立**：N1/N2/N3 三条阻塞 + N4–N10 五条修补，审核窗口逐条复现，**无一失实**。本裁断**接受 N4–N10 全部**，对 A–E 逐项定夺。

---

## 一、总裁断

| 待裁断                                 | 裁断                                                                   | 与开发窗口倾向                             |
| -------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------ |
| **A**（契约测试落点）                  | **选 (ii) 加注入缝**（缝定义在 `service.rs`，`registry/path.rs` 不动） | **不一致**（开发窗口倾向 (i)），理由见 §二 |
| **B**（Task 4 扩到 6 入口）            | **扩**                                                                 | 一致                                       |
| **C**（`clean` 缺 `persist_snapshot`） | **本波顺带修 + 声明为行为变化**                                        | 一致                                       |
| **D**（CLI 新增备份文件）              | **接受 + 记 CHANGELOG `### 变更` + README**                            | 一致                                       |
| **E**（CLI 死函数删除）                | **扩权删除**（用户已授权）                                             | 一致                                       |

**N4–N10 全部接受**，折入修订版 plan。

---

## 二、待裁断 A：为什么选 (ii) 而非 (i)

开发窗口倾向 (i)「抽纯函数 + 端到端移真机回归」。**审核窗口不同意**，理由来自开发窗口自己的分析：

> N1 证明「三条断言都做不到真机外验证」。而 (i) 把**成功路径**推给**手动真机回归**。

**这正是 5.1.5 用 SP-1 刚关掉的缺口类型**——「关键写路径只有手工验证，无回归防护」（见 `PathEditor-env子命令可用性收口审查报告.md` §三 SP-1）。F-08 收口的**全部价值**就是「唯一编排」；若这条编排的成功路径不可单测，则每次改动它都只能靠手工回归——**等于把 SP-1 的缺口从 env 复制到 PATH**。不能再开一个。

**裁断：选 (ii)，但按仓库既有模式落地，避免扩进 F-3 领地。**

- 缝**定义在 `core/src/service.rs` 内**（`pub(crate) trait`），真实现转调 `registry::load_*_paths` / `save_*_paths` / `backup::backup_registry`。**`registry/path.rs` 零改动**——故不触及 F-3（`String` 错误契约）领地，开发窗口 N1 表里 (ii) 的「扩大本波范围」顾虑**不成立**。
- 这是仓库**已有模式**的直接复用：`registry/env_var.rs` 的 `X` → `X_with_store(store: &dyn EnvHiveStore, ...)`（`env_var.rs:489/504/530`）。PATH 侧只是补上对应的 `_with_io`。
- **不是过度设计**：真注册表 + 内存 store = **两个适配器 = 真实缝**（`codebase-design` 判据）。

**缝的形态（spec 级；开发窗口有自由度）**：

```rust
/// PATH 读写的可注入缝（仿 `EnvHiveStore` / `registry/env_var.rs` 的 `_with_store` 模式）。
pub(crate) trait PathHiveIo {
    /// 读取某 hive 当前启用路径。
    fn read_enabled(&self, hive: EnvHive) -> Result<Vec<String>, CoreError>;
    /// 写入某 hive 启用路径。
    fn write_enabled(&self, hive: EnvHive, paths: &[String]) -> Result<(), CoreError>;
    /// 写前备份（真实现 → `backup::backup_registry(None)`）。
    fn backup(&self) -> Result<(), CoreError>;
}

/// 公开入口（真实现）。
pub fn save_path_with_sidecar(system: Option<PathHiveWrite>, user: Option<PathHiveWrite>)
    -> Result<ApplyOutcome, CoreError>;

/// 核心编排，IO 可注入（测试用；契约测试断言经此，**全程不碰真实注册表**）。
pub(crate) fn save_path_with_sidecar_io(io: &dyn PathHiveIo, system: ..., user: ...)
    -> Result<ApplyOutcome, CoreError>;
```

**契约测试断言（经 fake `PathHiveIo`，零真实注册表）**：

1. `backup_called_once_when_write_happens`——有写入时 `backup()` 被调用一次；无变更时不调用。
2. `concurrent_mismatch_writes_nothing_and_no_backup`——`expected` 不符 → 该 hive 不 `write_enabled`、不 `backup`、不产 pending。
3. `writes_are_sequential_system_then_user`——fake 记录调用顺序，断言 system 先于 user。

> 这样三条断言**全部真机外可验**，且**不写真实注册表**（Global Constraint 与 K5 的冲突由此消解）。

---

## 三、待裁断 B：Task 4 扩到全部 6 个入口

**采纳开发窗口 N2/N3 的完整重写形态**：

1. **6 个入口**（`load_and_save` / `load_operate_save` / `clean_one` / `cmd_toggle` / `cmd_import` / `profile_apply`）逐个改为**一次 `save_path_with_sidecar` 调用**（带 `PathHiveWrite { entries, expected }`）。
2. **删除 N6 的 7 处调用方广播**（`main.rs:368/407/426/507`、`import_export.rs:40`、`profile_ops.rs:67`、`runtime.rs:119`）——广播下沉 core。
3. **删除 3 个死函数**（E，已授权）：`verify_and_save`（`runtime.rs:53`）、`persist_snapshot`（`runtime.rs:164`）、`flush_pending_snapshot`（`runtime.rs:194`）。
4. **退出码不变**：并发冲突仍 `exit_err`（退出码 1），**不**改成 3。

---

## 四、待裁断 C / D

- **C**：`clean_one`（`main.rs:459`）收口到 `save_path_with_sidecar` 后**由构造**补上 sidecar 提交（修掉「clean 后快照陈旧」的既有 bug）。**声明为行为变化**，记 CHANGELOG `### 修复`。
- **D**：CLI 每次成功写 PATH 将产生 `path_backup_*.txt`（默认目录、保留 20 份）。**接受**（与 env 侧对称），**必须声明**——记 CHANGELOG `### 变更` + README。**这不是等价迁移**，如实说。

---

## 五、N4–N10 处置

| #   | 处置                                                                                                                                              |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| N4  | 见 §四 C：顺带修 + 声明                                                                                                                           |
| N5  | 见 §四 D：接受 + 声明                                                                                                                             |
| N6  | 见 §三.2：删 7 处调用方广播                                                                                                                       |
| N7  | Task 5 Files **补入 `tests/unit/app-store.test.ts`**（`:265-266/:281-282` 的 `mockResolvedValueOnce` 队列会因命令退役而错位）                     |
| N8  | Task 7 Step 1 的 grep 范围写明**含 `tests/unit/`**（不只 e2e）                                                                                    |
| N9  | Task 4 构造 `PathHiveWrite { entries }` 时，`enabled: true` 的等价性**写死注释**（输入本就是 `enabled_paths` 的启用集）                           |
| N10 | Task 3 移动 `save_path_with_sidecar` 后，**复核 `core/src/registry/golden/README.md:62` 的行号引用**（该引用已漂移 8 行，属既有小问题，顺带校正） |

---

## 六、spec 增补（本轮折入）

| #       | 增补         | 内容                                                                                       |
| ------- | ------------ | ------------------------------------------------------------------------------------------ |
| **D13** | 注入缝       | `service.rs` 加 `PathHiveIo` + `save_path_with_sidecar_io`（§二）；`registry/path.rs` 不动 |
| **D14** | clean 修复   | `clean_one` 收口顺带修 sidecar 陈旧（行为变化，记 CHANGELOG `### 修复`）                   |
| **D15** | CLI 备份声明 | CLI 每次写 PATH 产备份（行为变化，记 CHANGELOG `### 变更` + README）                       |
| **D16** | 删除授权扩展 | 删 CLI `verify_and_save` / `persist_snapshot` / `flush_pending_snapshot`（用户已授权）     |
| **K7**  | 契约测试经缝 | 三条断言经 fake `PathHiveIo`，零真实注册表（消解 K5 与 Global Constraint 的冲突）          |

---

## 七、交接链状态

| 文档                                                                     | 状态                                                              |
| ------------------------------------------------------------------------ | ----------------------------------------------------------------- |
| 评审报告 / 第 1 轮交接 / 第 1 轮裁断                                     | ✅ 已在库                                                         |
| spec（`2026-10-17-f08-closeout-and-deadcode-removal-design.md`）         | ✅ 已增补 D13–D16 / K7                                            |
| plan（`2026-10-17-f08-closeout-and-deadcode-removal-implementation.md`） | ✅ **已按本裁断修订**（Task 1 加缝、Task 4 覆盖 6 入口 + 删死码） |
| 本裁断（第 2 轮）                                                        | ✅ 本文件                                                         |
| 开发回执                                                                 | ⏳ 待开工后                                                       |

**下一步**：开发窗口按**修订版 plan** 施工（已授权改代码 + 真机回归 + 扩权删除；**未授权提交**），完成后落开发回执。
