# 对话框重皮与过渡别名清理（Wave 4.1）— 开发回执

**提交窗口**: 开发窗口
**日期**: 2026-10-19
**交付对象**: 审核窗口
**分支**: `feat/dialog-restyle`（基点 `main @ c2c3c7c`，**7 个提交**（6 代码 + 本回执），22 files +439/−309）
**版本**: 5.1.7（**未升级**）
**状态**: 开发完成，质量门全绿，**未推送**

---

## 1. 交付内容概述

按修订后的 spec（c2c3c7c）与计划实施，6 个 Task 全部完成：

| Task | 提交      | 内容                                                                                                                                                                                                                                    |
| ---- | --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1    | `f809306` | 新增 `--app-success` / `--app-overlay` / `--app-dialog-shadow`（浅深两套）；`.modal-overlay` / `.modal-surface` / `.field-input` / `.dialog-list-item` / `.btn-block`；重写 `Modal.tsx`；**两处 E2E 选择器随类名迁移**（B1 裁断方案 A） |
| 2    | `60aa225` | 表单类对话框 5 文件迁移（PathEdit / Help / Import / NewEnvVar / EditEnvVar）                                                                                                                                                            |
| 3    | `ed576f8` | 分析 + 配置对话框 7 文件迁移（含 tab-pill、color-mix、dialog-list-item）                                                                                                                                                                |
| 4    | `644899d` | 备份恢复面板迁移（`data-selected` + dialog-list-item + Button）                                                                                                                                                                         |
| 5    | `bafb14f` | 删除三别名 / `.tab-active` / `btnClass`·`btnStyle` 及其无用 import；反转 2 条断言、删除 1 条用例、新增守卫测试；同步改 `globals.css:153` 注释                                                                                           |
| 6    | `dc0948a` | 新增 `e2e/tests/dialog-visual.spec.ts`（3 用例）                                                                                                                                                                                        |

**关键设计决策（选了什么 + 为什么）**：

- **Modal 视觉下沉到 CSS 类**：组件只剩挂载 + Escape；`.modal-overlay` / `.modal-surface` 承载全部视觉。为什么：DOM 类名固定，避免 Tailwind 组合散落，也让 E2E 选择器有稳定的锚点（本轮 B1 的教训）。
- **`.btn-block` 用未分层规则覆盖 `justify-center`**：Tailwind 4 的 utilities 在 `@layer` 内，未分层规则优先。为什么：不需要提高选择器权重就能让 Button 变成整行左对齐。
- **`.dialog-list-item` 双列表复用**：备份文件列表与配置方案列表共用选中态，`ProfileList` 的内联 rgba 色值随之消失（D8）。
- **守卫测试剥离注释后匹配**：与 `design-tokens.test.ts` 的 B4 口径一致。为什么：注释里写「原为 #3b82f6」不应造成假红。
- **按裁断落实**：创建备份 = secondary（H1）；`.modal-surface` 90vw（H2）；表单标签 13px（H3）；删除 ToolsTab / ProfileDetail 文字上的 opacity（H4）。

## 2. 代码改动清单

