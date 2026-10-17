# GUI 重设计 — 分支合并前复核

**提交窗口**：开发窗口
**日期**：2026-10-17
**交付对象**：审核窗口 / 用户
**复核对象**：`feat/gui-redesign`（`0b77707`，22 commits，vs `main @ 4011c2d`）
**结论**：**发现 1 个阻塞级真 bug——⋯ 菜单被工具栏裁切，8 项中仅 1 项可见**。全部 312 单测、36 E2E、5 视觉用例**都通过，但没有一项能抓到它**。其余核对项（4 项阻塞修复、5 项高风险、安全不变式）**全部确认到位**。

---

## 1. 阻塞级发现：⋯ 菜单被裁切 ⛔

### 现象

截图实测（1200×800，浅色，`.superpowers` 临时目录，非仓库文件）：

菜单展开后**只显示「浏览...」一项**，其余 7 项与两个分组标题**全部不可见**。

### 实测数据

```json
{
  "toolbarOverflow": "hidden",
  "toolbarBottom": 130,
  "panelTop": 64,
  "panelBottom": 375,
  "panelHeight": 311,
  "itemCount": 8,
  "items": [
    "浏览...",
    "清理无效路径",
    "冲突分析",
    "配置方案",
    "深色模式",
    "切换语言",
    "帮助F1",
    "取消并关闭"
  ],
  "visibleItems": ["浏览..."]
}
```

**8 个条目全部存在于 DOM（311px 高），但只有 1 个落在 `toolbarBottom: 130px` 之内。**

### 根因

`src/styles/globals.css:343-350`：

```css
.toolbar {
  display: flex;
  align-items: center;
  gap: var(--gap);
  padding: 8px 10px;
  border-bottom: 1px solid var(--app-border);
  overflow: hidden; /* ← 元凶 */
}
```

`Menu.tsx` **未使用 `createPortal`**，菜单面板作为 `.toolbar` 的**后代**渲染（`position: absolute; top: calc(100% + 4px)`）。祖先的 `overflow: hidden` 会**裁切所有溢出内容**，包括绝对定位的子元素——因为 `.toolbar` 本身**不是** `position: relative` 的包含块（`.menu-panel` 的定位上下文是 `.relative` 包裹的触发按钮 div），但**裁切仍然生效**：`overflow` 作用于**所有后代**，无论其定位方式。

### 验证（已确证）

运行时把 `overflow` 覆盖为 `visible`：

```json
{ "itemCount": 8, "visibleCount": 8, "allItemsBelowToolbar": 8 }
```

**8 项全部可见。** 修复后的截图确认：维护组（浏览/清理无效路径/冲突分析/配置方案）、应用组（深色模式/切换语言/帮助 F1）、分隔线、取消并关闭（红色），层次清晰。

### 为什么 8 个 E2E + 5 个视觉用例全绿

实测 Playwright 行为（1200×800，菜单展开态）：

```json
{
  "itemBox": { "x": 980, "y": 153.25, "width": 188, "height": 30.75 },
  "toolbarBottom": 130,
  "playwrightIsVisible": true,
  "clickSucceeded": true
}
```

「冲突分析」位于 **y=153**，而 `toolbarBottom` 只有 **130**——**真实用户看不见它**。但：

- `item.isVisible()` → **`true`**（Playwright 只检查 `display`/`visibility`/`opacity`/尺寸，**不做祖先裁切检测**）；
- `item.click()` → **成功**（Playwright 的 actionability 检查同样不含「是否被祖先 `overflow` 裁掉」）。

**因此 8 个点了菜单项的 E2E 全部「通过」了——它们点的是用户看不到的按钮。**

### 修复方案（三选一）

| 方案          | 做法                                                                                    | 评价                                                                                                                                                                                                                                                                |
| ------------- | --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A（推荐）** | 删除 `.toolbar` 的 `overflow: hidden`，改为 `.toolbar { overflow: visible }` 或直接不写 | **最小改动**。需确认当初加它的目的（见下）仍能满足                                                                                                                                                                                                                  |
| **B**         | `Menu` 用 `createPortal` 渲染面板到 `document.body`                                     | **最健壮**，符合项目 React 规范（`rules/react/patterns.md:164`：「Use `createPortal` for modals, tooltips, toast containers — anything that must escape the parent's `overflow: hidden` or `z-index` stacking context」）。但需处理定位与 `Escape`/外部点击的作用域 |
| C             | 保留 `overflow: hidden`，菜单改为向上展开                                               | 脆弱：菜单高度变化时仍可能裁切；且 1200×800 下向上展开会遮挡路径列表                                                                                                                                                                                                |

