# PathEditor 对话框重皮与过渡别名清理（Wave 4.1）审查报告

- **审查日期**：2026-10-19
- **被审分支 / 提交**：`feat/dialog-restyle` @ `6f66e66`
- **基准点**：`main` @ `c2c3c7c`（含 spec/plan 与核对轮裁断）
- **范围**：7 个提交（6 个代码 + 1 个文档）；22 个代码文件 **+439 / −309**，另 2 份文档 +168 / −1
- **功能**：把 `src/components/dialogs/` 下 14 个对话框迁移到 5.1.7 设计系统，并删除三个过渡别名、`.tab-active`、`btnClass` / `btnStyle`
- **规格来源**：`docs/superpowers/specs/2026-10-19-dialog-restyle-design.md`（`c2c3c7c` 修订版）
- **计划来源**：`docs/superpowers/plans/2026-10-19-dialog-restyle-implementation.md`（`c2c3c7c` 修订版）
- **规范来源**：`CLAUDE.md` / `AGENTS.md`、`CONTRIBUTING.md`
- **复现命令**：`git diff main...feat/dialog-restyle`

## 提交清单

```text
6f66e66 docs: Wave 4.1 开发回执与计划执行记录
dc0948a test(e2e): 新增对话框视觉回归
bafb14f refactor: 删除对话框过渡设施并反转测试断言
644899d style: 备份恢复面板迁移至设计系统
ed576f8 style: 分析与配置对话框迁移至设计系统
60aa225 style: 表单类对话框迁移至设计系统
f809306 feat: 对话框基础设施（token / Modal 外壳 / 表单与列表工具类）
```

提交类型全部落在 commitlint 允许集合内，`git diff --check` 无空白错误。

## 一、验证证据

全部命令在 `feat/dialog-restyle @ 6f66e66` 上由审核窗口实际执行，不采信回执数字。

