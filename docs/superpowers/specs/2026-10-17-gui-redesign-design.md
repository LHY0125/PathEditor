# GUI 前端重设计 — 设计文档

- **日期**：2026-10-17
- **版本目标**：独立波次（建议 **Wave 4.0「GUI 重设计」**）
- **作者**：审核窗口
- **状态**：待用户审阅 → plan
- **性质**：**纯前端视觉与信息架构重构**，零 Rust 改动、零 IPC 契约改动
- **前置**：Wave 3.0 / 3.1 已闭合；5.1.6 已发布（`main` @ `4011c2d`）
- **修订**：v2（2026-10-17），按 `docs/审核和开发/2026.10.17/PathEditor-gui-redesign裁断.md` 修订 6 项（B1/B2/B3 + H1/H2/H3）

## 概述

当前 GUI 是 `TitleBar → 4 Tab → 两行工具栏（18 个按钮）→ 表格 → 状态栏` 的竖直堆叠，存在三类问题：**信息层次缺失**（18 个按钮同权重，用户每次都要扫一遍才能找到「保存」）、**设计 token 分裂**（`globals.css` 定义了 CSS 变量，组件却硬编码 `#3b82f6`/`#2563eb`/`#dc3545`）、**深色主题审美陈旧**（海军蓝 `#1a1a2e/#16213e/#0f3460`）。

本波把它们一次性解决：**浅色 = 中性现代（Linear 风），深色 = 玻璃深色（Raycast 风），两者共用同一套几何骨架**；工具栏从两行 18 按钮收口为**一行分层工具栏 + ⋯ 菜单**。

## 背景与现状（已核实代码事实）

| 事实                                                                                 | 位置                                                                                |
| ------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| 工具栏两行共 18 个按钮，全部同一套 `btnClass`/`btnStyle`                             | `src/components/toolbar/ToolBar.tsx:36-92`                                          |
| 按钮原语只有两个导出：`btnClass` 字符串 + `btnStyle` 对象                            | `src/components/ui/buttons.ts:1-8`                                                  |
| 硬编码颜色 `#3b82f6`/`#2563eb`/`#dc3545`/`#fd7e14`/`#6b7280`                         | `AppShell.tsx:164`、`ToolBar.tsx:54`、`PathTable.tsx:121-129`、`globals.css:79-100` |
| CSS 变量只有 **7** 个（`:root`，`globals.css:37-45`），且浅/深各一套（无中间语义层） | `src/styles/globals.css:37-56`                                                      |
| 深色主题为海军蓝配色                                                                 | `src/styles/globals.css:13-18`                                                      |
| 窗口 `width: 900 / minWidth: 800`                                                    | `gui/tauri.conf.json:16-19`                                                         |
| 快捷键已存在：`Ctrl+N/S/Z/Y`、`Delete`、`F1`                                         | `src/hooks/use-keyboard.ts:43-66`                                                   |
| `button.save` 中文为「确定」、英文为 `OK`                                            | `src/i18n/locales/zh-CN.json:89`、`en.json:89`                                      |
| E2E 按中文按钮名定位；**逐步取消复核：必须改 5 个文件、不受影响 5 个**               | 见 §H1 实测清单                                                                     |
| 前端零 UI 组件库依赖（仅 react/zustand/i18next/tanstack-virtual）                    | `package.json:32-43`                                                                |

## 设计决策（已与用户逐项确认）

用户通过视觉伴侣在 6 屏 mockup 上逐轮拍板，结论如下：

### D1 —— 双主题「一套几何，两种材质」

**用户明确要求**：浅色用中性现代、深色用玻璃深色。为避免切换主题时控件形状跳变，**几何统一为 8px 圆角**，仅材质分化。

| 维度         | 浅色 · 中性现代                                       | 深色 · 玻璃深色                                         |
| ------------ | ----------------------------------------------------- | ------------------------------------------------------- |
| 应用背景     | `#fafafa` 纯色                                        | `linear-gradient(160deg,#15151d,#0d0d12)`               |
| 表面         | `#ffffff`                                             | `rgba(255,255,255,0.04)` 半透明叠层                     |
| 边框         | `#e4e4e7` 实线                                        | `rgba(255,255,255,0.09)` 低透明度白线                   |
| 强调色       | `#6366f1`                                             | `#7c6cff`（提亮）+ 辉光 `0 0 18px rgba(124,108,255,.5)` |
| 浮层（菜单） | `#ffffff` + 投影                                      | `rgba(30,30,40,0.92)` + `backdrop-filter: blur(14px)`   |
| 几何         | **8px 圆角 / 28px 行高 / 6px 间距**（两主题完全相同） | 同左                                                    |

