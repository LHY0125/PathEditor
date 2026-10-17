# GUI 前端重设计 — 核对轮交接

**提交窗口**：开发窗口
**日期**：2026-10-17
**交付对象**：审核窗口
**核对对象**：`docs/superpowers/specs/2026-10-17-gui-redesign-design.md`
**结论**：**3 项阻塞、3 项高风险、4 项事实错误、4 项需补充说明**。设计方向（D1–D4）无异议；**D5 方向正确但影响面漏算一处**；**R7 目前不成立**——不是审美问题，是技术依赖冲突。

---

## 0. 核对方法

逐条核实 spec 的「背景与现状（已核实代码事实）」表与「设计决策」各条，全部结论来自实际命令输出。核对命令见各条目。

---

## 1. 阻塞项（开工前必须解决）

### B1 —— `--app-list-*` 删除与 R7「对话框不动」直接冲突 ⛔

**spec 原文**（组件改造清单）：`src/styles/globals.css` **重写** —— 删除现有的 `--color-light-*` / `--color-dark-*` 双套常量与 `--app-list-*` 等旧变量。

**实际情况**：`--app-list-bg` 有 **20 处**引用，其中 **14 处在 7 个对话框文件里**：

| 变量               | 总引用 | 对话框内 | 主界面内 |
| ------------------ | ------ | -------- | -------- |
| `--app-list-bg`    | 20     | **14**   | 6        |
| `--app-list-alt`   | 6      | 0        | 6        |
| `--app-select-row` | 2      | 0        | 2        |

对话框内引用 `--app-list-bg` 的 7 个文件：

| 文件                                    | 行号           |
| --------------------------------------- | -------------- |
| `dialogs/analyze/ToolsTab.tsx`          | 23, 48         |
| `dialogs/EditEnvVarDialog.tsx`          | 127            |
| `dialogs/env-backup/EnvBackupPanel.tsx` | 48, 171        |
| `dialogs/NewEnvVarDialog.tsx`           | 58, 75, 85, 95 |
| `dialogs/PathEditDialog.tsx`            | 43             |
| `dialogs/profile/ProfileDetail.tsx`     | 59, 81, 120    |
| `dialogs/ProfileDialog.tsx`             | 32             |

**冲突性质**：CSS 变量被删除后，`var(--app-list-bg)` 求值失败，`background-color` 回退为初始值（透明）。对话框输入框、面板底色会**失效**——这不是「视觉暂时不一致」，是**坏掉**。

**R7 的措辞是「对话框沿用旧样式，视觉会暂时不一致」。但 spec 同时要求删除对话框所依赖的变量，两者不能同时成立。**

**建议处理（三选一，需审核窗口裁断）**：

| 方案          | 做法                                                                                                                                                                  | 代价                                   |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| **A（推荐）** | `globals.css` **保留** `--app-list-bg` / `--app-list-alt` / `--app-select-row` 作为**过渡别名**，指向新语义变量；与 `buttons.ts` 的「保留旧导出名做过渡别名」策略一致 | 零；Wave 4.1 重做对话框时一并清理      |
| B             | 同步替换 7 个对话框文件里的 14 处引用                                                                                                                                 | 违反 R7 的字面范围，但改动机械、风险低 |
| C             | 本波不删旧变量，只新增语义层                                                                                                                                          | 与 spec「重写 globals.css」的表述冲突  |

> **注意**：spec 对 `buttons.ts` 明确写了「保留旧导出名做**过渡别名**，逐步替换」，但对 CSS 变量**没有**同样的安排。这是 spec 内部的不对称。

### B2 —— `toggle-save.spec.ts` 会被 D5 打断，spec 未识别 ⛔

**spec 原文**（D5）：影响面因此大幅缩小：`tests/unit/app-shell-env-vars.test.tsx` 中 9 处 `getByRole('button', { name: '确定' })` 与 `e2e/tests/env-vars.spec.ts` 中 3 处，**全部指向对话框**，**不受影响**。

**实际情况**：这 12 处确实全部指向对话框（已逐条核实 `env-vars.spec.ts:123/148/208` 与单测 9 处均在对话框流程内）。**但漏了第 13 处**：

```ts
// e2e/tests/toggle-save.spec.ts:19-20
// 点击"确定"保存
await page.click('text=确定');
```

该文件流程为「勾选 checkbox 禁用路径 → 点『确定』保存 → 断言状态栏『保存成功』」。**这里的「确定」就是工具栏保存按钮**，不是对话框。D5 把工具栏按钮改为「保存」后，`text=确定` 找不到目标 → **该用例必红**。