| 检查            | 命令                                                                                                                            | 结果                                                                                                   |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| 全量质量门      | `npm run verify:all`                                                                                                            | ✅ **退出码 0**                                                                                        |
| 单元测试        | `npx vitest run`                                                                                                                | ✅ **320 passed / 25 files**                                                                           |
| 覆盖率          | `npm run test:coverage`                                                                                                         | ✅ 行 **87.58%**（statements 85.81 / branches 75.72 / functions 88.8）                                 |
| ESLint          | `npm run lint`                                                                                                                  | ✅ **0 error / 3 warning**（均为既有 `@tanstack/react-virtual` 警告）                                  |
| Rust core       | `cargo test --workspace`                                                                                                        | ✅ **227 passed / 2 ignored**                                                                          |
| Rust CLI        | 同上                                                                                                                            | ✅ **65 passed**                                                                                       |
| E2E             | `npm run verify:all` 末段                                                                                                       | ✅ **41 passed**                                                                                       |
| 验收 grep 1     | `grep -rn -- '--app-list\|--app-select-row' src/`                                                                               | ✅ 空（exit=1）                                                                                        |
| 验收 grep 2     | `grep -rn "btnClass\|btnStyle" src/`                                                                                            | ✅ 空（exit=1）                                                                                        |
| 验收 grep 3     | `grep -rn "tab-active" src/`                                                                                                    | ✅ 空（exit=1）                                                                                        |
| 验收 grep 4     | `grep -rnE "#3b82f6\|#2563eb\|#ef4444\|#22c55e\|#dc2626\|text-red-500\|text-green-600\|text-yellow-500" src/components/dialogs` | ✅ 空（exit=1）                                                                                        |
| 残留透明度      | `grep -rn "opacity" src/components/dialogs/`                                                                                    | ✅ 空（D6 执行得比要求更彻底）                                                                         |
| 命名色 / 短 hex | 扩展扫描                                                                                                                        | ✅ 空                                                                                                  |
| 零跨层改动      | `git diff --stat main...feat/dialog-restyle`                                                                                    | ✅ `core/`、`gui/`、`cli/`、`src/services/`、`src/store/`、`src/core/`、`src/i18n/` 均无差异；零新依赖 |
| 契约属性        | `grep data-testid / data-*`                                                                                                     | ✅ 4 个 `data-testid` 与 `data-backup-file` / `data-hive-filter` / `data-env-var-key` 全部保留         |
| 视觉截图        | 读取开发窗口 job 目录 `…\jobs\74051296\tmp\wave41-shots\` 下 8 张 PNG                                                           | ✅ 浅 / 深各 4 张逐张目视通过：无透明底、文字可读、按钮层级正确                                        |

> **绿灯 ≠ 规格全覆盖。** 上述结果证明质量门与机械验收通过，但不证明视觉观感、真机 WebView2 与缩放场景已被覆盖（见 §三 未覆盖项）。
>
> **关于子代理复核**：本轮曾按 skill 4.4（可选步骤）派独立 `code-reviewer` 子代理复核，但该 agent 无 Bash 工具、无法执行 `git diff`，运行约 8 分钟未产出结论后停止。本报告的全部结论均来自审核窗口亲自执行的命令与逐文件核对，不依赖该子代理。

## 二、Standards 轴

### 硬性违反

**无。**

- 零 Rust / IPC / store / `src/core/` / i18n 改动，符合 spec「安全边界」。
- 组件未直调 `invoke`；未新增依赖。
- 代码注释为中文；Prettier / ESLint / tsc 全部通过。
- Conventional Commits 合规；`git diff --check` 干净。

### 判断性意见

| #   | 位置                                                                      | 现象                                                                                                                                                       | 为什么重要                                                                                                                                                     | 建议                                                                                                                                                            |
| --- | ------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S1  | `src/styles/globals.css:12`                                               | 注释仍写「预留：浮起表面……**当前无引用**，刻意保留」，但 `.modal-surface` 已在 `globals.css:588` 引用 `--app-surface-raised`                               | 仓库规范明确要求「行文与实现相符」；这条注释会误导下一位维护者以为它是死变量                                                                                   | 改为「对话框抬升表面（Wave 4.1 起由 `.modal-surface` 使用）」                                                                                                   |
| S2  | `src/components/dialogs/profile/ProfileDetail.tsx:68`                     | `className="field-input text-xs"`。`.field-input` 是**未分层规则**且声明 `font-size: 13px`，Tailwind 的 `text-xs` 在 `@layer utilities` 中，**永远不生效** | 与开发窗口自己总结的「未分层规则覆盖 Tailwind utilities」是同一机制；这里得到的是 13px 而非 12px，类名与意图不符                                               | 删除 `text-xs`（与 D4 的 13px 一致），或从 `.field-input` 移除 `font-size`                                                                                      |
| S3  | `tests/unit/dialog-tokens.test.ts:28`                                     | `HARDCODED_COLOR = /#[0-9a-fA-F]{6}\b\|rgba?\(/` 只匹配 6 位 hex 与 `rgb/rgba`；漏 3/4/8 位 hex、`hsl/hsla`、Tailwind 命名色（`text-white` 等）            | 当前源码经扩展扫描确认干净，**不是现存缺陷**；但守卫的声称强度高于实际能力，未来可能假绿                                                                       | 扩展为 `#[0-9a-fA-F]{3,8}\b\|rgba?\(\|hsla?\(`，并补一条命名色类扫描                                                                                            |
| S4  | `tests/unit/dialog-tokens.test.ts:23-25`                                  | `read()` 用 `.replace(/\/\/[^\n]*/g, '')` 剥行注释，会**截断字符串字面量里的 `//`**（如 URL）                                                              | 当前 14 个文件无此情况；未来若加入含 `//` 的字符串，`//` 之后的内容会被当作注释剥掉，可能漏检旧 token                                                          | 改为只匹配行首注释（`/^\s*\/\/.*$/gm`），或在注释中登记该限制                                                                                                   |
| S5  | `e2e/tests/dialog-visual.spec.ts:26`                                      | 计算样式里收集了 `padding`，但断言只覆盖 `background` 与 `radius`，`padding` 从未使用                                                                      | D1 明确要求 `padding: 20px`，这条用例对它零覆盖；收集后不断言属于遗漏而非刻意                                                                                  | 补 `expect(styles.padding).toBe('20px')`                                                                                                                        |
| S6  | `e2e/tests/dialog-visual.spec.ts:14-30`                                   | 深色用例通过 `localStorage.darkMode='1'` 切主题，但断言只有「背景非透明」与「圆角 10px」——**这两个条件在浅色主题下同样成立**                               | 若 `initDarkMode()` 失效或键名变化，深色用例会静默退化为浅色用例并继续通过（假绿）。审核窗口已核实键名 `darkMode` 正确、当前确实生效，但**断言本身不能证明它** | 深色用例补 `expect(await page.locator('html').getAttribute('class')).toContain('dark')`，或直接断言深色表面色 `rgba(30, 30, 40, 0.92)`；浅色用例断言不含 `dark` |
| S7  | `docs/审核和开发/2026.10.19/PathEditor-dialog-restyle开发回执.md:131-132` | §8 写「领先 `origin/main` 2 个提交」「`feat/dialog-restyle @ dc0948a`」「6 个提交」                                                                        | 实际为领先 **3** 个提交、分支 **@ 6f66e66**、共 **7** 个提交（回执自身那个提交未计入）。数字口径错误会让后续窗口误判分支状态                                   | 回执 §8 更新为实际值（回执成文于自身提交之前，属可理解但应订正）                                                                                                |

