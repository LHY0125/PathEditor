# GUI 前端重设计 — 核对轮交接（第 2 轮 · spec v2 复核）

**提交窗口**：开发窗口
**日期**：2026-10-17
**交付对象**：审核窗口
**复核对象**：`docs/superpowers/specs/2026-10-17-gui-redesign-design.md`（v2，353 行）
**基线**：`main @ 4011c2d`
**结论**：第 1 轮的 6 项裁断**全部认可**（B1 方案 A、B2、B3、H1、H2、H3）。**但 v2 自身有 4 处新问题**，其中 **1 处需用户拍板**（浏览... 的归位理由本身不成立），**2 处为 v2 内部不一致**，**1 处是 v2 新提出的「假绿」结论不成立**。

---

## 1. 第 1 轮 6 项的复核确认

全部独立复跑，结论一致，无争议。补充三处**计数口径澄清**（v2 与开发窗口用了不同口径，都没错，但文档里应统一）：

| 项                        | 开发窗口原词   | 审核窗口复核词 | 实测（精确命令）                                                                           | 口径说明                                                                  |
| ------------------------- | -------------- | -------------- | ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------- |
| `--app-list-bg`           | 20 处          | 25 处          | **20 处**（`grep -ro -- "--app-list-bg" src --include='*.ts' --include='*.tsx' \| wc -l`） | 25 应是含 `globals.css` 的定义行（实际含 css 为 **24**）；两者都不等于 25 |
| `--app-list-*` 三变量合计 | 未给           | 未给           | **27 处**（不含 css）/ **33 处**（含 css）                                                 |                                                                           |
| 对话框内                  | 14 处 / 7 文件 | 14 处 / 7 文件 | **14 处 / 7 文件** ✅                                                                      | 完全一致                                                                  |

> **不构成阻塞**：三个口径指向同一事实（对话框依赖 14 处），只是数字不同。建议 spec v3 统一写「**14 处 / 7 文件**」，并用可复现的命令注明口径。

---

## 2. v2 的新问题

### V1 —— 「浏览...」归位的**理由**不成立（但**结论**无害）⏸ 需用户拍板

**v2 原文**（D2 表，第 72 行）：

> **浏览...** | **⋯ 菜单·维护组首项** | **B3**：放主行会让 800px 余量归零（中文 4 字 / 英文 `Browse...` 9 字符）

**核实结果：这个理由与 6 屏 mockup 在 800px 下的实测布局矛盾。**

`width-test.html` 是专门测 800px 的页面，它给出的主行方案（`final-comparison.html` 与 `layout-structure.html` 一致）是：

```
[搜索] [新建][编辑][删除] [上移][下移] [撤销][重做] [导入][导出] [保存] [⋯]
```

**该方案本身就没有给「浏览」留位置**——因为它是在**假设浏览已移出**的前提下画的。因此「放主行会让余量归零」这一条**无法从 mockup 验证**，属于事后补的理由。

同时，**6 个 mockup 全文扫描：`浏览` / `Browse` 出现 0 次**。即 mockup 从未画出浏览按钮，v2 把它安进「维护组首项」是**新增的、未经视觉确认的决策**。

**结论评价**：把浏览放进 ⋯ 菜单**本身是合理的**（`handleBrowse` 是 `open({directory:true})` + `addPath`，与「新建」语义高度重叠，低频）。**但理由需要改写成真实的那一条**：「与新建语义重叠、低频、避免主行第 19 个按钮」。**不要保留一个无法验证的宽度论证。**

**需用户确认**：v2 目前的选择是「⋯ 维护组首项」。备选是把浏览**并入「新建」做下拉**（更贴近它「新建一条路径」的实质），但这会让按钮变复合控件，成本更高。**开发窗口推荐维持 ⋯ 维护组首项**，只改理由。

### V2 —— v2 把 12 处「确定」再次写成「不受影响」，与前一句自相矛盾 ⛔

**v2 第 284 行**（测试策略表）：

> 现有单元测试 | 实测 **50 处** `getByRole('button')`；**工具栏保存按钮改用新 key `button.savePath`，对话框 `button.save`（「确定」）不变** → **12 处 `'确定'` 断言不受影响**。

**v2 第 329 行**（R3）：

> ...**12 处对话框断言不受影响** + **1 处必须改**（`toggle-save.spec.ts:19`）。**初版「已消解」结论有误**

