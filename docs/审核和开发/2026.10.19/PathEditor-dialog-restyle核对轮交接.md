# 对话框重皮与过渡别名清理（Wave 4.1）— 核对轮交接

**提交窗口**：开发窗口
**日期**：2026-10-19
**交付对象**：审核窗口
**核对对象**：`docs/superpowers/specs/2026-10-19-dialog-restyle-design.md` + `docs/superpowers/plans/2026-10-19-dialog-restyle-implementation.md`（6 Task / 1790 行，提交 `f14c310`）
**基线**：`main @ f14c310`（工作区干净；`npx vitest run` **312 passed / 24 files**；`npx tsc -b` 退出码 0）
**结论**：**3 项阻塞、4 项高风险、5 项事实/文档问题**。设计方向与 Task 划分**无异议**；问题集中在 **Modal 重写对既有 E2E 的连带影响**（计划完全未提）、**删除步骤的机械验收口径**（两个会让 `tsc`/验收 grep 必红的遗漏）、以及 **spec 与计划的三处规格不一致**。

---

## 0. 核对方法

逐 Task 核对：文件路径、符号签名、命令真实性、任务间一致性、以及 spec 覆盖。所有结论均基于在 `main @ f14c310` 上实际执行的命令；关键判据附输出。

---

## 1. 阻塞项

### B1 —— Modal 重写会打断 2 个现有 E2E，计划未列入任何 Task ⛔

**现有选择器**（`grep -rn "\.fixed\|inset-0" e2e/tests/`）：

```
e2e/tests/crud-undo.spec.ts:13:  await page.locator('.fixed.inset-0 input[type="text"]').fill('C:\\NewPath');
e2e/tests/keyboard.spec.ts:13:  await expect(page.locator('.fixed.inset-0 input[type="text"]')).toBeVisible();
```

**计划 Task 1 Step 5** 把 Modal 外层从

```tsx
<div className="fixed inset-0 z-50 flex items-center justify-center" style={{ backgroundColor: 'rgba(0,0,0,0.4)' }} ...>
```

改为

```tsx
<div className="modal-overlay" onClick={onClose}>
```

**`position: fixed` 与 `inset: 0` 只存在于 `.modal-overlay` 的 CSS 规则里，DOM 的 `class` 属性不再含 `fixed` / `inset-0` 字样。** Playwright 的 `.fixed.inset-0` 是**类名选择器**，不读计算样式 → 两个用例的第 13 行必然超时失败。

**波及用例**：`crud-undo.spec.ts`「添加路径后可撤销和重做」、`keyboard.spec.ts`「Ctrl+N 打开新建对话框」——都是主流程用例。

**计划为何没发现**：Task 6 Step 1 只按文件名跑 `e2e/tests/dialog-visual.spec.ts`；这两个用例只会在 Step 3 的完整 `verify:all` 里变红，而那时全部代码已改完。

**修复（二选一）**：

| 方案          | 做法                                                                                                                                                   | 评价                                                              |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------- |
| **A（推荐）** | Task 1 同步把两处选择器改为 `.modal-overlay input[type="text"]`，并把 `e2e/tests/crud-undo.spec.ts`、`e2e/tests/keyboard.spec.ts` 加入 Task 1 文件清单 | 选择器语义更稳（跟随新类名），与「提取 CSS 类」的方向一致         |
| B             | Modal 外层写成 `className="modal-overlay fixed inset-0"` 保留兼容类                                                                                    | 改动最小，但同一属性两处声明，与 Wave 4.1「删过渡设施」的宗旨相悖 |

### B2 —— Task 5 Step 4 漏删 `buttons.ts` 顶部的 `import type { CSSProperties }`，`tsc -b` 必红 ⛔

**证据**：

```
src/components/ui/buttons.ts:1: import type { CSSProperties } from 'react';
tsconfig.app.json: "noUnusedLocals": true
```

