# PathEditor v6.0.0 M2 开工提示词

> 交给开发窗口。本文件是**唯一的开工指令**，其余细节以计划为准。
> 若本文件与计划冲突，**以计划为准**，并把冲突报告给审核窗口。

---

## 你是开发窗口

按 `.claude/skills/patheditor-dev-window` 的流程走。与审核窗口不共享上下文，**唯一信道是仓库文档**。

---

## 一、任务

实现 `docs/superpowers/plans/2026-10-20-v6-sqlite-m2-plan.md` 的全部 13 个 Task。

**开工前必读**（按顺序）：

1. `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` —— §4.1 数据模型、§4.2 不变量、§6.2 Tier 2、§3.2.1 IPC 边界
2. `docs/superpowers/plans/2026-10-20-v6-sqlite-m2-plan.md` —— 1967 行，13 Task + 两张裁断表
3. `docs/审核和开发/2026.10.20/PathEditor-v6-M2-核对轮交接.md`（339 行）
4. `docs/审核和开发/2026.10.20/PathEditor-v6-M2-核对第二轮.md`（312 行）

第 3、4 份是**你自己前两轮提的意见**。计划的末尾有两张裁断表逐条记录了采纳情况——
先确认你的意见被正确理解，再开工。

---

## 二、当前状态（开工前的基线）

```
分支        main（M1 已合并并推送）
HEAD        f6584bb
版本号      5.1.7（M2 不升版本）
工作区      干净

Rust        22 binaries, 313 passed, 0 failed, 2 ignored
前端        27 files, 336 tests passed
E2E         44 passed
```

**任何一项测试数下降都要当场说明原因。** 上表是基线，不是目标——增加是正常的，减少必须解释。

---

## 三、硬约束（违反即被打回，不接受「下次注意」）

### 1. 一文件一测试

任何使用 SQLCipher 的 `.rs` 测试文件**只放一个 `#[test]`**。同一进程内第二个
SQLCipher 测试会 `STATUS_STACK_OVERFLOW`（GNU/MSVC 均已复现，`--test-threads=1` 无效）。

**M2 有 13 个 Task，计划里标了 10 个 `Test:` 行（部分 Task 写两个文件名）。** 每个 Task 的
`Test:` 行已经标明文件路径，**照写，不要合并**。若某个 Task 天然需要两个测试，
计划里会写两个文件名（如 Task 2 与 Task 5）。

### 2. `config_dir()` 无法被环境变量重定向

`dirs::home_dir()` 在 Windows 上走 `SHGetKnownFolderPath`，**`HOME` 与 `USERPROFILE` 都不读**。

**后果**：任何测试只要碰到 `profiles_dir()` / `disabled_file_path()` / `config_dir()`，
就会写进用户真实的 `C:\Users\<用户>\.patheditor\`。审核窗口已经因此写坏过一次用户的
`disabled.json`。

**规则**：所有测试必须**显式传 `home: &Path` 参数**（`tempfile::TempDir`），
不得依赖 `set_var`。**若某个函数只能算路径不能接收路径，先改函数签名再做测试**
（计划 Task 8 的 N-1 就是这个模式）。

### 3. 派生表不是真相

`search_fts` / `daily_stats` / `var_references` 必须能从 `snapshots` / `events` /
`path_entries` **完整重建**。每个派生模块都要有 `rebuild_*` 与对应的重建测试
（清空 → 重建 → 断言数据恢复）。

### 4. `search_fts` 绝不索引 env 变量的值

只索引：PATH 条目串、env 变量名、配置的 name/description/tags。
`env_values.value` 是 SQLCipher 保护的明文，**不进任何派生表**。

### 5. 敏感值默认掩码

任何返回快照 / 对比结果 / 搜索结果的命令，敏感值恒为 `null`。
需要明文必须走独立的 `history_reveal_env_value`（Task 7）。

### 6. 不写真实注册表、不删除用户文件

E2E 用 mock IPC。测试用 `TempDir` 或 fake IO。

---

## 四、交付纪律

- **不提交、不推送、不升级版本号**，除非用户明确授权。每轮重新确认。
- 每个 Task 一个提交，Conventional Commits：`feat(v6): ...` / `test(v6): ...`。
- 每个 Task 结束跑完整质量门：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo test --workspace`、`npm test`。**不要攒到最后一起跑**（M1 曾先写完 8 个 Task 再首次编译，
  一次暴露 5 处编译错误）。
- 中文注释；Rust 4 空格；TS 2 空格、单引号、100 列。
- 所有 `pub fn` / `pub(crate)` 中文 `///` 注释。

---

## 五、M2 特有的三个陷阱

这三条不是我凭空提醒，是前几轮实际踩过的。

### 陷阱一：字段在 Interfaces 写了、代码块没写

首版计划里 `Dashboard` 在 Interfaces、实现、Self-Review 三处有三种形状；
`CompareResult` 的 `env_*` 三字段只在 Interfaces 出现。**两次都被你的核对抓出来了。**

**开工建议**：每个 Task 写完代码块后，回看它的 Interfaces 段，逐个字段对一遍。
计划已做过一次全篇扫描（6 个 struct 全部一致），但**实现时仍可能引入新的不一致**。