> **说明**：强调色两套色值不是随意复制，而是**WCAG 对比度要求的结果** —— `#6366f1` 在深色背景上偏「沉」，提亮到 `#7c6cff` 才能达到同等可读性。

### D2 —— 分层工具栏（用户选定方案一）

主工具栏收口为**单行**，按频率分层：

```
[搜索] │ [新建][编辑][删除] │ [上移][下移] │ [撤销][重做] ...推... [导入][导出] [保存] [⋯]
```

- **规则**：高频用文字按钮，工具类用描边按钮，低频全部进 ⋯ 菜单。
- **保存永远可见 + 永远有强调色**（不再只在 `isModified` 时才上色，见现状 `ToolBar.tsx:53`）。
- **未保存状态**：保存按钮额外显示一个实心圆点徽标，而非改变整个按钮配色。

**8 个动作的完整归属**（修订：补 B3 遗漏的 `浏览...`）：

| 动作                           | 归属                  | 说明                                                                                                                                          |
| ------------------------------ | --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| 新建 / 编辑 / 删除             | 主行·编辑组           |                                                                                                                                               |
| 上移 / 下移                    | 主行·排序组           |                                                                                                                                               |
| 撤销 / 重做                    | 主行·撤销组           |                                                                                                                                               |
| 导入 / 导出                    | 主行·批量组           |                                                                                                                                               |
| 保存                           | 主行·右端             | 强调色                                                                                                                                        |
| **浏览...**                    | **⋯ 菜单·维护组首项** | **B3**：`handleBrowse` = `open({directory:true})` + `addPath`，**与「新建」是同一件事的另一种输入方式**；且低频（可直接粘贴路径）、主行需收敛 |
| 清理无效 / 冲突分析 / 配置方案 | ⋯ 菜单·维护组         |                                                                                                                                               |
| 深色模式 / 语言 / 帮助         | ⋯ 菜单·应用组         |                                                                                                                                               |
| 取消并关闭                     | ⋯ 菜单·底部           | 分隔线隔开                                                                                                                                    |

### D3 —— ⋯ 菜单收纳低频操作

| 分组     | 条目                                       |
| -------- | ------------------------------------------ |
| **维护** | 清理无效路径 · 冲突分析 · 配置方案         |
| **应用** | 深色模式 · 切换语言 · 帮助（`F1`）         |
| ——       | 取消并关闭（用分隔线与上方隔开，永远置底） |

菜单中显示**真实已存在的快捷键**（`use-keyboard.ts` 已实现 `Ctrl+N/S/Z/Y`、`Delete`、`F1`），**不新造快捷键**。

### D4 —— 英文简写（用户钦定）

800px 最窄窗口实测：中文全文字刚好放下**余量为零**，英文全文字**必然换行**。用户决定英文用简写。

| 位置       | 中文                                | 英文                            |
| ---------- | ----------------------------------- | ------------------------------- |
| 搜索占位   | 搜索路径…                           | Search…                         |
| 编辑组     | 新建 / 编辑 / 删除                  | New / Edit / Delete             |
| 排序组     | 上移 / 下移                         | Up / Down                       |
| 撤销组     | 撤销 / 重做                         | Undo / Redo                     |
| 批量       | 导入 / 导出                         | Import / Export                 |
| **主操作** | **保存**                            | **Save**                        |
| ⋯ 维护     | 清理无效路径 / 冲突分析 / 配置方案  | Clean / Analyze / Profiles      |
| ⋯ 应用     | 深色模式 / 语言 / 帮助 / 取消并关闭 | Dark / Language / Help / Cancel |

> **`Delete` 用全拼不用 `Del`**：800px 下全拼放得下，且 `Del` 在英文 UI 有歧义（可能被读作 Delimiter）。**宽度问题已经用简写解决，不应再牺牲可读性。**

### D4.1 —— 「取消并关闭」用独立 key（修订：H2）