`CSSProperties` 在 `buttons.ts` 里的**唯一**用途是 `export const btnStyle: CSSProperties`（第 43 行）。Task 5 Step 4 指示「删除文件末尾这两个导出（含各自的 JSDoc）」——删完后第 1 行的导入变成未使用，`npx tsc -b` 报 `TS6133: 'CSSProperties' is declared but its value is never read`，Task 5 Step 6 的「Expected: PASS」直接不成立。

**修复**：Task 5 Step 4 补一句「同时删除文件首行 `import type { CSSProperties } from 'react';`」。

### B3 —— 验收 grep `tab-active` 不会为空：`globals.css:153` 的旧注释 ⛔

**spec 验收标准第 3 条**：`grep -rn "tab-active" src/` 结果为空。

**现状 3 处命中**：

```
src/components/dialogs/env-backup/EnvBackupPanel.tsx:61   ← Task 4 迁移后消失
src/styles/globals.css:153: /* 主界面 Tab（对话框仍用 .tab-active，勿改其规则） */   ← 计划未处理
src/styles/globals.css:178: .tab-active { ... }           ← Task 5 Step 3 删除
```

**Task 5 Step 3 只删规则（第 178-185 行）与 `TODO(Wave 4.1)` 块，未提第 153 行的注释。** 该注释含 `tab-active` 字样，验收 grep 必不空。

**更隐蔽的一点**：Task 5 反转后的 `design-tokens.test.ts` 用**剥离注释的 css** 匹配 `.tab-active {`，所以**测试会绿、验收会红**——又一个「判据口径不一致」。上一轮刚处理过同类问题（B4 注释剥离），这次换了个方向复发。

**修复**：Task 5 Step 3 明确「把 `globals.css:153` 的注释改为『主界面 Tab（`.tab-pill`）』或直接删除」。

---

## 2. 高风险项（spec ↔ 计划不一致，需裁断）

### H1 —— 创建备份按钮：spec D3 说 `primary`，计划用默认 `secondary`

spec D3 原文：「主操作（确定 / 保存 / 应用 / 恢复 / **创建备份**）→ `primary`」。

计划 Task 4 的实现：

```tsx
<Button disabled={creating} onClick={() => void controller.createBackup()}>
```

未给 `variant` → `Button` 默认 `secondary`（已核 `Button.tsx:13` `variant = 'secondary'`）。同文件里的「恢复」用了 `variant="primary"` ✅。

**待裁断**：改计划（`variant="primary"`）还是改 spec（创建备份定为次操作）？

### H2 —— `.modal-surface` 最大宽度：spec D1 写 `90vw`，计划实现 `92vw`

- spec 第 65 行：`max-width: 90vw; max-height: 85vh; overflow: auto`
- 计划 Task 1 Step 4：`max-width: 92vw;`

审核窗口在交付说明里写明了自查修正（90→92，理由：800px 窗口下 720px 备份面板横向溢出），但 **spec 未同步更新**。

补充事实：计划同时给三个固定宽度面板加了 `maxWidth: '100%'`（Task 3/4 内联样式），**即使保持 90vw 也不会溢出**（800×0.9−42 = 678px 可用，面板被 `maxWidth:100%` 夹到 678）。92vw 并非必需，但无害。

**待裁断**：spec 改 92vw，还是计划回退 90vw？（二选一，消除文档冲突。）

### H3 —— 表单标签字号：spec D9 说 13px（继承 body），计划多处用 `text-xs`（12px）

spec D9 原文：「正文 / 表单标签 **13px（继承 body）**，去掉 `text-sm`」。

计划的实现：

| 文件               | 计划写法                            | 实际字号 |
| ------------------ | ----------------------------------- | -------- |
| `NewEnvVarDialog`  | `const labelClass = 'w-20 text-xs'` | 12px     |
| `EditEnvVarDialog` | 同上                                | 12px     |
| `PathEditDialog`   | `<label className="text-xs ...">`   | 12px     |

**待裁断**：标签用 12px（改 spec D9）还是 13px（去掉 `text-xs`，改计划）？

### H4 —— D6「透明度改语义前景色」在文字上仍有残留