### 陷阱二：只处理了一半的维度

`compare()` 首版只比 `system`，user 侧静默丢弃——类型正确、编译通过、测试也能过，
唯一的问题是「只有用户 PATH 变了时显示没有差异」。这是本轮唯一「给出错误结论而不报错」的缺陷。

**开工建议**：每个「只处理 X」的地方，问一句「那 Y 呢」。具体到 M2：

- 对比：**system 与 user 都要**（Task 5 有两个测试文件守这条）
- 派生表：三个都要有重建路径（Task 12）
- 冲突历史：**首次出现 + 最近出现都要**（Task 6 的 `first_seen` / `last_seen`）
- 引用图：**前向（dependencies）+ 反向（dependents）+ 环（cycles）都要**

### 陷阱三：复制已有的逻辑

首版的 `load_profile_from` 和已有的 `read_profile_file` 逐行重复。
今天的危险不是不一致，是**一年后改版本迁移规则时只改一处**。

**开工建议**：新增函数前先 grep 一遍同模块，确认没有等价实现。

---

## 六、你可能会遇到的三个具体问题

### 1. SQLite 版本

已实测：**3.51.3**，`ENABLE_FTS5 = 1`，`FULL OUTER JOIN = true`。
Task 3 的 `FULL OUTER JOIN` 可以直接用，不需要降级写法。

### 2. FTS5 的 MATCH 语法

`search()` 里 `WHERE search_fts MATCH ?1` 会把用户输入**当查询语法**。
用户输入 `C:\Program Files` 或 `"` 会导致 `fts5: syntax error`。

**需要处理**：把查询串用双引号包起来做 phrase 匹配（`format!("\"{}\"", q.replace('"', "\"\""))`），
或在 `search()` 的文档注释里写明「调用方必须转义」并让 GUI 侧做。

**计划里没写这一条**——实现时请自行决定，并把决定记进回执的偏离记录。

### 3. `'localtime'` 依赖进程时区

`date(..., 'localtime')` 取 C 运行时时区，代码里没有任何地方强制 `Asia/Shanghai`。

**Task 3 的测试必须避开跨日边界**——不要在 23:59 附近写事件，断言只比较「天数」
不比较具体日期。

---

## 七、完成的判定标准

M2 完成时必须同时满足：

1. 13 个 Task 全部实现，每个 Task 一个提交。
2. 质量门全绿，且测试数 **≥ 基线**（313 Rust / 336 前端 / 44 E2E）。
3. `core/tests/` 下新增的每个文件**只有一个 `#[test]`**（`grep -c '#\[test\]'` 逐个验证）。
4. 三个派生表都能「清空 → 重建 → 数据恢复」（Task 12 的测试）。
5. `CompareResult` 的 system / user **两侧都有测试覆盖**（Task 5 的两个文件）。
6. 未覆盖项表（计划 §Self-Review 的 4 行）**原样抄进回执**，不得省略。

---

## 八、交付物

完成后写 **`docs/审核和开发/YYYY.MM.DD/PathEditor-v6-M2-开发回执.md`**，结构照
`docs/审核和开发/2026.10.20/PathEditor-v6-M1-缺口补齐轮开发回执.md`：

| 节  | 内容                                      |
| --- | ----------------------------------------- |
| §0  | 事故与异常（如有），放在最前              |
| §1  | 交付内容：每个 Task 一段                  |
| §2  | 设计决策：「选了什么 + 为什么」           |
| §3  | 偏离记录：计划原文 / 实际实现 / 处理方式  |
| §4  | 质量门：命令与原始输出                    |
| §5  | 未覆盖项（**必须原样抄计划里的 4 行表**） |
| §6  | 残余风险                                  |
| §7  | 需审核窗口重点关注的项                    |
| §8  | 交付物位置索引                            |
| §9  | 提交清单与状态                            |

**回执里的每个「已通过」都必须附实际执行过的命令与输出。** 没跑过的不要写。

---

## 九、开工时先做的三件事

1. **确认基线**：跑一遍四个质量门，确认数字与本文件 §二 一致。不一致先报告，不要开始。
2. **读计划的两张裁断表**：`## 核对轮裁断（2026-10-20）` 与 `## 核对轮第二轮裁断（2026-10-20）`。
   这 22 条是你前两轮的意见 + 审核窗口的裁断，逐条确认理解无误。
3. **确认 Task 1 的 DDL 与 spec §4.1 逐字一致**：`scan_runs` / `var_references` /
   `daily_stats` / `search_fts` 四张表的列名与类型。**这是唯一一处「照抄权威文档」的地方，
   不要发挥。**

---

## 十、边界

出现下面任一情况，**停下来问用户**，不要自作主张：

- 需要删除文件、推送、升级版本号、写真实注册表
- 发现 spec 本身有结构性缺陷（不是计划的问题，是规格的问题）
- M2 的范围需要扩张（例如「顺手修一下 CLI 的派生表」——那是未覆盖项 #2，明确不做）
- 某个 Task 的实现与计划的 Interfaces 冲突，且无法在不改变对外形状的前提下解决

---

**开工。** 第一个 Task 是 schema v2，它决定后面 12 个 Task 的地基，慢一点没关系。
