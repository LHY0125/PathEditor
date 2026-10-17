# F-08 收口 + 死码清理 — 核对轮交接（开发窗口 → 审核窗口）

- **日期**：2026-10-17
- **提交窗口**：开发窗口
- **交付对象**：审核窗口
- **核对对象**：
  - `docs/superpowers/specs/2026-10-17-f08-closeout-and-deadcode-removal-design.md`
  - `docs/superpowers/plans/2026-10-17-f08-closeout-and-deadcode-removal-implementation.md`
- **当前状态**：**第 1 轮核对**。计划与 spec 已逐条核对（只读），**未开工、未改任何源文件**。
- **本轮用户授权**：改代码 + 真机回归 ✅；删除 §3.0-A / §3.0-C 清单 ✅；提交 / 推送 / 升版本号 ❌（未授权）。

> 本文件是「核心细节必须落盘」规则下的产物。以下每条自带 `file:line` 或可复现的核实结论。
>
> **核实方法**：Grep / Glob / Read 只读检索。**本轮未运行任何构建或测试**，不含质量门结论。
>
> **结论摘要**：spec 的设计判断我**全部认同**（K1 的长度分家尤其对，我上一轮措辞确实偏宽）。但 **plan 有 3 条阻塞性缺陷 + 5 条需修补项**，其中 N1（契约测试无处安放）与 N2（Task 4 少覆盖 4 个入口）**不解决就无法满足 spec 自己的验收标准**。

---

## 1. 先确认认同项（不是走过场，是划定分歧边界）

| spec 决策                                                          | 开发窗口立场                                                                                                                                                                                                         |
| ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **K1 长度分家**（逐 hive 上限归 core、组合约束留 GUI）             | ✅ **完全认同**。我上一轮说「Rust 缺长度预检」是措辞偏宽 —— 我追到 `path.rs:129` 就停了，没沿 `save_path_with_sidecar → write_hive_registry → save_system_paths → save_paths` 走到 `path.rs:134`。裁断更正**正确**。 |
| D1 A′ 分步收口 / D2 组合约束不搬 / D3 顺序写 / D6 pending 统一磁盘 | ✅ 认同                                                                                                                                                                                                              |
| D7 / K4 GUI 终态不归零                                             | ✅ 认同（与我上轮 §3.9 一致）                                                                                                                                                                                        |
| D8 CLI 也切换                                                      | ✅ **认同且必须**（不切换则验收 #6 不成立）—— 但 **Task 4 的范围不足**，见 N2                                                                                                                                        |
| K5 契约测试先行 / K6 冻结分叉                                      | ✅ 认同 —— 但 **契约测试的落点有问题**，见 N1                                                                                                                                                                        |
| D11 改名 Wave 3.0                                                  | ✅ 认同                                                                                                                                                                                                              |

**下面全部是「plan 怎么写才能落地」的问题，不涉及 spec 的设计取舍。**

---

## 2. 阻塞性异议

### N1【阻塞】Task 1 / 2 / 3 的契约测试**无处安放**：`save_path_with_sidecar` 没有可注入的缝

**主张**：spec 要求（K5 + 验收 #5）「契约测试断言『写前备份产生 / 并发冲突不写且不产 pending / 顺序写』」，且 Global Constraints 要求「**Task 1-7 全程不写真实注册表**」。**这两条同时满足不了。**

**证据链（逐跳可复现）**：