**建议处理**：把 `toggle-save.spec.ts:20` 纳入 D5 的影响面清单，改为 `text=保存`（或更好的 `data-testid`）。同时修正 R3 的结论——**不是「零影响」，是「12 处不受影响 + 1 处必须改」**。

### B3 —— `button.browse`（「浏览...」）在重设计中无归属 ⛔

**spec 原文**（D2/D3）：主工具栏单行分层为 `[搜索] │ [新建][编辑][删除] │ [上移][下移] │ [撤销][重做] …推… [导入][导出] [保存] [⋯]`；⋯ 菜单收纳「清理无效路径 / 冲突分析 / 配置方案 / 深色模式 / 切换语言 / 帮助 / 取消并关闭」。

**实际情况**：现状工具栏共 **18** 个按钮（spec 数字正确），其中 `ActionButtons.tsx` 有 **7** 个：

```tsx
// src/components/toolbar/ActionButtons.tsx:33-53
new / edit / browse / delete / moveUp / moveDown / clean
```

spec 的分层表只安排了 6 个（new/edit/delete/moveUp/moveDown/clean），**「浏览...」（`button.browse`，`ActionButtons.tsx:39-41`）既不在主行也不在 ⋯ 菜单**。

**建议处理**：明确 browse 的去向（进 ⋯ 菜单？还是随编辑组保留在主行？）。这是规格缺口，不是实现细节。

---

## 2. 高风险项（影响面低估）

### H1 —— E2E 受影响面是 5 个文件，spec 说 2 个

**spec 原文**（测试策略）：E2E 2 文件 5 处受影响；`startup.spec.ts` 与 `env-vars.spec.ts` 里对「上移/下移/一键清理」的断言需改为「先开 ⋯ 菜单再定位」。

**实际情况**：按 D3/D4 把按钮移入 ⋯ 菜单 + 改名后，受影响的是 **5 个文件**：

| 文件                   | 断言行             | 断裂原因                      |
| ---------------------- | ------------------ | ----------------------------- |
| `analyze.spec.ts`      | 12 `text=分析`     | 「分析」→「冲突分析」且进菜单 |
| `profiles.spec.ts`     | 11 `text=配置`     | 「配置」→「配置方案」且进菜单 |
| `search-clean.spec.ts` | 45 `text=一键清理` | 改名 + 进菜单                 |
| `env-vars.spec.ts`     | 69/70/71           | 上移/下移/一键清理进菜单      |
| `toggle-save.spec.ts`  | 20 `text=确定`     | 见 B2                         |

`import-export.spec.ts:11/15`（`text=导出` / `text=导入`）预计**安全**——两个按钮留在主行且文案不变。

> **补充**：`text=分析` 与 `text=配置` 是 Playwright 的**子串匹配**，「冲突分析」包含「分析」、「配置方案」包含「配置」，所以**文案改名本身不会导致这两处断裂**——真正断裂的原因是**按钮进了菜单、未展开时不可见**。这一点影响修复方式（改选择器 vs. 先开菜单），值得在 plan 里写清。

### H2 —— `button.cancel` 是共享 key，D4 表却把文案写成「取消并关闭」

**spec 原文**（D4 表）：⋯ 应用 | 深色模式 / 语言 / 帮助 / **取消并关闭** | Dark / Language / Help / Cancel

**实际情况**：`button.cancel` 是**共享 key**，被 **3 处**使用：

| 位置                   | 语义       | 应显示        |
| ---------------------- | ---------- | ------------- |
| `ToolBar.tsx:62`       | 关闭窗口   | 取消并关闭    |
| `EditEnvVarDialog.tsx` | 对话框取消 | 取消 / Cancel |
| `NewEnvVarDialog.tsx`  | 对话框取消 | 取消 / Cancel |

**这与 D5 的 `button.save` 问题完全同构**——spec 在 D5 里做了这个分析，却在 D4 表里对 `button.cancel` 直接写了新文案，**没有做同样的检查**。

若直接改 `button.cancel` 的值，2 个对话框的取消按钮会变成「取消并关闭」（语义错误），且单测 **5 处** `name: '取消'`（`app-shell-env-vars.test.tsx:261/273/284/620/637`）会断。

**建议处理**：与 D5 一致——**新增独立 key**（如 `button.cancelAndClose`），保留 `button.cancel` 原样。