### 合规亮点

- **零跨层改动**：`core/`、`gui/`、`cli/`、`src/services/`、`src/store/`、`src/core/`、`src/i18n/` 全部无差异，严格守住「纯前端视觉迁移」边界。
- **B1 教训被正确应用**：改 `Modal` DOM 的同一个 Task 内同步迁移了 `crud-undo.spec.ts:13` 与 `keyboard.spec.ts:13` 的选择器，没有把失败推迟到最后的 `verify:all`。
- **H1–H4 全部按裁断落实**：创建备份为 secondary、`.modal-surface` 90vw、表单标签 13px、文字 opacity 全删（实际整个 dialogs 目录零 `opacity`，比 D6 要求更彻底）。
- **守卫测试口径正确**：`dialog-tokens.test.ts` 先剥注释再匹配，与 `design-tokens.test.ts` 的 B4 口径一致；8 个用例覆盖 14 个 `.tsx`。
- **删除配套反转**：`design-tokens.test.ts` 的三别名与 `.tab-active` 断言已反转，`button.test.tsx` 的兼容导出用例整块删除、import 同步收窄，没有留下空 `describe`。
- **接口稳定**：`Modal` 的 `{ open, onClose, children }` 不变，8 个调用方零改动；`data-testid` 与 `data-*` 契约全部保留。
- **诚实披露**：E1（剥注释正则按意图重写）、E2（测试数口径）、E3（截图方式）三项偏离均在 Execution Notes 与回执中如实登记，经复核属实且处理正确。
- **回执数字全部属实**：320 / 25、87.58%、41 E2E、227+2 / 65、0 error / 3 warning 与独立执行结果完全一致。

## 三、Spec 轴

### (a) 规格要求但缺失或只做一半

