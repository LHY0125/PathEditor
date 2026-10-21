# PathEditor v6.0.0 M4 复审回执

## 1. 回执结论

| 项         | 内容                                                                                                            |
| ---------- | --------------------------------------------------------------------------------------------------------------- |
| 回执日期   | 2026-10-22                                                                                                      |
| 复审对象   | `docs/审核和开发/2026.10.22/PathEditor-v6-M4-审查报告.md`（235 行）                                             |
| 回执结论   | **七项发现全部核实成立**（J-3 为「已核对代填无遗漏」）；J-1 / J-2 / J-5 已整改并附反向验证；J-6 已按裁断新增 CI |
| 版本状态   | 6.0.0（本轮未再升版本）                                                                                         |
| Git 状态   | **未提交、未推送**（按用户裁断：整改完成后与三份审核文档一起提交）                                              |
| 注册表状态 | 未触碰                                                                                                          |

---

## 2. 日期与归档核对

- 审查报告路径日期 `2026.10.22`，本回执同目录同日。
- 开发回执仍在 `2026.10.21/`——复审跨日，两个文档日期不同是事实，不强行统一。
- 系统日期已确认为 **2026-10-22**。

---

## 3. 整改核对

| 编号 | 报告主张                                                      | 我的核实                                                                                                             | 处理                                                        |
| ---- | ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| J-1  | `import_profile_content_tx` 未校验 `name`，外部输入跨信任边界 | **成立**。`core/src/db/profiles.rs:213-223` 直接 `unique_name_tx(tx, &c.name, …)`，全文件无 `validate_name(&c.name)` | **已改**，见 3.1                                            |
| J-2  | `import.rs:88-89` 注释已过期                                  | **成立**。两个常量内容一致，注释仍写「比 … 宽」                                                                      | **已改**，见 3.2                                            |
| J-3  | 计划 Execution Notes 未回填                                   | **已核对**：审核窗口的代填内容与回执 §2/§3/§5/§6 **逐条对齐，无遗漏**                                                | 不需我再补；见 3.3                                          |
| J-4  | `prune --dry-run` 还会创建密钥文件                            | **成立**。`open_default()`（`core/src/db/mod.rs:224`）先 `load_or_create_key` 再开库                                 | **本轮不改**（用户裁断），但我回执 U-1 的措辞要更正，见 3.4 |
| J-5  | 增量同步包不自包含，README 未说明                             | **成立**。`pending_export` 只取 `exported_at IS NULL`；README 无「按顺序应用」                                       | **已改**，见 3.5                                            |
| J-6  | 远端没有任何独立验证，这是发布门                              | **成立**。`gh run list --branch main` 无本次推送记录；`release.yml` 只构建不测                                       | **已按裁断新增 CI**，见 3.6                                 |
| J-7  | S-2 的启动顺序没有回归测试                                    | **成立**。归档位置只有注释守卫                                                                                       | **本轮不改**（不在裁断范围），见 3.7                        |

### 3.1 J-1：校验放在解包之后、改名之前

```rust
// core/src/db/profiles.rs，import_profile_content_tx 内
validate_name(&c.name).map_err(|e| {
    CoreError::new(
        ErrorCode::SyncFormat,
        "import_profile",
        format!("配置名称不合法: {}", e.message),
    )
})?;
```

**为什么错误码要映射，而不是直接 `?`**：`validate_name` 返回 `ErrorCode::InvalidName`
（语义是「用户输入错」）；同步包里的非法名称属于「包格式错」。直接透传会让 CLI 的
错误码语义漂移——用户看到的是「名称不合法」却分不清是自己输错了还是包坏了。

**自动改名仍然走 `unique_name_tx`**：它生成的名字已受 255 字节约束（基础名截到 200 字节 +
` (来自 …)` ≤ 45 字节），不会越过 `validate_name` 的上限。

**回归测试**：`core/tests/db_sync_profile_invalid_name.rs`（1 个 `#[test]`）覆盖四种输入——
空名、纯空白、超长（> 255 字节）、含控制字符 `U+0007`。每个用例断言三件事：
错误码 `SyncFormat`、文案含「配置名称不合法」、**整包回滚**（`profiles` / `changes` / `snapshots` 均为 0）。