> **方法论提示**：`button.edit` / `button.delete` 也各是 **4 处**共享（`EnvVarTable.tsx` ×2、`EnvVarToolbar.tsx`、`ActionButtons.tsx`）。本次 D4 表里这两个词的文案未变，所以**实际无影响**——但分析过程应当一致。建议 plan 里加一条通则：**改任何 i18n key 的值前，先 `grep -rn "t('<key>')" src` 数引用点**。

### H3 —— `tab-active` 类名跨范围复用

**spec 原文**（组件改造清单）：`AppShell.tsx` 改样式 —— Tab 改为 pill 样式。

**实际情况**：`.tab-active` 类（`globals.css:78-81`，当前是 `border-bottom: 2px solid #3b82f6`）被 **3 处**使用：

| 位置                    | 用途               | 是否本波范围                 |
| ----------------------- | ------------------ | ---------------------------- |
| `AppShell.tsx:163`      | 主 Tab             | ✅ 本波                      |
| `EnvVarToolbar.tsx:86`  | hive 过滤器        | ✅ 本波                      |
| `EnvBackupPanel.tsx:61` | 备份文件列表选中项 | ❌ **对话框内（R7 范围外）** |

若把 `.tab-active` 重写为 pill 样式，`EnvBackupPanel` 的选中态会**跟着变**——又是跨范围副作用。

**建议处理**：把 pill 样式做成**新类**（如 `.tab-pill`），只在主界面两处使用；`.tab-active` 保持原样供对话框使用。

---

## 3. 事实错误（不影响方向，但需更正）

### F1 —— `data-testid` 消费方列 9 项中 6 项错

**spec 原文**（R2 清单）：数量 9 个 ✅ **正确**；但「消费方」一列大部分有误。

| testid                 | spec 写的消费方             | 实际消费方                                                                                   |
| ---------------------- | --------------------------- | -------------------------------------------------------------------------------------------- |
| `path-table`           | 单元测试                    | ✅ 正确（`tests/unit/app-shell-env-vars.test.tsx`）                                          |
| `path-row`             | 单元测试                    | ❌ 实际是 **5 个 e2e**：`env-vars` / `keyboard` / `search-clean` / `startup` / `toggle-save` |
| `path-enabled`         | 单元测试                    | ❌ 实际是 `e2e/tests/toggle-save.spec.ts`                                                    |
| `path-text`            | 单元测试                    | ❌ 实际是 `e2e/tests/toggle-save.spec.ts`                                                    |
| `path-drop-zone`       | E2E (`env-vars.spec.ts:81`) | ❌ 实际是**单测 + e2e** 两处                                                                 |
| `env-var-table`        | 单元测试                    | ❌ 实际是 **2 个单测**文件                                                                   |
| `backup-summary`       | 单元测试                    | ❌ 实际是 `e2e/tests/env-backup.spec.ts`                                                     |
| `backup-removed-names` | 单元测试                    | ✅ 单测 + e2e                                                                                |
| `backup-outcome`       | 单元测试                    | ✅ 单测 + e2e                                                                                |

**影响**：R2 的结论（「全部保留、不得改名」）**依然成立且更应强调**——因为 `path-row` 被 5 个 e2e 依赖，改名的代价比 spec 以为的高。

### F2 —— 前置版本号过期

**spec 原文**：前置：Wave 3.0 / 3.1 已闭合（`main` @ `d7f6540`）

**实际**：`HEAD` = `4011c2d`（`docs: 同步 CLAUDE.md / AGENTS.md / README.md 至 5.1.6 实际状态`）。`d7f6540` 之后还有 `08cc19b`（发布 5.1.6）与 `4011c2d`。

### F3 —— mockup 提交状态过期

**spec 原文**（交接链）：视觉参考 `.superpowers/brainstorm/1093-1792212204/content/`（6 屏 mockup，**不提交**）

**实际**：这 6 个 HTML **已在 HEAD 中**（`git ls-files .superpowers/` 确认），随 `4011c2d` 提交。spec 的「不提交」已过时。

> 附：同目录的 `.last-token` / `.last-port` / `state/` 5 个运行时文件**未提交**且已加入 `.gitignore`（含本地服务 token，仓库为 PUBLIC）。

### F4 —— 单测按钮查询数口径

**spec 原文**：`tests/unit/` 中 **51** 处按钮查询需复核

**实际**：`grep -rn "getByRole('button'" tests/` = **50** 处。差异 1 处，可能是统计口径不同（如是否含 `getAllByRole`）。不影响结论，但建议 plan 里以实测数为准。

---

## 4. 需补充说明（低风险，但 plan 应覆盖）

### S1 —— 视觉回归无基线

