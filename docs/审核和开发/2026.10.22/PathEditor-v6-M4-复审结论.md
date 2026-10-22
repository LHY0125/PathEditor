# PathEditor v6.0.0 M4 复审结论

- **复审日期**：2026-10-22
- **被审对象**：`main` @ `37075cf`（与 `origin/main` 一致）
- **复审依据**：`PathEditor-v6-M4-审查报告.md`（235 行，2026-10-22）
- **整改回执**：`PathEditor-v6-M4-复审回执.md`（310 行，2026-10-22）

## 一、判定

**通过。** 审查报告的七项发现全部核实并闭环；J-1 / J-2 / J-5 已整改，J-6 新增 CI 并四次运行收敛，
J-3 的代填内容无遗漏，J-4 / J-7 作为已知残余接受。

**决定：同意打 `v6.0.0` tag。** 唯一未预演的环节是 MSVC 下的 `npx tauri build` + NSIS 打包
（见 §五），判断为低风险且可恢复。

## 二、独立验证（最终树，审核窗口执行）

| #   | 命令                                                    | 结果                                                                                        |
| --- | ------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| 1   | `cargo fmt --all -- --check`                            | exit 0                                                                                      |
| 2   | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0                                                                                      |
| 3   | `cargo test --workspace`                                | **57 binaries, 350 passed, 0 failed, 2 ignored**                                            |
| 4   | `npx tsc --noEmit`                                      | exit 0                                                                                      |
| 5   | `npx vitest run`                                        | **33 files, 398 passed**                                                                    |
| 6   | `npm run test:e2e`                                      | **54 passed (1.8m)**                                                                        |
| 7   | `git rev-parse HEAD` vs `origin/main`                   | 一致（`37075cf`）                                                                           |
| 8   | `git tag --list`                                        | 无 `v6.0.0`（未打）                                                                         |
| 9   | CI `39641907325`                                        | **success**，18 个步骤全部 success（含 MSVC fmt / clippy / 两组 cargo test / tsc / vitest） |
| 10  | CI `39639157635`                                        | success（冷缓存，16m14s）                                                                   |

与复审回执 §4.1 完全一致：Rust 56/349 → 57/350（+1 J-1 回归测试），前端 398 持平，E2E 54 持平。

## 三、七项发现闭环

| #                        | 状态         | 核实                                                                                                                                                                                     |
| ------------------------ | ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| J-1 名称校验             | **已改**     | `profiles.rs:221-234` 在解包后、改名前后调用 `validate_name`，错误码映射 `SyncFormat`；新增 `db_sync_profile_invalid_name.rs`（空名/空白/超长/控制字符，均断言整包回滚）；反向验证已记录 |
| J-2 过期注释             | **已改**     | `import.rs:87-90` 改为「当前一致；保留分离是有意的」，解释了 R-1 的中间态                                                                                                                |
| J-3 Execution Notes      | **已核对**   | 代填内容与回执 §2/§3/§5/§6 逐条一致；本轮已把 D-14–D-18 并入计划                                                                                                                         |
| J-4 prune dry-run 建密钥 | **接受残余** | 复审回执 §3.4 更正了 U-1 措辞；R-11 登记                                                                                                                                                 |
| J-5 增量包说明           | **已改**     | README 同步章节新增「从第一份包开始按顺序应用」及 `SyncFormat` 后果                                                                                                                      |
| J-6 CI 发布门            | **已闭环**   | 新增 `ci.yml`；MSVC 下四次运行收敛；顺带修掉两条既有环境假设（见 §四）                                                                                                                   |
| J-7 顺序回归测试         | **接受残余** | R-12 登记                                                                                                                                                                                |

## 四、CI 复核

`gh run view 39641907325` 显示 18 个步骤全部 success，包括：

```text
cargo fmt / cargo clippy
cargo test（core / cli）
cargo test（GUI lib，PATHEDITOR_MSVC_TEST_MANIFEST=1）
npx tsc --noEmit / npx vitest run
```

三次失败的原因与回执 §4.4 一致，且**都不是 M4 的产品代码缺陷**：

1. `CVT1100`：初版无条件注入 MSVC 清单，与 Tauri 的 `resource.lib` 撞车 → 改为按需注入。
2. `db_v5_read_from_db`：路径大小写断言只在特定机器成立 → `eq_ignore_ascii_case`。
3. `env_ops` 夹具：干净 runner 用户变量不足 4 个 → CI 预置 4 个。