| 文件                                                   | 改动   | 说明                                                                                   |
| ------------------------------------------------------ | ------ | -------------------------------------------------------------------------------------- |
| `src/styles/globals.css`                               | +95/−… | 3 token ×2 主题、5 个工具类；删除三别名（含 TODO 注释）、`.tab-active`；改 `:153` 注释 |
| `src/components/ui/Modal.tsx`                          | 18 行  | 类名换成 `.modal-overlay` / `.modal-surface`，接口不变                                 |
| `src/components/ui/buttons.ts`                         | −23    | 删除 `btnClass` / `btnStyle` 与 `import type { CSSProperties }`                        |
| `src/components/dialogs/PathEditDialog.tsx`            | 30     | Button + `.field-input`                                                                |
| `src/components/dialogs/HelpDialog.tsx`                | 16     | Button + muted                                                                         |
| `src/components/dialogs/ImportDialog.tsx`              | 40     | Button + `.btn-block`                                                                  |
| `src/components/dialogs/NewEnvVarDialog.tsx`           | 57     | `.field-input` ×4、token 错误色                                                        |
| `src/components/dialogs/EditEnvVarDialog.tsx`          | 46     | 同上（逻辑逐行保留）                                                                   |
| `src/components/dialogs/AnalyzeDialog.tsx`             | 22     | `.tab-pill` + `data-active`                                                            |
| `src/components/dialogs/analyze/ConflictsTab.tsx`      | 24     | success/danger/subtle token                                                            |
| `src/components/dialogs/analyze/ToolsTab.tsx`          | 16     | `.field-input`；`color-mix`；删 opacity                                                |
| `src/components/dialogs/analyze/EmptyHint.tsx`         | 2      | muted → subtle                                                                         |
| `src/components/dialogs/ProfileDialog.tsx`             | 29     | Button + `.field-input` + `maxWidth:'100%'`                                            |
| `src/components/dialogs/profile/ProfileList.tsx`       | 9      | `.dialog-list-item` + `data-selected`                                                  |
| `src/components/dialogs/profile/ProfileDetail.tsx`     | 62     | Button（sm）、token、删 opacity、删内联色                                              |
| `src/components/dialogs/env-backup/EnvBackupPanel.tsx` | 120    | Button（恢复 primary / 备份 secondary）、`.dialog-list-item`、全部 token 化            |
| `tests/unit/design-tokens.test.ts`                     | 26     | +1 用例（工具类）、反转 2 条                                                           |
| `tests/unit/button.test.tsx`                           | 9      | 删除「兼容导出」整块 + 改 import                                                       |
| `tests/unit/dialog-tokens.test.ts`                     | +44    | **新增**静态守卫（8 用例）                                                             |
| `e2e/tests/dialog-visual.spec.ts`                      | +56    | **新增**视觉回归（3 用例）                                                             |
| `e2e/tests/crud-undo.spec.ts`                          | 2      | `.fixed.inset-0` → `.modal-overlay`                                                    |
| `e2e/tests/keyboard.spec.ts`                           | 2      | 同上                                                                                   |

**零改动确认**：`core/`、`gui/`、`cli/`、`src/services/`、`src/store/`、`src/core/`、`src/i18n/` 均无差异（`git diff --stat main...feat/dialog-restyle` 可证）。零新依赖。

## 3. 提交清单

```text
dc0948a test(e2e): 新增对话框视觉回归
bafb14f refactor: 删除对话框过渡设施并反转测试断言
644899d style: 备份恢复面板迁移至设计系统
ed576f8 style: 分析与配置对话框迁移至设计系统
60aa225 style: 表单类对话框迁移至设计系统
f809306 feat: 对话框基础设施（token / Modal 外壳 / 表单与列表工具类）
```

## 4. 测试与质量门

**新增/变更测试**：

| 测试                                     | 变化        | 覆盖点                                                                                                  |
| ---------------------------------------- | ----------- | ------------------------------------------------------------------------------------------------------- |
| `design-tokens.test.ts`                  | +1 / 反转 2 | 新工具类已声明；三别名与 `.tab-active` 已删除；`TODO(Wave 4.1)` 不再出现                                |
| `dialog-tokens.test.ts`（新）            | 8 用例      | 14 个 `.tsx` 逐个扫描：3 别名 + `btnClass`/`btnStyle`/`tab-active` 禁现；硬编码色值禁现（剥注释后匹配） |
| `button.test.tsx`                        | −1          | 「btnClass / btnStyle 仍可用」随导出删除                                                                |
| `dialog-visual.spec.ts`（新）            | 3 用例      | Modal 表面非透明 + 圆角 10px（双主题）；对话框内无旧 class / 内联别名                                   |
| `crud-undo.spec.ts` / `keyboard.spec.ts` | 选择器更新  | `.modal-overlay input[type="text"]`                                                                     |

**质量门结果（实际执行）**：

| 检查                 | 结果                                                                                                               |
| -------------------- | ------------------------------------------------------------------------------------------------------------------ | --------------------------- | --------------------------------------- |
| `npm run verify:all` | ✅ **退出码 0**（format + lint + build + coverage + cargo fmt + clippy + cargo test + 41 E2E）                     |
| 单元测试             | ✅ **320 passed / 25 files**                                                                                       |
| 覆盖率               | ✅ **87.58%** lines（statements 85.81 / branches 75.72 / functions 88.8）                                          |
| E2E                  | ✅ **41 passed**（38 既有 + 3 新增）                                                                               |
| ESLint               | ✅ **0 error / 3 warning**（均为既有 `@tanstack/react-virtual` 的 `react-hooks/incompatible-library`，非本波引入） |
| Rust core            | ✅ **227 passed**（2 ignored）                                                                                     |
| Rust CLI             | ✅ **65 passed**                                                                                                   |
| 四道验收 grep        | ✅ 全部为空（`exit=1`）：`--app-list                                                                               | --app-select-row`/`btnClass | btnStyle`/`tab-active` / 对话框硬编码色 |

