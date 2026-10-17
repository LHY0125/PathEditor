# GUI 前端重设计 — 计划核对轮交接

**提交窗口**：开发窗口
**日期**：2026-10-17
**交付对象**：审核窗口
**核对对象**：`docs/superpowers/plans/2026-10-17-gui-redesign-implementation.md`（9 Task / 54 步）
**基线**：`main @ 4011c2d`
**结论**：**4 项阻塞、5 项高风险、4 项事实错误**。设计方向与 Task 划分**无异议**；问题集中在**测试基础设施**（jest-dom 未注册）与**验证手段有效性**（Task 9 两条断言实测假绿）。

---

## 0. 核对方法

逐 Task 核对：文件路径、符号签名、命令真实性、任务间一致性、以及**每条验证命令是否真的能验证它声称的东西**。关键结论用**可执行探针**坐实（探针脚本已删除，命令附于各条目）。

---

## 1. 阻塞项

### B1 —— jest-dom 未注册，Task 2/4 的测试**必然报错** ⛔

**plan 原文**：Task 2 Step 1 用 `expect(btn).toBeDisabled()`；Task 4 Step 1 用 `toBeInTheDocument()`（4 处）。

**实测**（探针，`tests/unit/__probe/p1.test.tsx`，已删）：

```
Received: "THREW: Invalid Chai property: toBeInTheDocument"
Received: "THREW: Invalid Chai property: toBeDisabled"
```

**根因**：`@testing-library/jest-dom` 虽在 `devDependencies`（`^6.9.1`），但 **`vitest.config.ts` 没有 `setupFiles`**，仓库内**也没有任何 `setup.ts`**：

```bash
$ grep -rn "jest-dom" src tests e2e *.ts *.json | grep -v node_modules
package.json:51:    "@testing-library/jest-dom": "^6.9.1"     # ← 仅声明，从未 import
```

**旁证**：现有 17 个单测文件**零使用**这些匹配器（全用 `toBeNull()` / `not.toBeNull()` / `toBe()`），所以这个缺口从未暴露。

**修复（二选一）**：

| 方案          | 做法                                                                                                                                                                                                                                                                                           |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A（推荐）** | 新建 `tests/setup.ts` 内容 `import '@testing-library/jest-dom/vitest';`，并在 `vitest.config.ts` 加 `test.setupFiles: ['tests/setup.ts']`。**注意**：`@testing-library/jest-dom` v6 的 vitest 入口是 **`/vitest`** 子路径（已核实 `package.json` exports 含 `./vitest`），不是 `/jest-globals` |
| B             | 把 plan 里 5 处匹配器改成仓库既有风格（`expect(btn).toBeDisabled()` → `expect((btn as HTMLButtonElement).disabled).toBe(true)`；`toBeInTheDocument()` → `not.toBeNull()`）                                                                                                                     |

> **方案 A 更好**：它同时修好了「仓库装了 jest-dom 却没用」这个既有缺口，且 plan 里 5 处断言可原样保留。**但 A 会改 `vitest.config.ts`，属于 plan 未列出的文件**，需在 plan 的文件清单里补上。

### B2 —— Task 9 的两条断言**实测假绿**（撑爆 1618px 仍通过）⛔

**plan 原文**（Task 9 Step 1）：

```ts
const box = await toolbar.boundingBox();
expect(box!.height).toBeLessThan(48); // 断言 1
const overflow = await page.evaluate(
  () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
);
expect(overflow).toBeLessThanOrEqual(0); // 断言 2
```

**实测探针**（复刻 plan Task 6 的 `.toolbar` 样式 + 30 个按钮，`e2e/tests/__probe3.spec.ts`，已删）：

```json
{
  "toolbarHeight": 45,
  "assertion1_pass": true,
  "overflowDelta": 0,
  "assertion2_pass": true,
  "toolbarScrollW": 2418,
  "toolbarClientW": 800,
  "clippedPx": 1618,
  "lastButtonRight": 2408,
  "viewportW": 800
}
```

**工具栏被撑爆 1618px、最后一个按钮跑到 2408px（视口仅 800px），两条断言全部通过。**

**根因**：plan 自己在 Task 6 给 `.toolbar` 写了 `overflow: hidden`：

```css
.toolbar { display: flex; align-items: center; gap: var(--gap); ... overflow: hidden; }
```

`overflow: hidden` 使内容**被裁切而不产生滚动**，于是：

- 断言 1：`flex` 单行 + `overflow: hidden` → 高度**永远**是单行高度（45px < 48），**与是否放得下无关**；
- 断言 2：`documentElement` 的 `scrollWidth` 不增长（内容被裁在 `.toolbar` 内部），**永远为 0**。