`button.cancel`（「取消」/`Cancel`）是**共享 key**，3 处引用：

| 位置                       | 语义               | 应显示的文案            |
| -------------------------- | ------------------ | ----------------------- |
| `ToolBar.tsx:62`           | 取消编辑并关闭窗口 | **取消并关闭 / Cancel** |
| `EditEnvVarDialog.tsx:143` | 对话框取消         | 取消 / Cancel           |
| `NewEnvVarDialog.tsx:110`  | 对话框取消         | 取消 / Cancel           |

**直接改 `button.cancel` 的值会让 2 个对话框显示「取消并关闭」**，且 5 处单测断言（`env-var-toolbar` / `app-shell-env-vars`）断掉。

**裁断**：新增 `button.cancelAndClose`，仅 `ToolBar.tsx` 使用；`button.cancel` 保持不变。这与 D5 是**同一模式**。

### D5 —— 工具栏保存按钮文案改为「保存」（新增独立 key，**不改共享 key**）

现状 `zh-CN.json:89` 为「确定」、`en.json:89` 为 `OK`。「确定」在中文 UI 中专属于**对话框确认**语义，而工具栏按钮做的是**写入注册表**（可能改系统 PATH）。改为「保存」/`Save` 能明确传达「提交」含义。

> **⚠️ 关键约束（自审发现，原方案有误）**：`button.save` 是**共享 key**，被 4 处使用：
>
> | 位置                           | 语义       | 应显示的文案    |
> | ------------------------------ | ---------- | --------------- |
> | `ToolBar.tsx:59`               | 写入注册表 | **保存 / Save** |
> | `EditEnvVarDialog.tsx:151`     | 对话框确认 | 确定 / OK       |
> | `NewEnvVarDialog.tsx:118`      | 对话框确认 | 确定 / OK       |
> | `profile/ProfileDetail.tsx:91` | 对话框确认 | 确定 / OK       |
>
> **因此不得直接改 `button.save` 的值**（那样会让 3 个对话框也说「保存」，语义错误）。正确做法：
>
> 1. **新增** `button.savePath` = 「保存」/`Save`，仅 `ToolBar.tsx` 使用；
> 2. **保留** `button.save` = 「确定」/`OK` 原样，3 个对话框不动；
>
> **影响面因此大幅缩小**：`tests/unit/app-shell-env-vars.test.tsx` 中 9 处 `getByRole('button', { name: '确定' })` 与 `e2e/tests/env-vars.spec.ts` 中 3 处，**全部指向对话框**，**不受影响**。

## 设计 Token 表

### 语义层（组件只允许使用这一层）

```
--app-bg             应用背景
--app-surface        表面（表头、工具栏底）
--app-surface-raised 抬升表面（菜单、弹窗）
--app-border         边框
--app-border-strong  强边框（输入框聚焦前）
--app-fg             主文字
--app-fg-muted       次文字
--app-fg-faint       弱文字（序号、占位符）
--app-accent         强调色
--app-accent-fg      强调色上的文字
--app-accent-bg      强调色浅底（选中 Tab、选中行）
--app-accent-glow    强调辉光（深色专用）
--app-row-even       偶数行
--app-row-odd        奇数行
--app-row-selected   选中行
--app-hover          悬停底色
--app-danger         无效路径
--app-warning        重复路径
--app-disabled-fg    已禁用条目文字
```

### 几何层（两主题共用，不可覆盖）

```
--radius-sm  6px   小控件（复选框、徽标）
--radius     8px   标准控件（按钮、输入框、Tab）
--radius-lg  10px  容器（表格外框、菜单）
--row-h      28px  表格行高
--gap        6px   工具栏元素间距
```

> **迁移要求**：`globals.css` 删除现有的 `--color-light-*` / `--color-dark-*` 双套常量，组件中所有硬编码色值替换为语义变量。

### 过渡别名（R7 必需，修订：B1）

> **⚠️ 这是本 spec 最容易踩坏功能的一处。** 旧变量 `--app-list-*` 等**不能直接删除**，因为它们仍被 **R7 范围外的对话框**引用。

独立复核实测。**口径**：`grep -rn -- '--app-list' src/components/dialogs` = **14 处 / 7 文件**（本节统一用此口径，不再在正文提单文件计数）：