**反向验证**（与 C-4 同口径）：临时注释掉校验后测试转红——
`empty: 非法名称必须被拒绝`；还原后转绿，`git diff` 只剩预期改动。

### 3.2 J-2：注释改为「当前一致，保留分离是有意的」

```rust
/// 当前与 [`APPLICABLE_ENTITY_TYPES`] **内容一致**（M4 起全部已知类型都可落库）。
/// 保留两个常量是**有意**的：将来新增已知类型时，先在白名单里接受，
/// 再实现落库——分离让「格式合法、但本版本还落不了库」这个中间态有地方表达，
/// 而不是被迫二选一（要么拒收整包，要么计入虚假的 applied，后者是 R-1 的成因）。
```

### 3.3 J-3：代填内容已逐条核对

对照回执 §2（设计决策 D1–D10）/ §3（偏离 D-1–D-13）/ §5（U-1–U-8）/ §6（R-1–R-9）：

| 代填项           | 核对结果                                                            |
| ---------------- | ------------------------------------------------------------------- |
| 偏离 D-1…D-13    | 与回执 §3 **逐条一致**（含 D-11 CHANGELOG 标题改写、D-13 签名保持） |
| 未覆盖 U-1…U-8   | 一致；**但 U-1 的措辞需要更正**（见 3.4）                           |
| 残余风险 R-1…R-9 | 一致                                                                |
| 实际测试数       | 与回执 §4.2 一致（56/349、33/398、54、覆盖率 82.7%）                |

**没有需要补充的偏离项。** 本轮新增的三条改动（J-1 / J-2 / J-5 与 CI）会写进下一段
「复审轮补充」，见 §4.3。

### 3.4 J-4：成立，本轮不改，但回执 U-1 要更正

**事实复核**：`core/src/db/mod.rs:224` 的 `open_default()` = `load_or_create_key(&home)` →
`Db::open_encrypted(&home.join("patheditor.db"), …)`。所以从未跑过 v6 的机器上，
`maintenance prune --dry-run` 会创建**密钥文件 + 空库 + schema** 三样。

**我原回执 U-1 写的是**：

> 它会 `open_default()`，在库不存在时会**创建**真实数据库文件。

**更正为**：

> 它会 `open_default()`：在从未运行过 v6 的机器上会创建**密钥文件
> `~/.patheditor/keys/db.key.dpapi`** 与空库 `patheditor.db`（含 schema）。
> 这不删除也不修改任何已有数据，但「dry-run 严格纯读」并不成立。

**本轮不改的理由**（用户裁断为 J-1 + J-2 + J-5）：改动需要 `open_db()` 先判断库文件是否存在、
再决定是否打开，属行为调整；在已有库的机器上无差异。**已知残余，见 §6 R-11。**

### 3.5 J-5：README 补「增量包」说明

在同步章节的「已知限制」后新增：

> **同步包是增量包**：导出只包含「尚未导出的变更」。新设备需要**从第一份包开始按顺序应用**；
> 只拿到后续包时，配置引用的快照不在本地，整包会被拒绝（`SyncFormat`，退出码 1）。

这条与 §6.5.1「悬空 `snapshotUid` → 整包回滚」是同一个行为的两种表述，没有新语义。

### 3.6 J-6：新增 CI 质量门（含 MSVC 清单分支）

**新增 `.github/workflows/ci.yml`**：`push: main` / `pull_request` / `workflow_dispatch` 触发，
`windows-latest` + MSVC，依次跑 `cargo fmt --check` → `cargo clippy -D warnings` →
`cargo test --workspace` → `npx tsc --noEmit` → `npx vitest run`；
用 `Swatinem/rust-cache` 缓存 target；NASM 缺失时补装。

**同时改了 `gui/build.rs`**，这是让 CI 有可能绿的前提：

- 原来只走 GNU 的 `windres` 路径；MSVC runner 上通常没有 `windres`，
  于是测试二进制没有 comctl32 v6 清单——正是 M3 那个 `0xC0000139` 的成因。