1. `core/src/service.rs:193-196` `save_path_with_sidecar(system: Option<Vec<PathEntry>>, user: Option<Vec<PathEntry>>)` —— **没有任何 store / root / sub_path 参数**。
2. → `core/src/service.rs:83-92` `write_hive_registry` 硬编码：`EnvHive::System => registry::save_system_paths(enabled)`、`EnvHive::User => registry::save_user_paths(enabled)`。
3. → `core/src/registry/path.rs:95/104` 硬编码 `HKEY_LOCAL_MACHINE` / `HKEY_CURRENT_USER` 与 `SYS_REG_PATH` / `USER_REG_PATH`。
4. → `core/src/registry/path.rs:48-71` `save_paths` 用 `open_subkey_with_flags(KEY_READ|KEY_WRITE)` + `set_raw_value` —— **真实注册表写入**。
5. **项目已有的注册表测试惯例在 `save_paths` 层，不在 `save_path_with_sidecar` 层**：`core/src/registry/path.rs:328-359` 的 `save_paths_keeps_expand_sz_in_isolated_key` 确实写真实注册表，但用的是 **`HKCU` + 唯一子键** —— 之所以能这么做，是因为 **`save_paths(root: winreg::HKEY, sub_path: &str, ..)` 是参数化的**（`path.rs:48-53`）。`save_path_with_sidecar` **没有这个参数**，这条惯例**延伸不过去**。
6. **现有 `service.rs` 测试之所以安全，是因为它们的输入在触碰注册表前就失败**：`core/src/service.rs:355-367`（null 字节）与 `:372-380`（超长 32767）都走「注册表阶段失败」路径。`save_path_with_sidecar` **从未在测试中被成功执行过**。
7. `PATHEDITOR_BACKUP_DIR`（`backup.rs:148`）只重定向**备份目录**，不重定向注册表。

**结论**：Task 1 的 `save_produces_backup_file` 需要一个**成功的注册表写入**才会产生备份（spec 3.0-B-1 不变量 1：备份「仅当确实将发生写入时」）；`concurrent_modification_is_rejected_without_write` 需要读**当前注册表**与 `expected` 比对；`writes_are_sequential_system_then_user` 需要观测两次真实写入。**三条都做不到。**

**可选解法（请裁断）**：

| 方案                                         | 内容                                                                                                                                                                                                                                                                                                             | 代价                                                                         |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| **(i) 抽纯函数（开发窗口推荐）**             | 把不碰注册表的部分抽成纯函数并单测：并发判定 `fn expected_matches(current: &[String], expected: &Option<Vec<String>>) -> Result<(), CoreError>`；「是否需要备份」判定；「system 先于 user」的口径。`save_path_with_sidecar` 退化为薄壳。**成功路径的端到端（真备份文件产生）移到 Task 8 真机回归**（已授权写）。 | 小；但要承认「契约测试」= 纯函数测试 + 真机端到端，而非单测内跑通全链路      |
| **(ii) 给 `write_hive_registry` 加注入缝**   | 形如 `#[cfg(test)]` 的 root/sub_path 参数，或引入 `PathWriter` trait                                                                                                                                                                                                                                             | 中；触及 `registry/path.rs`（`String` 错误模块，F-3 领地），**扩大本波范围** |
| **(iii) 测试内用 HKCU + 保存/还原真实 PATH** | 同 `save_paths` 惯例但作用在真实 PATH 上                                                                                                                                                                                                                                                                         | **不推荐**：违反 Global Constraints，且依赖环境、易 flaky                    |

### N2【阻塞】Task 4 只覆盖 6 个 CLI 保存入口中的 **2 个** → 直接违反 spec 验收 #6 与 K6

**主张**：D8 要求「CLI 切换到 `save_path_with_sidecar`」，验收 #6 要求「全仓无其它『读-比-写 + 备份 + sidecar』编排副本」。但 **CLI 有 6 个写 PATH 的入口**，Task 4 只点名了 `load_and_save` / `load_operate_save`。

**完整清单（本次实测）**：

| #   | 入口                | 命令                            | 位置                            | 编排                                                                                        | Task 4 覆盖？ |
| --- | ------------------- | ------------------------------- | ------------------------------- | ------------------------------------------------------------------------------------------- | ------------- |
| 1   | `load_and_save`     | `add` / `move-up` / `move-down` | `cli/src/runtime.rs:70-89`      | `verify_and_save`(`:81/:86`) + `persist_snapshot`(`:82/:87`)                                | ✅ Step 1     |
| 2   | `load_operate_save` | `remove` / `edit`               | `cli/src/runtime.rs:92-120`     | `verify_and_save`(`:111`) + `persist_snapshot`(`:113/:115`) + broadcast(`:119`)             | ✅ Step 1     |
| 3   | `clean_one`         | `clean`                         | `cli/src/main.rs:430-471`       | **仅** `verify_and_save`(`:459`)                                                            | ❌ **未覆盖** |
| 4   | `cmd_toggle`        | `enable` / `disable`            | `cli/src/main.rs:473-510`       | `verify_and_save`(`:499`) + `persist_snapshot`(`:503/:505`) + broadcast(`:507`)             | ❌ **未覆盖** |
| 5   | `cmd_import`        | `import`                        | `cli/src/import_export.rs:6-41` | `verify_and_save`(`:14/:20/:27/:31`) + `persist_snapshot`(`:15/:21/:33`) + broadcast(`:40`) | ❌ **未覆盖** |
| 6   | `profile_apply`     | `profile apply`                 | `cli/src/profile_ops.rs:45-69`  | `verify_and_save`(`:63/:64`) + `persist_snapshot`(`:65`) + broadcast(`:67`)                 | ❌ **未覆盖** |