| 引用数 | 文件                                    | 是否在 R7 范围外 |
| ------ | --------------------------------------- | ---------------- |
| 4      | `dialogs/NewEnvVarDialog.tsx`           | ⚠️ **是**        |
| 3      | `dialogs/profile/ProfileDetail.tsx`     | ⚠️ **是**        |
| 2      | `dialogs/env-backup/EnvBackupPanel.tsx` | ⚠️ **是**        |
| 2      | `dialogs/analyze/ToolsTab.tsx`          | ⚠️ **是**        |
| 1      | `dialogs/ProfileDialog.tsx`             | ⚠️ **是**        |
| 1      | `dialogs/PathEditDialog.tsx`            | ⚠️ **是**        |
| 1      | `dialogs/EditEnvVarDialog.tsx`          | ⚠️ **是**        |
| 2      | `path-list/MergePreview.tsx`            | 范围内           |
| 3      | `path-list/PathTable.tsx`               | 范围内           |
| 3      | `env-list/EnvVarTable.tsx`              | 范围内（仅换色） |
| 1      | `toolbar/SearchInput.tsx`               | 范围内           |
| 1      | `layout/StatusBar.tsx`                  | 范围内           |
| 1      | `env-list/EnvVarToolbar.tsx`            | 范围内           |

**对话框内合计 14 处，跨 7 个文件。**

### 死代码（v3 实测，重写时可直接不写）

以下类与变量在 `globals.css` 之外 **0 引用**，重写时无需保留：

| 类别   | 名称                                                                             |
| ------ | -------------------------------------------------------------------------------- |
| CSS 类 | `.row-even` / `.row-odd` / `.btn-disabled` / `.text-invalid` / `.text-duplicate` |
| 变量   | `--color-path-invalid` / `--color-path-duplicate` / `--color-path-valid`         |

> 实测命令：`grep -rn "<name>" src | grep -v globals.css`，结果均为 0。

**后果**：CSS 自定义属性在未定义时**整条声明失效**（无回退值 → `background-color` 回到 `initial` = `transparent`）。对话框输入框与面板底会变透明，深色模式下文字不可辨 —— **这是功能受损，不是视觉不一致**。

**做法**：保留以下旧变量作**过渡别名**，指向新语义层。

> **⚠️ 命名冲突（自审发现）**：新语义层里已有 `--app-bg` / `--app-fg` / `--app-border` / `--app-hover` 四个同名变量，**它们本身就是新定义，不需要别名**（旧组件的 `var(--app-fg)` 会自动拿到新值）。真正需要别名的是**只有旧组件在用、新语义层不再提供**的三个：`--app-list-bg` / `--app-list-alt` / `--app-select-row`。

```css
/* TODO(Wave 4.1)：以下为过渡别名 —— 对话框重做后删除。不得静默遗留。 */
:root {
  /* 仅旧组件（对话框）使用的三个变量 → 映射到新语义层 */
  --app-list-bg: var(--app-surface);
  --app-list-alt: var(--app-row-odd);
  --app-select-row: var(--app-row-selected);
}
/* --app-bg / --app-fg / --app-border / --app-hover 由新语义层直接提供，无需别名。 */
```

**三条约束**：

1. 别名只写在 `:root`（**不随 `.dark` 变化** —— 新语义变量本身已按主题切换，别名只做转发）。
2. 必须带 `TODO(Wave 4.1)` 注释，**不得静默遗留**。
3. **Wave 4.1 验收标准**：`grep -rn -- '--app-list' src` 结果为空（**v3 更正：范围是整个 `src/`，不只是 `dialogs/`** —— 否则 `MergePreview` / `EnvVarTable` 会静默遗留别名）。

> **为什么范围要扩到 `src/`**：范围内组件本波已改用新语义变量，Wave 4.1 后别名只应存在于「对话框 + 别名定义本身」。若只查 `dialogs/`，误用别名的范围内组件永远查不出来。

> **为什么这里要别名，而 `buttons.ts` 也要别名**：spec 原本只对 `buttons.ts` 安排了「保留旧导出名做过渡别名」，对 CSS 变量没有同样安排 —— **同一把尺子要量两次**。

## 组件改造清单