> **注意 `overflow: hidden` 的来历**：它由 `fa5bf44`（Toolbar 单行化）引入，与「窄窗口下搜索框弹性收缩」配套——目的是**防止工具栏内容溢出到主区域**。若改用方案 A，需保证宽度约束改由**别的机制**承担（现状已有：`8691fa3` 给 `.toolbar-search` 加了 `flex: 0 1 160px; min-width: 0` 的弹性收缩）。**建议先验证方案 A 下 800px 是否仍不裁切**（`e2e/tests/visual.spec.ts` 的 `toolbarClip` 断言正是为此设计，可直接复用）。

> **方案 B 的补充论据**：即使方案 A 修好了当前布局，**将来任何给工具栏加 `overflow` 或 `contain` 的改动都会静默复发**。portal 是「结构上不可能被裁」的解法。approval 建议 **A + 一条回归断言**（断言菜单面板所有条目 `getBoundingClientRect().bottom <= window.innerHeight` 且 `top >= 0`），B 可作为后续加固。

### 现有测试为何抓不到——需要补的断言

```ts
// 建议加入 e2e/tests/visual.spec.ts 或新建 menu.spec.ts
test('⋯ 菜单全部条目在视口内可见', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.getByRole('button', { name: '更多操作' }).click();

  const clipped = await page.getByTestId('menu-panel').evaluate(
    (panel) =>
      Array.from(panel.querySelectorAll('[role="menuitem"]')).filter((el) => {
        const r = el.getBoundingClientRect();
        // 被祖先 overflow 裁掉的元素：其可视矩形与几何矩形不一致
        return r.bottom > window.innerHeight || r.top < 0 || r.height === 0;
      }).length,
  );
  expect(clipped, '菜单条目被裁切或超出视口').toBe(0);
});
```

**更强的判据**（能直接抓本次 bug）：用 `elementFromPoint` 检查每个条目的**中心点**是否真的命中它自己：

```ts
const occluded = await page.getByTestId('menu-panel').evaluate(
  (panel) =>
    Array.from(panel.querySelectorAll('[role="menuitem"]')).filter((el) => {
      const r = el.getBoundingClientRect();
      const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
      return !el.contains(hit) && hit !== el;
    }).length,
);
expect(occluded).toBe(0);
```

> `document.elementFromPoint` 会**穿过被裁切区域**返回真正可见的元素，因此被 `overflow: hidden` 裁掉的条目会返回 `null` 或别的元素——**这是能直接失败的正确判据**。

---

## 2. 计划核对轮 4 项阻塞——全部确认修复 ✅

| #      | 项                      | 修复证据                                                                                                                                                                 |
| ------ | ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **B1** | jest-dom 未注册         | `tests/setup.ts` 存在（`import '@testing-library/jest-dom/vitest'`），`vitest.config.ts:12` 有 `setupFiles`。仓库现有 **18 处** jest-dom 匹配器在用，测试全绿 → 真实生效 |
| **B2** | Task 9 断言假绿         | `e2e/tests/visual.spec.ts` 改为检测 `.toolbar` 自身 `scrollWidth - clientWidth`，**且新增了一条「可证伪」测试**：注入 3000px 元素后断言必须 `> 0`。**修法优于我的建议**  |
| **B3** | `menu.darkMode` 死键    | `zh-CN.json` / `en.json` 的 `menu` 对象均无 `darkMode` ✅                                                                                                                |
| **B4** | 注释未剥离导致假红/假绿 | `tests/unit/design-tokens.test.ts:11` 有 `const css = raw.replace(/\/\*[\s\S]*?\*\//g, '')`，且注释说明了两类误报                                                        |

## 3. 五项高风险——处理确认 ✅

| #      | 项                                | 实际处理                                                                                                                                     |
| ------ | --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| **H1** | 斑马纹 `nth-child` 与虚拟滚动冲突 | 改用 **`data-odd` 属性**（`globals.css:441-455`），注释明确写了「交替底色由逻辑索引 data-odd 决定，不能用 `:nth-child(odd)`」。✅ 采纳方案 B |
| **H2** | 验证命令漏 `#6b7280`              | `src/components/` 下（非 dialogs）**零硬编码色值** ✅。剩余硬编码全在 `dialogs/`（R7 范围外，符合预期）                                      |
| **H3** | `dirty-dot` 影响可访问名          | **该元素已被删除**（`52153bf refactor: 移除按钮内不可见的未保存指示点`），改由状态栏承担。✅ 比修 `aria-label` 更彻底                        |
| **H4** | `btnStyle` 值改动波及对话框       | **值保持 `var(--app-bg)` 不变**，并在 JSDoc 里写明「H4：值必须保持不变」及理由 ✅                                                            |
| **H5** | `.tab-active` 值改动              | 已按「有意偏离」处理并记录（用户报告中有说明）                                                                                               |

