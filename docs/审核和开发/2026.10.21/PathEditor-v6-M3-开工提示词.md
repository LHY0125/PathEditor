# PathEditor v6.0.0 M3 开工提示词

> 交给开发窗口。本文件是**唯一的开工指令**，其余细节以计划为准。
> 若本文件与计划冲突，**以计划为准**，并把冲突报告给审核窗口。

---

## 你是开发窗口

按 `.claude/skills/patheditor-dev-window` 的流程走。与审核窗口不共享上下文，**唯一信道是仓库文档**。

---

## 一、任务

实现 `docs/superpowers/plans/2026-10-21-v6-sqlite-m3-plan.md` 的全部 **11 个 Task**。

**开工前必读**（按顺序）：

1. `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` —— **§4.1 表结构**、**§4.2 不变量 7/8/9**、**§5.2 同步包安全**、**§6.3 Tier 3**、**§6.4 Bundle Schema**、**§8 退出码**
   > ⚠️ spec 刚改过两处：§6.3 移除了 `events` 同步；§8 拆分了 `Sync` / `SyncFormat`。
2. `docs/superpowers/plans/2026-10-21-v6-sqlite-m3-plan.md` —— 981 行，11 Task + 核对轮裁断表
3. `docs/审核和开发/2026.10.21/PathEditor-v6-M3-核对轮交接.md`（315 行）—— **你自己提的意见**

第 3 份是你上一轮交的。计划末尾的裁断表逐条记录了采纳情况——先确认你的意见被正确理解，再开工。

---

## 二、当前状态（开工前的基线）

```
分支        main（M2 已合并并推送）
HEAD        62c6438
版本号      5.1.7（M3 不升版本）
工作区      干净

Rust        37 binaries, 329 passed, 0 failed, 2 ignored
前端        31 files, 374 tests passed
E2E         50 passed
```

**任何一项测试数下降都要当场说明原因。** 上表是基线，不是目标——增加是正常的，减少必须解释。

---

## 三、硬约束

### 1. 一文件一测试

任何使用 SQLCipher 的 `.rs` 测试文件**只放一个 `#[test]`**。同一进程内第二个 SQLCipher 测试会 `STATUS_STACK_OVERFLOW`（GNU/MSVC 均已复现，`--test-threads=1` 无效）。

M3 在 `core/tests/` 下声明 **10 个**新测试文件：Task 1 两个、Task 6 两个、Task 2 / 2b / 3 / 4 / 5 / 7 各一（另有两个前端测试文件）。
**Task 5 的 `merge` 是纯函数，不建库**——但为了保持一致性，仍然一个文件一个 `#[test]`。

### 2. `config_dir()` 无法被环境变量重定向

`dirs::home_dir()` 在 Windows 上走 `SHGetKnownFolderPath`，**`HOME` 与 `USERPROFILE` 都不读**。

所有测试必须**显式传 `home: &Path` 参数**（`tempfile::TempDir`），不得依赖 `set_var`。
M1 已因此写坏过用户的真实 `disabled.json`。

### 3. 每个新增 `pub fn` 必须有生产调用者 —— **这次是硬性门**

**这条已经出过三次**：M1 的 `.pebak` 写入器、M2 的 `index_snapshot_tx`、M3 计划首版的 `record_change_tx`。

```bash
# 0. 先用已知有调用者的函数验证 grep 模式有效
grep -rn "dashboard(" --include=*.rs core/src gui/src cli/src | grep -v "fn dashboard"
#    必须 ≥1 命中，否则模式本身有问题

# 1. 再查目标函数
grep -rn "<fn_name>(" --include=*.rs core/src gui/src cli/src | grep -v "fn <fn_name>"
```

⚠️ **不要用 `\b` 词边界**——它匹配不到 `refs::find_cycles` 这类带模块前缀的调用。

### 4. 同步不写注册表

导入只改 DB。用户要生效必须走既有 `apply_path_snapshot` / `apply_profile`。

### 5. 敏感值

- `changes.old_value` / `new_value` **仅用于同步**，位于 SQLCipher 加密库与加密 bundle 内
- **不得写入 `events`**（spec §4.2 第 7 条）
- UI 展示敏感值时仍须掩码；`sync_conflicts.local_value` / `remote_value` 同样按 `sensitive` 掩码（§4.2 第 9 条）

### 6. 禁止在 bundle 中出现本地整数主键

所有引用用 uid（`parentUid` / `snapshotUid` / `profileUid`）。spec §6.4 明确。

### 7. 导入失败必须整体回滚

单事务，不产生半应用状态（spec §8）。

### 8. 口令安全

- 口令**不落盘**、不进 shell history、不出现在 `ps` 输出
- **错误文案不得包含口令**，日志同样
- CLI 用 `rpassword` 隐藏输入；GUI 用 `type="password"` 且**不写入 store**

---

## 四、交付纪律

- **不提交、不推送、不升级版本号**，除非用户明确授权。每轮重新确认。
- 每个 Task 一个提交，Conventional Commits。
- 每个 Task 结束跑完整质量门：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo test --workspace`、`npm test`。**不要攒到最后一起跑**。
- 中文注释；Rust 4 空格；TS 2 空格、单引号、100 列。
- 所有 `pub fn` / `pub(crate)` 中文 `///` 注释。

---

## 五、M3 特有的四个陷阱

### 陷阱一：写路径没接变更日志（S-1，已经踩过）

Task 2b 是专门为此新增的。它的验收不是「函数存在」，而是「**调用 core 的写函数后，`changes` 表自己多出了行**」。

写完 Task 2b 后，**逐条对照计划里那张 7 行写入口表**，确认每一行都真的接了。

