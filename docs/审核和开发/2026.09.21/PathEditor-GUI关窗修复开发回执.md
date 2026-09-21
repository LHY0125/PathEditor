# PathEditor GUI 关窗修复开发回执

- **日期**：2026-09-21
- **执行窗口**：开发窗口（worktree `gui-fix`，分支 `worktree-gui-fix`）
- **实施计划**：`docs/superpowers/plans/2026-09-21-gui-close-fix-implementation.md`（Tasks 1–5）
- **诊断依据**：`docs/审核和开发/2026.09.21/PathEditor-GUI关不掉问题诊断报告.md`
- **范围声明**：本波次只做本地提交，不推送、不出 release、不改版本号；除 WM_CLOSE 进程级冒烟外无任何注册表写入。

---

## 一、任务映射（T1–T5 → commit → 诊断报告建议）

| 任务 | commit | 内容 | 对应诊断报告修复建议 |
| ---- | ------ | ---- | -------------------- |
| T1 | `b934b54` | 关窗确认换 Tauri 异步对话框（`backend.confirmDialog` + `dialog:allow-confirm` 权限 + G-B2 的 `preventDefault` 裁断落实） | P1-1 关窗链路去阻塞化 |
| T2 | `32c24fc` | 其余阻塞式 `window.confirm` 全部换异步对话框（confirmRemoveVar、profile apply/delete） | P1-1 引申：同机制替换防复发 |
| T3 | `2f1dc89` + `b8387e8` | gui bin 改名 `PathEditor` + CI CLI 构建独立 target 目录隔离 + portable zip 取 GUI 本体；`b8387e8` 更正文档中「改名后即无冲突」的 inaccurate 表述（NTFS 大小写不敏感，共享 target 目录仍同名冲突） | P1-2 产物污染根治 |
| T4 | `84e8276` | `onCloseRequested` 关窗回调直接单测（4 用例：无 pending / 确认 / 取消 / IPC 异常兜底） | P2-3 关窗链路测试补底 |
| T5 | 本提交 | 收口质量门 + WM_CLOSE 自动化冒烟 + 发现并修复 **P0 权限回归**（见 §二） + 本回执 | — |

`b8387e8` 同时修复两处事实错误：`.claude/skills/patheditor-scoop-release/SKILL.md` 与 `.agents/skills/` 双副本（zip 内容 exe 名）、`CLAUDE.md` 快速命令区（共享 target 目录下 CLI 构建仍会覆盖 GUI 产物）。

---

## 二、T5 收口冒烟发现 P0 回归：ACL 缺 `core:window:allow-destroy`（本提交核心变更）

### 2.1 发现过程（真实证据，非推测）

T5 Step 2 的 WM_CLOSE 自动化冒烟（诊断报告 §五 A 线方法，进程级，无注册表写入）对新构建连续复现不退出：

| 对象 | 结果 |
| ---- | ---- |
| 新构建（84e8276 + `npx tauri build`） | **3/3 不退出**（WM_CLOSE 后 12s 存活，60s 仍存活） |
| 旧构建（main@145b3a3，同本机 GNU，今日 0:03 构建） | 3/3 正常退出（exit 0） |
| 死锁画像（诊断报告 §二） | 完全吻合：主窗口与 `Tao Thread Event Target` 均响应消息、无任何对话框窗口、进程不退出 |

### 2.2 根因（CDP + ACL 清单证据）