spec 说「新增 800×600 与 1200×800 两档、浅/深两主题的 Playwright 截图，共 4 张」。仓库当前**没有** `toHaveScreenshot` 的使用，也没有截图基线目录。首次运行会**生成**基线而非**比对**——第一次跑永远是绿的。

**需说明**：基线如何建立、由谁审查、存放在哪（是否入库）。

### S2 —— Playwright 未配 viewport

`e2e/playwright.config.ts` 未设置 `viewport`，用的是 Playwright 默认 1280×720。N1 要求 800px 断言，需新增配置（或在用例内 `page.setViewportSize`）。

### S3 —— 图标清单不足

spec N3 说「图标一律手写内联 SVG（**约 6 个**：加/减/铅笔/垃圾桶/上箭头/下箭头）」。但：

- `ActionButtons` 有 **7** 个按钮（含 browse）
- ⋯ 菜单本身还需要一个**省略号**图标

实际需要 **8–9** 个。

### S4 —— `dialog.search` 复用确认（无问题，仅记录）

`SearchInput.tsx:28` 用 `t('dialog.search')`（zh-CN.json:166 =「搜索路径...」）。该 key **仅此一处**使用，改文案安全。D4 表把搜索占位改为「Search…」可行。

---

## 5. 对用户四个问题的回答

| 问题                                               | 开发窗口回答                                                                                                                                                                                                   |
| -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **D1–D5 是否符合预期**                             | D1–D4 **无异议**，设计决策本身合理（双主题一套几何、分层工具栏、⋯ 菜单、英文简写都是正确的取舍）。**D5 方向正确**（工具栏「确定」确实语义错误），但**影响面漏算 1 处**（见 B2）。                              |
| **R7：对话框本波不重做，视觉暂时不一致——接受吗？** | **不能按现状接受。** 不是因为「不一致」，而是因为 spec 同时要求删除对话框所依赖的 CSS 变量（`--app-list-bg` 14 处），这会让对话框**坏掉**而非「不一致」。**采用 B1 的方案 A（保留过渡别名）后，R7 可以接受。** |
| **R6：150%/200% 缩放不在本波——接受吗？**           | **接受。** 登记为后续项合理。                                                                                                                                                                                  |
| **范围外：CLAUDE.md 不碰**                         | **接受。** 与仓库硬约束一致（未经明确指令不得修改任何 CLAUDE.md）。                                                                                                                                            |

---

## 6. 待裁断

**待裁断 1（B1）**：`--app-list-*` 三个变量如何处理？

- 选项 A（推荐）：保留为过渡别名，指向新语义变量
- 选项 B：本波同步替换 7 个对话框的 14 处引用
- 选项 C：本波不删旧变量

**待裁断 2（B3）**：「浏览...」按钮去哪？

- 选项 A：进 ⋯ 菜单「维护」组
- 选项 B：留在主行编辑组
- 选项 C：本波移除该入口（需确认是否有其他入口）

**待裁断 3（H2）**：「取消并关闭」是否新增独立 key？

- 推荐：是，新增 `button.cancelAndClose`，与 D5 的 `button.savePath` 策略一致

---

## 7. 核对命令索引

```bash
# B1：旧变量引用面
grep -rn "app-list-bg\|app-list-alt\|app-select-row" src --include=*.tsx

# B2：确定按钮的全部引用
grep -rn "name: '确定'" tests/          # 9 处（对话框）
grep -rn "确定" e2e/                    # 4 处，其中 toggle-save.spec.ts:20 是工具栏

# B3：工具栏按钮全量
cat src/components/toolbar/ActionButtons.tsx   # 7 个按钮

# H1：E2E 文本选择器全量
grep -rn "text=\|getByText" e2e/tests/

# H2：共享 key 引用计数
for k in save cancel edit delete; do grep -rn "t('button.$k')" src --include=*.tsx; done

# H3：tab-active 引用
grep -rn "tab-active" src --include=*.tsx

# F1：testid 消费方
for id in path-row path-enabled path-text backup-summary; do grep -rl "$id" tests/ e2e/; done
```

---

## 8. 结论

**spec 的设计方向可以进入 plan 阶段，但有 3 项必须先解决**：B1（否则对话框坏掉）、B2（否则 E2E 必红且 spec 的 R3 结论错误）、B3（否则「浏览」按钮无归属）。

**未做任何代码改动、未修改 spec、未提交任何文件。** 本文件为新建的核对轮交接文档。

**签章**：以上结论均基于 2026-10-17 在 `main @ 4011c2d` 上实际执行的命令输出。