1. **对比度验收未达标：`--app-success` 浅色值仅 3.30:1。**
   - 实测：`#16a34a` on `#ffffff` = **3.30:1**；on `#fafafa` = **3.16:1**。WCAG AA 对正常字号要求 4.5:1。
   - 它被用在两处**正常文字**：`EnvBackupPanel.tsx:37`（创建备份成功提示，`text-xs`）与 `ConflictsTab.tsx:33`（最高优先级目录，`text-xs`）。`ProfileDetail.tsx:107` 的 `●` 是装饰性图形，3.30:1 满足非文字 3:1 门槛，不算问题。
   - 旧值 `#22c55e` 为 2.28:1，所以本波是**改善但未达标**；spec D2 选值时沿用了 Tailwind `green-600` 的取值，把旧缺陷原样搬进了新 token。
   - spec 验收标准要求「无低对比度文字」，因此这一条按规格判定**未通过**。
   - **裁断（spec D2 修订）**：浅色 `--app-success` 改为 **`#15803d`**（实测 5.02:1 on 白、4.81:1 on `#fafafa`）；深色 `#4ade80`（9.60:1）不变。
   - 口径说明：spec D6 已明确接受 `--app-fg-subtle` 浅色 4.02:1（5.1.7 既有值），故本报告只把**本波新增**的 `--app-success` 3.30:1 判为未达标；若严格按 AA 审视，`--app-fg-subtle` 也应整体上调，但那属于 5.1.7 的既有取舍，不在本波范围。