通过 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port` 挂接 CDP 取得决定性证据：

1. `@tauri-apps/api/window.js:1632-1640`：`onCloseRequested` 包装层在 handler 返回后 `if (!evt.isPreventDefault()) await this.destroy()`；
2. `window.js:959-962`：`destroy()` = `invoke('plugin:window|destroy')`；
3. 新构建运行时 ACL **拒绝** `plugin:window|destroy`（CDP 实测报「Command plugin:window|destroy not allowed by ACL」），而 `gui/capabilities/default.json` 从 v5.1（cbf99f1）起**从未声明过 `core:window:allow-destroy`**；
4. 旧代码为何没触发：旧回调 `pending && !window.confirm(...)` 时才 `preventDefault`，无 pending 时同样走 wrapper 的 `destroy()`——**旧构建同样没有 destroy 权限**。旧构建关窗能成功，是因为阻塞式 `window.confirm` 挂起期间 tao 的默认关窗拦截路径在本地 GNU 构建上直接放行（诊断报告 §3.2 确认该路径只在 CI/MSVC 构建出错）。换言之：**阻塞式 confirm 的死锁掩盖了 destroy 权限缺失；异步化让关窗路径真正依赖 `destroy()` IPC，权限缺失随即暴露**。异步化本身没有引入新缺陷，它移除了掩盖。

### 2.3 修复与验证

本提交在 `gui/capabilities/default.json` 增加 `"core:window:allow-destroy"`（与既有 `allow-close` 并列），重建后复测：

| 验证项 | 结果 |
| ------ | ---- |
| WM_CLOSE 三连测（无 pending） | 3/3 正常退出（exit 0） |
| CDP 探测 `plugin:window|destroy` | 放行，进程退出 |
| CDP 触发 `plugin:dialog|message`（`confirm()` 的实际底层命令，`plugin-dialog@2.7.1` 的 `confirm` 实现走 `messageCommand` → `plugin:dialog|message` + `OkCancel` 按钮） | 原生 `#32770` 对话框弹出、主窗口消息循环保持响应（非阻塞确认成立）、可正常关闭 |

### 2.4 关于 `dialog:allow-confirm` 的勘误（计划前提错误，实际无害）

计划 Task 1 Step 1 称「confirm 需单独授权 `dialog:allow-confirm`」。实施后核实（`gui/gen/schemas/acl-manifests.json` + `@tauri-apps/plugin-dialog@2.7.1/dist-js/index.js`）：

- `allow-confirm` 权限在 plugin 2.7.1 中**只是 `allow-message` 的 deprecated 别名**（其 `commands.allow` 数组内容是 `"message"`）；
- 前端 `confirm()` 实际 invoke 的命令是 `plugin:dialog|message`，而 `dialog:default` 已含 `allow-message`——**该命令本来就在 ACL 放行范围内**；
- 因此 `dialog:allow-confirm` 行保留（无害、面向 v3 移除前的兼容），但「漏加该行导致 confirm reject」的计划前提不成立。真正的坑是 §2.2 的 `allow-destroy`。

---

## 三、重大偏离记录（Task 3）

计划原文：「gui crate 显式 `[[bin]] name = "PathEditor"` 后，`target\release\patheditor.exe` 只可能由 CLI 产出，CI 引用保持不变」。

实际偏离：**计划前提「bin 改名即分离产物」在 Windows 不成立**。NTFS 大小写不敏感，`PathEditor.exe` 与 `patheditor.exe` 是**同一个目录项**——gui bin 改名后，CI 若仍把 CLI 构建到共享 `target\release`，CLI 链接出的 `patheditor.exe` 会直接覆盖 GUI 的 `PathEditor.exe`（复审独立复现）。两轮修复：

1. `2f1dc89`：gui bin 改名保留（与 tauri productName 对齐），CI CLI 构建改用独立 target 目录 `--target-dir target/cli`，portable zip 明确取 `target\release\PathEditor.exe`（GUI 本体）；
2. `b8387e8`：更正 CLAUDE.md/AGENTS.md 中「改名后即不再同名冲突」的错误表述，以及 scoop-release skill 双副本中的产物名（zip 内容为 `PathEditor.exe` + `WebView2Loader.dll`）。

本机实验证据：共享 target 目录下先 CLI 后 tauri build 复现覆盖（GUI 产物 MD5 == CLI）；改用 `--target-dir target/cli` 隔离后两产物 MD5 不同、大小差异显著（GUI ≈20MB / CLI ≈3MB）。

---

## 四、Minor 处置汇总（三态：已修 / 登记 / 豁免）

**权威清单**（与各波审查报告的登记项对账）：

### T1（关窗异步化）