**两处不矛盾**（12 不受影响 + 1 必须改 = 13），但第 284 行**只写了前半句**，读者会得到「全部安全」的错误印象。第 329 行才是正确的完整表述。

**处理**：第 284 行补上「+ 1 处必须改（见 R3）」。**这是文档一致性问题，非技术问题。**

### V3 —— 「假绿」结论**不成立**（应改为「会红」） ⛔

**v2 第 327 行**（H1 小节）：

> **⚠️ `env-vars.spec.ts:71` 会「假绿」**：`expect(...).toHaveCount(0)` 在按钮移进未展开的菜单后**永远为真**，即使按钮真的渲染了。**必须改成「先展开 ⋯ 菜单，再断言 count 为 0」**

**实测反驳**：该用例的前三行（`env-vars.spec.ts:67-71`）是：

```ts
await page.getByRole('button', { name: '全部变量' }).click();
await expect(page.locator('[data-env-var-key="user:JAVA_HOME"]')).toBeVisible();

await expect(page.getByRole('button', { name: '上移' })).toHaveCount(0); // :69
await expect(page.getByRole('button', { name: '下移' })).toHaveCount(0); // :70
await expect(page.getByRole('button', { name: '一键清理' })).toHaveCount(0); // :71
```

**关键前提**：这个用例的 `beforeEach` 在**默认的「系统 PATH」tab** 下运行，然后切到「全部变量」。切到 allVars 后渲染的是 `EnvVarToolbar`（`EnvVarToolbar.tsx:56-96`），**它是独立工具栏，不含 ToolBar、也不含任何 PATH 按钮**（`ToolBar` 只在 `activeTab !== 'allVars'` 时渲染，见 `AppShell.tsx:173-224`）。

**所以这三条断言测的是「allVars 下渲染的是 EnvVarToolbar 而非 ToolBar」，与按钮是否进菜单无关。** 只要 `EnvVarToolbar` 仍是独立组件、`ToolBar` 仍不在 allVars 下渲染，这些断言**继续有效，既不会红也不会假绿**。

**真正的假绿风险在别处**：如果将来有人把 `ToolBar` 也挂到 allVars 下（或把 ⋯ 菜单做成全局常驻），断言才会失效。**当前不会发生。**

**但同一形态的单测 5 处确有风险**（`tests/unit/app-shell-env-vars.test.tsx:198-202`）：

```ts
expect(screen.queryByRole('button', { name: '上移' })).toBeNull();
expect(screen.queryByRole('button', { name: '下移' })).toBeNull();
expect(screen.queryByRole('button', { name: '一键清理' })).toBeNull();
expect(screen.queryByRole('button', { name: '导入' })).toBeNull(); // ← 注意
expect(screen.queryByRole('button', { name: '导出' })).toBeNull(); // ← 注意
```

后两条**同样测的是 allVars 下不渲染 ToolBar**，逻辑正确。**但注意「导入/导出」在 D2 里是留在主行的**——若有人误把它们也移进菜单，后两条会静默假绿。这是同一类风险的**真实位置**，v2 指错了地方。

**处理**：把 H1 的「假绿」警示从 `env-vars.spec.ts:71` 移到 `tests/unit/app-shell-env-vars.test.tsx:198-202`，并说明判据是「allVars 下渲染的是 EnvVarToolbar 而非 ToolBar」。**v2 提出的「先开 ⋯ 再断言」做法对这个用例是错的**——allVars 下根本没有 ⋯ 菜单。

### V4 —— 组件改造清单漏了 2 个文件，而它们都依赖将被改写的变量 ⛔

**v2 的「组件改造清单」（第 227-241 行）共 13 个文件，不含**：

- `src/components/path-list/MergePreview.tsx`
- `src/components/env-list/EnvVarTable.tsx`

**但这两个文件在 v2 的「过渡别名」表（第 191/193 行）被标为「范围内」**，且它们实际依赖：

| 文件               | 依赖                                                                                                |
| ------------------ | --------------------------------------------------------------------------------------------------- |
| `MergePreview.tsx` | `--app-list-alt` ×1（:50）、`--app-list-bg`/`--app-list-alt` 斑马纹（:77）、硬编码 `#6b7280`（:66） |
| `EnvVarTable.tsx`  | `--app-list-alt` ×2（:75/:113）、`--app-select-row`（:110）、`--app-list-bg`（:112）                |

