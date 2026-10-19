# 对话框重皮与过渡别名清理（Wave 4.1）— 设计文档

- **日期**：2026-10-19
- **版本目标**：Wave 4.1（5.1.7 之后的独立波次）
- **作者**：审核窗口
- **状态**：待开发窗口核对 → plan
- **前置**：5.1.7 已发布（GUI 重设计落地，主界面已迁移到设计 token）

## 背景与目标

5.1.7 完成了主界面的设计系统迁移，但**对话框被明确排除在外**（spec R7）：`src/components/dialogs/` 下 14 个文件仍使用旧样式——硬编码色值、废弃的 `btnClass`/`btnStyle`、旧 `.tab-active`、以及三个专为它们保留的过渡别名（`--app-list-bg` / `--app-list-alt` / `--app-select-row`）。

这造成两个后果：

1. **视觉断裂**：用户从主界面打开任一对话框，会看到另一套视觉语言（更大的圆角、更旧的蓝色、更小的控件）。
2. **技术债悬空**：三个过渡别名的唯一存在理由就是「等对话框迁移」，迁移不做，它们就永远删不掉。

**本波目标**：把全部对话框迁移到 5.1.7 的设计系统，并**删除全部过渡设施**（三个别名 + `.tab-active` 遗留规则 + `btnClass`/`btnStyle` 废弃导出）。

**完成判定**（可机械验证）：

```bash
grep -rn -- '--app-list\|--app-select-row' src/          # 必须为空
grep -rn "btnClass\|btnStyle" src/                       # 必须为空
grep -rn "tab-active" src/                               # 必须为空
npm run verify:all                                       # 必须退出码 0
```

## 现状盘点（已核实）

| 项                                    | 位置                                                                         | 数量                                                                                                                   |
| ------------------------------------- | ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| 对话框文件                            | `src/components/dialogs/`（含 `analyze/`、`env-backup/`、`profile/` 子目录） | 14 个，1097 行                                                                                                         |
| `Modal` 外壳                          | `src/components/ui/Modal.tsx`                                                | 1 个，37 行                                                                                                            |
| `--app-list-bg` 引用                  | 7 个对话框文件                                                               | 14 处                                                                                                                  |
| `--app-list-alt` / `--app-select-row` | —                                                                            | **0 处**（死变量，仅为 spec 承诺保留）                                                                                 |
| `btnClass` / `btnStyle`               | `EnvBackupPanel.tsx`                                                         | 4 处                                                                                                                   |
| 旧 `.tab-active`                      | `EnvBackupPanel.tsx:61`                                                      | 1 处                                                                                                                   |
| 硬编码色值                            | 9 个文件                                                                     | 18 处：`#3b82f6`×5、`#2563eb`×5、`#ef4444`×5、`#22c55e`×2、`#dc2626`×1、`rgba(59,130,246,0.15)`、`rgba(239,68,68,0.1)` |
| Tailwind 默认色类                     | 5 个文件                                                                     | 8 处：`text-red-500`×5、`text-green-600`×1、`text-yellow-500`×1                                                        |
| 透明度层级（`opacity-40/50/60/70`）   | 全部对话框                                                                   | 大量，用于表达次要文字                                                                                                 |
| 测试断言                              | `tests/unit/design-tokens.test.ts`、`tests/unit/button.test.tsx`             | **4 条断言要求旧设施存在，必须反转**                                                                                   |

### 必须同步反转的测试断言（关键）

| 文件:行                       | 现有断言                             | Wave 4.1 后                                          |
| ----------------------------- | ------------------------------------ | ---------------------------------------------------- |
| `design-tokens.test.ts:51-57` | 「保留 R7 过渡别名」三个变量必须声明 | **断言相反**：三个变量均不存在                       |
| `design-tokens.test.ts:59-62` | `.tab-active` 必须存在               | **断言相反**：`.tab-active` 不存在；`.tab-pill` 保留 |
| `button.test.tsx:68-71`       | `btnClass`/`btnStyle` 仍可用         | **删除该用例**（导出本身被移除）                     |