**后果**：若按现 plan 施工，收口后 CLI 会**同时存在两套编排**（入口 1/2 走新的，3/4/5/6 走旧的），**验收 #6 必然不通过**，K6「冻结分叉」也名存实亡。

### N3【阻塞】Task 4 Step 2「`verify_and_save` / `persist_snapshot` 体内转调新函数」**不可实现**

**主张**：该步与本步 Step 1 及其余 4 个未覆盖入口**互相矛盾**。

**理由**：

1. `verify_and_save(target: &str, original: &[String], new_list: Vec<String>)`（`runtime.rs:53`）**只拿得到 `Vec<String>`**，没有 `PathEntry`、也没有 sidecar 需要的数据。把它「体内转调 `save_path_with_sidecar`」就意味着它要**独自完成注册表写入 + sidecar 提交**——但它的签名决定了它拿不到 sidecar 内容。
2. 若强行让它转调，则**入口 3/4/5/6 会双重提交 sidecar**：它们现在是「`verify_and_save` 然后 `persist_snapshot`」，转调后 `verify_and_save` 内部已提交一次，调用方的 `persist_snapshot` 再提交一次。
3. 反之，若 `verify_and_save` **不**转调，则 Task 4 就只改了入口 1/2 —— 回到 N2。

**结论**：Step 2 必须重写。正确形态应是**把 6 个入口逐个改为「一次 `save_path_with_sidecar` 调用」**，随后 `verify_and_save` / `persist_snapshot` 若不再有调用点，**即成为死码，`clippy -D warnings` 会因 `dead_code` 报错**（`pub(crate)` 未使用会被 lint）。`persist_snapshot` 另有 4 个调用点在 3/4/5/6 入口内，故**只有在 6 个入口全部转换后**才会变死码——这进一步说明 Task 4 必须一次覆盖全部 6 个。

---

## 3. 需修补项（非阻塞，但 plan 没写清）

### N4【新发现】`patheditor clean` 从不提交 sidecar 快照 —— 既有不一致

`cli/src/main.rs:457-470`：非 dry-run 分支只调 `verify_and_save(target, &list, kept)`（`:459`），**没有 `persist_snapshot`**；`cmd_clean`（`:410-428`）也**不调 `flush_pending_snapshot`**。

对比：其余 5 个入口**都**调 `flush_pending_snapshot`（`runtime.rs:75/:98`、`main.rs:475`、`import_export.rs:7`、`profile_ops.rs:46`）**且**调 `persist_snapshot`。

**影响**：`patheditor clean` 后注册表条目被移除，但 `disabled.json` 里的有序快照仍是旧的 —— 下次 `load_path_snapshot` 合并时可能把已清理的条目带回。**这是本波之前的既有行为**（不是本波引入）。

**与本波的关系**：Task 4 若按 N2 的建议把 `clean_one` 也收口到 `save_path_with_sidecar`，会**顺带修掉**这个不一致（新函数内部会 `retry_pending_path_state` + `commit_sidecar`）。这是**行为变化**，须明确声明（是修 bug，不是等价迁移）。**请裁断**：本波顺带修，还是只登记不动？

### N5【行为变化未声明】CLI 保存将开始产生 `path_backup_*.txt`