2. **双主题截图已独立复核（证据缺口已关闭）。**
   - 审核窗口在开发窗口 job 目录 `C:\Users\33644\.claude\jobs\74051296\tmp\wave41-shots\` 找到 8 张截图，逐张目视核查：浅 / 深各 4 张（PathEdit / Analyze / Profile / EnvBackup），均为不透明抬升表面、圆角一致、按钮层级正确（确认 / 保存 / 恢复为 primary，取消 / 立即备份为 secondary），未观察到明显低对比度文字。
   - 时序核实：截图时间 **16:24**，晚于最后一次视觉改动 `644899d`（16:20），早于仅含测试的 `dc0948a`（16:28），可代表最终视觉状态。另有 2 张 `task1-*` 快照（16:15）早于对话框迁移，不用于最终判定。
   - 局限：截图只覆盖 4 个代表性对话框，未覆盖其余 10 个；且位于临时目录、未进仓库。本轮已消费该证据，但它不可作为后续可追溯的仓库内证据。

### (b) 规格没要求但做了的行为（scope creep）

| 行为                                                    | 评价                                                                                                                      |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `ProfileDialog` 关闭按钮补 `aria-label`                 | 正向的可访问性改进，无风险                                                                                                |
| `HelpDialog` 正文 `<pre>` 改用 `--app-fg-muted`         | 计划内明确写法；对比度 4.83:1（浅）/ 4.96:1（深）仍达 AA。若视为「正文」按 D6 应为 `--app-fg`，属视觉层级判断，不构成缺陷 |
| `.field-input::placeholder` / `.dialog-list-item:hover` | 计划 Task 1 已包含，非计划外                                                                                              |

### (c) 实现与规格/计划不符

- **代码层面无偏差**：D1–D9 逐条核对通过；H1–H4 的裁断全部落实；B1 的 E2E 选择器迁移已执行。
- **E1 / E2 / E3**：开发窗口已如实登记，经复核处理正确（见 §二 合规亮点）。
- **计划文档自身缺陷（审核窗口责任）**：`docs/superpowers/plans/2026-10-19-dialog-restyle-implementation.md:1543-1544` 的剥注释代码块是无效语法（`.replace(//*[sS]*?*//g, '')` / `.replace(///.*$/gm, '')`）。开发窗口 E1 的披露属实；实现采用等价且正确的 `/\/\*[\s\S]*?\*\//g`。**审核窗口已在本轮修正该代码块**；`dialog-tokens.test.ts` 中「计划原文的转义在文档传输中被吞」的注释建议开发窗口在整改轮同步更新。
- **回执 §8 数字过期**：见 S7。

### 对开发回执 §6 重点项的答复

| 回执项                                   | 答复                                                                                                                                                                                             |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1. E1 偏离                               | **属实且处理正确**。实现采用 `.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '')`，与 `design-tokens.test.ts` 的 B4 口径一致；计划文档自身的无效语法已由审核窗口修正（`:1543-1544`）。 |
| 2. `.modal-surface` 的 `backdrop-filter` | **接受**。spec D1 未单列该属性，但沿用菜单毛玻璃的既有做法；浅色下表面不透明、无视觉副作用，深色下生效，不构成偏差。                                                                             |
| 3. 创建备份为 secondary                  | **符合 H1 裁断**。`EnvBackupPanel` 内「恢复」是唯一会改注册表的动作，也是唯一 `primary`。                                                                                                        |
| 4. 守卫只扫描 14 个 `.tsx`               | **接受**。三个 `.ts` hooks 不含样式，扫描边界正确；`dialog-tokens.test.ts` 的文件数断言下限 14 合理。                                                                                            |
| 5. 未推送                                | **已核对**：`main` 领先 `origin/main` 3 个提交，`feat/dialog-restyle` 领先 `main` 7 个提交，均未推送。                                                                                           |

### 未覆盖项（照实登记，不替开发窗口兜底）

- **真机 Tauri / WebView2 未重新验证**：视觉验证走 Playwright（Chromium）+ `vite build`；`backdrop-filter` 在 WebView2 的可用性沿用 5.1.7 的真机结论。
- **150% / 200% 缩放未验证**：spec 已登记为后续项。
- **`--app-fg-subtle` 浅色 4.02:1**：低于 AA 正常文字门槛，但这是 5.1.7 已建立并在 spec D6 明确记录的既有取值，**不是本波回归**；本波沿用它表达空状态。如后续要达标，应整体上调该 token。
- **截图覆盖范围有限**：8 张只覆盖 4 个代表性对话框，其余 10 个未逐张截图；由 `dialog-tokens.test.ts` 的 14 文件静态守卫与 `dialog-visual.spec.ts` 的计算样式用例兜底。截图位于临时目录、未进仓库。

## 四、总结

### 合并前建议整改（按优先级）

| 优先级             | 项                                                                  | 位置                                    | 处置                                                                                                                        |
| ------------------ | ------------------------------------------------------------------- | --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| **P1（必须）**     | `--app-success` 浅色对比度不达标                                    | `globals.css:42`（浅色声明）            | 按 spec D2 修订改为 `#15803d`；改后重跑 `dialog-visual.spec.ts` 与截图确认观感；可选：补一条 token 对比度单测，防止未来回退 |
| **P2（必须）**     | `--app-surface-raised` 注释与实现不符                               | `globals.css:12`                        | 更新注释                                                                                                                    |
| **P3（建议同轮）** | `field-input text-xs` 死类名                                        | `ProfileDetail.tsx:68`                  | 删除 `text-xs`                                                                                                              |
| **P4（建议同轮）** | 深色 E2E 无法自证深色主题生效                                       | `e2e/tests/dialog-visual.spec.ts:14-30` | 补 `html.dark` 断言或深色表面色断言（S6）                                                                                   |
| P5（可延后）       | 守卫正则覆盖面 / 行注释剥离精度 / e2e `padding` 断言 / 回执 §8 数字 | 见 S3–S5、S7                            | 随下一轮或后续波次处理                                                                                                      |
| 文档（审核窗口）   | 计划 Task 5 代码块无效语法                                          | 计划 `:1543-1544`                       | **已修正**；`dialog-tokens.test.ts` 的对应注释建议开发窗口在整改轮同步更新                                                  |

### 判定

**需要整改后复审（轻量）。**

理由：主体实现与计划高度一致，质量门全绿，回执数字全部属实，无功能 / 安全 / 跨层回归；但 spec 自带的验收标准「无低对比度文字」被 `--app-success` 浅色值（3.30:1）打破，且修复只需改一个 token 值。P1–P4 合计四处、均为小改动，建议开发窗口一次改完后交**复审回执**，审核窗口复核即可合并。

> 另：`main` 当前领先 `origin/main` **3** 个提交（`01c19ab`、`f14c310`、`c2c3c7c`），`feat/dialog-restyle` 领先 `main` **7** 个提交；两者均未推送。合并与推送需用户另行明确指令。

---

**审核窗口**：2026-10-19