**这正是审核窗口在 spec v2 里担心、当时判为「不存在」的假绿——它在 plan 里真实出现了。**

**修复**：断言必须改为**检测裁切**，而非检测高度/文档溢出：

```ts
// 断言 1（保留，但不足以单独成立）
const box = await toolbar.boundingBox();
expect(box!.height).toBeLessThan(48);

// 断言 2（替换）：工具栏内容不得被裁切
const clipped = await toolbar.evaluate(
  (el) => (el as HTMLElement).scrollWidth - (el as HTMLElement).clientWidth,
);
expect(clipped, '工具栏内容被裁切（按钮溢出视口）').toBeLessThanOrEqual(1);

// 断言 3（新增，最强）：最后一个可见按钮的右边界不得超出视口
const lastRight = await toolbar.evaluate((el) => {
  const kids = Array.from(el.children).filter((c) => (c as HTMLElement).offsetWidth > 0);
  const last = kids[kids.length - 1];
  return last ? last.getBoundingClientRect().right : 0;
});
expect(lastRight).toBeLessThanOrEqual(800);
```

> **探针已证明**：`clippedPx` 在撑爆时为 1618，正常时应为 0——这是**能真正失败**的判据。

### B3 —— Task 5 的 `menu.darkMode` 自相矛盾（plan 自审声称已修，实际未修）⛔

**plan 自审记录**（用户转述）说：「② 深色模式菜单项的自相矛盾……已改为复用既有两个 key」。

**实际文本**：

| 位置                                           | 内容                                     |
| ---------------------------------------------- | ---------------------------------------- |
| 第 1035 行（Interfaces）                       | Produces 列表**不含** `menu.darkMode` ✅ |
| 第 1063-1074 行（测试 `NEW_KEYS`）             | **不含** `menu.darkMode` ✅              |
| 第 1261 行（Task 6 说明）                      | 「**不新增** `menu.darkMode`」✅         |
| **第 1131 行（Task 5 Step 3 写入的 JSON）**    | **`"darkMode": "深色模式",`** ❌         |
| **第 1153 行（Task 5 Step 4 写入的 en JSON）** | **`"darkMode": "Dark mode",`** ❌        |

**Step 3/4 的 JSON 代码块仍会写入 `menu.darkMode`**，与 Interfaces、测试、Task 6 说明**三处冲突**。

**后果**：`i18n-keys.test.ts` 的「中英文键集合一致」用例**不会失败**（两边都多一个键，仍一致），所以这个矛盾**不会被测试抓住**——它只会让 `zh-CN.json` / `en.json` 里多一个**死键**（`menu.darkMode` 无人引用）。

**修复**：删除 Task 5 Step 3 第 1131 行与 Step 4 第 1153 行的 `darkMode` 行。

### B4 —— Task 1 的测试会**误报通过**：`declares()` 正则匹配到注释 ⛔

**plan 原文**（Task 1 Step 1）：

```ts
function declares(name: string): boolean {
  return new RegExp(`^\\s*${name}\\s*:`, 'm').test(css);
}
```

**问题**：该正则**不区分注释与声明**。plan Task 1 Step 3 的 CSS 里，过渡别名上方有一段注释：

```css
/* TODO(Wave 4.1)：以下为过渡别名 —— 对话框重做后删除。不得静默遗留。
     引用方：dialogs/ 下 7 个文件共 14 处（NewEnvVarDialog 4、ProfileDetail 3、
     EnvBackupPanel 2、ToolsTab 2、ProfileDialog 1、PathEditDialog 1、EditEnvVarDialog 1）。 */
--app-list-bg: var(--app-surface);
```

**实测**：`^\s*--app-list-bg\s*:` 在该文件中**只匹配真声明**（注释行以 `引用方` 等文字开头，不以 `--` 开头），**所以这条恰好安全**。

**但同类风险真实存在**：Task 1 的「不包含死代码」用例：

```ts
for (const dead of ['--color-path-invalid', ...]) {
  expect(css.includes(dead), `死变量 ${dead} 未清理`).toBe(false);
}
```

`css.includes()` 是**裸子串匹配**——若重写后的 `globals.css` 在注释里提到「已删除 `--color-path-invalid`」，测试**会失败**（假红）；反之若把死变量注释掉而不删除，测试**会通过**（假绿）。

**修复**：`declares()` 与死代码检查都应**先剥离注释**再匹配：

```ts
const stripped = css.replace(/\/\*[\s\S]*?\*\//g, '');
function declares(name: string): boolean {
  return new RegExp(`^\\s*${name}\\s*:`, 'm').test(stripped);
}
// 死代码检查同样用 stripped
expect(stripped.includes(dead), `死变量 ${dead} 未清理`).toBe(false);
```