## 4. 安全不变式——全部成立 ✅

| 不变式             | 核实                                                                                    |
| ------------------ | --------------------------------------------------------------------------------------- |
| 过渡别名在         | `globals.css:57-59` 三个别名齐全 ✅                                                     |
| 对话框零改动       | `git diff --stat main...feat/gui-redesign -- src/components/dialogs/` **无输出** ✅     |
| 9 个原 testid 完好 | 全部在，另新增 `menu-panel` / `toolbar` 两个 ✅                                         |
| 零新依赖           | `git diff main...feat/gui-redesign -- package.json package-lock.json` **无依赖变更** ✅ |

## 5. 指标复核

| 项       | 声明                                     | 实测                                                           |
| -------- | ---------------------------------------- | -------------------------------------------------------------- |
| 单元测试 | 312 通过 / 24 文件                       | ✅ **312 passed (24 files)**                                   |
| Rust     | core 227 / CLI 65                        | 未独立重跑（用户报告）                                         |
| 分支     | `feat/gui-redesign`，`d200cb0 → 0b77707` | ✅ 基点存在，22 commits                                        |
| 变更     | 34 文件                                  | 实测 **43 文件**（含 9 个文档文件；`git diff --stat` 输出 43） |

> **文件数口径差异**：用户说 34，实测 43。差集是 `docs/审核和开发/2026.10.17/` 下的 5 份交接/裁断文档 + `docs/superpowers/{specs,plans}/`。**非代码问题**，但合并前应确认这些文档是否要一并进 main。

## 6. 视觉验证结果（首次有人真正看界面）

| 截图          | 结果                                                                                     |
| ------------- | ---------------------------------------------------------------------------------------- |
| 浅色 1200×800 | ✅ 布局干净：品牌块、Tab pill、单行工具栏、路径表格、状态栏分层次清晰                    |
| 深色 1200×800 | ✅ 深色渐变背景、半透明表面、强调色辉光正常                                              |
| 浅色 800×600  | ✅ **单行不换行**，搜索框弹性收缩到约 160px                                              |
| 深色 800×600  | ✅ 同上                                                                                  |
| 英文 800×600  | ✅ `New / Edit / Delete / Up / Down / Undo / Redo / Import / Export / Save` 全部单行放下 |
| 菜单展开      | ❌ **只显示 1/8 项**（见 §1）；修复后 ✅ 8/8 完整                                        |

## 7. 合并建议

**不建议直接合并。** 顺序建议：

1. **修复 §1 的菜单裁切**（方案 A：删除 `.toolbar` 的 `overflow: hidden`，并跑 `visual.spec.ts` 确认 800px 仍不裁切）；
2. **补一条能抓这类问题的回归断言**（§1 末尾的 `elementFromPoint` 版本）；
3. 重跑 `npm run verify:all`；
4. 再合并。

若时间紧、必须现在合并：**这是已知且有明确修复路径的单个 bug**，风险可控——但**菜单是这波新功能的唯一入口**（浏览/清理/分析/配置/主题/语言/帮助/关闭 8 项全部只能从这里进），不能用。

## 8. 待确认

**待确认 1**：`docs/审核和开发/2026.10.17/` 下 5 份文档（核对交接 3 份 + 裁断 2 份）与 `docs/superpowers/` 的 spec/plan 是否随分支一起进 main？

- 建议**一并合并**：它们是本轮决策的完整记录链，与代码同等重要。

**待确认 2**：`.superpowers/sdd/2026-10-17-gui-redesign-implementation/`（ledger 与任务报告，gitignored）合并后删除还是保留？

---

# 第二轮复核（修复后 + Tauri 真机确认）

**对象**：`feat/gui-redesign @ 4c6ed72`（新增 `89a49f2` portal 修复、`4c6ed72` 遮挡回归测试）

## 9. 菜单裁切修复——确认有效 ✅

独立双向验证（非采信报告）：