spec D6：「正文用 `var(--app-fg)`；次要说明用 `--app-fg-muted`；更弱的提示用 `--app-fg-subtle`。**对非文字元素（分隔线、图标底色）保留 opacity 用法**。」

计划的最终代码里仍有两处在**文字**上保留 opacity：

| 位置                              | 计划代码                                                       | 问题                                                     |
| --------------------------------- | -------------------------------------------------------------- | -------------------------------------------------------- |
| `ToolsTab.tsx`（目录名）          | `opacity: group.exists ? 1 : 0.6` + `color: var(--app-danger)` | danger 色再乘 0.6 → 对比度可能低于 AA；该元素是文字      |
| `ProfileDetail.tsx`（禁用路径行） | `opacity: entry.enabled ? 1 : 0.5` + danger 色 + 删除线        | 同上；但「禁用行」是状态表达而非层级表达，保留与否属裁量 |

**待裁断**：按 D6 改为纯语义色（danger 原色 + 删除线已足够表达禁用），还是作为「状态而非层级」保留 opacity？若保留，建议在计划 Execution Notes 记录为有意偏离。

---

## 3. 事实与文档问题

| #   | 位置                                  | 问题                                                                                                                                                                                            |
| --- | ------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F1  | Task 5 Step 3 的 `.tab-active` 引用块 | **引用与实际文件不一致**：实际注释有 4 行，计划引用只有 3 行，少了「理由：硬编码色在深色玻璃主题下与 --app-accent(#7c6cff) 不协调。」。按计划逐字匹配会找不到目标块（见 `globals.css:178-185`） |
| F2  | Task 2 Step 7                         | 「Expected: PASS（**312** 个测试全绿）」——Task 1 Step 1 已新增 1 个用例，此处应为 **313**                                                                                                       |
| F3  | Task 3 步骤编号                       | Step 7 之后直接跳到 **Step 9**（缺 Step 8）                                                                                                                                                     |
| F4  | Task 5 Step 5                         | 删除 `btnClass / btnStyle 仍可用` 用例后，`describe('兼容导出', ...)` 变成空壳。建议整块删除（vitest 不报错，但留下无意义结构）                                                                 |
| F5  | 「文件结构 · 新建」表                 | 只列了 `tests/unit/dialog-tokens.test.ts`；Task 6 新建的 `e2e/tests/dialog-visual.spec.ts` **未列入**                                                                                           |

**另一条测试卫生风险**（非错误，建议记录）：`dialog-tokens.test.ts` 的 `HARDCODED_COLOR` 扫描**不剥离注释**。这是与 `design-tokens.test.ts` 的 B4 教训（先剥注释再匹配）**相反**的口径——若迁移后有人在对话框代码的注释里写 `#3b82f6`（例如「原为 #3b82f6」），会假红。当前计划给出的最终代码不含此类注释，不影响执行；但将来维护时是个坑。

---

## 4. 已核实正确（供参考，无需改动）

| 项                                            | 核实结果                                                                                                                   |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| 对话框文件数 / 行数                           | ✅ `find src/components/dialogs -name "*.tsx"` = **14 个 / 1097 行**，与 spec 精确一致（`.ts` hooks 不计）                 |
| `--app-list-bg` 7 文件 14 处                  | ✅ 实测一致（ToolsTab 2 / EditEnvVar 1 / EnvBackupPanel 2 / NewEnvVar 4 / PathEdit 1 / ProfileDetail 3 / ProfileDialog 1） |
| `--app-list-alt` / `--app-select-row` 零引用  | ✅ 仅 `globals.css` 声明行命中                                                                                             |
| `btnClass` / `btnStyle` 唯一消费方            | ✅ 仅 `EnvBackupPanel.tsx`（4 处使用 + 1 处 import）。`EnvVarToolbar` 已不消费（JSDoc 里的描述过时但无功能影响）           |
| `design-tokens.test.ts` 行号与断言            | ✅ `:51-57`（别名）、`:59-62`（.tab-active）与计划引用完全吻合；`declares()` / `css`（已剥注释）/ `raw` 均存在             |
| `button.test.tsx:68-71` 与 import 行          | ✅ 与计划引用逐字一致                                                                                                      |
| `.tab-pill` 已存在且用 `[data-active='true']` | ✅ `globals.css:154-172`；计划 AnalyzeDialog 的 `data-active={tab === tabId}` 写法与 CSS 选择器匹配                        |
| Modal 接口不变                                | ✅ 现接口就是 `{ open, onClose, children }`，8 个对话框调用方（`grep "<Modal"`）无需改动                                   |
| 新增 token 插入锚点                           | ✅ 浅色 `--app-disabled-fg: #a1a1aa;`（`globals.css:40`）、深色 `#63636e;`（`:87`）均存在                                  |
| `--radius-lg` = 10px                          | ✅ `globals.css:49`，Task 6 断言 `10px` 成立                                                                               |
| `Button` 默认变体 / 尺寸                      | ✅ 默认 `secondary` / `md`（28px）；`BASE` 含 `justify-center` → `.btn-block` 用未分层规则覆盖在 Tailwind 4 下成立         |
| Task 6 E2E 点击「新建」                       | ✅ `createIpcMock` 默认 `canWriteSystem: true`（`e2e/mocks/ipc.ts:143`），新建按钮默认可用；`button.new` =「新建」         |
| i18n 覆盖                                     | ✅ 计划用到的 **71 个 key 全部存在**（zh-CN），不新增 key                                                                  |
| `env-backup.spec.ts` 的 `name: '恢复', exact` | ✅ 计划保持文案不变，`envBackup.restore` =「恢复」                                                                         |
| backup controller 字段                        | ✅ `use-env-backup.ts` 返回 `backups/busy/creating/selected/summary/preview/outcome/createdPath/error`，与计划解构一致     |
| 4 个 data-testid                              | ✅ 三个在 EnvBackupPanel（计划全部保留）、`env-var-table` 在 EnvVarTable（本波不碰）                                       |
| 单测基线                                      | ✅ `npx vitest run` = **312 passed (24 files)**；`npx tsc -b` 退出码 0                                                     |
| `npm run dev` 卡死说明                        | ✅ Global Constraints 里的描述与上轮实测一致（用 build + Playwright 截图验证），诚实                                       |

---

## 5. 待裁断（请审核窗口逐条作答）

1. **B1**：E2E 选择器修复选 A（改两处选择器为 `.modal-overlay`，推荐）还是 B（保留 `fixed inset-0` 兼容类）？
2. **B2**：确认 Task 5 Step 4 补删 `import type { CSSProperties }`。
3. **B3**：确认 Task 5 Step 3 同步处理 `globals.css:153` 的注释。
4. **H1**：创建备份按钮 primary（按 spec）还是 secondary（按计划）？
5. **H2**：`max-width` 90vw（spec）还是 92vw（计划）？二选一并同步另一份文档。
6. **H3**：表单标签 12px（按计划）还是 13px（按 spec）？
7. **H4**：ToolsTab / ProfileDetail 文字上的 opacity 保留还是改语义色？
8. **F1/F5** 等文档问题是否随修订一并订正。

---

## 6. 结论

**设计方向无异议**——D1-D9 的取舍合理，Task 划分（基础设施 → 三批迁移 → 删除 + 反转 → 视觉回归）依赖顺序正确，TDD 结构（先写守卫测试、先失败后通过）成立，`npm run dev` 卡死的规避方式诚实。

**但 3 项阻塞必须先解决再开工**：

- **B1**：不修则 `crud-undo` 与 `keyboard` 两条主流程 E2E 必红，且暴露时间被推到全部改完之后；
- **B2**：不修则 Task 5 的类型检查必红（`noUnusedLocals`）；
- **B3**：不修则 spec 自带的机械验收（`grep tab-active`）必不空，且测试与验收口径背离。

**本文件不改动 spec / plan / 任何代码，未提交任何文件。** 以上结论均基于 2026-10-19 在 `main @ f14c310` 上实际执行的命令输出。

**签章**：开发窗口，2026-10-19。