---

## 2. 高风险项

### H1 —— Task 7 的 `nth-child(odd)` 斑马纹与虚拟滚动冲突，且与 plan 自己的表述矛盾

**plan 两处自相矛盾**：

| 位置                               | 表述                                                                            |
| ---------------------------------- | ------------------------------------------------------------------------------- |
| 第 56 行（文件清单）               | PathTable「**移除斑马纹**；语义变量」                                           |
| 第 1741 行（Task 7 Step 3 的 CSS） | `.path-row:nth-child(odd) { background: var(--app-row-odd); }` ← **保留斑马纹** |

**更严重的是技术问题**：`PathTable` 用 `@tanstack/react-virtual`，行是**绝对定位**（`position: absolute` + `translateY`），且**只渲染视口内的行**（`getVirtualItems()`）。因此：

- `nth-child(odd)` 的奇偶**取决于当前渲染了哪些行**，不是逻辑索引；
- 滚动时虚拟窗口滑动，**同一行的奇偶会翻转**——斑马纹会**闪烁/跳变**。

**现状**用的是 `rowIdx % 2`（逻辑索引，`PathTable.tsx:147`），**是正确的**。

**修复**：二选一——

- **A（推荐）**��按第 56 行执行，**真移除斑马纹**（`.path-row:nth-child(odd)` 整条删除），用 hover + 选中态区分行。这也符合 spec D1「移除斑马纹、改用 `--app-row-*`」的意图。
- B：保留斑马纹，但必须用 `data-odd={rowIdx % 2 === 1}` 属性选择器，**不能用 `nth-child`**。

### H2 —— Task 7 Step 4 的验证命令**漏掉 `#6b7280`**

**plan 原文**（第 1798 行）：

```bash
grep -rn -- '--app-list\|--app-select-row\|#3b82f6\|#2563eb\|#dc3545\|#fd7e14' \
  src/components/layout src/components/path-list src/components/env-list
```

**实测漏网**（`grep -rn "#6b7280"`）：

```
src/components/path-list/MergePreview.tsx:66:  const textColor = enabled ? 'var(--app-fg)' : '#6b7280';
src/components/path-list/PathTable.tsx:129:            textColor = '#6b7280';
```

`#6b7280`（禁用条目灰）**不在验证命令的模式里**，所以改完跑命令会「干净通过」，但这两个文件**仍残留硬编码色值**。

**修复**：命令补 `#6b7280`；并建议改为**白名单式**——列出允许出现的色值（应只剩 `globals.css`），而非黑名单式排除。

### H3 —— Task 6 的 `dirty-dot` 会**改变保存按钮的可访问名**，Task 8 的 E2E 选择器有风险

**plan 原文**（Task 6 Step 5）：

```tsx
<Button variant="primary" disabled={!canWrite} onClick={props.onSave}>
  {t('button.savePath')}
  {isModified && <span className="dirty-dot" aria-label={t('status.modified')} />}
</Button>
```

`status.modified` = 「已修改」。`aria-label` 会**加入按钮的可访问名**，于是未保存时按钮的可访问名变成 **「保存 已修改」**。

**Task 8 Step 1 的选择器**：

```ts
await page.getByRole('button', { name: '保存' }).click();
```

Playwright 的 `name` 是**子串匹配**（已核实类型定义注释：`matching is case-insensitive and searches for a substring`），所以 `'保存'` **仍能匹配**「保存 已修改」——**这条安全**。

**但 Task 2 的单测不安全**（第 382 行）：

```ts
screen.getByRole('button', { name: '保存' }).click();
```

Testing Library 的 `name` 默认是**精确匹配**（`exact: true`）。若该测试渲染的组件带 `isModified`，会匹配失败。

**核实**：Task 2 的测试渲染的是独立 `<Button>保存</Button>`，**不带 dirty-dot**，所以**当前安全**。但这是一颗**定时炸弹**——将来若有人把该测试改为渲染 `ToolBar`，就会断。

**修复**：`dirty-dot` 用 `aria-hidden="true"` 而非 `aria-label`（视觉指示不应进入可访问名），或把徽标移到按钮**外部**。

### H4 —— `btnStyle` 的值被改，但**对话框仍在用**，会跨范围影响

**plan 原文**（Task 2 Step 3）：

```ts
export const btnStyle: React.CSSProperties = {
  backgroundColor: 'var(--app-surface)', // ← 原为 var(--app-bg)
  color: 'var(--app-fg)',
  borderColor: 'var(--app-border)',
};
```