| 步骤                                      | 结果                              |
| ----------------------------------------- | --------------------------------- |
| 回退 `Menu.tsx` 到 `0b77707`（非 portal） | 遮挡测试 **FAIL**，`occluded = 8` |
| 恢复 portal 版本                          | 遮挡测试 **PASS**，`occluded = 0` |

E2E 全量独立重跑：**38 passed**（含新增的两条可证伪守卫）。

## 10. Tauri 真机验证——本轮最重要的一节

### 10.1 首次真机验证暴露的三个问题

**问题 A：`npm run dev` 卡死（既有环境问题，非本分支回归）**

`vite dev` 在**原仓库**稳定卡死（CPU 累计 320 秒满载，日志停在 `[optimizer] scanning dependencies...`）；在 **worktree** 中用**同一份源码、同一个 `node_modules`** 却正常运行（HTTP 200，437ms 就绪）。

排除过程（逐一实测）：

- 整个 `src/` 换成 feat 版 → worktree 仍正常；
- `globals.css` 单独替换 → 不触发；
- 移除 `.codegraph` / `target` / `.git` / `coverage` / `dist` / `test-results` → 均不触发；
- 清空 `node_modules/.vite` 缓存 → 不触发（原仓库仍卡）。

**结论：`vite dev` 卡死是环境层面问题（与分支代码无关），且 `vite preview` 正常**（HTTP 200，E2E 与 `verify:all` 走的正是 preview）。**不阻塞发布**，但登记为待查项。

> **副作用（已完全恢复）**：排查中曾把 `.git` 改名为 `.git.off`，因 `cd` 被重置导致恢复命令未执行。已确认 `.git` 完好、`git status` 正常、HEAD 仍是 `4c6ed72`。

**问题 B：`dist/` 是旧产物（我自己的教训）**

首次 Tauri 截图显示「菜单仍在 `/relative` 内、`inlineStyle: null`、右侧溢出 138px」。经查是 **`dist/` 停留在 19:46 的旧构建**，未包含新的 portal 代码。**重新 `npm run build` 后一切正常**。

> 教训：**用 `vite preview` 验证改动前必须先 rebuild**，否则验证的是旧产物。

**问题 C（真实、仍存在）：`index.html` 标题陈旧**

```html
index.html:6 <title>PathEditor v5.1.2</title>
```

Tauri 窗口标题来自 `gui/tauri.conf.json`（`v5.1.6`），但 WebView2 的 `document.title` 仍是 **`v5.1.2`**，UIA/CDP 均可读到。**本波未修**，登记为遗留项。

### 10.2 修复后的真机结果（全新构建）

**浅色（900×700 真实窗口）**：

```json
{
  "portalParentIsBody": true,
  "inlineStyle": "top: 124.787px; left: 674px;",
  "overflowRight": -26,
  "overflowBottom": -265,
  "itemCount": 8,
  "occluded": []
}
```

截图确认：维护组（浏览…/清理无效路径/冲突分析/配置方案）、应用组（深色模式/切换语言/帮助 F1）、分隔线、红色「取消并关闭」——**8 项完整可见**。

**深色（同窗口，切换后）**：

```json
{
  "isDark": true,
  "bodyBg": "linear-gradient(160deg, rgb(21,21,29), rgb(13,1...",
  "panelBg": "rgba(30, 30, 40, 0.92)",
  "backdrop": "blur(14px)",
  "overflowRight": -26,
  "occluded": 0
}
```

截图确认：深色渐变背景、毛玻璃菜单、条目随主题变「浅色模式」、`F1` 快捷键提示、红色「取消并关闭」——**全部正常**。

> **R4（WebView2 `backdrop-filter`）实测成立**：WebView2 `155.0.4283.60`，`blur(14px)` 真实生效。

## 11. 合并结论（修订）

**原「不建议合并」已可撤销。** 菜单裁切已修复并经真机确认，8 项全部可见、零遮挡、浅深双主题正常。

合并前建议只需处理或不处理（均非阻塞）：

| 项                         | 建议                                           |
| -------------------------- | ---------------------------------------------- |
| `index.html` 标题 `v5.1.2` | 建议顺手改为从版本号注入或手动更新；**不阻塞** |
| `vite dev` 卡死            | 既有环境问题，**不阻塞**；建议单独排查         |
| 文档一并合并               | 建议一并进 main                                |

---

**签章**：以上结论基于 2026-10-17 在 `feat/gui-redesign @ 4c6ed72` 上实际执行的命令、Playwright 探针、CDP 直连截图与 Tauri 真机截图。所有探针脚本与临时 worktree 已删除；截图存于 job 临时目录，未进仓库。
