# PathEditor v6.0.0 M4 开工提示词

> 交给开发窗口。本文件是**唯一的开工指令**，其余细节以计划为准。
> 若本文件与计划冲突，**以计划为准**，并把冲突报告给审核窗口。

---

## 你是开发窗口

按 `.claude/skills/patheditor-dev-window` 的流程走。与审核窗口不共享上下文，**唯一信道是仓库文档**。

---

## 一、任务

实现 `docs/superpowers/plans/2026-10-21-v6-sqlite-m4-plan.md` 的全部 **11 个 Task**。

**开工前必读**（按顺序）：

1. `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` —— **§6.5（本次新增，M4 全部决策）**、§6.3 / §6.4（同步范围与 bundle schema）、§4.2 第 10 条（保留策略）、§十二（验收）、§十三（M4 里程碑）
2. `docs/superpowers/plans/2026-10-21-v6-sqlite-m4-plan.md` —— 2307 行，11 Task + Self-Review
3. `docs/审核和开发/2026.10.21/PathEditor-v6-M3-复审结论.md` —— R-1 / B' 的裁决、N-1~N-3
4. `docs/审核和开发/2026.10.21/PathEditor-v6-M3-复审回执.md` —— `skippedUnsupported` 的语义与第二层陷阱

第 3、4 份决定了 M4 的起点：**配置同步在 M3 是「格式合法但不落库」**，M4 要把它真正接上；同时不要再引入「报告了不存在的成功」。

---

## 二、当前状态（开工前的基线）

```text
分支        main
HEAD        a65f650（M4 计划与 spec §6.5 已提交）
版本号      5.1.7（Task 11 才升 6.0.0）
工作区      干净

Rust        48 binaries, 340 passed, 0 failed, 2 ignored
前端        33 files, 397 passed
E2E         54 passed
```

**任何一项测试数下降都要当场说明原因。** 统计用可复现命令，不要手算：

```powershell
cargo test --workspace 2>&1 | Select-String "test result:"
npx vitest run --reporter=dot 2>&1 | Select-String "Test Files|Tests "
npm run test:e2e 2>&1 | Select-String "passed|failed"
```

> M3 曾两次因为手算测试数报错。基线是门槛，不是目标。

---

## 三、硬约束

### 1. 一文件一测试

任何使用 SQLCipher 的 `.rs` 测试文件**只放一个 `#[test]`**。同一进程内第二个 SQLCipher 测试会 `STATUS_STACK_OVERFLOW`（GNU/MSVC 均已复现，`--test-threads=1` 无效）。

M4 在 `core/tests/` 下新增 **8 个**测试文件：Task 1/2/3/4/5/6/7/9 各一。
Task 8 的测试放在 `core/src/db/maintenance.rs` 的 `#[cfg(test)]` 里（DPAPI，不建 SQLCipher），该文件也只有那一个测试。

### 2. `config_dir()` 无法被环境变量重定向

`dirs::home_dir()` 在 Windows 上走 `SHGetKnownFolderPath`，**`HOME` 与 `USERPROFILE` 都不读**。

所有涉及用户目录的函数**必须显式接收路径参数**（`home: &Path` / `dir: &Path`），测试用 `tempfile::TempDir`。
M1 已因此写坏过用户的真实 `disabled.json`。

### 3. 每个新增 `pub fn` / `pub(crate) fn` 必须有生产调用者

**这条已经出过三次**：M1 的 `.pebak` 写入器、M2 的 `index_snapshot_tx`、M3 计划首版的 `record_change_tx`。