Task 2 把写前备份加进 `save_path_with_sidecar`；D8 让 CLI 走同一函数。因此收口后 **`patheditor add/remove/edit/move/clean/enable/disable/import/profile apply` 每次成功写 PATH 都会产生一个 `path_backup_*.txt`**（默认目录、保留 20 份）。

现状：CLI 只有显式的 `patheditor backup` 才产备份文件。**这是用户可见的行为变化**，spec/plan 均未列为「变更」。建议写入 `CHANGELOG.md` 的 `### 变更` 与 `README.md`。

（一致性上**站得住**：env 侧本来就是每次写都备份。但「收口 = 行为等价迁移」的框架下，这一条**不是等价**，要如实说。）

### N6【双重广播】转换后 `broadcast_env_change` 会被调用两次

`save_path_with_sidecar` 内部已广播（`core/src/service.rs:214-216`）。而调用方**各自又广播一次**：

| 调用方              | 位置                          |
| ------------------- | ----------------------------- |
| `cmd_add`           | `cli/src/main.rs:368`         |
| `cmd_move`          | `cli/src/main.rs:407`         |
| `cmd_clean`         | `cli/src/main.rs:426`         |
| `cmd_toggle`        | `cli/src/main.rs:507`         |
| `cmd_import`        | `cli/src/import_export.rs:40` |
| `profile_apply`     | `cli/src/profile_ops.rs:67`   |
| `load_operate_save` | `cli/src/runtime.rs:119`      |

**这 7 处调用方广播在收口后都需删除**（广播下沉 core）。plan 未提。

### N7【测试遗漏】Task 5 未列 `tests/unit/app-store.test.ts`

`tests/unit/app-store.test.ts:265-266` 与 `:281-282` 用 `mockResolvedValueOnce` 依次 mock `save_system_paths` / `save_user_paths`。`path-session.ts` 薄化后不再调这两个命令，`mockResolvedValueOnce` 队列会**错位**，相关断言会失败。

Task 5 的 Files 只写「`tests/unit/`（`path-session` 相关、`backend-env-contract` 若涉及）」——**未点名 `app-store.test.ts`**。须补入。

### N8【口径】Task 7 Step 1 的 grep 会命中 e2e 与单测，plan 只提了 e2e

`grep -rn "scan_conflicts\|scan_tools\|get_appdata_dir\|save_system_paths\|save_user_paths" src e2e` 的实际命中还包括：

- `e2e/mocks/ipc.ts:154-155`（`save_system_paths` / `save_user_paths`）→ plan Step 4 已提「同步清理」✅
- `tests/unit/app-store.test.ts:265-266/:281-282` → **未提**（见 N7）

### N9【口径】`PathHiveWrite` 转换会丢弃 `enabled` 信息，需注释写死为何等价

`verify_and_save` 现在的 `new_list: Vec<String>` 来自 `enabled_paths(...)`（已过滤 `enabled=true`）。构造 `PathHiveWrite { entries }` 时只能落成 `PathEntry { path, enabled: true }`。这是**等价的**（因为输入本就是启用集），但必须在代码注释里写死，否则后人会以为漏了 `enabled`。

### N10【文档耦合】`core/src/registry/golden/README.md:62` 引用 `service.rs` 的行号

该行写「`core/src/service.rs:207`：`save_path_with_sidecar` 任一 hive 写成功后广播」。Task 3 会移动该函数位置 —— 此引用**须同步复核**（当前实际广播在 `service.rs:215`，该引用已经与代码漂移了 8 行，属既有小问题）。

---

## 4. 建议的 Task 4 重写形态（供审核窗口采纳）

把 Task 4 从「改 2 个入口」改写为「**6 个入口全部收口 + 清理调用方广播 + 处理死码**」：

1. 6 个入口（§N2 表）逐个改为**一次 `save_path_with_sidecar(Some/None PathHiveWrite)` 调用**；
2. 删除 §N6 的 7 处调用方广播；
3. `verify_and_save` / `persist_snapshot` / `flush_pending_snapshot` 的**去留逐一定夺**：
   - 若 6 个入口全转换后无调用点 → **删除**（否则 `dead_code` 触发 `-D warnings`）。**注意：删除函数在本波授权范围外**（授权仅限 §3.0-A / §3.0-C 清单）——若需删除，请补授权或在 spec 里列明；
   - `flush_pending_snapshot` 因 `save_path_with_sidecar` 内部已 `retry_pending_path_state`，会**冗余**——删除同样需授权。