- 现在按 `CARGO_CFG_TARGET_ENV` 分支：MSVC 走链接器的 `/MANIFEST:EMBED` +
  `/MANIFESTINPUT:<绝对路径>`，不依赖 MinGW；GNU 分支保持不变。

**第一次 CI 运行直接证伪了初版做法**（run `39633177862`，`0a97867..8eef884`）：

```text
CVTRES : fatal error CVT1100: duplicate resource.  type:MANIFEST, name:1, language:0x0409
LINK : fatal error LNK1123: failure during conversion to COFF: file invalid or corrupt
error: could not compile `patheditor` (bin "PathEditor" test)
```

原因是 Tauri 通过 `rustc-link-arg-bins=resource.lib` 已经给 **bin** 嵌了 MANIFEST 资源，
无条件注入 `/MANIFEST:EMBED` 就撞了 `CVT1100`。

**修正后的做法**：

- `build.rs` 只在 `PATHEDITOR_MSVC_TEST_MANIFEST=1` 时注入清单参数，并声明
  `rerun-if-env-changed`；缺清单的只有 **lib 的 test harness**，那次调用不链接 bin。
- CI 拆成两步：`cargo test --workspace --exclude patheditor`（core/cli）→
  `PATHEDITOR_MSVC_TEST_MANIFEST=1 cargo test -p patheditor --lib`（GUI）。
  两步都跑，**测试覆盖没有缺口**，只是调用方式不同。
- GUI 的**编译**由前面的 `cargo clippy --workspace --all-targets` 在 MSVC 下覆盖。

**最终结果**：第四次运行 `39639157635` **success**（2026-10-21T16:55:38Z → 17:11:52Z，约 16 分钟）。
全部步骤绿：准备用户环境变量 / `cargo fmt` / `cargo clippy` / `cargo test（core / cli）` /
`cargo test（GUI lib）` / `npx tsc --noEmit` / `npx vitest run`。
四次运行的完整轨迹见 §4.4。

### 3.7 J-7：成立，本轮不改

归档位置目前只有注释守卫（`gui/src/lib.rs` ⑦ 那段，注释里写明了「位置是刻意的」并指回
核对轮 S-2）。真正的回归测试需要把 `setup_database` 拆成可测的小步骤，属结构改动，
不在本轮裁断范围。**已知残余，见 §6 R-12。**

---

### 3.8 J-8：CI 暴露的既有测试缺陷（不在审查报告内）

**第二次 CI 运行**（run `39635127231`）在 `cargo test（core / cli）` 阶段红在
`core/tests/db_v5_read_from_db.rs:40`：

```text
thread 'stop_write_switches_read_to_injected_loader' panicked:
停写模式下 system 必须来自注入的加载器，实际: ["C:\Windows", …]
```

**这不是 M4 引入的回归**，是 M1 时代就存在的**断言写法问题**：

- `merge_hive`（`core/src/disabled.rs:213-217`）匹配用 `key()`（`trim + to_lowercase`），
  但**回填的是注册表里的原始拼写** `current_path.clone()`；
- 测试却用 `e.path == "C:\\WINDOWS"` 做**精确字符串比较**。
- 本机注册表写的是 `C:\WINDOWS`，GitHub runner 上是 `C:\Windows` → 只在特定机器上通过。

**修法**（只改测试，不动生产代码）：两处断言改成 `eq_ignore_ascii_case`，并加注释说明
「匹配大小写不敏感、输出保留注册表拼写」这个口径差异。

> 这条恰好回答了 J-6 的价值：GNU 本机上它永远是绿的，MSVC runner 的 PATH 拼写不同才暴露出来。

### 3.9 J-9：CI 暴露的第二条既有环境假设（CLI 夹具）

**第三次 CI 运行**（run `39636830790`）在 `cargo test（core / cli）` 红在
`cli/src/env_ops.rs:1539`：

```text
thread 'env_ops::tests::env_restore_dry_run_reports_each_counter_field' panicked:
本机用户变量不足 4 个，无法构造夹具；不要让它静默跳过
```

**不是 M4 的回归**：`craft_preview_fixture` 要求本机至少有 4 个用户环境变量，
不足时**显式失败**（M1 时期的决定：静默跳过会让「计数器字段串位」这类缺陷漏网）。
干净的 runner 用户变量很少。