| 文件                                         | 改动                | 说明                                                                                                                                      |
| -------------------------------------------- | ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `src/styles/globals.css`                     | **重写**            | 删除 `@theme` 双套色板；改为语义层 + 几何层；**保留 3 个过渡别名**（B1）；**保留 `.tab-active` 原样**（H3）；新增 `.tab-pill` 与 `.glass` |
| `src/components/ui/buttons.ts`               | **重写为原语**      | 导出 `buttonVariants`：`primary` / `secondary` / `ghost` / `danger`；保留旧导出名做**过渡别名**，逐步替换                                 |
| `src/components/ui/Menu.tsx`                 | **新建**            | 无依赖下拉菜单：受控开合 + 点击外部关闭 + `Escape` 关闭 + 键盘上下导航                                                                    |
| `src/components/toolbar/ToolBar.tsx`         | **重构**            | 两行 → 单行分层；接入 `Menu`；保存按钮改用 `button.savePath`，取消按钮改用 `button.cancelAndClose`                                        |
| `src/components/toolbar/ActionButtons.tsx`   | **重构**            | 编辑组 + 排序组，用分隔线分块；`onBrowse` 移入 ⋯ 菜单                                                                                     |
| `src/components/toolbar/UndoRedoButtons.tsx` | **改样式**          | 使用 `ghost` 变体                                                                                                                         |
| `src/components/toolbar/SearchInput.tsx`     | **改样式**          | 使用新 token                                                                                                                              |
| `src/components/layout/TitleBar.tsx`         | **改样式**          | 加品牌色块占位（纯 CSS，不引 SVG 资源）                                                                                                   |
| `src/components/layout/AppShell.tsx`         | **改样式**          | Tab 改用 **`.tab-pill`**（H3：不动 `.tab-active`）；移除硬编码 `#3b82f6`                                                                  |
| `src/components/path-list/PathTable.tsx`     | **改样式**          | 移除斑马纹、改用 `--app-row-*`；无效/重复改用语义变量                                                                                     |
| `src/components/path-list/MergePreview.tsx`  | **改样式**（v3 补） | 改用语义变量，脱离过渡别名                                                                                                                |
| `src/components/env-list/EnvVarTable.tsx`    | **改样式**（v3 补） | 改用语义变量，脱离过渡别名                                                                                                                |
| `src/components/layout/StatusBar.tsx`        | **改样式**          | 使用语义变量                                                                                                                              |
| `src/components/env-list/EnvVarToolbar.tsx`  | **改样式**          | 与主工具栏视觉对齐（结构不动，**沿用 `.tab-active`**）                                                                                    |
| `src/i18n/locales/zh-CN.json` / `en.json`    | **改文案**          | 按 D4 表；**新增** `button.savePath`、`button.cancelAndClose`、`menu.*`；**既有 `button.save` / `button.cancel` 不改值**                  |

### H3 —— `.tab-active` 波及对话框（修订）

`.tab-active` 定义于 `globals.css:78`，3 处使用：

| 位置                                           | 是否在 R7 范围外 |
| ---------------------------------------------- | ---------------- |
| `AppShell.tsx:163`                             | 范围内           |
| `EnvVarToolbar.tsx:86`                         | 范围内           |
| **`dialogs/env-backup/EnvBackupPanel.tsx:61`** | ⚠️ **是**        |

**裁断**：新建 `.tab-pill` 供 `AppShell` 使用；`.tab-active` **原样保留**（不重写其规则），供对话框与 `EnvVarToolbar` 过渡使用。

## 非功能需求

### N1 —— 800px 最窄窗口不回退

`gui/tauri.conf.json:18` 的 `minWidth: 800` 是用户真实可达的极限。**工具栏在 800px 下必须单行不换行**，中英文都要满足。验证方式：Playwright 以 `viewport: { width: 800, height: 600 }` 跑一次截图断言。

### N2 —— i18n 布局以最长语言为基准

中文「上移」2 字 vs 英文 `Move Up` 7 字，宽度差 3.5 倍。任何新增按钮文案都必须在中英两种语言下复验 800px，**仅中文通过不算通过**。

### N3 —— 不引入 UI 依赖

前端保持零 UI 组件库。图标一律**手写内联 SVG**（约 6 个：加/减/铅笔/垃圾桶/上箭头/下箭头），不新增 `dependencies`。

### N4 —— 可访问性