**实测 `btnStyle` 消费方**（`grep -rn "btnStyle" src`）——**共 5 个文件，其中 2 个在 R7 范围外**：

| 文件                                        | 是否 R7 范围内                        |
| ------------------------------------------- | ------------------------------------- |
| `toolbar/ToolBar.tsx`                       | ✅ 范围内（Task 6 重写）              |
| `toolbar/ActionButtons.tsx`                 | ✅ 范围内（Task 6 重写）              |
| `toolbar/UndoRedoButtons.tsx`               | ✅ 范围内（Task 6 重写）              |
| `env-list/EnvVarToolbar.tsx`                | ✅ 范围内（Task 7 改样式）            |
| **`dialogs/env-backup/EnvBackupPanel.tsx`** | ⚠️ **范围外**（R7：对话框本波不重做） |

**后果**：改 `btnStyle` 的值会让 `EnvBackupPanel` 的按钮底色从 `--app-bg` 变成 `--app-surface`。**这不是「坏掉」（两个变量都存在），但是一次未经 spec 授权的跨范围视觉变更**——spec R7 明确说对话框「沿用旧样式」。

**修复**：Task 2 的 `btnStyle` **保持原值不变**（`var(--app-bg)`），只在 `@deprecated` 注释里标注；或明确在 plan 里记录「此变更会波及 EnvBackupPanel，属已知偏离」。

### H5 —— Task 1 改写 `.tab-active` 的规则，但对话框依赖它

**plan 原文**（Task 1 Step 3）：

```css
/* 保留：对话框 EnvBackupPanel.tsx:61 依赖 */
.tab-active {
  border-bottom: 2px solid var(--app-accent); /* 原为 #3b82f6 */
  color: var(--app-accent); /* 原为 #3b82f6 */
}
```

**问题**：spec H3 的裁断是「`.tab-active` **原样保留**（不重写其规则）」。plan 虽然保留了**类名**，但**改了它的值**（`#3b82f6` → `var(--app-accent)`）。

**影响评估**：`--app-accent` 浅色为 `#6366f1`（靛蓝），深色为 `#7c6cff`。这**实际上比原来更好**（原来深色下 `#3b82f6` 是硬编码的，不随主题变）。**但这是 spec 未授权的变更**，且会同时影响 `EnvVarToolbar.tsx:86`（范围内）与 `EnvBackupPanel.tsx:61`（范围外）。

**修复**：在 plan 里**明确记录**这是对 spec H3 的有意偏离及理由（「原规则硬编码 `#3b82f6`，深色主题下对比度不足；改为语义变量同时修复该问题」），或严格按 spec 保留原值。

---

## 3. 事实错误

| #   | plan 原文                                             | 实际                                                                                                                                                                                                                                                                                                     |
| --- | ----------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F1  | 第 1189 行「`onNew` … `onProfiles` 共 **17** 个回调」 | **16 个**（`grep -c "^  on[A-Z]"` = 16；plan 自己第 1419-1436 行的 `ToolBarProps` 也是 16 个）                                                                                                                                                                                                           |
| F2  | Task 9 第 1936 行「Expected: PASS（**5 个用例**）」   | 代码生成 **4 + 1 = 5** 个 ✅ **正确**（`SIZES` 2 档 × 2 主题 = 4，加英文 1 个）                                                                                                                                                                                                                          |
| F3  | Task 8 第 1866 行「全部 **10 个** spec 通过」         | ✅ **正确**（`ls e2e/tests/*.spec.ts` = 10）                                                                                                                                                                                                                                                             |
| F4  | 第 1866 行「`startup.spec.ts` 无需改动即应通过」      | ⚠️ **需复核**：`startup.spec.ts:33` 用 `getByRole('button', { name: '用户 PATH' })`，Task 7 把 Tab 从 `<button>` 改为 `<button className="tab-pill">`（**仍是 button**）✅ 安全。但 `startup.spec.ts:32` 断言「新建」按钮 `toBeDisabled()` —— 「新建」在 Task 6 后**仍在主行**（`ActionButtons`）✅ 安全 |

---

## 4. 已核实**正确**的部分（供参考，无需改动）