| 项 | 处置 | 说明 |
| -- | ---- | ---- |
| 工具栏「取消」有双重确认风险（工具栏取消 + 关窗确认两条路径） | **登记** | 预存行为，本波不扩大改动面；观察用户反馈后再收敛 |
| `catch` 冗余的 `destroy().catch(() => {})` 注释可更精炼 | **登记** | 风格类，现注释已表达意图 |
| describe 覆盖口径（「关窗确认纳入环境变量草稿」改为异步版描述） | **视为已修** | T4 补了直接覆盖，口径问题被实质消解 |

### T2（其余 confirm 替换）

| 项 | 处置 | 说明 |
| -- | ---- | ---- |
| e2e mock 缺 `plugin:dialog|confirm`（若未来 e2e 覆盖删除确认流会报 mock 缺失） | **登记** | 当前 E2E 不覆盖该流，暴露时再加 |
| `await-helper` 风格（`.then/.catch` vs `async/await` 混用） | **豁免** | 与既有 `ask()` 先例（use-profiles.ts）保持一致，一致性优先 |

### T3（bin 改名 / 产物隔离）

| 项 | 处置 | 说明 |
| -- | ---- | ---- |
| CI 冷构建时间（`--target-dir target/cli` 使 CLI 无法与 GUI 共享编译缓存） | **登记** | 加 CI 缓存时把 `target/cli` 纳入缓存路径一并处理 |
| scoop 清单过渡期风险（v5.1.3 清单 hash 针对旧 zip，shortcuts 已改 `PathEditor.exe` 但对旧 zip 无效） | **登记** | 见 §五，过渡期勿手动 `scoop update` |
| SKILL.md 双副本旧产物名 | **已修** | `b8387e8`（`.claude/skills` 与 `.agents/skills` 双副本同步） |
| CLAUDE.md:24 快速命令区事实错误（改名后仍同名冲突） | **已修** | `b8387e8`（改为「共享 target 目录下仍覆盖，隔离构建须 --target-dir target/cli」） |
| `.gitignore:42` 忽略 `CLAUDE.md` 致 Prettier/工具链漂移（文件已被跟踪，ignore 行是 git 空操作，危害在误导 lint-staged 等工具） | **登记** | 建议后续 un-ignore（删除该行），本波不动——与 lint-staged 配置联动，需单独小改 |

### T4（关窗链路测试）

| 项 | 处置 | 说明 |
| -- | ---- | ---- |
| 50ms settle（「用户取消」用例用 `setTimeout(50)` 断言未 destroy，非零窗口期） | **豁免** | jsdom 下无更精确的信号；失败会以 expose 的 destroy 调用暴露 |
| case 2 文案断言（确认弹窗文案断言较脆弱） | **登记** | i18n key 稳定后可改断言 key 而非文案 |
| `fireClose` 在 describe 内重复定义 | **豁免** | 单文件局部助手，抽取反而增加间接层 |

### T5（本任务新增）

| 项 | 处置 | 说明 |
| -- | ---- | ---- |
| `core:window:allow-destroy` 权限缺失（P0） | **已修** | 本提交；见 §二 |
| `dialog:allow-confirm` 计划前提错误（实为 deprecated 别名） | **已修（勘误记录）** | 权限行保留无害，计划文本以本回执 §2.4 更正 |

---

## 五、跨仓改动记录（G-B1）

`D:\settings\settings\Scoop\buckets\lhy\bucket\patheditor-gui.json`：

- `shortcuts` 已改指 `PathEditor.exe`（与 portable zip 新内容一致）；
- **bucket 仓库 git 未提交**（`M bucket/patheditor-gui.json`），提交与否由用户决定；
- `version` 与 `hash` 字段未动：清单仍指向 v5.1.3 旧 zip（内容里是 `patheditor.exe`），shortcuts 改动**只对下一个版本发布后的 autoupdate 生效**；
- **「本波次不出 release，勿手动 `scoop update patheditor-gui`」**——旧 zip 内容 + 新 shortcuts 会导致快捷方式指向不存在的文件，这正是 G-B1 要防的事故。

---

## 六、质量门终态数字

口径：本 worktree（84e8276 + capabilities 修复后的最终代码）、本机（Windows 11 26200、GNU 工具链 rustc 1.95.0、Node 20）、`npm run verify:all` 一次性通过。

