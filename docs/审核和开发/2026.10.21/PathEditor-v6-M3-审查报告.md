# PathEditor v6.0.0 M3 审查报告

- **审查日期**：2026-10-21
- **被审对象**：`main` @ `fc62758`，13 个提交（`6520ad1..HEAD`）
- **基准点**：`main @ 6520ad1`（M3 开工提示词）
- **规格来源**：`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`
- **计划来源**：`docs/superpowers/plans/2026-10-21-v6-sqlite-m3-plan.md`
- **回执**：`docs/审核和开发/2026.10.21/PathEditor-v6-M3-开发回执.md`

> **本报告有一处自我更正。** 初稿判定「`_ => Ok(())` 静默吞掉未知 `entityType`，违反 spec §6.4」为阻塞项。
> 逐字复读 `import.rs` 后发现 **第 99-109 行已有白名单校验**，未知类型会正确返回 `SyncFormat`。
> 我初稿只读了 `apply_entity_tx`（`:265-280`）就下结论，漏掉了前面的前置校验。该条已撤回，
> 更正后的结论见 §三。教训见 §七。

---

## 一、验证证据（审核窗口独立执行）

| #   | 命令                                                    | 结果                                             |
| --- | ------------------------------------------------------- | ------------------------------------------------ |
| 1   | `cargo fmt --all -- --check`                            | exit 0                                           |
| 2   | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0                                           |
| 3   | `cargo test --workspace`                                | **47 binaries, 339 passed, 0 failed, 2 ignored** |
| 4   | `npx vitest run`                                        | **33 files, 395 passed**                         |
| 5   | `npm run test:e2e`                                      | **54 passed** (1.6m)                             |
| 6   | `cargo test -p patheditor --lib`                        | **1 passed**（验证 build.rs 修复有效）           |
| 7   | 新增 pub fn 调用者扫描                                  | 12 个全部有生产调用者                            |
| 8   | 工作区                                                  | 干净                                             |

**全部数字与回执一致**。Rust +10、前端 +21、E2E +4，无一项下降。

---

## 二、三项重点核实

### 1. 构建修复（`gui/build.rs` + `test.rc` + `test.manifest`）—— ✅ 有效，有平台边界

**已确认有效**：`cargo test -p patheditor --lib` 实际跑起来了（1 passed）。修复前该二进制以 `0xC0000139` 退出。

**根因判断正确**：`windows-sys` 启用 `Win32_UI_Controls`，导入 `comctl32.dll!TaskDialogIndirect`，该符号只在 v6 存在。Tauri 用 `rustc-link-arg-bins` 只覆盖 bin，lib 的 test harness 拿不到清单。

**`LANGUAGE 0, 0` 的选择正确**：与 Tauri 的 1033 资源身份不同，避免链接器报重复。注释还解释了「为什么不用 `rustc-link-arg-tests`」——那个只覆盖 `tests/*.rs`，不覆盖 lib 的 `--test` 编译。这个区别很容易搞错。

**平台边界（F-4，非阻塞）**：`build.rs` 调用 `windres`（MinGW 工具）。纯 MSVC 环境中不存在 → 只打 `cargo:warning`、不阻断构建 → 测试二进制缺清单 → 运行时 `0xC0000139`。

当前不阻塞（CI 不跑测试；本机有 windres），但这是**静默降级**：警告容易忽略，失败发生在运行时。建议在 `Err` 分支加可操作提示。

### 2. R-1（配置导入不落库）—— ✅ 披露准确，**但严重性被归类为「残余风险」**

**已核实的三段**：

| 环节   | 行为                                                                                                                                      |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| 导出   | `export.rs:106-115` **确实**为 `profile` 构建 content；`profile_revision` / `path_entry` / `env_value` 走 `_ => Ok(None)`（内容随父实体） |
| 写路径 | `profiles.rs` **确实**有 4 处 `record_change_tx`                                                                                          |
| 导入   | `apply_entity_tx`（`import.rs:265-280`）**只处理 `"snapshot"`**，其余 `_ => Ok(())`                                                       |

**净效果**：`import.rs:193` 的 `report.applied += 1` 是**无条件**的，所以含配置的包会被统计为「已应用」，但配置一条都没落库。前端 `sync-store.ts:118` 显示「导入完成：应用 N 条」。