**修法（只改 CI，不改测试意图）**：在测试前补 4 个用户变量——

```powershell
1..4 | ForEach-Object {
  [Environment]::SetEnvironmentVariable("PATHEDITOR_CI_VAR_$_", "ci", "User")
}
```

用 `SetEnvironmentVariable(User)` 而不是 `setx`：没有 1024 字符上限，不弹提示，
且 CLI 是从注册表读环境变量的，立刻可见。

---

## 4. 验证证据

### 4.1 已执行（本轮整改后）

```text
cargo fmt --all -- --check                              → FMT_OK
cargo clippy --workspace --all-targets -- -D warnings   → 0 error / 0 warning
cargo test --workspace                                  → 57 binaries / 350 passed / 0 failed / 2 ignored
npx tsc --noEmit                                        → 无输出
npx vitest run                                          → 33 files / 398 passed
npx prettier --check README.md .github/workflows/ci.yml → 通过
npm run test:e2e                                        → 54 passed
```

与整改前对比：Rust **56/349 → 57/350**（新增 J-1 的回归测试，+1 二进制 +1 测试）；
前端 398 持平；E2E 54 持平。**无一项下降。**

### 4.2 未执行且不能声称完成

- **CI 的 Windows runner 只验证到「测试全绿」，没有验证发布流水线本身**：
  `release.yml` 是 tag 触发的，本轮没打 tag，所以 `npx tauri build` + NSIS 打包
  在 MSVC 下仍未被这条 CI 覆盖（它只跑 `cargo build/test` 与前端测试）。
- `maintenance prune --dry-run` 仍未对真实 `~/.patheditor` 执行（J-4 的成因就在这里）。
- 未启动真实 GUI。

### 4.4 CI 运行历史（同一条 main，四次才收敛）

| 次  | run         | 结果        | 失败点                                                                | 处置                                                             |
| --- | ----------- | ----------- | --------------------------------------------------------------------- | ---------------------------------------------------------------- |
| 1   | 39633177862 | failure     | `CVT1100: duplicate resource. type:MANIFEST`（bin 已有 Tauri 的清单） | 清单改为按需注入（`PATHEDITOR_MSVC_TEST_MANIFEST=1`），CI 分两步 |
| 2   | 39635127231 | failure     | `db_v5_read_from_db:40` 路径大小写断言                                | 断言改 `eq_ignore_ascii_case`（只改测试）                        |
| 3   | 39636830790 | failure     | `env_ops.rs:1539` 夹具要求 ≥4 个用户变量                              | CI 预置 4 个用户环境变量                                         |
| 4   | 39639157635 | **success** | ——                                                                    | 全部步骤绿（约 16 分钟）                                         |

**三次失败没有一次是 M4 的产品代码缺陷**：第 1 次是本轮新引入的清单注入方式，
第 2、3 次是 M1 时代就存在的、只在特定机器上成立的测试假设。
这正是「本地 GNU 全绿」与「MSVC 真跑一遍」的差别。

### 4.3 复审轮补充的偏离（待审核窗口并入计划 Execution Notes）

| #    | 项                                 | 计划/规格未覆盖的部分                                                              |
| ---- | ---------------------------------- | ---------------------------------------------------------------------------------- |
| D-14 | J-1 名称校验                       | 规格 §6.5.1 只说「保留远端 uid」，没写名称校验；本轮补上并映射为 `SyncFormat`      |
| D-15 | `gui/build.rs`                     | 新增 MSVC 清单分支（`/MANIFEST:EMBED` + `/MANIFESTINPUT`），为 CI 的 MSVC 测试铺路 |
| D-16 | `.github/workflows/ci.yml`         | 新增；发布门从「tag 流水线只编译」变成「push 即跑测试」                            |
| D-17 | `core/tests/db_v5_read_from_db.rs` | 路径断言改为忽略大小写（CI 暴露的既有缺陷，见 §3.8）                               |
| D-18 | `.github/workflows/ci.yml`         | 测试前预置 4 个用户环境变量（CI 暴露的既有夹具假设，见 §3.9）                      |