| 门 | 数字 |
| -- | ---- |
| Prettier / ESLint / tsc / vite build | 通过（ESLint 0 错误 3 警告，均为既有 TanStack Virtual `incompatible-library` 警告） |
| Vitest | 18 文件 / **240 用例全过**（T4 新增 4 用例包含在内） |
| 覆盖率 | 行 **88.01%**（门槛 80%）；分支 76.03%、函数 90.14%、语句 86.18% |
| cargo fmt / clippy（-D warnings） | 通过 |
| cargo test --workspace | **192 通过**（core 151 + CLI 41），2 ignored（均有 `#[ignore]` 注明原因：winreg lossy 解码不可达分支 / 需真实注册表写权限的隔离键测试） |
| Playwright E2E | **24 passed**（mock IPC，生产构建 + 2 workers，17.0s） |
| `npx tauri build` | 成功；NSIS `PathEditor_5.1.3_x64-setup.exe` 4,525,302 字节；`target\release\PathEditor.exe` **20,939,140 字节（≈20MB，无产物污染）** |

时间点：第一轮 verify:all 于 17:03 完成全绿；capabilities 修复后第二轮 verify:all 重跑确认全绿（数字以第二轮为准）；tauri build 于同日完成两轮（第二轮含 allow-destroy）。

### 未覆盖项（如实列明）

1. **真实 GUI 手工冒烟待用户**：点 X / 编辑后关窗确认 / 取消保留 / 无挂起四步交互未由人手执行（见 §七口径声明）。
2. **CI（MSVC）构建环境验证待下版发布**：死锁只复现于 CI/MSVC 构建，本机 GNU 构建无法等价验证。
3. profile hook（`use-profiles.ts`）无专属测试文件，走计划的降级路径（全量回归 + `window.confirm` 归零 grep）。
4. E2E 不覆盖关窗与删除确认流（mock IPC 无原生窗口事件；确认对话框为原生 `#32770`，Playwright 页面内不可见）。
5. 「有草稿 + 用户取消/确认」的端到端进程级冒烟未做（需写注册表制造草稿，违反本波无注册表写入约束）；该分支由 T4 单测覆盖（jsdom 层）。

---

## 七、G-N2 措辞（强制口径）

**本机 GNU 冒烟 ≠ CI 构建环境验证。**

诊断报告 §二复现矩阵：CI 版 GUI 死锁 3/3、本地 GNU 构建 2/2 正常——死锁只复现于 CI/MSVC 构建。本波修复消除了死锁前提（JS 线程不再被阻塞式 `window.confirm` 挂起），并在本机 GNU 构建上验证了 WM_CLOSE 退出与异步确认对话框弹出（见 §六/§二）；但 **CI 环境的最终验证只能等下一次真实发布构建后由用户侧确认**。本回执与任何后续文档不得写成本修复「已在 CI 环境验证」。

另外必须如实记录：本波修复的收口冒烟（WM_CLOSE 自动化）恰恰发现了异步化后暴露的 `allow-destroy` 权限缺失（§二）——这证明自动化冒烟环节有真实价值，也说明「异步化 = 修复完成」的假设需要构建级验证兜底。

---

## 八、Execution Notes（计划原文 / 实际 / 处理）

| 计划原文 | 实际 | 处理 |
| -------- | ---- | ---- |
| Task 5 Step 2：tauri build 后手工冒烟（点 X 等 3 步） | 执行窗口无法点 GUI，改为 WM_CLOSE 自动化冒烟 + CDP 对话框验证 | 冒烟发现 P0 权限回归并修复（§二）；手工冒烟仍留作用户清单 |
| Task 1 Step 1：`dialog:allow-confirm` 为 confirm 必需权限 | 实为 `allow-message` 的 deprecated 别名，`dialog:default` 已覆盖真实命令 | 权限行保留，勘误见 §2.4 |
| Task 3：bin 改名后两产物永不互相覆盖 | NTFS 大小写不敏感使前提不成立，追加独立 target 目录方案 | `2f1dc89` + `b8387e8`（§三） |
| T5 冒烟预期「通过」 | 首轮冒烟不通过（新构建 3/3 不退出 vs 旧构建 3/3 退出） | STOP → CDP 定位根因 → 修复 → 复测通过（§二） |