- 菜单支持 `Escape` 关闭、焦点回还触发按钮、`role="menu"` / `role="menuitem"`。
- 所有图标按钮必须有 `aria-label` 与 `title`。
- 两套主题下正文对比度 ≥ 4.5:1。

### N5 —— 尊重减少动效

`@media (prefers-reduced-motion: reduce)` 下禁用菜单展开动画与按钮过渡。

## 测试策略

| 层级         | 策略                                                                                                                                                                                                                        |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 单元测试     | `Menu.tsx` 新增测试：开合、点击外部关闭、`Escape`、键盘导航、焦点回还                                                                                                                                                       |
| 现有单元测试 | 实测 **50 处** `getByRole('button')`；`app-shell-env-vars.test.tsx:193-203` 的 5 条 `toBeNull()` 在 `allVars` 分支，**不受影响**（v3 更正）。**12 处对话框 `'确定'` 断言不受影响 + 1 处必须改**（`toggle-save.spec.ts:19`） |
| E2E          | **必须改 4 个文件**：`analyze` / `profiles` / `search-clean` / `toggle-save`；**不受影响 6 个**：`crud-undo` / `import-export` / `startup` / `keyboard` / `env-backup` / `env-vars`（v3 更正）                              |
| 视觉回归     | 新增 800×600 与 1200×800 两档、浅/深两主题的 Playwright 截图，共 4 张                                                                                                                                                       |
| 质量门       | `npm run verify:all` 必须绿；Rust 侧零改动，`cargo test --workspace` 不受影响                                                                                                                                               |

## 风险与未覆盖项

| #   | 风险                                                      | 缓解                                                         |
| --- | --------------------------------------------------------- | ------------------------------------------------------------ |
| R1  | **E2E 选择器断裂**：低频按钮移入 ⋯ 菜单后，未展开时不可见 | 见下方 **H1 实测清单**；测试改为「先点 ⋯ 再定位」            |
| R2  | **`data-testid` 保活**：共 9 个                           | **全部保留、不得改名**（消费方见下，**比初版以为的更重要**） |

**`data-testid` 保活清单**（复核实测，共 9 个）：

| testid                 | 定义位置                 | 消费方（复核更正）                                                 |
| ---------------------- | ------------------------ | ------------------------------------------------------------------ |
| `path-table`           | `PathTable.tsx:99`       | 单元测试                                                           |
| `path-row`             | `PathTable.tsx:137`      | ⚠️ **被多个 E2E 依赖**（`toggle-save` 等），**比初版以为的更重要** |
| `path-enabled`         | `PathTable.tsx:161`      | 单元测试 + E2E                                                     |
| `path-text`            | `PathTable.tsx:171`      | 单元测试 + E2E                                                     |
| `path-drop-zone`       | `AppShell.tsx:229`       | E2E (`env-vars.spec.ts:81`)                                        |
| `env-var-table`        | `EnvVarTable.tsx:63`     | 单元测试                                                           |
| `backup-summary`       | `EnvBackupPanel.tsx:89`  | 单元测试                                                           |
| `backup-removed-names` | `EnvBackupPanel.tsx:100` | 单元测试                                                           |
| `backup-outcome`       | `EnvBackupPanel.tsx:137` | 单元测试                                                           |

### H1 —— E2E 受影响面实测清单（v3 修订）

**共 10 个 E2E 文件，逐条核对后分类如下**：

| 文件                    | 行    | 引用                       | 分类                         |
| ----------------------- | ----- | -------------------------- | ---------------------------- |
| `analyze.spec.ts`       | 12    | `text=分析`                | ⚠️ **必须改**（进菜单）      |
| `profiles.spec.ts`      | 11    | `text=配置`                | ⚠️ **必须改**（进菜单）      |
| `search-clean.spec.ts`  | 45    | `text=一键清理`            | ⚠️ **必须改**（进菜单）      |
| `toggle-save.spec.ts`   | 19    | `text=确定`                | ⚠️ **必须改**（B2 文案变更） |
| `crud-undo.spec.ts`     | 11    | `text=新建`                | ✅ 不受影响（主行保留）      |
| `import-export.spec.ts` | 11,15 | `text=导出`/`text=导入`    | ✅ 不受影响（主行保留）      |
| `startup.spec.ts`       | 32,34 | `name: '新建'`             | ✅ 不受影响（主行保留）      |
| `keyboard.spec.ts`      | 25    | `text=快捷键`              | ✅ 不受影响（帮助对话框内）  |
| `env-backup.spec.ts`    | —     | 对话框按钮                 | ✅ 不受影响                  |
| **`env-vars.spec.ts`**  | 66-73 | `allVars` 下 `toBeNull` ×3 | ✅ **不受影响（v3 更正）**   |