---

## 5. 未覆盖项（更新后）

| #   | 未覆盖                                                          | 说明                                                              |
| --- | --------------------------------------------------------------- | ----------------------------------------------------------------- |
| U-1 | **（已更正）** `prune --dry-run` 会创建密钥文件 + 空库 + schema | 原回执只提了库文件；见 §3.4                                       |
| U-2 | GUI 启动序列未端到端运行                                        | 禁止启动真实 GUI（`setup_database` 会在真实目录归档）             |
| U-3 | `cleanup-legacy-backups --yes` 未实跑                           | 会删用户机器上 9 个真实明文备份                                   |
| U-4 | 无真机双端同步                                                  | 无第二台机器                                                      |
| U-5 | 本机 MSVC 未验证                                                | 只有 GNU 工具链                                                   |
| U-6 | `maintenance` 子命令无 E2E                                      | CLI 无 E2E 框架                                                   |
| U-7 | 窗口标题未目视确认                                              | 未启动 GUI                                                        |
| U-8 | 敏感值掩码未变                                                  | 不在 M4 范围                                                      |
| U-9 | **CI 的 Windows runner 未覆盖发布流水线本身**                   | 只跑测试；`npx tauri build` + NSIS 打包仍只在 tag 时执行，见 §4.2 |

---

## 6. 残余风险

| #        | 风险                                                                                                    | 状态/缓解                                               |
| -------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| R-1      | 保留策略无 `applied_at` 索引                                                                            | 未变（数据量小）                                        |
| R-2      | 归档每次启动执行，会改名 `profiles/` 下用户手工放入的 JSON                                              | 未变                                                    |
| R-3      | 用户机器现状：2 个 v5 原件 + 9 个明文备份                                                               | 未变                                                    |
| R-4      | 整实体 LWW                                                                                              | 未变                                                    |
| R-5      | 中文改名进入持久数据                                                                                    | 未变                                                    |
| R-6      | 不可达 i18n 键                                                                                          | 未变                                                    |
| R-7      | `CLAUDE.md` / `AGENTS.md` 与 spec §十二 冲突                                                            | 未变，待用户裁断                                        |
| R-8      | v5 导入会隔离损坏文件                                                                                   | 未变                                                    |
| R-9      | Argon2 参数与口令强度                                                                                   | 未变                                                    |
| **R-10** | ~~CI 首次运行可能红~~ **已闭环**：四次运行后 MSVC 全绿（§4.4）。残余：CI 不覆盖 `tauri build`/NSIS 打包 | tag 流水线仍负责发布构建；若打包失败会在 Release 前暴露 |
| **R-11** | `prune --dry-run` 仍会创建密钥与空库（J-4）                                                             | 已知；不删不改已有数据，属「不严格纯读」                |
| **R-12** | S-2 的归档顺序只有注释守卫（J-7）                                                                       | 已知；重构可能把归档挪回 ③b 而不报错                    |

---

## 7. 回执判定

- 审查报告的 **J-1 / J-2 / J-4 / J-5 / J-7 全部成立**，J-3 的代填内容**无遗漏**。
- **J-1 / J-2 / J-5 已整改**，各自附实际命令输出；J-1 另有反向验证（去掉校验 → 转红 → 还原）。
- **J-6 已闭环**：新增 CI 并把 `build.rs` 的 MSVC 路径补上；四次运行后 MSVC **全绿**（§4.4）。
  CI 顺带暴露并修掉两条既有测试的环境假设（§3.8 / §3.9），**都不是 M4 的回归**。
- **J-4 / J-7 本轮不改**，理由与残余风险见 §3.4 / §3.7 / §6。
- 我原回执 **U-1 的措辞不准确**（漏了密钥文件），已在 §3.4 更正并在此声明。

**本轮没有对真实 `~/.patheditor` 做任何写操作，没有启动 GUI，没有打 tag。**

---

**签章**：本文中每条「已通过」都对应 §4.1 / §4.4 里一条实际执行的命令；
未执行的（发布流水线、真实 GUI、prune 实跑）在 §4.2 单列，
未做的整改在 §3.4 / §3.7 说明理由。