**这三次失败正是「本地 GNU 全绿」与「MSVC 真跑一遍」的差别**，也是 J-6 的价值所在。

## 五、tag 决定与已知残余

### 5.1 已核对的发布前置

- 版本四处一致：`package.json` / `Cargo.toml` / `gui/tauri.conf.json` / README，均为 6.0.0；
  两个锁文件同步。
- `CHANGELOG.md` 的 `## 6.0.0 (2026-10-21)` 能被 `release.yml` 的版本段落正则命中。
- 待打 tag 的提交 `37075cf` 已有全绿 CI（`39641907325`），发布树 = 已测试树。
- `release.yml` 的版本一致性校验会读 `package.json` / `tauri.conf.json` / `Cargo.toml`，三处均匹配。

### 5.2 唯一未预演环节

**MSVC 下的 `npx tauri build` + NSIS 打包没有任何本地或 CI 证据。**
CI 只跑测试（`cargo test` / `tsc` / `vitest`），不跑 Tauri 打包流水线。

**风险评估：低。**

- `gui/build.rs` 的 M4 改动只在 `PATHEDITOR_MSVC_TEST_MANIFEST=1` 时注入清单；
  发布构建不设该变量，bin 仍由 Tauri 的 `resource.lib` 提供清单——与 v5.1.7 的发布路径一致。
- 发布工作流在 MSVC 下已多次成功（v5.1.7）。
- 若打包失败：`release.yml` 的「创建 GitHub Release」在构建与产物整理**之后**，
  因此**不会留下一个空 Release**；修复后可删除并重推 tag（不要用 `gh release create` 手工补发）。

若要把风险降到接近零，可选：给 `release.yml` 加 `workflow_dispatch` + `dry_run` 输入，
先跑一次不创建 Release 的 MSVC 打包。**非必需，不阻塞本次决定。**

### 5.3 非阻塞的后续观察

| #   | 观察                                                                                                    | 建议                                                       |
| --- | ------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| N-1 | 自动改名的设备后缀取自外部 `device_id`（前 8 字符），未单独过滤控制字符；`validate_name` 只校验原始名称 | v6.0.1 把后缀限制为 `[A-Za-z0-9-]`，或对最终名称再校验一次 |
| N-2 | CI 不跑 Playwright E2E（本地 54 条全绿）                                                                | 可在后续把 E2E 加入 CI；当前不影响发布证据                 |
| N-3 | 发布流水线本身未预演（§5.2）                                                                            | 接受，或加 dry-run dispatch                                |
| N-4 | `gui/build.rs:40-41` 的「MSVC 侧则是把两份清单合并」在按需注入后已不准确                                | 注释修正，非功能问题                                       |
| N-5 | `CHANGELOG` 标题日期是 2026-10-21，tag 若在 10-22 打                                                    | 可选：把标题日期改为实际发布日                             |

## 六、审核窗口本轮的文档修订（未提交）

1. `docs/superpowers/plans/2026-10-21-v6-sqlite-m4-plan.md`
   - Execution Notes 追加「复审轮补充」：D-14–D-18、U-5 更新、U-9、R-10–R-12、57/350 与 CI 四次轨迹。
2. `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` §6.5.1
   - 新增「名称校验」条目：`profile.name` 过本地同一套校验，非法名称 → `SyncFormat` 整包回滚。

两份文件均过 Prettier，U+FFFD 为零。

## 七、下一步

- **打 tag 是用户动作**，需要你明确授权；审核窗口不执行。
- 建议命令（确认后执行）：

```powershell
git tag -a v6.0.0 -m "PathEditor v6.0.0"
git push origin v6.0.0
```

- 推送 tag 后**等待 CI 完成**，不要手工 `gh release create`（2026-09-18 事故教训）。
- 若打包失败：修复 → 删除本地/远端 tag → 重推；释放前不要动 Release。

## 八、本轮未执行

- 未打 tag、未创建 Release、未修改任何 CLAUDE.md。
- 未对真实 `~/.patheditor` 执行 prune / cleanup 实跑，未启动真实 GUI。
- 未预演 MSVC 打包（§5.2）。
- 未做真机双端同步。

---

**审核窗口签章**：§二 的每条结果均来自最终树上的独立命令；§三 的每条闭环均附 `file:line`；
§五 的发布判断基于 `release.yml` 的实际步骤顺序与 `gui/build.rs` 的最终分支。