> **⚠️ v3 更正：`env-vars.spec.ts:71` 不会假绿。**
>
> v2 曾判定该用例「断言永远为真」。**这是审核窗口的错误**。实际：该用例先点击「全部变量」切到 `allVars` 分支，而 `AppShell.tsx:173` 是 `{activeTab === 'allVars' ? <EnvVarToolbar/> : <ToolBar/>}` —— **`allVars` 下 `ToolBar` 根本不渲染**，`ToolBar` 内部布局怎么变都不影响该断言。它测的是「allVars 分支不渲染 PATH 按钮」，一条**正确且充分**的断言。
>
> **同理** `tests/unit/app-shell-env-vars.test.tsx:193-203` 的 5 条 `toBeNull()` 也不受本波影响。
>
> **本波不存在「假绿」风险。** 存在的（且可接受的）只是「跨分支断言不覆盖 ToolBar 内部搬家」——由 `crud-undo` / `import-export` 等主行用例覆盖「导入/导出可见」，已足够。

**必须改的 E2E：4 个文件**（`analyze` / `profiles` / `search-clean` / `toggle-save`）。
**不受影响的 E2E：6 个文件。**

| R3 | **「确定→保存」断言** | 采用 D5「新增独立 key」方案后：**12 处对话框断言不受影响 + 1 处必须改**（`toggle-save.spec.ts:19`）。见测试策略表（两处表述已统一，v3） |

| R4 | **深色玻璃效果在 WebView2 上的表现** | `backdrop-filter` 需 WebView2 ≥ 93；Tauri 2 的默认 WebView2 满足。若降级则回落为不透明 `#1e1e28` |
| R5 | **800px 下英文仍可能溢出** | 把 N1 做成 Playwright 断言而非人工目测，溢出即失败 |
| R6 | **未覆盖**：缩放到 150%/200% 时的布局 | 本波不做，登记为后续项（用户已接受） |
| R7 | **对话框本波不重皮** | **需配合 B1 过渡别名**才成立（否则 14 处引用失效 → 功能受损）。用户已接受「采用方案 A 后接受」 |
| R8 | **`.tab-active` 被对话框使用**（H3） | 保留原类不动，新建 `.tab-pill` 供主界面 |
| R9 | **i18n 共享 key 误改**（H2） | 通则：**改任何 i18n key 的值前，先 `grep -rn "t('<key>')" src` 数引用点**。本波新增 `button.savePath` / `button.cancelAndClose`，不动既有 key |

## 范围外

- Rust / CLI 零改动。
- 不改 IPC 契约、不改 `backend.ts`、不改 store 逻辑。
- 不动测试 mock 与 E2E 业务断言（只改由布局变化导致的选择器）。
- **不碰 `CLAUDE.md`**（需用户明确授权另行处理）。

## 交接链

| 文档                                                                    | 状态                                                                                                                                        |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| 本 spec                                                                 | ✅ 本文件                                                                                                                                   |
| plan `docs/superpowers/plans/2026-10-17-gui-redesign-implementation.md` | ⏳ 待用户审阅 spec 后编写                                                                                                                   |
| 视觉参考                                                                | `.superpowers/brainstorm/1093-1792212204/content/`（6 屏 mockup，**已随 `4011c2d` 入库，作为设计留档**；含 token 的运行时文件已 gitignore） |
| 核对交接                                                                | `docs/审核和开发/2026.10.17/PathEditor-gui-redesign核对轮交接.md`（开发窗口）                                                               |
| **核对裁断**                                                            | `PathEditor-gui-redesign裁断.md` + `PathEditor-gui-redesign裁断-v3.md`                                                                      |
| **v3 修订说明**                                                         | 撤回「假绿」错误结论（V3）、补 2 个改造文件（V4）、替换 `浏览...` 理由（V1）、统一口径（V2）                                                |