**这不是「少同步了一部分」，是「报告了不存在的成功」。**

回执 §6 R-1 的披露是准确的（「UI 会显示『应用 N 条』，但那 N 条里若含配置，实际没落库」），但被归入「残余风险」。**我认为它应该是合并前必须处理项**——理由不是「功能没做完」（那可以接受），而是**UI 主动告诉用户成功了**。

### 3. `content` 字段的 spec 补充 —— ✅ 成立，同意改 spec

spec §6.4 的 `Change` 只有 `oldValue` / `newValue`，而导出侧依赖 `content` 重建实体（`export.rs:62`）。dev window 的判断正确：只有变更行、没有实体内容，接收端无法重建实体。

**同意修订 spec §6.4**，在 `Change` 定义里增加 `content` 字段并说明语义（可选，仅 `snapshot` / `profile` 携带）。

---

## 三、需要处理的问题（更正后）

### R-1【阻塞】含配置的包报告假成功

**位置**：`core/src/db/sync/import.rs:265-280` + `:193` + `src/store/sync-store.ts:118`

**现状**：

```rust
// import.rs:271-279
match entity_type {
    "snapshot" => { ... }
    _ => Ok(()),        // profile / profile_revision / path_entry / env_value 全部静默跳过
}
// import.rs:193
report.applied += 1;    // 无条件累加
```

**为什么阻塞**：用户导入含配置的包，界面明确显示「应用 5 条」，而其中配置条目**没有任何落库**。这是**主动的误导**，不是被动的缺失。

**三个整改选项**（按推荐排序）：

**A（推荐）**：`apply_entity_tx` 对**已知但未实现**的类型返回 `SyncFormat`，让整个导入失败。

```rust
"profile" | "profile_revision" | "path_entry" | "env_value" => {
    return Err(CoreError::new(
        ErrorCode::SyncFormat,
        "import",
        format!("暂不支持的实体类型: {entity_type}（M3 仅支持 snapshot）"),
    ));
}
```

代价：含配置的包**整体无法导入**（连快照也导不进去）。这是诚实的失败。

**B**：保持部分成功，但 `report.applied` 只统计真正落库的实体数，新增 `skipped_unsupported`，UI 分开显示「落库 M 条 / 跳过 N 条（暂不支持）」。

**C**：只改 UI 文案，把「应用 N 条」改为「处理 N 条」，并在下方注明「配置同步将于 M4 提供」。

> **不建议只做 C**：它仍然是「操作成功」的语气。A 或 B 才让用户知道发生了什么。

**无论选哪个，都要补测试**：断言含 `profile` 变更的包在导入后的行为（报错 / 计数分开 / 明确跳过）。

### F-2【应改】白名单无测试覆盖

`import.rs:99-109` 的实体类型白名单**实现是正确的**（我初稿误判了它不存在）。但**没有测试**：

```bash
grep -rn "未知的实体类型" core/tests/ → 零命中
```

白名单是「防未来版本兼容问题」的唯一闸门，建议补一条：构造 `entityType: "futureType"` 的包，断言返回 `SyncFormat` 且事务回滚。

### F-3【应改】死函数 `_unused`

**位置**：`core/src/db/sync/import.rs:322-323`

```rust
#[allow(dead_code)]
fn _unused(_: &BundleHeader) {}
```

你本轮正确执行了「每个新增 pub fn 必须有生产调用者」并删掉三个零调用者（`latest_for` / `raise_lamport` / `find_by_uid`）。但这个 private 函数用 `#[allow(dead_code)]` 绕过了同一判据，注释是「独立入口留给 M4」。

**「为未来预留」是死代码最常见的借口。** M4 需要时重写成本一样。**建议删除。**

### F-4【应改】store 层硬编码中文

**位置**：`src/store/sync-store.ts:34/36/37/118`

```ts
:34  if (err.code === 'database') return '同步不可用（降级模式）';
:36  if (err.code === 'syncFormat') return err.message || '同步包无效';
:37  return err.message.trim().length > 0 ? err.message : '同步操作失败';
:118 notice: `导入完成：应用 ${report.applied}，跳过 ${report.skipped}，冲突 ${report.conflicts}`,
```