> 若不反转这些断言，Wave 4.1 的删除步骤会直接把测试跑红——这不是回归，而是**旧契约到期**。

## 视觉规格

### D1 —— Modal 外壳规格

| 属性     | 现状                     | Wave 4.1                                              |
| -------- | ------------------------ | ----------------------------------------------------- |
| 圆角     | `rounded-lg`（16px）     | `var(--radius-lg)`（10px）                            |
| 内边距   | `p-6`（24px）            | `20px`                                                |
| 表面色   | `var(--app-bg)`          | `var(--app-surface-raised)`（深色下为半透明抬升表面） |
| 遮罩     | `rgba(0,0,0,0.4)` 硬编码 | `var(--app-overlay)`（浅色 0.4 / 深色 0.65）          |
| 阴影     | 无                       | `var(--app-dialog-shadow)`（新增 token）              |
| 最大尺寸 | 无约束                   | `max-width: 90vw; max-height: 85vh; overflow: auto`   |

**放弃**：把对话框统一改造成「标题栏 / 内容区 / 底部按钮栏」三段式结构。用户明确选择「完全对齐设计系统」而非「统一结构」——改变组件内部结构会牵动 14 个组件的布局与测试选择器，收益低于风险。

### D2 —— 新增三个 token

```css
/* :root（浅色） */
--app-success: #15803d;
--app-overlay: rgb(0 0 0 / 40%);
--app-dialog-shadow: 0 24px 64px rgb(0 0 0 / 16%), 0 4px 12px rgb(0 0 0 / 8%);

/* :root.dark（深色） */
--app-success: #4ade80;
--app-overlay: rgb(0 0 0 / 65%);
--app-dialog-shadow: 0 24px 64px rgb(0 0 0 / 60%), 0 0 0 1px rgb(255 255 255 / 6%);
```

> **审查轮修订（2026-10-19）**：浅色 `--app-success` 由 `#16a34a` 改为 `#15803d`。原因：`#16a34a` 在白底仅 **3.30:1**，不满足 WCAG AA（4.5:1），而它在 `EnvBackupPanel` 与 `ConflictsTab` 中用于正常字号文字；`#15803d` 实测 5.02:1（白）/ 4.81:1（`#fafafa`）。深色值 `#4ade80`（9.60:1）不变。

**为什么新增 `--app-success`**：对话框用 `#22c55e` / `text-green-600` 表达「成功 / 有效」，而 token 层只有 `--app-danger` 与 `--app-warning`，缺一档。补 token 而不是继续硬编码。

**为什么深色遮罩更重**：深色背景下 40% 黑色几乎看不出层次，0.65 才能把对话框与背景分开。

### D3 —— 按钮映射规则

所有原生 `<button>`（除纯导航用途）改用 `src/components/ui/Button.tsx`：

| 语义                                        | 变体        | 例子                                             |
| ------------------------------------------- | ----------- | ------------------------------------------------ |
| 主操作（确定 / 保存 / 应用 / 恢复）         | `primary`   | `PathEditDialog` 的确认、`EnvBackupPanel` 的恢复 |
| 次操作（取消 / 重命名 / 关闭 / 创建备份）   | `secondary` | 取消按钮、`EnvBackupPanel` 的创建备份            |
| 低权重行内操作（编辑 / 删除 / 显示 / 隐藏） | `ghost`     | `EnvVarTable` 已采用的模式                       |
| 破坏性主操作（删除配置 / 删除变量）         | `danger`    | `ProfileDetail` 的删除                           |

**尺寸**：对话框内默认 `size="md"`（28px 高）；密集列表的行内操作用 `size="sm"`（24px 高）。

**放弃**：为对话框单独做一套按钮尺寸。`Button` 已有 `sm`/`md` 两档，够用。