```powershell
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

- `changes.old_value` / `new_value` 只在加密库与加密 bundle 内
- 不得写入 `events`（spec §4.2 第 7 条）
- UI 展示敏感值时仍须掩码

### 6. **绝不操作真实用户文件**（M4 最重要的一条）

M4 新增了三类会改动用户磁盘的功能：`.v5.bak` 归档、遗留 `.json` 清理、保留策略删除。**开发窗口的职责是实现它们，不是执行它们。**

- 所有验证只用 `TempDir` 集成测试或单元测试。
- `maintenance archive-v5` 只跑 `--dry-run`。
- `maintenance cleanup-legacy-backups` 只跑 `--dry-run`；**不得加 `--yes`**。
- `maintenance prune` 只跑 `--dry-run`。
- **不要启动 `npx tauri dev` / 真实 GUI 来「验证」Task 6/7/8**——`setup_database` 会在真实 `~/.patheditor` 上执行清理与归档。这一条是硬性的。
- 不删除、不改名任何仓库外文件。仓库内文件的删除同样需要用户书面同意。

### 7. 归档/清理只改名或删除**本功能生成的文件**

- `.v5.bak` 归档：只处理 `disabled.json` 与 `profiles/*.json`；目标已存在则跳过，**绝不覆盖**。
- 遗留备份清理：只处理 `env_backup_YYYYMMDD_HHMMSS_mmm.json`（严格匹配），不碰 `env_backup_x.json` 这类用户文件。

### 8. 版本号只在 Task 11 升

前 10 个 Task 保持 5.1.7。Task 11 一次性升到 6.0.0，四处必须一致：`package.json` / `Cargo.toml` / `gui/tauri.conf.json` / `README.md`。

---

## 四、M4 特有的六个陷阱

### 陷阱一：墓碑必须与内容变更在同一时间线比较

`profile` 的内容变更 `field = null`，墓碑 `field = "deleted_at"`。若沿用 M3 的字段级 LWW，两者互不可见，**旧内容会让已删除的配置复活**。

计划 Task 1 已加 `latest_for_entity_tx`：`snapshot` / `profile` 取该实体最新一条变更（不限 field）。Task 4 专门守这个行为。

### 陷阱二：profile content 引用 snapshotUid

`profile` 的 `content.snapshotUid` 指向同一包里的快照。**必须先落快照**。
计划 Task 1 用稳定排序 `snapshot → profile → profile_revision → path_entry → env_value`；测试故意把 profile 放在 snapshot 前面。

### 陷阱三：同名改名的设备后缀要按字符截取

`device_id` 来自外部包，可能是多字节字符。`&device_id[..8]` 会在字符边界 panic。用 `chars().take(8).collect::<String>()`。

改名只影响本地行，**不改变包内 uid**；本地现有同名配置不得被改。

### 陷阱四：清理绝不能删 `exported_at IS NULL`

那是待导出队列，删了就**永久丢变更**。计划里的谓词是：

```sql
exported_at IS NOT NULL AND applied_at > 0 AND applied_at < ?
```

冲突侧只删 `resolved_at IS NOT NULL AND resolved_at < ?`。

### 陷阱五：遗留备份解析不能复用 `read_env_backup`

`backup::read_env_backup` 在解析失败时会经 `persist::parse_error_quarantined` 把文件重命名为 `.corrupt-<ts>`。清理命令如果复用它，**`--dry-run` 就不再是纯读**。

计划 Task 8 用 `serde_json::from_str::<Versioned<EnvBackupPayload>>` + `persist::migrate`，不隔离、不改名。

### 陷阱六：转换顺序不可颠倒

```text
正确：读 .json → 写 .pebak → 回读校验 → 删除 .json
错误：读 .json → 写 .pebak → 删除 .json →（发现写失败，明文已没了）
```

任一失败都保留原文件。

---

## 五、授权与交付纪律

### 1. 本轮授权（用户已明确）

- ✅ 每个 Task 一个提交
- ✅ M4 全部完成、质量门通过后**推送 `main`**
- ✅ Task 11 升版本号到 **6.0.0**
- ❌ **不打 tag**
- ❌ **不创建 GitHub Release**
- ❌ **不修改任何 `CLAUDE.md`**（全局规则；Task 11 只记录 spec §十二 与当前 CLAUDE.md 的冲突，由用户裁断）
- ❌ **不删除文件**（未经用户书面同意）

### 2. 提交

- Conventional Commits：`feat` / `fix` / `test` / `docs` / `chore`。
- **按文件名精确 `git add`，不要 `git add -A`**。
- 每个 Task 一个提交，不要攒成一个大提交。
- 推送只在全部完成后做一次，推送前先跑一遍 `git status` 确认没有意外文件。

### 3. 每个 Task 结束的质量门

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npx tsc --noEmit
npx vitest run
```

**不要攒到最后一起跑。**

### 4. 代码风格

中文注释；Rust 4 空格；TS 2 空格、单引号、100 列；所有 `pub fn` / `pub(crate) fn` 中文 `///`。

### 5. 文本完整性

M3 期间出现过 `#1#1`、`#34#34` 这类重复片段与 U+FFFD 损坏。每次写完文档后扫描：

```powershell
node -e "const t=require('fs').readFileSync(process.argv[1],'utf8');console.log('FFFD='+(t.match(/\uFFFD/g)||[]).length,'DUP='+((t.match(/#(\d+)#\1/g)||[]).join(',')||'none'))" <文件>
```

---

## 六、完成的判定标准

M4 完成时必须同时满足：

1. **11 个 Task 全部实现**，每个 Task 一个提交。
2. 质量门全绿，测试数 **≥ 基线**（340 Rust / 397 前端 / 54 E2E）。
3. `core/tests/` 下每个新增文件**只有一个 `#[test]`**（逐个 `grep -c` 验证）。
4. **配置同步五条**全部有断言：
   - profile 与 revision 落库（Task 1）
   - 同名自动改名 + `renamedProfiles` 计数（Task 2）
   - 墓碑软删除（Task 3）
   - 旧内容不复活（Task 4）
   - 重复导入幂等（Task 5）
5. **清理四条**：
   - 90 天清理只删「已导出且到期」的 `changes`
   - 30 天清理只删「已解决且到期」的冲突
   - `.v5.bak` 归档幂等、不覆盖
   - 遗留 `.json` 转换成功才删除
6. `profile import` 能把 v5 JSON 读回 DB（Task 9）。
7. 未覆盖项表**原样抄进回执**。
8. `npm run verify:all` exit 0。

---

## 七、交付物

完成后写 **`docs/审核和开发/2026.10.21/PathEditor-v6-M4-开发回执.md`**，结构照
`docs/审核和开发/2026.10.21/PathEditor-v6-M3-开发回执.md`：

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

## 八、开工时先做的三件事

1. **确认基线**：跑一遍四个质量门，确认数字与本文件 §二 一致。不一致先报告，不要开始。
2. **读计划的 Self-Review**：确认规格→Task 的映射没有缺口，尤其 §6.5.1 的实体级 LWW。
3. **确认 spec 的新决策**：§6.5 是本次新增；`content` / `seq` / 实体级 LWW / 同名改名 / 清理顺序都在里面。
   **如果你按 M3 的记忆写「profile 不落库」，会把已经修好的问题又写回去。**

---

## 九、边界

出现下面任一情况，**停下来问用户**，不要自作主张：

- 需要**执行**归档/清理（而不是实现它）
- 需要打 tag、创建 Release、删除文件
- 需要修改任何 `CLAUDE.md`
- 发现 spec 本身有结构性缺陷
- M4 范围需要扩张（env N 路对比、CLI 派生表、历史面板缺口、`schemaVersion` 校验、`sync-store` 中文兜底——这些都**明确不做**）
- 某个 Task 的实现与计划的 Interfaces 冲突，且无法在不改变对外形状的前提下解决

---

**开工。** 第一个 Task 是把 profile 真正接进导入管线——它是后面四个同步 Task 的地基。
**特别注意**：Task 1 必须同时改 `APPLICABLE_ENTITY_TYPES`、依赖排序与实体级 LWW 三处，
少任何一处，Task 3 的墓碑与 Task 4 的时序测试都过不了。