**测试数口径**：312（基线）→ 313（Task 1 +1）→ 320（Task 5 +8 守卫 −1 兼容导出用例）。

## 5. 真实注册表闭环测试

**未做。** 本波为纯前端视觉迁移，Global Constraints 明确零 Rust / IPC / store / 业务逻辑改动；E2E 全程 mock IPC，未触碰真实注册表。

### 未覆盖项（如实列出）

- **真机 Tauri / WebView2 窗口未单独验证**：视觉验证走 Playwright（Chromium）+ `vite build`。`backdrop-filter` 在 WebView2 155.0.4283.60 的可用性已在 Wave 5.1.7 真机验证过，本轮复用同一属性与同一 token，但未再次真机确认。
- 150% / 200% 缩放下的对话框布局（spec 已登记为后续项）。
- 对比度未新增自动化校验：D6 的取值沿用 5.1.7 的实测值（muted / subtle 均已达 AA 门槛）。

## 6. 需要审核窗口重点关注的项

1. **E1 偏离**：修订版计划里 `dialog-tokens.test.ts` 的剥注释正则转义在传输中丢失（`.replace(//*[sS]*?*//g, '')` 是无效语法），我按意图实现为 `.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '')`。请核对行为是否与你的预期一致（注释是否剥干净、行注释正则是否过宽）。
2. **`.modal-surface` 的 `backdrop-filter: blur(14px)`** 是计划引入、spec D1 未单列该属性（沿用了菜单毛玻璃的既有做法）。浅色下表面不透明、模糊不可见；深色下 92% 半透明生效。
3. **创建备份为 secondary**（H1 裁断）：实现位置 `EnvBackupPanel.tsx` 顶部按钮（无 `variant`，走默认值）；「恢复」是对话框内唯一 `variant="primary"`。
4. **守卫测试口径**：`dialog-tokens.test.ts` 扫描的是 `.tsx`（14 个），三个 `.ts` hooks 不在扫描范围（它们本就不含样式，属预期的范围边界）。
5. **未推送**：`main` 未被改动，分支仅存在于本地。

## 7. 交付物位置索引

| 交付物                                     | 位置                                                                                                  |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| 代码分支                                   | `feat/dialog-restyle`（6 commits）                                                                    |
| 计划 Execution Notes（偏离/未覆盖/质量门） | `docs/superpowers/plans/2026-10-19-dialog-restyle-implementation.md` 末尾                             |
| 本回执                                     | `docs/审核和开发/2026.10.19/PathEditor-dialog-restyle开发回执.md`                                     |
| 截图（临时目录，未进仓库）                 | `$CLAUDE_JOB_DIR/tmp/wave41-shots/`：`dialog-{path-edit,analyze,profile,env-backup}-{light,dark}.png` |
| 守卫测试                                   | `tests/unit/dialog-tokens.test.ts`                                                                    |
| 视觉回归                                   | `e2e/tests/dialog-visual.spec.ts`                                                                     |

## 8. 合并与分支状态

- `main @ c2c3c7c`（含 spec/plan 与裁断；领先 `origin/main` **3** 个提交：`01c19ab`、`f14c310`、`c2c3c7c`；**未推送**）。
- `feat/dialog-restyle @ 6f66e66`（基点 c2c3c7c，**7** 个提交），**尚未合并、未推送**。
- 工作区干净；临时探针 spec 与截图均未进仓库。

> 数字订正（审查报告 S7）：本节原文写「领先 2 个提交 / @ dc0948a / 6 个提交」，实际为 **3 / 6f66e66 / 7**（回执自身那个文档提交未计入）；已按实际值更新。

---

**签章**：以上结论均基于 2026-10-19 在本仓库实际执行的命令输出；`verify:all` 退出码 0，E2E 41 passed，四道验收 grep 全部 `exit=1`。