> **裁断（核对轮 H1）**：「创建备份」定为**次操作**，而非初稿的主操作。理由：同一对话框里「恢复」是唯一会改注册表的动作，两个 `primary` 会稀释主次层级。

### D4 —— 表单控件统一 `.field-input`

对话框里 10 个 input/select 重复了同一套内联样式。新增一个类：

```css
.field-input {
  padding: 4px 10px;
  border: 1px solid var(--app-border);
  border-radius: var(--radius);
  background: var(--app-surface);
  color: var(--app-fg);
  font-size: 13px;
  outline: none;
}

.field-input:focus {
  border-color: var(--app-accent);
}
```

所有对话框内的 `input` / `select` / `textarea` 改用 `className="field-input"` + 按需的宽度类，删除内联 `style`。

### D5 —— 硬编码色替换表

| 现值                         | 替换为                                                     | 出现位置             |
| ---------------------------- | ---------------------------------------------------------- | -------------------- |
| `#3b82f6` / `#2563eb`        | `var(--app-accent)`（或直接用 `Button variant="primary"`） | 9 处                 |
| `#ef4444` / `#dc2626`        | `var(--app-danger)`                                        | 6 处                 |
| `#22c55e` / `text-green-600` | `var(--app-success)`                                       | 3 处                 |
| `text-red-500`               | `var(--app-danger)`                                        | 5 处                 |
| `text-yellow-500`            | `var(--app-warning)`                                       | 1 处                 |
| `rgba(59,130,246,0.15)`      | `var(--app-row-selected)`                                  | `ProfileList.tsx:26` |
| `rgba(239,68,68,0.1)`        | `color-mix(in srgb, var(--app-danger) 10%, transparent)`   | `ToolsTab.tsx:36`    |

### D6 —— 透明度层级改为语义前景色

对话框目前用 `opacity-40/50/60/70` 表达文字层级。透明度会**同时削弱对比度**且不随主题调整，改为语义前景色：

| 用途                            | 现状                        | 替换为                                                     |
| ------------------------------- | --------------------------- | ---------------------------------------------------------- |
| 正文                            | `var(--app-fg)`             | 不变                                                       |
| 次要说明 / 元数据 / 类型 / 来源 | `opacity-60` / `opacity-70` | `var(--app-fg-muted)`                                      |
| 更弱的提示 / 空状态             | `opacity-40` / `opacity-50` | `var(--app-fg-subtle)`（4.02:1 / 5.99:1，已在 5.1.7 建立） |

> 对**非文字**元素（分隔线、图标底色）保留 opacity 用法。

### D7 —— 删除过渡设施

| 设施                    | 删除条件                               | 处置                                                    |
| ----------------------- | -------------------------------------- | ------------------------------------------------------- |
| `--app-list-bg`         | 14 处引用全部迁移                      | 删声明 + 删 `TODO(Wave 4.1)` 注释                       |
| `--app-list-alt`        | 0 处引用                               | 直接删                                                  |
| `--app-select-row`      | 0 处引用                               | 直接删                                                  |
| `.tab-active` 规则      | `EnvBackupPanel.tsx:61` 迁移后无消费者 | 删规则                                                  |
| `btnClass` / `btnStyle` | 迁移后无消费者                         | 从 `buttons.ts` 删除导出；删 `button.test.tsx` 对应用例 |

### D8 —— 列表项选中态

`EnvBackupPanel` 的备份文件列表用 `.tab-active`（下边框 + 强调色）表达选中，语义不符。改为与 `ProfileList` 一致的行选中样式：

```css
.dialog-list-item[data-selected='true'] {
  background: var(--app-row-selected);
  color: var(--app-accent-text);
}
```

两个列表（备份文件、配置方案）共用这个类，消除 `ProfileList` 里的内联色值。

### D9 —— 字号层级