| 项                                                       | 核实结果                                                                                                     |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `tsc -b --noEmit` 可用性                                 | ✅ 探针验证：能捕获真实类型错误（`error TS2322`），且退出码正确                                              |
| `@/` 路径别名                                            | ✅ `vitest.config.ts` 与 `tsconfig.app.json` 均已配置                                                        |
| `import.meta.url` 在 e2e 中可用                          | ✅ `e2e/mocks/ipc.ts:4` 已在用                                                                               |
| `__dirname` 在 vitest 中可用                             | ✅ 探针验证返回 `'string'`                                                                                   |
| Task 2 的 `buttonVariants` 类名断言                      | ✅ 探针验证：`btn-primary` / `rounded-lg` / `whitespace-nowrap` 均包含                                       |
| Task 4 的 `Menu` 用 `React.ReactNode` 不 import React    | ✅ 探针验证：`tsc -b --noEmit` 退出码 0（`jsx: react-jsx` + 全局 `React` UMD 命名空间）                      |
| zh/en 键集合当前一致                                     | ✅ 175 = 175，无单边键                                                                                       |
| `data-testid` 9 个                                       | ✅ 与 spec 一致                                                                                              |
| Task 6 保持 `ToolBar` props 不变                         | ✅ 16 个 prop 全量转发，`AppShell.tsx` 确实无需改                                                            |
| `ActionButtons` 无直连测试                               | ✅ `grep -rn "ActionButtons" tests/` 无结果，移除 props 安全                                                 |
| `theme-store` 的 `darkMode` key                          | ✅ 与 Task 9 的 `localStorage.setItem('darkMode', '1')` 一致                                                 |
| Task 9 的 `documentElement` 溢出断言**在有滚动条时**有效 | ✅ 探针 2 验证：塞入 3000px 元素后检测到 2200px 溢出（**但见 B2——被 `.toolbar` 的 `overflow:hidden` 挡住**） |

---

## 5. 待裁断

**待裁断 1（B1）**：jest-dom 未注册，选方案 A（加 `tests/setup.ts` + 改 `vitest.config.ts`）还是 B（改写 5 处断言）？

- 推荐 **A**：一次修好既有缺口，plan 断言原样保留；但需在 plan 文件清单补 `tests/setup.ts` 与 `vitest.config.ts`。

**待裁断 2（H1）**：PathTable 斑马纹——真移除（方案 A）还是改用 `data-odd` 属性（方案 B）？

- 推荐 **A**：符合 spec D1 意图，且避开虚拟滚动的 `nth-child` 陷阱。

**待裁断 3（H5）**：`.tab-active` 的值改动（`#3b82f6` → `var(--app-accent)`）是「有意偏离」还是「严格保留」？

- 推荐**有意偏离并记录**：新值在深色下对比度更好，是净改进。

---

## 6. 核对命令索引

```bash
# B1：jest-dom 未注册
grep -rn "jest-dom" src tests e2e *.ts *.json | grep -v node_modules
grep -n "setupFiles" vitest.config.ts

# B2：假绿探针（复刻 .toolbar + 30 按钮，断言仍通过）
#   见本文件 §1.B2 的 JSON 输出

# B3：menu.darkMode 矛盾
grep -n "menu.darkMode\|darkMode" docs/superpowers/plans/2026-10-17-gui-redesign-implementation.md

# H1：斑马纹矛盾
grep -n "移除斑马纹\|nth-child(odd)" docs/superpowers/plans/2026-10-17-gui-redesign-implementation.md

# H2：漏掉的硬编码色
grep -rn "#6b7280" src/components/layout src/components/path-list src/components/env-list

# H4：btnStyle 跨范围消费方
grep -rn "btnStyle" src --include='*.tsx' | grep -v "ui/buttons.ts"

# F1：props 计数
grep -c "^  on[A-Z]" src/components/toolbar/ToolBar.tsx
```

---

## 7. 结论

**Task 划分与设计方向无异议**，9 个 Task 的依赖顺序、TDD 结构、Global Constraints 都合理。

**但 4 项阻塞必须先解决**：

- **B1**：不修则 Task 2/4 的测试**根本跑不起来**（`Invalid Chai property`）；
- **B2**：不修则 Task 9 的视觉回归**形同虚设**（撑爆 1618px 仍绿）；
- **B3**：不修则 i18n 文件多一个死键（测试抓不到）；
- **B4**：不修则 Task 1 的死代码检查可能假红/假绿。

**B2 尤其值得注意**：它是审核窗口在 spec v2 里**担心过、当时判定不存在**的那类问题，在 plan 里**真实发生了**。区别在于——spec 那次是**误判**（断言其实有效），plan 这次是**真存在**（`overflow: hidden` 让断言失效）。**同一个概念，两次判断，一次错一次对**——这说明「假绿」必须**用探针实测**，不能靠推理。

**未做任何代码改动、未修改 plan 或 spec、未提交任何文件。** 探针脚本已全部删除。

**签章**：以上结论均基于 2026-10-17 在 `main @ 4011c2d` 上实际执行的命令与探针输出。