**对照**：`SyncCenter.tsx` 用了 **20 处 `t()`**，locale 的 `sync` 段有 **21 个键**——组件层做对了，store 层漏了。

`:118` 是**导入完成的主反馈**，英文用户会看到整句中文。

**注意这是既有模式**：M2 的 `history-store.ts:26-28` 同样硬编码，`env-store.ts` 用 `i18n.t('error.code.*')`。三种写法并存说明仓库缺统一约定。**M3 至少应把 `:118` 移到 locale。**

### F-5【记录】`build.rs` 的平台边界

见 §二-1。非阻塞，但建议在 `Err` 分支加可操作提示。

---

## 四、合规亮点

- **构建修复的注释质量高**：不只写「怎么修」，还写了「为什么不用 `rustc-link-arg-tests`」「为什么用中性语言」。两个都是容易搞错的点。
- **`find_by_uid` 的处置方式值得记**：它不是简单删除，而是改造成 `change_exists_tx` 接进幂等判据——「让它去它该去的地方」比单纯删除更好。
- **两次反向验证**：把 AAD 收窄成 `salt+nonce`、把 `mark_exported` 提到写文件之前，两次都精确失败在预期的断言行。**这证明了测试有证伪能力**，而不只是「跑绿了」。
- **导出顺序的测试设计正确**：`db_sync_export.rs` 先测失败路径（目录不存在 → 待导出队列必须不变），再测成功路径。失败路径优先是对的——它是顺序反了会永久丢变更的那一侧。
- **R-1 的披露诚实**：README / CHANGELOG / 回执三处都写了，没有藏。
- **`cargo clean` 后的发现很有价值**：GUI 测试二进制从 M1 起就是坏的，只是没人重建过。**一个从没被重编过的测试二进制，永远不会暴露它的启动失败。**

---

## 五、判定

**需要整改后复审。**

三项重点里，构建修复有效、content 补充成立；**R-1 的严重性需要重新定级**。

### 合并前必须处理

1. **R-1**：三选一（A 整体拒绝 / B 计数分开 / C 改文案），**并补测试**。
   我推荐 **A**——含配置的包整体拒绝，比部分成功诚实。

### 建议同批

2. **F-2**：补白名单测试（未知 `entityType` → `SyncFormat` + 回滚）。
3. **F-3**：删掉 `_unused`。
4. **F-4**：把 `sync-store.ts:118` 的导入提示移到 locale。

### 可以合并后做

5. **F-5**：`build.rs` 无 `windres` 时的可操作提示。

---

## 六、本轮未执行（如实列出）

- **两库真实同步的手工验证**：未做。单元测试是构造级别的两库。
- **真实注册表闭环**：未做（需用户授权）。
- **CI（MSVC）**：未跑；且 CI 当前不跑任何测试。
- **R-1 的运行时复现**：未做。判定基于代码阅读——`export.rs` 为 `profile` 建 content，`import.rs` 的 `apply_entity_tx` 只处理 `snapshot`，`applied` 无条件累加。

---

## 七、我自己的错误（必须记录）

初稿把「`_ => Ok(())` 静默吞掉未知 `entityType`」判为阻塞项，并写进了报告。**这个判定是错的**：

- 我只读了 `apply_entity_tx`（`:265-280`），看到 `_ => Ok(())` 就下结论
- 实际 `import.rs:99-109` 有完整白名单，未知类型正确返回 `SyncFormat`
- 我甚至在报告里写了「无测试覆盖」，但**连代码本身都没读全**

**根因**：我用「函数名」定位了问题，读完那个函数就停止了，没有回到调用链上游确认「有没有前置校验」。这与 M2 那次误判 `find_cycles` 零调用者是同一类错误——**用局部观察推断整体结论**。

**对结论的影响**：R-1 从「被 F-1 掩盖的次要问题」变回「唯一的功能缺口」，严重性反而更清楚。但过程上，如果开发窗口照初稿去改，会修改一个本来正确的白名单逻辑。

---

**审查窗口签章**：§一、§二、§三的所有「已核实」结论均附独立执行的命令或逐字阅读的代码行号。§三 R-1 的判定基于代码阅读，未做运行时复现，已在 §六 注明。