**后果**：若只按「组件改造清单」施工，这两个文件会**继续用过渡别名**，而 spec 的 Wave 4.1 验收标准是「`grep --app-list src/components/dialogs` 为空」——**它只检查 dialogs，不检查这两个文件，于是别名会被静默遗留**。

**处理**：把这两个文件补进「组件改造清单」（改样式，用新 `--app-row-*` / `--app-fg-faint` 语义变量），并在 Wave 4.1 验收命令里把范围从 `src/components/dialogs` 扩到**整个 `src/`**。

---

## 3. 顺带核实到的既有死代码（供 plan 参考，非阻塞）

| 项                                  | 事实                                                                     | 说明                                                                                                   |
| ----------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| `.row-even` / `.row-odd`            | `globals.css:84-90` 定义，**全仓 0 引用**                                | 死代码，重写 `globals.css` 时可直接不写                                                                |
| `.btn-disabled`                     | `globals.css:103-106` 定义，**全仓 0 引用**                              | 同上                                                                                                   |
| `.text-invalid` / `.text-duplicate` | `globals.css:93-100` 定义，**全仓 0 引用**                               | 同上（组件用的是 `#dc3545`/`#fd7e14` 内联）                                                            |
| `--color-path-*`                    | `@theme` 内定义，**全仓 0 引用**（`var(--color-path-invalid)` 从未使用） | 同上                                                                                                   |
| `:root` 实际变量数                  | **7 个**（spec 说 8 个）                                                 | `--app-bg`/`--app-list-bg`/`--app-list-alt`/`--app-fg`/`--app-border`/`--app-hover`/`--app-select-row` |
| `Modal.tsx`                         | 只用 `--app-bg` / `--app-fg`（同为语义层同名变量）                       | **R7 安全**，因为这两个名字在新语义层里继续存在                                                        |

> **`Modal.tsx` 的核实很重要**：所有对话框都经 `Modal` 渲染，而它只用 `--app-bg` / `--app-fg`。这两个名字 v2 的新语义层**保留**，所以对话框的**外框**在 B1 后是安全的；不安全的只有对话框**内部**那 14 处 `--app-list-*`。

---

## 4. 待用户拍板（唯一一处）

**V1：浏览... 的位置**。v2 选「⋯ 维护组首项」，开发窗口**认可该结论**，但**理由需改写**（现理由「放主行会余量归零」无法验证，且 mockup 从未画过浏览按钮）。

选项：

- **A（推荐）**：维持「⋯ 维护组首项」，理由改为「与新建语义重叠（都是新增一条路径）+ 低频 + 主行已有 18 个按钮」
- B：并入「新建」做成下拉
- C：留主行编辑组（需重新做 800px 实测，可能溢出）

---

## 5. 核对命令索引（可复现）

```bash
# V1：mockup 中是否有浏览
grep -o "浏览[^<]*\|Browse[^<]*" .superpowers/brainstorm/1093-1792212204/content/*.html

# V3：假绿判据 —— ToolBar 是否在 allVars 下渲染
sed -n '171,225p' src/components/layout/AppShell.tsx
cat src/components/env-list/EnvVarToolbar.tsx

# V4：遗漏文件
grep -n "var(--app-list\|var(--app-select\|#[0-9a-fA-F]\{6\}" \
  src/components/path-list/MergePreview.tsx src/components/env-list/EnvVarTable.tsx

# 死代码
grep -rn "row-even\|row-odd\|btn-disabled\|text-invalid\|text-duplicate" src --include='*.tsx' --include='*.ts'

# Modal 依赖
grep -n "var(--" src/components/ui/Modal.tsx
```

---

## 6. 结论

- 第 1 轮 6 项裁断 **全部认可**，无异议。
- v2 有 **4 处新问题**：V1（理由不成立，结论可用）、V2（文档不一致）、V3（假绿结论错，指错位置）、V4（清单漏 2 文件 + Wave 4.1 验收范围过窄）。
- **V2/V4 是纯文档修正，开发窗口可在 plan 阶段自行订正并记录**；**V3 需要更正 spec 的结论**（否则 plan 会写一个错误的测试改造方案）；**V1 需用户拍板**（仅涉及理由措辞，结论已认）。

**未做任何代码改动、未修改 spec、未提交任何文件。**

**签章**：以上结论均基于 2026-10-17 在 `main @ 4011c2d` 上实际执行的命令输出。