```
save_profile_tx     → profile / None
delete_profile      → profile / deleted_at
rename_profile      → profile / name
insert_snapshot_tx  → snapshot / None
path_entries        → 随快照，不单独记
env_values          → 随快照，不单独记
profile_revisions   → profile_revision / None
```

### 陷阱二：AAD 没覆盖 header

spec §5.2 要求「AAD 包含 header（版本/设备/序号）」。如果只把 `Salt + Nonce` 作为 AAD，
**header 可被篡改而不被发现**——攻击者改 `changeCount` 或 `deviceId` 不会触发 AEAD 验证失败。

Task 3 的测试必须有一条「篡改 header → 拒绝」。

### 陷阱三：导出顺序反了会丢变更

```
正确：收集 → 加密 → 写文件 → （成功后）标记 exported_at
错误：收集 → 加密 → 标记 exported_at → 写文件
```

后者在写文件失败时，变更**既没导出成功，又不在待导出队列里了**——永久丢失。

Task 4 的测试必须覆盖「写文件失败时 `pending_export` 不变」（用只读目录或非法路径构造）。

### 陷阱四：Lamport 时钟的单调性

`record_change_tx` 内部要同时推进 `seq`（本设备单调序号）与 `lamport`（逻辑时间），
并写回 `app_state.lamport`。

**不是两个独立的计数器**——如果 `lamport` 落后于 `seq`，导入时的 LWW 会用错误的时间比较。

Task 2 的测试要断言：插入两条变更后 `current_lamport() >= 2`。

---

## 六、具体的三个技术点

### 1. 依赖版本

三个新增依赖都已实测可通过南大镜像获取：

| crate              | 用途                                             |
| ------------------ | ------------------------------------------------ |
| `argon2`           | 口令派生                                         |
| `chacha20poly1305` | AEAD                                             |
| `rpassword`        | CLI 隐藏口令输入（**你上一轮发现计划漏了这个**） |

`cargo add --dry-run` 已验证，未改动文件。

### 2. Argon2id 参数（spec §5.2 写死）

```
m = 65536 KiB (64 MiB)
t = 3
p = 1
salt = 16 B
输出 = 32 B
```

**不要调这些参数**——它们影响 bundle 的跨设备兼容性。

### 3. FTS5 的教训不要重演

M2 的 `escape_query` 是「计划没写、实现者自己决定」的设计点。M3 里类似的点：

- **bundle 里的 JSON 序列化是否要排序键**？（影响同一份数据的字节稳定性，进而影响 payload_hash 之类的判断）
- **`changes.seq` 是全局还是分设备**？（spec 写「本设备单调序号」，DDL 的索引是 `(device_id, seq)`）

遇到计划没说清的地方：**自己决定 + 写进回执的偏离记录**，不要停下来等。

---

## 七、完成的判定标准

M3 完成时必须同时满足：

1. **11 个 Task 全部实现**，每个 Task 一个提交。
2. 质量门全绿，测试数 **≥ 基线**（329 Rust / 374 前端 / 50 E2E）。
3. `core/tests/` 下新增的每个文件**只有一个 `#[test]`**（逐个 `grep -c` 验证）。
4. **S-1 的验收**：`db_change_log_wiring.rs` 通过，且 `grep -rn "record_change_tx(" core/src | grep -v "fn record_change_tx"` 有命中。
5. **Task 5 的 LWW 四种情况**全部有断言（高 lamport / 低 lamport / 同 lamport 不同 device / 同 lamport 同 device）。
6. **篡改拒绝**：改密文、改 header、错口令，三者都必须拒绝且不产生写入。
7. 未覆盖项表**原样抄进回执**。

---

## 八、交付物

完成后写 **`docs/审核和开发/YYYY.MM.DD/PathEditor-v6-M3-开发回执.md`**，结构照
`docs/审核和开发/2026.10.21/PathEditor-v6-M2-开发回执.md`：

| 节  | 内容                            |
| --- | ------------------------------- |
| §0  | 事故与异常（如有），放在最前    |
| §1  | 交付内容：每个 Task 一段        |
| §2  | 设计决策：「选了什么 + 为什么」 |
| §3  | 偏离记录                        |
| §4  | 质量门：命令与原始输出          |
| §5  | 未覆盖项                        |
| §6  | 残余风险                        |
| §7  | 需审核窗口重点关注的项          |
| §8  | 交付物位置索引                  |
| §9  | 提交清单与状态                  |

**回执里的每个「已通过」都必须附实际执行过的命令与输出。**

---

## 九、开工时先做的三件事

1. **确认基线**：跑一遍四个质量门，确认数字与本文件 §二 一致。不一致先报告，不要开始。
2. **读计划末尾的裁断表**：12 条，逐条确认理解无误。特别是 **S-1**——它是给你新增的 Task 2b。
3. **确认 spec 的两处改动**：§6.3 已移除 `events`；§8 已拆分 `Sync` / `SyncFormat`。
   **如果你按旧记忆写「同步 events」，会多写一半的代码。**

---

## 十、边界

出现下面任一情况，**停下来问用户**，不要自作主张：

- 需要删除文件、推送、升级版本号、写真实注册表
- 发现 spec 本身有结构性缺陷
- M3 的范围需要扩张（例如「顺手把 CLI 的派生表也接上」——那是 M2 的未覆盖项 #2，明确不做）
- 某个 Task 的实现与计划的 Interfaces 冲突，且无法在不改变对外形状的前提下解决

---

**开工。** 第一个 Task 是 schema v3，它决定后面 10 个 Task 的地基。
**特别注意**：`PRAGMA user_version` 必须写在**所有分支之后**——M2 在这里出过一次错，
计划已加 `db_schema_v2_to_v3.rs` 专门守这条。