| 层级                     | 规格                                  | 现状                     |
| ------------------------ | ------------------------------------- | ------------------------ |
| 对话框标题               | `text-base`（16px）`font-semibold`    | 已如此，保留             |
| 正文 / 表单标签          | **13px（继承 body）**，去掉 `text-sm` | 现为 `text-sm`（14px）   |
| 次要说明 / 元数据        | `text-xs`（12px）+ `--app-fg-muted`   | 混用 `text-sm` + opacity |
| 等宽内容（路径、变量名） | `text-xs` + `ui-monospace`            | 部分已是                 |

## 安全边界

**本波是纯视觉迁移，不触碰任何业务逻辑。**

- 零 Rust 改动：`core/` / `gui/` / `cli/` 不动。
- 零 IPC 契约改动：`src/services/backend.ts` 不动。
- 零 store 改动：`src/store/` 不动。
- **敏感值通路不变**：`reveal_env_var` / `displayValue` / 掩码逻辑一律不动——只换展示样式，不改何时显示。
- **权限判定不变**：`canEdit` / `canDelete` 仍只读 Rust 计算结果，前端不新增任何判定。
- **备份恢复语义不变**：`EnvBackupPanel` 的四个计数、警告文案、`--force` 语义全部保持现状。
- 所有 `data-testid` / `data-backup-file` / `data-hive-filter` / `data-env-var-key` 属性**不得改名**。

## 验收标准

- [ ] `grep -rn -- '--app-list\|--app-select-row' src/` 结果为空
- [ ] `grep -rn "btnClass\|btnStyle" src/` 结果为空
- [ ] `grep -rn "tab-active" src/` 结果为空
- [ ] `grep -rnE "#3b82f6|#2563eb|#ef4444|#22c55e|#dc2626|text-red-500|text-green-600|text-yellow-500" src/components/dialogs` 结果为空
- [ ] `design-tokens.test.ts` 的三条相关断言已反转并全部通过
- [ ] `npm run verify:all` 退出码 0（含 312+ 单元测试、38 E2E）
- [ ] 双主题截图：`PathEditDialog`、`AnalyzeDialog`、`ProfileDialog`、`EnvBackupDialog` 在浅色/深色下均无透明底、无低对比度文字
- [ ] 现有 4 个 `data-testid`（`backup-summary` / `backup-removed-names` / `backup-outcome` / `env-var-table`）全部保留

## 不在本版范围

- **不改对话框内部结构**（三段式布局、标签页重排等）。
- **不改任何交互行为**（快捷键、焦点陷阱、关闭确认）。
- **不改主界面**（5.1.7 已完成）。
- **不新增动画**。
- **不处理** `--app-row-even` / `--row-h` / `--app-surface-raised` 之外的历史遗留 token（它们已在 5.1.7 注释标注为后续波次）。

## 风险与未覆盖项

| 风险                                                | 缓解                                                                                                                   |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| 删除三个别名后，若有遗漏引用会导致对话框透明底      | 删除前跑 `grep`；验收标准第 1 条强制校验；E2E + 截图双重确认                                                           |
| 反转测试断言时误删无关用例                          | 只改 `design-tokens.test.ts` 的三条与 `button.test.tsx` 的一条，其余不动                                               |
| **Modal 重写改变 DOM 类名，打断依赖类名的既有 E2E** | `crud-undo.spec.ts:13` 与 `keyboard.spec.ts:13` 用 `.fixed.inset-0 input` 定位；Task 1 同步改为 `.modal-overlay input` |
| 字号 14px→13px 后个别文案换行                       | 截图逐对话框确认；必要时对特定元素保留 `text-sm`                                                                       |
| 遮罩加深后深色主题下背景过黑                        | 截图确认；0.65 可回调                                                                                                  |
| **未覆盖**：150%/200% 缩放下的对话框布局            | 与 5.1.7 同样登记为后续项                                                                                              |