4. **退出码不变**：并发冲突仍走 `exit_err`（退出码 1）。

> **⚠️ 授权缺口**：本波删除授权**仅**覆盖 `src/core/` 3 模块 + 3 测试 + 3 个孤儿 IPC 命令。**CLI 侧的 `verify_and_save` / `persist_snapshot` / `flush_pending_snapshot` 若要删除，不在授权内**，请审核窗口确认是否扩权。

---

## 5. 待裁断（汇总）

| #     | 问题                                             | 选项                                                             | 开发窗口倾向                                           |
| ----- | ------------------------------------------------ | ---------------------------------------------------------------- | ------------------------------------------------------ |
| **A** | Task 1/2/3 契约测试落点（N1）                    | (i) 抽纯函数 + 端到端移真机 / (ii) 加注入缝 / (iii) HKCU 真 PATH | **(i)**                                                |
| **B** | Task 4 是否扩到全部 6 个入口（N2）               | 扩 / 不扩                                                        | **扩**（否则验收 #6 不成立）                           |
| **C** | `clean` 缺 `persist_snapshot` 的既有不一致（N4） | 本波顺带修 / 只登记                                              | 倾向**顺带修**（收口本来就会碰到），但需声明为行为变化 |
| **D** | CLI 新增备份文件（N5）如何声明                   | 记 CHANGELOG `### 变更` + README                                 | **记**                                                 |
| **E** | CLI 侧函数删除授权（§4.3）                       | 扩权 / 保留 `#[allow(dead_code)]`                                | 倾向**扩权删除**（保留死码更糟）                       |

---

## 6. 未核验项（如实列出）

| 项                                                                                                        | 状态                                                                                                                                        |
| --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `gui/src/commands/service.rs` 改新签名后，Tauri IPC 对 `Option<PathHiveWrite>` 的反序列化行为             | **未核**（未运行；属实现期验证）                                                                                                            |
| `backend.ts` 现有形状校验框架能否直接容纳 `ApplyOutcome`                                                  | **未核**（未读该段）                                                                                                                        |
| `e2e/mocks/ipc.ts` 是否还有其它受 Task 5/7 影响的命令                                                     | 仅核了 `:154-155`                                                                                                                           |
| spec 里除 K1–K6 外的行号引用（如 `gui/src/commands/registry.rs:13/23`、`scanner.rs:4/8`、`backup.rs:10`） | **部分核**：`registry.rs:13/23` ✅ 命中；`scanner.rs:4/8` ✅（`scan_conflicts`:4 / `scan_tools`:8）；`backup.rs:10` ✅（`get_appdata_dir`） |
| `src/config/default.json` 删除两键后是否有其它 TS 类型依赖                                                | ✅ **已核**：仅 `path-session.ts:113-117` 使用（`maxSystemLength`/`maxUserLength`/`maxCombinedLength`），无其它消费者 → Task 5 Step 5 安全  |

---

## 7. 交接链状态

| 文档                                                                  | 状态                                    |
| --------------------------------------------------------------------- | --------------------------------------- |
| 评审报告 / 核对轮交接 / 裁断（2026-10-17）                            | ✅ 均已在库                             |
| spec `2026-10-17-f08-closeout-and-deadcode-removal-design.md`         | ✅ 已产出（未提交）                     |
| plan `2026-10-17-f08-closeout-and-deadcode-removal-implementation.md` | ✅ 已产出（未提交）→ **待按本交接修订** |
| 本核对轮交接（第 1 轮）                                               | ✅ 本文件                               |
| 开发回执                                                              | ⏳ 待开工后                             |

**下一步（开发窗口的请求）**：

1. 审核窗口就 **§5 待裁断 A–E** 给出裁断；
2. 按 N2/N3 重写 **plan Task 4**；按 N1 重写 **Task 1/2/3 的测试**；补入 N6/N7/N9/N10；
3. 方向收敛后开发窗口按修订版 plan 施工。

**开发窗口本轮未开工，等裁断。**
