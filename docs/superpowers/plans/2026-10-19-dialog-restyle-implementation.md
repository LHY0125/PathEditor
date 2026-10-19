# 对话框重皮与过渡别名清理（Wave 4.1）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `src/components/dialogs/` 下 14 个对话框与 `Modal` 外壳迁移到 5.1.7 的设计系统，然后删除全部过渡设施（三个 CSS 别名、`.tab-active` 遗留规则、`btnClass`/`btnStyle` 废弃导出）。

**Architecture:** 先建立基础设施（新增 3 个 token、重写 `Modal` 外壳、新增 `.field-input` / `.dialog-list-item` / `.btn-block` 三个工具类），再按「简单表单 → 分析/配置 → 备份恢复」三批迁移组件，最后统一删除旧设施并反转测试断言。全程纯视觉，零 Rust / IPC / store / 业务逻辑改动。

**Tech Stack:** React 19、TypeScript strict、Tailwind 4、Vitest、Playwright

**Spec:** `docs/superpowers/specs/2026-10-19-dialog-restyle-design.md`

## Global Constraints

- **零业务逻辑改动**：`core/` / `gui/` / `cli/` 一行不动。`src/services/backend.ts`、`src/store/`、`src/core/` 不动。
- **零新依赖**：不新增任何 `dependencies` / `devDependencies`。
- **敏感值通路不变**：`reveal_env_var` / `displayValue` / 掩码逻辑只换样式，不改何时显示。
- **权限判定不变**：`canEdit` / `canDelete` 仍只读 Rust 计算结果。
- **属性保活**：`data-testid`（`backup-summary` / `backup-removed-names` / `backup-outcome` / `env-var-table`）、`data-backup-file`、`data-hive-filter`、`data-env-var-key` 一律不得改名。
- **中文注释**：所有新增注释用中文。
- **Conventional Commits**：`feat` / `fix` / `refactor` / `style` / `test` / `chore` / `docs`。
- **质量门**：每个 Task 结束跑 `npx tsc -b && npx vitest run`；全部完成后跑 `npm run verify:all`（必须退出码 0）。
- **几何**：对话框圆角 `var(--radius-lg)`（10px）、内边距 20px；按钮用 `Button` 原语（`sm`=24px / `md`=28px）。
- **字号**：正文 13px（继承 body，不用 `text-sm`）；次要文字 12px（`text-xs`）+ `var(--app-fg-muted)`；标题 `text-base`。
- **视觉验证**：`npm run dev` 在本机卡死，**用 `vite build` + Playwright 截图**验证，不要声称看过浏览器而实际没看。

---

## 文件结构

### 新建

| 文件                               | 职责                                                             |
| ---------------------------------- | ---------------------------------------------------------------- |
| `tests/unit/dialog-tokens.test.ts` | 静态守卫：扫描 `src/components/dialogs/`，禁止旧设施与硬编码色值 |
| `e2e/tests/dialog-visual.spec.ts`  | 对话框视觉回归（Task 6）                                         |

### 修改

| 文件                                                   | 改动                                                                                                                                    |
| ------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| `src/styles/globals.css`                               | 新增 3 个 token、`.field-input`、`.dialog-list-item`、`.btn-block`、`.modal-*`；删除 3 个过渡别名、`.tab-active`、`TODO(Wave 4.1)` 注释 |
| `src/components/ui/Modal.tsx`                          | 重写：token 化遮罩/表面/圆角/内边距/阴影/最大尺寸                                                                                       |
| `src/components/ui/buttons.ts`                         | 删除 `btnClass` / `btnStyle` 导出                                                                                                       |
| `src/components/dialogs/PathEditDialog.tsx`            | 按钮 → `Button`，输入 → `.field-input`                                                                                                  |
| `src/components/dialogs/HelpDialog.tsx`                | 按钮 → `Button`，文字 → token                                                                                                           |
| `src/components/dialogs/ImportDialog.tsx`              | 按钮 → `Button` + `.btn-block`                                                                                                          |
| `src/components/dialogs/NewEnvVarDialog.tsx`           | 全部按钮/输入/错误色迁移                                                                                                                |
| `src/components/dialogs/EditEnvVarDialog.tsx`          | 同上                                                                                                                                    |
| `src/components/dialogs/AnalyzeDialog.tsx`             | 标签页 → `.tab-pill`，错误色 → token                                                                                                    |
| `src/components/dialogs/analyze/ConflictsTab.tsx`      | 状态色 → `--app-success` / `--app-danger`                                                                                               |
| `src/components/dialogs/analyze/ToolsTab.tsx`          | 输入 → `.field-input`，缺失目录底色 → `color-mix`                                                                                       |
| `src/components/dialogs/analyze/EmptyHint.tsx`         | `opacity` → `--app-fg-subtle`                                                                                                           |
| `src/components/dialogs/ProfileDialog.tsx`             | 输入/按钮迁移                                                                                                                           |
| `src/components/dialogs/profile/ProfileList.tsx`       | 选中态 → `.dialog-list-item`                                                                                                            |
| `src/components/dialogs/profile/ProfileDetail.tsx`     | 按钮/路径状态/透明度迁移                                                                                                                |
| `src/components/dialogs/env-backup/EnvBackupPanel.tsx` | `btnClass`/`btnStyle`/`.tab-active`/硬编码色全部迁移                                                                                    |
| `tests/unit/design-tokens.test.ts`                     | 反转 2 条断言，新增 3 个 token 断言                                                                                                     |
| `tests/unit/button.test.tsx`                           | 删除 `btnClass / btnStyle 仍可用` 用例                                                                                                  |

---

### Task 1: 基础设施（token + Modal 外壳 + 工具类）

**Files:**

- Modify: `src/styles/globals.css`
- Modify: `src/components/ui/Modal.tsx`
- Modify: `e2e/tests/crud-undo.spec.ts`
- Modify: `e2e/tests/keyboard.spec.ts`
- Test: `tests/unit/design-tokens.test.ts`

**Interfaces:**

- Produces: CSS 变量 `--app-success` / `--app-overlay` / `--app-dialog-shadow`；CSS 类 `.field-input` / `.dialog-list-item` / `.btn-block` / `.modal-overlay` / `.modal-surface`

- [ ] **Step 1: 扩展 `design-tokens.test.ts` 的语义变量断言**

在 `tests/unit/design-tokens.test.ts` 的「声明全部语义变量」用例中，把数组追加三项：

```ts
      '--app-danger',
      '--app-warning',
      '--app-disabled-fg',
      '--app-success',
      '--app-overlay',
      '--app-dialog-shadow',
```

并新增一个用例：

```ts
it('声明对话框专用工具类', () => {
  for (const cls of [
    '.field-input',
    '.dialog-list-item',
    '.btn-block',
    '.modal-overlay',
    '.modal-surface',
  ]) {
    expect(css.includes(cls), `缺少 ${cls}`).toBe(true);
  }
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `npx vitest run tests/unit/design-tokens.test.ts`
Expected: FAIL —— 「缺少 --app-success」与「缺少 .field-input」

- [ ] **Step 3: 在 `globals.css` 两套主题中新增 token**

在浅色 `:root` 块的 `--app-disabled-fg: #a1a1aa;` 之后插入：

```css
--app-success: #16a34a;
--app-overlay: rgb(0 0 0 / 40%);
--app-dialog-shadow: 0 24px 64px rgb(0 0 0 / 16%), 0 4px 12px rgb(0 0 0 / 8%);
```

在深色 `:root.dark` 块的 `--app-disabled-fg: #63636e;` 之后插入：

```css
--app-success: #4ade80;
--app-overlay: rgb(0 0 0 / 65%);
--app-dialog-shadow: 0 24px 64px rgb(0 0 0 / 60%), 0 0 0 1px rgb(255 255 255 / 6%);
```

- [ ] **Step 4: 在 `globals.css` 末尾追加对话框工具类**

```css
/* ── 对话框基础设施（Wave 4.1） ─────────────────────────────── */

.modal-overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--app-overlay);
}

.modal-surface {
  max-width: 90vw;
  max-height: 85vh;
  overflow: auto;
  padding: 20px;
  border: 1px solid var(--app-border);
  border-radius: var(--radius-lg);
  background: var(--app-surface-raised);
  color: var(--app-fg);
  box-shadow: var(--app-dialog-shadow);
  backdrop-filter: blur(14px);
}

/* 表单控件：对话框内 input / select 统一使用，替代内联颜色样式 */
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

.field-input::placeholder {
  color: var(--app-fg-subtle);
}

/* 对话框内列表项（备份文件、配置方案共用） */
.dialog-list-item {
  display: block;
  width: 100%;
  text-align: left;
  border-radius: var(--radius-sm);
  color: var(--app-fg);
  cursor: pointer;
}

.dialog-list-item:hover {
  background: var(--app-hover);
}

.dialog-list-item[data-selected='true'] {
  background: var(--app-row-selected);
  color: var(--app-accent-text);
}

/* 整行左对齐按钮：Button 的 BASE 是 justify-center，此处用未分层规则覆盖 */
.btn-block {
  width: 100%;
  justify-content: flex-start;
}
```

- [ ] **Step 5: 重写 `Modal.tsx`，并同步两处 E2E 选择器**

````tsx
**C2 —— 同步 E2E 选择器（必做，否则 crud-undo / keyboard 必红）**：

`Modal` 外层类名由 Tailwind 的 `fixed inset-0 …` 改为 `.modal-overlay` 后，DOM 的 `class` 属性不再含 `fixed` / `inset-0`，而下列两条用例用的是**类名选择器**：

```ts
// e2e/tests/crud-undo.spec.ts:13
await page.locator('.fixed.inset-0 input[type="text"]').fill('C:\\NewPath');
// e2e/tests/keyboard.spec.ts:13
await expect(page.locator('.fixed.inset-0 input[type="text"]')).toBeVisible();
````

两处都改为：

```ts
// e2e/tests/crud-undo.spec.ts:13
await page.locator('.modal-overlay input[type="text"]').fill('C:\\NewPath');
// e2e/tests/keyboard.spec.ts:13
await expect(page.locator('.modal-overlay input[type="text"]')).toBeVisible();
```

> Playwright 的 `.fixed.inset-0` 只匹配 `class` 属性，不读计算样式 —— 换成 CSS 类后属性消失即失配。

```tsx
import { useEffect, type ReactNode } from 'react';

interface ModalProps {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
}

/**
 * 对话框外壳。
 *
 * 视觉规格由 globals.css 的 .modal-overlay / .modal-surface 提供（Wave 4.1），
 * 组件本身只管挂载与 Escape 关闭 —— 业务内容由调用方决定。
 */
export function Modal({ open, onClose, children }: ModalProps) {
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-surface" onClick={(e) => e.stopPropagation()}>
        {children}
      </div>
    </div>
  );
}
```

- [ ] **Step 6: 运行测试确认通过**

Run: `npx tsc -b && npx vitest run tests/unit/design-tokens.test.ts`
Expected: PASS

- [ ] **Step 7: 构建并截图确认 Modal 外壳未破坏现有对话框**

```bash
npx vite build
```

用 Playwright 打开应用、点击「新建路径」（打开 `PathEditDialog`），浅色与深色各截一张图，确认：对话框有背景色、无透明底、圆角明显小于之前的 16px、遮罩可见。

- [ ] **Step 8: 提交**

```bash
git add src/styles/globals.css src/components/ui/Modal.tsx tests/unit/design-tokens.test.ts
git commit -m "feat: 对话框基础设施（token / Modal 外壳 / 表单与列表工具类）"
```

---

### Task 2: 简单表单对话框

**Files:**

- Modify: `src/components/dialogs/PathEditDialog.tsx`
- Modify: `src/components/dialogs/HelpDialog.tsx`
- Modify: `src/components/dialogs/ImportDialog.tsx`
- Modify: `src/components/dialogs/NewEnvVarDialog.tsx`
- Modify: `src/components/dialogs/EditEnvVarDialog.tsx`
- Test: 既有 `tests/unit/app-shell-env-vars.test.tsx`（回归）

**Interfaces:**

- Consumes: Task 1 的 `.field-input` / `.btn-block`；`Button`（`@/components/ui/Button`，`variant: primary|secondary|ghost|danger`，`size: sm|md`）

- [ ] **Step 1: 跑一次基线测试**

Run: `npx vitest run tests/unit/app-shell-env-vars.test.tsx tests/unit/env-var-toolbar.test.tsx`
Expected: PASS（记录基线，后续必须保持绿）

- [ ] **Step 2: 重写 `PathEditDialog.tsx`**

```tsx
import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';

interface PathEditDialogProps {
  open: boolean;
  title: string;
  initialValue: string;
  onConfirm: (value: string) => void;
  onCancel: () => void;
}

export function PathEditDialog({
  open,
  title,
  initialValue,
  onConfirm,
  onCancel,
}: PathEditDialogProps) {
  const { t } = useTranslation();
  const [value, setValue] = useState(initialValue);
  const prevOpen = useRef(open);

  useEffect(() => {
    if (open && !prevOpen.current) setValue(initialValue);
    prevOpen.current = open;
  }, [open, initialValue]);

  return (
    <Modal open={open} onClose={onCancel}>
      <h2 className="text-base font-semibold mb-4">{title}</h2>
      <label className="mb-2 block" style={{ color: 'var(--app-fg-muted)' }}>
        {t('dialog.pathLabel')}
      </label>
      <input
        type="text"
        autoFocus
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') onConfirm(value);
        }}
        className="field-input w-full min-w-[400px]"
      />
      <div className="flex justify-end gap-2 mt-4">
        <Button onClick={onCancel}>{t('dialog.cancel')}</Button>
        <Button variant="primary" onClick={() => onConfirm(value)}>
          {t('dialog.confirm')}
        </Button>
      </div>
    </Modal>
  );
}
```

- [ ] **Step 3: 重写 `HelpDialog.tsx`**

```tsx
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import packageJson from '../../../package.json';

interface HelpDialogProps {
  open: boolean;
  onClose: () => void;
}

export function HelpDialog({ open, onClose }: HelpDialogProps) {
  const { t } = useTranslation();

  return (
    <Modal open={open} onClose={onClose}>
      <h2 className="text-base font-semibold mb-4">{t('dialog.helpTitle')}</h2>
      <pre
        className="whitespace-pre-wrap font-sans leading-relaxed max-w-lg"
        style={{ color: 'var(--app-fg-muted)' }}
      >
        {t('help.content', { version: packageJson.version })}
      </pre>
      <div className="flex justify-end mt-4">
        <Button variant="primary" onClick={onClose}>
          {t('dialog.confirm')}
        </Button>
      </div>
    </Modal>
  );
}
```

- [ ] **Step 4: 重写 `ImportDialog.tsx`**

```tsx
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';

interface ImportDialogProps {
  open: boolean;
  systemCount: number;
  userCount: number;
  canWriteSystem: boolean;
  canWriteUser: boolean;
  onSelect: (target: 'system' | 'user' | 'both') => void;
  onCancel: () => void;
}

export function ImportDialog({
  open,
  systemCount,
  userCount,
  canWriteSystem,
  canWriteUser,
  onSelect,
  onCancel,
}: ImportDialogProps) {
  const { t } = useTranslation();

  return (
    <Modal open={open} onClose={onCancel}>
      <h2 className="text-base font-semibold mb-4">{t('dialog.importTarget')}</h2>
      <p className="text-xs mb-4" style={{ color: 'var(--app-fg-muted)' }}>
        {systemCount > 0 && t('dialog.importSystemCount', { count: systemCount })}
        {systemCount > 0 && userCount > 0 && ' | '}
        {userCount > 0 && t('dialog.importUserCount', { count: userCount })}
      </p>
      <div className="flex flex-col gap-2">
        {systemCount > 0 && (
          <Button
            className="btn-block"
            disabled={!canWriteSystem}
            onClick={() => onSelect('system')}
          >
            {t('dialog.importSystem')}
          </Button>
        )}
        {userCount > 0 && (
          <Button className="btn-block" disabled={!canWriteUser} onClick={() => onSelect('user')}>
            {t('dialog.importUser')}
          </Button>
        )}
        {systemCount > 0 && userCount > 0 && (
          <Button
            className="btn-block"
            disabled={!canWriteSystem || !canWriteUser}
            onClick={() => onSelect('both')}
          >
            {t('dialog.importBoth')}
          </Button>
        )}
        <Button className="mt-2" onClick={onCancel}>
          {t('dialog.cancel')}
        </Button>
      </div>
    </Modal>
  );
}
```

- [ ] **Step 5: 重写 `NewEnvVarDialog.tsx`**

```tsx
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import { useEnvStore } from '@/store/env-store';
import { validateVarName, type EnvHive, type EnvValueKind } from '@/core/env-var';

interface NewEnvVarDialogProps {
  /** 系统 hive 不可写时禁用「系统」来源（Spec 权限矩阵：非管理员不得新建系统变量）。 */
  canWriteSystem: boolean;
  /** 用户 hive 不可写时禁用「用户」来源（F-08：此前只门禁系统侧）。 */
  canWriteUser: boolean;
  onCancel: () => void;
  /** 返回是否创建成功；失败时弹窗保留并显示 store 的错误消息。 */
  onConfirm: (hive: EnvHive, name: string, value: string, kind: EnvValueKind) => Promise<boolean>;
}

const labelClass = 'w-20';
const labelStyle = { color: 'var(--app-fg-muted)' };

export function NewEnvVarDialog({
  canWriteSystem,
  canWriteUser,
  onCancel,
  onConfirm,
}: NewEnvVarDialogProps) {
  const { t } = useTranslation();
  const [hive, setHive] = useState<EnvHive>(
    canWriteUser ? 'user' : canWriteSystem ? 'system' : 'user',
  );
  const [name, setName] = useState('');
  const [value, setValue] = useState('');
  const [kind, setKind] = useState<EnvValueKind>('string');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const submit = async () => {
    const invalid = validateVarName(name);
    if (invalid !== null) {
      setError(invalid);
      return;
    }
    setError(null);
    setSubmitting(true);
    // Rust 是重复变量与权限的最终裁判：失败时保留弹窗，把 store 的错误透出。
    const ok = await onConfirm(hive, name, value, kind);
    setSubmitting(false);
    if (!ok) setError(useEnvStore.getState().statusMessage);
  };

  return (
    <Modal open onClose={onCancel}>
      <div className="flex flex-col gap-2">
        <h2 className="text-base font-semibold mb-1">{t('envVar.newVarTitle')}</h2>

        <label className="flex items-center gap-2">
          <span className={labelClass} style={labelStyle}>
            {t('envVar.source')}
          </span>
          <select
            value={hive}
            onChange={(e) => setHive(e.target.value as EnvHive)}
            className="field-input"
          >
            <option value="user" disabled={!canWriteUser}>
              {t('merge.user')}
            </option>
            <option value="system" disabled={!canWriteSystem}>
              {t('merge.system')}
            </option>
          </select>
        </label>

        <label className="flex items-center gap-2">
          <span className={labelClass} style={labelStyle}>
            {t('envVar.newVarName')}
          </span>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="field-input flex-1"
          />
        </label>

        <label className="flex items-center gap-2">
          <span className={labelClass} style={labelStyle}>
            {t('envVar.newVarValue')}
          </span>
          <input
            value={value}
            onChange={(e) => setValue(e.target.value)}
            className="field-input flex-1"
          />
        </label>

        <label className="flex items-center gap-2">
          <span className={labelClass} style={labelStyle}>
            {t('envVar.newVarKind')}
          </span>
          <select
            value={kind}
            onChange={(e) => setKind(e.target.value as EnvValueKind)}
            className="field-input"
          >
            <option value="string">String</option>
            <option value="expandString">ExpandString</option>
          </select>
        </label>

        {error && (
          <p className="text-xs" style={{ color: 'var(--app-danger)' }}>
            {error}
          </p>
        )}

        <div className="flex justify-end gap-2 mt-1">
          <Button onClick={onCancel}>{t('button.cancel')}</Button>
          <Button variant="primary" disabled={submitting} onClick={() => void submit()}>
            {t('button.save')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
```

- [ ] **Step 6: 重写 `EditEnvVarDialog.tsx`**

保持全部逻辑不变，只替换渲染层。完整文件：

```tsx
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import { useEnvStore } from '@/store/env-store';
import { findMetaByKey } from '@/core/env-var';

interface EditEnvVarDialogProps {
  /**
   * 稳定键（`${hive}:${name}`）。meta 不随 prop 固化 —— 每次提交前由
   * 调用方从最新快照派生，冲突刷新后重试自动携带新 revision（F-02）。
   */
  varKey: string;
  onCancel: () => void;
  /**
   * 返回是否保存成功；失败时弹窗保留并显示 store 的错误消息。
   * readRevision 是弹窗读取原值时的 revision，供保存点陈旧判定（F-01）。
   */
  onConfirm: (value: string, readRevision: string | null) => Promise<boolean>;
}

const labelClass = 'w-20';
const labelStyle = { color: 'var(--app-fg-muted)' };

/**
 * 编辑环境变量值的弹窗。
 *
 * - 打开时经 `fetchFullValue` 取**完整原值**：preview 是截断/净化后的展示
 *   摘要，严禁作为编辑数据源，否则原样保存也会损坏长值/含换行的值（F-01）。
 * - 输入实时同步进 store 草稿：真实输入参与关窗确认，X/Alt+F4 不会静默丢弃（F-04）。
 * - 草稿策略统一为「镜像弹窗输入」：冲突后保留（输入不丢、revision 已随
 *   快照刷新），成功或取消时清除（c2）。
 */
export function EditEnvVarDialog({ varKey, onCancel, onConfirm }: EditEnvVarDialogProps) {
  const { t } = useTranslation();
  const setDraftByKey = useEnvStore((s) => s.setDraftByKey);
  const clearDraftByKey = useEnvStore((s) => s.clearDraftByKey);
  // 每次提交前由调用方从最新快照派生 meta；此处派生仅用于展示与禁用态。
  const meta = useEnvStore((s) => (s.snapshot ? findMetaByKey(s.snapshot, varKey) : null));
  const gone = meta === null;
  const [value, setValue] = useState<string | null>(null); // null = 加载完整原值中
  // 读取原值时的 revision（F-01）：提交时与最新快照比对，判断值是否已陈旧。
  const [readRevision, setReadRevision] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [name] = useState(
    () =>
      findMetaByKey(
        useEnvStore.getState().snapshot ?? { system: [], user: [], capturedAt: 0 },
        varKey,
      )?.name ?? varKey,
  );

  // 打开时取完整原值；仅在打开/换键时取一次。
  useEffect(() => {
    const store = useEnvStore.getState();
    const current = store.snapshot ? findMetaByKey(store.snapshot, varKey) : null;
    if (!current) return;
    let cancelled = false;
    store
      .fetchFullValue(current.hive, current.name)
      .then((full) => {
        if (cancelled) return;
        setValue(full.value);
        setReadRevision(full.revision);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(String(err));
          setValue('');
        }
      });
    return () => {
      cancelled = true;
    };
  }, [varKey]);

  const submit = async () => {
    if (value === null) return;
    setSubmitting(true);
    // F-01：提交前若快照 revision 已变（外部修改），先重取最新完整值，
    // 本次不保存；用户看到新值后可再次确认。
    const store = useEnvStore.getState();
    const current = store.snapshot ? findMetaByKey(store.snapshot, varKey) : null;
    if (current && readRevision !== null && current.revision !== readRevision) {
      // 重取是异步 IPC，可能被拒绝（变量被外部删除/类型转 Unsupported）；
      // 必须容错，否则 rejection 会让 setSubmitting 永不执行，按钮永久禁用。
      try {
        const fresh = await store.fetchFullValue(current.hive, current.name);
        setValue(fresh.value);
        setReadRevision(fresh.revision);
        setDraftByKey(varKey, fresh.value);
        setError(t('envVar.staleReloaded'));
      } catch (err: unknown) {
        setError(String(err));
      }
      setSubmitting(false);
      return;
    }
    const ok = await onConfirm(value, readRevision);
    setSubmitting(false);
    if (!ok) setError(useEnvStore.getState().statusMessage);
    // 失败不清草稿（c2 统一策略）：草稿镜像输入，供重试与关窗确认。
  };

  return (
    <Modal open onClose={onCancel}>
      <div className="flex flex-col gap-2">
        <h2 className="text-base font-semibold mb-1">{t('envVar.editVarTitle')}</h2>

        <label className="flex items-center gap-2">
          <span className={labelClass} style={labelStyle}>
            {t('envVar.newVarName')}
          </span>
          <span className="flex-1 font-mono text-xs" title={name}>
            {name}
          </span>
        </label>

        <label className="flex items-center gap-2">
          <span className={labelClass} style={labelStyle}>
            {t('envVar.newVarValue')}
          </span>
          <input
            autoFocus
            disabled={value === null}
            value={value ?? ''}
            placeholder={value === null ? t('status.loading') : ''}
            onChange={(e) => {
              setValue(e.target.value);
              // 实时镜像进草稿：真实输入纳入关窗确认（F-04）
              setDraftByKey(varKey, e.target.value);
            }}
            className="field-input flex-1 font-mono"
          />
        </label>

        {gone && (
          <p className="text-xs" style={{ color: 'var(--app-warning)' }}>
            {t('envVar.varGone')}
          </p>
        )}
        {error && (
          <p className="text-xs" style={{ color: 'var(--app-danger)' }}>
            {error}
          </p>
        )}

        <div className="flex justify-end gap-2 mt-1">
          <Button
            onClick={() => {
              clearDraftByKey(varKey);
              onCancel();
            }}
          >
            {t('button.cancel')}
          </Button>
          <Button
            variant="primary"
            disabled={submitting || value === null || meta === null}
            onClick={() => void submit()}
          >
            {t('button.save')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
```

- [ ] **Step 7: 运行类型检查与全部相关测试**

Run: `npx tsc -b && npx vitest run`
Expected: PASS（313 个测试全绿，与基线一致）

- [ ] **Step 8: 提交**

```bash
git add src/components/dialogs/PathEditDialog.tsx src/components/dialogs/HelpDialog.tsx src/components/dialogs/ImportDialog.tsx src/components/dialogs/NewEnvVarDialog.tsx src/components/dialogs/EditEnvVarDialog.tsx
git commit -m "style: 表单类对话框迁移至设计系统"
```

---

### Task 3: 分析与配置对话框

**Files:**

- Modify: `src/components/dialogs/AnalyzeDialog.tsx`
- Modify: `src/components/dialogs/analyze/ConflictsTab.tsx`
- Modify: `src/components/dialogs/analyze/ToolsTab.tsx`
- Modify: `src/components/dialogs/analyze/EmptyHint.tsx`
- Modify: `src/components/dialogs/ProfileDialog.tsx`
- Modify: `src/components/dialogs/profile/ProfileList.tsx`
- Modify: `src/components/dialogs/profile/ProfileDetail.tsx`

**Interfaces:**

- Consumes: Task 1 的 `.field-input` / `.dialog-list-item`；`Button`

- [ ] **Step 1: 重写 `EmptyHint.tsx`**

```tsx
export function EmptyHint({ text }: { text: string }) {
  return (
    <div className="text-center py-12 text-xs" style={{ color: 'var(--app-fg-subtle)' }}>
      {text}
    </div>
  );
}
```

- [ ] **Step 2: 重写 `ConflictsTab.tsx`**

```tsx
import { useTranslation } from 'react-i18next';
import type { ConflictEntry } from '@/services/backend';
import { EmptyHint } from './EmptyHint';

export function ConflictsTab({ conflicts }: { conflicts: ConflictEntry[] }) {
  const { t } = useTranslation();
  if (conflicts.length === 0) {
    return <EmptyHint text={t('analyze.noConflicts')} />;
  }

  return (
    <div>
      <p className="text-xs mb-2" style={{ color: 'var(--app-fg-muted)' }}>
        {t('analyze.conflictCount', { count: conflicts.length })}
      </p>
      <table className="w-full border-collapse">
        <thead>
          <tr style={{ borderBottom: '1px solid var(--app-border)' }}>
            <th className="text-left py-1.5 pr-3 font-medium text-xs">EXE</th>
            <th className="text-left py-1.5 font-medium text-xs">{t('analyze.priority')}</th>
          </tr>
        </thead>
        <tbody>
          {conflicts.map((conflict) => (
            <tr key={conflict.name} style={{ borderBottom: '1px solid var(--app-border)' }}>
              <td className="py-1.5 pr-3 font-mono text-xs">{conflict.name}</td>
              <td className="py-1.5">
                {conflict.locations.map((location, index) => (
                  <div
                    key={`${location.dir}-${location.priority}`}
                    className="text-xs py-0.5"
                    style={{
                      color: index === 0 ? 'var(--app-success)' : 'var(--app-danger)',
                    }}
                  >
                    {index === 0 ? '✓' : '✗'} {location.dir}
                    {index > 0 && (
                      <span className="ml-1" style={{ color: 'var(--app-fg-subtle)' }}>
                        ({t('analyze.shadowed')})
                      </span>
                    )}
                  </div>
                ))}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
```

- [ ] **Step 3: 重写 `ToolsTab.tsx`**

```tsx
import { useTranslation } from 'react-i18next';
import type { ToolGroup } from '@/services/backend';
import { EmptyHint } from './EmptyHint';

interface ToolsTabProps {
  groups: ToolGroup[];
  query: string;
  onQueryChange: (query: string) => void;
}

export function ToolsTab({ groups, query, onQueryChange }: ToolsTabProps) {
  const { t } = useTranslation();

  return (
    <div>
      <input
        type="text"
        value={query}
        onChange={(event) => onQueryChange(event.target.value)}
        placeholder={t('analyze.searchPlaceholder')}
        className="field-input w-full mb-3"
      />
      {groups.length === 0 ? (
        <EmptyHint text={t('analyze.noTools')} />
      ) : (
        groups.map((group) => (
          <div key={group.dir} className="mb-3">
            <div
              className="text-xs font-mono py-1 px-2 rounded"
              style={{
                backgroundColor: group.exists
                  ? 'transparent'
                  : 'color-mix(in srgb, var(--app-danger) 10%, transparent)',
                color: group.exists ? 'var(--app-fg)' : 'var(--app-danger)',
              }}
            >
              {group.dir} {!group.exists && t('analyze.notExists')}
            </div>
            <div className="flex flex-wrap gap-1 mt-1 ml-2">
              {group.exes.map((exe) => (
                <span
                  key={exe}
                  className="text-xs font-mono px-1.5 py-0.5 rounded"
                  style={{ backgroundColor: 'var(--app-surface)', color: 'var(--app-fg)' }}
                >
                  {exe}
                </span>
              ))}
            </div>
          </div>
        ))
      )}
    </div>
  );
}
```

- [ ] **Step 4: 重写 `AnalyzeDialog.tsx`**

```tsx
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { ConflictsTab } from './analyze/ConflictsTab';
import { ToolsTab } from './analyze/ToolsTab';
import { useAnalyzeData } from './analyze/use-analyze-data';

type TabType = 'conflicts' | 'tools';

interface Props {
  open: boolean;
  onClose: () => void;
}

export function AnalyzeDialog({ open, onClose }: Props) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<TabType>('conflicts');
  const { loading, error, conflicts, filteredTools, searchQuery, setSearchQuery } =
    useAnalyzeData(open);

  return (
    <Modal open={open} onClose={onClose}>
      <div className="flex flex-col" style={{ width: 680, maxWidth: '100%', maxHeight: '75vh' }}>
        <div
          className="flex items-center justify-between px-1 py-2"
          style={{ borderBottom: '1px solid var(--app-border)' }}
        >
          <h2 className="text-base font-semibold">{t('analyze.title')}</h2>
          <div className="flex gap-1">
            {(['conflicts', 'tools'] as TabType[]).map((tabId) => (
              <button
                key={tabId}
                type="button"
                data-active={tab === tabId}
                className="tab-pill text-xs"
                onClick={() => setTab(tabId)}
              >
                {tabId === 'conflicts' ? t('analyze.conflicts') : t('analyze.tools')}
              </button>
            ))}
          </div>
        </div>

        <div className="flex-1 overflow-auto p-3">
          {loading ? (
            <div
              className="flex items-center justify-center py-12 text-xs"
              style={{ color: 'var(--app-fg-muted)' }}
            >
              {t('analyze.scanning')}
            </div>
          ) : error ? (
            <div className="text-center py-12 text-xs" style={{ color: 'var(--app-danger)' }}>
              {t('analyze.error')}: {error}
            </div>
          ) : tab === 'conflicts' ? (
            <ConflictsTab conflicts={conflicts} />
          ) : (
            <ToolsTab groups={filteredTools} query={searchQuery} onQueryChange={setSearchQuery} />
          )}
        </div>
      </div>
    </Modal>
  );
}
```

- [ ] **Step 5: 重写 `ProfileList.tsx`**

```tsx
import { useTranslation } from 'react-i18next';
import type { ProfileMeta } from '@/services/backend';

interface ProfileListProps {
  profiles: ProfileMeta[];
  selected: string | null;
  onSelect: (name: string) => void;
}

export function ProfileList({ profiles, selected, onSelect }: ProfileListProps) {
  const { t } = useTranslation();

  return (
    <div className="w-48 border-r overflow-auto p-2" style={{ borderColor: 'var(--app-border)' }}>
      {profiles.length === 0 ? (
        <div className="text-xs text-center py-6" style={{ color: 'var(--app-fg-subtle)' }}>
          {t('profile.noProfiles')}
        </div>
      ) : (
        profiles.map((profile) => (
          <div
            key={profile.name}
            onClick={() => onSelect(profile.name)}
            data-selected={selected === profile.name}
            className="dialog-list-item px-2 py-1.5 text-xs mb-0.5"
          >
            {profile.name}
          </div>
        ))
      )}
    </div>
  );
}
```

- [ ] **Step 6: 重写 `ProfileDetail.tsx`**

```tsx
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/Button';
import type { PathEntry } from '@/core/path-entry';
import type { ProfileData } from '@/services/backend';

interface ProfileDetailProps {
  data: ProfileData | null;
  hasProfiles: boolean;
  renameOpen: boolean;
  renameValue: string;
  onRenameOpen: () => void;
  onRenameValueChange: (value: string) => void;
  onApply: () => void;
  onRename: () => void;
  onDelete: (name: string) => void;
}

export function ProfileDetail({
  data,
  hasProfiles,
  renameOpen,
  renameValue,
  onRenameOpen,
  onRenameValueChange,
  onApply,
  onRename,
  onDelete,
}: ProfileDetailProps) {
  const { t } = useTranslation();

  if (!data) {
    return (
      <div className="flex-1 p-3 overflow-auto">
        <div className="text-center py-10 text-xs" style={{ color: 'var(--app-fg-subtle)' }}>
          {hasProfiles ? t('profile.selectProfile') : t('profile.noProfiles')}
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 p-3 overflow-auto">
      <div className="flex items-center gap-2 mb-3">
        <span className="font-semibold text-xs">{data.name}</span>
        <span className="text-xs" style={{ color: 'var(--app-fg-subtle)' }}>
          {data.modified}
        </span>
      </div>

      <div className="flex gap-1.5 mb-3">
        <Button size="sm" variant="primary" onClick={onApply}>
          {t('profile.apply')}
        </Button>
        <Button size="sm" onClick={onRenameOpen}>
          {t('profile.rename')}
        </Button>
        <Button size="sm" variant="danger" onClick={() => onDelete(data.name)}>
          {t('profile.delete')}
        </Button>
      </div>

      {renameOpen && (
        <div className="flex gap-2 mb-2">
          <input
            type="text"
            value={renameValue}
            onChange={(event) => onRenameValueChange(event.target.value)}
            className="field-input text-xs"
          />
          <Button size="sm" variant="primary" onClick={onRename}>
            {t('button.save')}
          </Button>
        </div>
      )}

      <PathSection title={`${t('merge.system')} PATH (${data.sys.length})`} paths={data.sys} />
      <PathSection title={`${t('merge.user')} PATH (${data.user.length})`} paths={data.user} />
    </div>
  );
}

function PathSection({ title, paths }: { title: string; paths: PathEntry[] }) {
  const { t } = useTranslation();
  return (
    <div className="mb-2">
      <div className="text-xs font-medium mb-1" style={{ color: 'var(--app-fg-muted)' }}>
        {title}
      </div>
      {paths.length === 0 ? (
        <div className="text-xs" style={{ color: 'var(--app-fg-subtle)' }}>
          {t('profile.empty')}
        </div>
      ) : (
        <div className="space-y-0.5 max-h-48 overflow-auto">
          {paths.map((entry) => (
            <div
              key={entry.path}
              className="text-xs font-mono px-2 py-0.5 rounded flex items-center gap-1.5"
              style={{
                backgroundColor: 'var(--app-surface)',
                color: entry.enabled ? 'var(--app-fg)' : 'var(--app-danger)',
                textDecoration: entry.enabled ? 'none' : 'line-through',
              }}
            >
              <span
                style={{
                  color: entry.enabled ? 'var(--app-success)' : 'var(--app-danger)',
                  fontSize: 10,
                }}
              >
                {entry.enabled ? '●' : '○'}
              </span>
              {entry.path}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 7: 重写 `ProfileDialog.tsx`**

```tsx
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import { ProfileDetail } from './profile/ProfileDetail';
import { ProfileList } from './profile/ProfileList';
import { useProfiles } from './profile/use-profiles';

interface Props {
  open: boolean;
  onClose: () => void;
}

export function ProfileDialog({ open, onClose }: Props) {
  const { t } = useTranslation();
  const profiles = useProfiles(open, onClose);

  return (
    <Modal open={open} onClose={onClose}>
      <div className="flex flex-col" style={{ width: 680, maxWidth: '100%', maxHeight: '75vh' }}>
        <div
          className="flex items-center justify-between px-1 py-2"
          style={{ borderBottom: '1px solid var(--app-border)' }}
        >
          <h2 className="text-base font-semibold">{t('profile.title')}</h2>
          <div className="flex gap-2 items-center">
            <input
              type="text"
              value={profiles.newName}
              onChange={(event) => profiles.setNewName(event.target.value)}
              placeholder={t('profile.namePlaceholder')}
              className="field-input w-44"
            />
            <Button
              variant="primary"
              disabled={profiles.saving || !profiles.newName.trim()}
              onClick={profiles.handleSave}
            >
              {t('profile.save')}
            </Button>
            <Button
              variant="ghost"
              onClick={onClose}
              title={t('button.close')}
              aria-label={t('button.close')}
            >
              ✕
            </Button>
          </div>
        </div>

        <div className="flex flex-1 overflow-hidden">
          <ProfileList
            profiles={profiles.profiles}
            selected={profiles.selected}
            onSelect={profiles.handleLoad}
          />
          <ProfileDetail
            data={profiles.selectedData}
            hasProfiles={profiles.profiles.length > 0}
            renameOpen={profiles.renameOpen}
            renameValue={profiles.renameValue}
            onRenameOpen={() => {
              if (!profiles.selectedData) return;
              profiles.setRenameOpen(true);
              profiles.setRenameValue(profiles.selectedData.name);
            }}
            onRenameValueChange={profiles.setRenameValue}
            onApply={profiles.handleApply}
            onRename={profiles.handleRename}
            onDelete={profiles.handleDelete}
          />
        </div>
      </div>
    </Modal>
  );
}
```

- [ ] **Step 8: 运行类型检查与测试**

Run: `npx tsc -b && npx vitest run`
Expected: PASS

- [ ] **Step 9: 提交**

```bash
git add src/components/dialogs/AnalyzeDialog.tsx src/components/dialogs/analyze/ src/components/dialogs/ProfileDialog.tsx src/components/dialogs/profile/
git commit -m "style: 分析与配置对话框迁移至设计系统"
```

---

### Task 4: 备份与恢复面板

**Files:**

- Modify: `src/components/dialogs/env-backup/EnvBackupPanel.tsx`
- Test: `tests/unit/env-backup-panel.test.tsx`（回归）

**Interfaces:**

- Consumes: Task 1 的 `.field-input` / `.dialog-list-item` / `Button`

- [ ] **Step 1: 跑基线测试**

Run: `npx vitest run tests/unit/env-backup-panel.test.tsx tests/unit/env-backup-dialog.test.tsx`
Expected: PASS

- [ ] **Step 2: 重写 `EnvBackupPanel.tsx` 的渲染层**

保持全部 controller 调用与文案不变，仅替换样式。完整文件：

```tsx
import { useTranslation } from 'react-i18next';
import { formatSize } from '@/core/env-backup';
import { Button } from '@/components/ui/Button';
import type { EnvBackupController } from './use-env-backup';

interface EnvBackupPanelProps {
  controller: EnvBackupController;
}

const sectionTitleClass = 'text-xs font-semibold uppercase tracking-wide';

/**
 * 备份与恢复对话框主体。
 *
 * 差异摘要**如实展示四个计数，不做任何换算**：`modified` 恒为 0（Rust 侧
 * 「值变了」≡「备份过期」，一律计 conflict），强制覆盖低报改动量 —— 两个
 * 取舍都在界面上写明，而不是悄悄修正数字。
 */
export function EnvBackupPanel({ controller }: EnvBackupPanelProps) {
  const { t } = useTranslation();
  const { backups, busy, creating, selected, summary, preview, outcome, createdPath, error } =
    controller;

  return (
    <div
      className="flex flex-col gap-3"
      style={{ width: 720, maxWidth: '100%', maxHeight: '78vh' }}
    >
      <div className="flex items-center justify-between">
        <h2 className="text-base font-semibold">{t('envBackup.title')}</h2>
        <Button disabled={creating} onClick={() => void controller.createBackup()}>
          {creating ? t('envBackup.creating') : t('envBackup.create')}
        </Button>
      </div>

      {createdPath && (
        <p className="text-xs break-all" style={{ color: 'var(--app-success)' }}>
          {t('envBackup.created', { path: createdPath })}
        </p>
      )}

      <div className="flex gap-3 overflow-hidden" style={{ minHeight: 260 }}>
        {/* 左：备份文件列表（Rust 侧已按时间倒序，前端不再排序） */}
        <div
          className="w-72 flex flex-col rounded border overflow-auto"
          style={{ borderColor: 'var(--app-border)', backgroundColor: 'var(--app-surface)' }}
        >
          <div
            className={sectionTitleClass + ' px-3 py-2'}
            style={{ color: 'var(--app-fg-subtle)' }}
          >
            {t('envBackup.title')}
          </div>
          {backups === null ? (
            <p className="px-3 py-2 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('envBackup.loading')}
            </p>
          ) : backups.length === 0 ? (
            <p className="px-3 py-2 text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('envBackup.empty')}
            </p>
          ) : (
            backups.map((info) => (
              <button
                key={info.file}
                type="button"
                data-backup-file={info.file}
                data-selected={selected?.file === info.file}
                className="dialog-list-item px-3 py-2 text-xs"
                style={{ borderBottom: '1px solid var(--app-border)' }}
                disabled={busy}
                onClick={() => void controller.select(info)}
              >
                <div className="font-mono truncate" title={info.file}>
                  {info.file}
                </div>
                <div style={{ color: 'var(--app-fg-muted)' }}>
                  {info.timestamp} · {formatSize(info.sizeBytes)}
                </div>
              </button>
            ))
          )}
        </div>

        {/* 右：差异预览与恢复 */}
        <div className="flex-1 flex flex-col gap-2 overflow-auto">
          {selected === null ? (
            <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('envBackup.selectHint')}
            </p>
          ) : busy && preview === null ? (
            <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('envBackup.previewing')}
            </p>
          ) : preview === null ? (
            <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
              {t('envBackup.noPreview')}
            </p>
          ) : (
            <>
              <div className={sectionTitleClass} style={{ color: 'var(--app-fg-subtle)' }}>
                {t('envBackup.preview')}
              </div>
              <p data-testid="backup-summary" className="text-xs">
                {summary?.text}
              </p>
              <div className="grid grid-cols-4 gap-2 text-xs">
                <Count label={t('envBackup.addedLabel')} value={preview.added} />
                <Count label={t('envBackup.modifiedLabel')} value={preview.modified} />
                <Count label={t('envBackup.removedLabel')} value={preview.removed} />
                <Count label={t('envBackup.conflictsLabel')} value={preview.conflicts} />
              </div>
              {preview.removed > 0 && (
                <div
                  data-testid="backup-removed-names"
                  className="rounded border p-2 text-xs"
                  style={{ borderColor: 'var(--app-danger)' }}
                >
                  <p className="font-semibold" style={{ color: 'var(--app-danger)' }}>
                    {t('envBackup.deleteWarning')}
                  </p>
                  <ul className="font-mono mt-1">
                    {(summary?.removedNames ?? []).map((name) => (
                      <li key={name}>• {name}</li>
                    ))}
                  </ul>
                  <p className="mt-1" style={{ color: 'var(--app-fg-muted)' }}>
                    {t('envBackup.deleteWarningHint')}
                  </p>
                </div>
              )}
              {preview.modified === 0 && (
                <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
                  {t('envBackup.modifiedAlwaysZero')}
                </p>
              )}
              {preview.conflicts > 0 && (
                <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
                  {t('envBackup.forceUnderreport')}
                </p>
              )}
              <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
                {t('envBackup.manualFallback')}
              </p>
              <p className="text-xs font-mono" style={{ color: 'var(--app-fg-muted)' }}>
                patheditor backup
              </p>
              <p className="text-xs font-mono" style={{ color: 'var(--app-fg-muted)' }}>
                patheditor env backup
              </p>
              {!summary?.hasChanges && !summary?.hasConflicts && (
                <p className="text-xs" style={{ color: 'var(--app-fg-muted)' }}>
                  {t('envBackup.noDiffHint')}
                </p>
              )}
              <Button variant="primary" disabled={busy} onClick={() => void controller.restore()}>
                {busy ? t('envBackup.restoring') : t('envBackup.restore')}
              </Button>
            </>
          )}

          {outcome !== null && (
            <div data-testid="backup-outcome" className="rounded border p-2 text-xs">
              <p>
                {outcome.failures.length === 0
                  ? t('envBackup.restoreDoneAll', { applied: outcome.applied })
                  : t('envBackup.restoreDone', {
                      applied: outcome.applied,
                      failed: outcome.failures.length,
                    })}
              </p>
              {outcome.failures.length > 0 && (
                <>
                  <p className="mt-1" style={{ color: 'var(--app-danger)' }}>
                    {t('envBackup.failedTitle')}
                  </p>
                  <ul className="font-mono">
                    {outcome.failures.map((failure) => (
                      <li key={failure}>{failure}</li>
                    ))}
                  </ul>
                </>
              )}
            </div>
          )}

          {error && (
            <p className="text-xs" style={{ color: 'var(--app-danger)' }}>
              {error}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

/** 单个差异计数。 */
function Count({ label, value }: { label: string; value: number }) {
  return (
    <div
      className="rounded border px-2 py-1"
      style={{ borderColor: 'var(--app-border)', backgroundColor: 'var(--app-surface)' }}
    >
      <div style={{ color: 'var(--app-fg-muted)' }}>{label}</div>
      <div className="font-semibold">{value}</div>
    </div>
  );
}
```

- [ ] **Step 3: 运行测试**

Run: `npx tsc -b && npx vitest run`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add src/components/dialogs/env-backup/EnvBackupPanel.tsx
git commit -m "style: 备份恢复面板迁移至设计系统"
```

---

### Task 5: 删除过渡设施 + 反转测试断言

**Files:**

- Modify: `src/styles/globals.css`
- Modify: `src/components/ui/buttons.ts`
- Modify: `tests/unit/design-tokens.test.ts`
- Modify: `tests/unit/button.test.tsx`
- Create: `tests/unit/dialog-tokens.test.ts`

- [ ] **Step 1: 先写守卫测试（此时应全部通过）**

创建 `tests/unit/dialog-tokens.test.ts`：

```ts
import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';

const ROOT = resolve(__dirname, '../../src/components/dialogs');

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) return walk(full);
    return full.endsWith('.tsx') ? [full] : [];
  });
}

const files = walk(ROOT);
/**
 * 剥离注释后再匹配 —— 与 design-tokens.test.ts 的 B4 口径保持一致。
 * 不剥离时，注释里写「原为 #3b82f6」这类说明会造成假红。
 */
const read = (f: string) =>
  readFileSync(f, 'utf8')
    .replace(//*[sS]*?*//g, '')
    .replace(///.*$/gm, '');

const LEGACY_TOKENS = ['--app-list-bg', '--app-list-alt', '--app-select-row'];
const LEGACY_APIS = ['btnClass', 'btnStyle', 'tab-active'];
const HARDCODED_COLOR = /#[0-9a-fA-F]{6}\b|rgba?\(/;

describe('对话框不依赖旧样式设施（Wave 4.1 验收）', () => {
  it('扫描到全部 14 个对话框文件', () => {
    expect(files.length).toBeGreaterThanOrEqual(14);
  });

  it.each([...LEGACY_TOKENS, ...LEGACY_APIS])('不使用 %s', (token) => {
    const offenders = files.filter((f) => read(f).includes(token));
    expect(offenders, `${token} 仍出现在: ${offenders.join(', ')}`).toEqual([]);
  });

  it('不含硬编码色值', () => {
    const offenders = files.filter((f) => HARDCODED_COLOR.test(read(f)));
    expect(offenders, `硬编码色值仍出现在: ${offenders.join(', ')}`).toEqual([]);
  });
});
```

Run: `npx vitest run tests/unit/dialog-tokens.test.ts`
Expected: PASS（Tasks 2-4 已迁移完毕）

- [ ] **Step 2: 反转 `design-tokens.test.ts` 的两条断言**

把「保留 R7 过渡别名（对话框 14 处引用）」整条用例替换为：

```ts
it('过渡别名已随 Wave 4.1 删除', () => {
  for (const v of ['--app-list-bg', '--app-list-alt', '--app-select-row']) {
    expect(declares(v), `过渡别名 ${v} 应已删除`).toBe(false);
  }
  expect(raw).not.toContain('TODO(Wave 4.1)');
});
```

把「保留 .tab-active（对话框依赖）并新增 .tab-pill」替换为：

```ts
it('.tab-active 已删除，.tab-pill 保留', () => {
  expect(css).not.toMatch(/\.tab-active\s*\{/);
  expect(css).toMatch(/\.tab-pill\s*\{/);
});
```

Run: `npx vitest run tests/unit/design-tokens.test.ts`
Expected: **FAIL** —— 「过渡别名 --app-list-bg 应已删除」

- [ ] **Step 3: 从 `globals.css` 删除过渡设施**

删除 `:root` 块中的这段（含注释）：

```css
/* TODO(Wave 4.1)：以下为过渡别名 —— 对话框重做后删除。不得静默遗留。
     引用方：dialogs/ 下 7 个文件共 14 处（NewEnvVarDialog 4、ProfileDetail 3、
     EnvBackupPanel 2、ToolsTab 2、ProfileDialog 1、PathEditDialog 1、EditEnvVarDialog 1）。 */
--app-list-bg: var(--app-surface);
--app-list-alt: var(--app-row-odd);
--app-select-row: var(--app-row-selected);
```

并把 `globals.css:153` 的注释 `/* 主界面 Tab（对话框仍用 .tab-active，勿改其规则） */` 改为 `/* 主界面 Tab */` —— 该注释含 `tab-active` 字样，不改则验收 grep 必不空（测试剥注释会绿、验收不剥会红，口径必须一致）。

再删除 `.tab-active` 规则整块：

```css
/* 保留：对话框 EnvBackupPanel.tsx:61 依赖。
   有意偏离（H5）：色值由硬编码 #3b82f6 改为 var(--app-accent)。
   结构（border-bottom: 2px）不变，仅色值跟随主题。
   理由：硬编码色在深色玻璃主题下与 --app-accent(#7c6cff) 不协调。 */
.tab-active {
  border-bottom: 2px solid var(--app-accent);
  color: var(--app-accent);
}
```

- [ ] **Step 4: 从 `buttons.ts` 删除废弃导出与随之无用的 import**

删除文件末尾这两个导出（含各自的 JSDoc），**并删除文件首行 `import type { CSSProperties } from 'react';`** —— 它唯一用途是 `btnStyle` 的类型标注，删后 `noUnusedLocals: true` 会报 TS6133：

```ts
/**
 * @deprecated 过渡导出：Task 6-7 完成后删除。
 *
 * **H4：值必须保持不变。** 消费方是 R7 范围外的 `EnvBackupPanel.tsx:30/127`（对话框）
 * 与 `EnvVarToolbar.tsx`。把 `--app-bg` 改成 `--app-surface` 属于跨范围视觉变更，
 * 会让对话框内按钮底色在当前浅色主题下从 `#fafafa` 变成 `#ffffff`。
 * 新语义层里 `--app-bg` 与 `--app-surface` 名字都保留，故旧值仍可用。
 */
export const btnStyle: React.CSSProperties = {
  backgroundColor: 'var(--app-bg)',
  color: 'var(--app-fg)',
  borderColor: 'var(--app-border)',
};

/**
 * @deprecated 过渡导出：Task 5-8 完成后删除。
 * 使用 Button 组件或 buttonVariants 代替。
 */
export const btnClass =
  'px-3 py-1 text-sm rounded border transition-colors disabled:opacity-40 disabled:cursor-not-allowed';
```

- [ ] **Step 5: 删除 `button.test.tsx` 的过渡用例**

删除：

整块删除（含空壳 `describe`，避免留下无意义结构）：

```ts
describe('兼容导出', () => {
  it('btnClass / btnStyle 仍可用', () => {
    expect(typeof btnClass).toBe('string');
    expect(btnStyle).toHaveProperty('backgroundColor');
  });
});
```

并把文件顶部的 import 从：

```ts
import { buttonVariants, btnClass, btnStyle } from '@/components/ui/buttons';
```

改为：

```ts
import { buttonVariants } from '@/components/ui/buttons';
```

- [ ] **Step 6: 运行测试确认全部转绿**

Run: `npx tsc -b && npx vitest run`
Expected: PASS（含反转后的 design-tokens、dialog-tokens、button 三者）

- [ ] **Step 7: 机械验收四个 grep**

```bash
grep -rn -- '--app-list\|--app-select-row' src/ ; echo "exit=$?"
grep -rn "btnClass\|btnStyle" src/ ; echo "exit=$?"
grep -rn "tab-active" src/ ; echo "exit=$?"
grep -rnE "#3b82f6|#2563eb|#ef4444|#22c55e|#dc2626|text-red-500|text-green-600|text-yellow-500" src/components/dialogs ; echo "exit=$?"
```

Expected: 四条全部输出空、`exit=1`

- [ ] **Step 8: 提交**

```bash
git add src/styles/globals.css src/components/ui/buttons.ts tests/unit/design-tokens.test.ts tests/unit/button.test.tsx tests/unit/dialog-tokens.test.ts
git commit -m "refactor: 删除对话框过渡设施并反转测试断言"
```

---

### Task 6: 视觉回归与质量门

**Files:**

- Create: `e2e/tests/dialog-visual.spec.ts`

- [ ] **Step 1: 新建对话框视觉回归测试**

```ts
import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

/**
 * 对话框视觉回归：确认外壳用 token 渲染（非透明底），且背景与文字有对比。
 * isVisible() 不检测祖先裁切，故这里用计算样式直接断言关键属性。
 */
const CASES = [
  { name: '浅色', dark: false },
  { name: '深色', dark: true },
];

for (const c of CASES) {
  test(`PathEditDialog 外壳在${c.name}下使用 token 表面色`, async ({ page }) => {
    await page.setViewportSize({ width: 1200, height: 800 });
    await page.addInitScript(createIpcMock());
    if (c.dark) await page.addInitScript(() => localStorage.setItem('darkMode', '1'));
    await page.goto('/');

    await page.getByRole('button', { name: '新建' }).click();
    const surface = page.locator('.modal-surface');
    await expect(surface).toBeVisible();

    const styles = await surface.evaluate((el) => {
      const s = getComputedStyle(el);
      return { background: s.backgroundColor, radius: s.borderRadius, padding: s.padding };
    });

    // 背景必须不是透明（transparent 会让深色主题下文字不可读）
    expect(styles.background).not.toBe('rgba(0, 0, 0, 0)');
    // 圆角符合 --radius-lg（10px）
    expect(styles.radius).toBe('10px');
  });
}

test('对话框内不存在使用旧 class 的元素', async ({ page }) => {
  await page.setViewportSize({ width: 1200, height: 800 });
  await page.addInitScript(createIpcMock());
  await page.goto('/');

  await page.getByRole('button', { name: '新建' }).click();
  const legacy = await page.evaluate(() => {
    const panel = document.querySelector('.modal-surface');
    if (!panel) return ['modal-surface 不存在'];
    const hits: string[] = [];
    for (const el of panel.querySelectorAll('*')) {
      const cls = el.getAttribute('class') ?? '';
      // 注意：btn-* 是 Button 原语的合法类名，不能当旧设施。
      if (cls.includes('tab-active')) hits.push(cls);
      const style = el.getAttribute('style') ?? '';
      if (style.includes('--app-list')) hits.push(style);
    }
    return hits;
  });
  expect(legacy).toEqual([]);
});
```

Run: `npx playwright test --config e2e/playwright.config.ts e2e/tests/dialog-visual.spec.ts`
Expected: PASS（3 个用例）

- [ ] **Step 2: 手工截图确认四个对话框**

用临时 spec（跑完删除）依次打开并截图：

- `PathEditDialog`（新建路径）
- `AnalyzeDialog`（⋯ → 冲突分析）
- `ProfileDialog`（⋯ → 配置方案）
- `EnvBackupDialog`（全部变量 → 环境变量备份与恢复）

每个在浅色 / 深色各截一张，**逐张检查**：无透明底、无白字白底、按钮高度一致、圆角 10px。

- [ ] **Step 3: 运行完整质量门**

Run: `npm run verify:all`
Expected: 退出码 0；单元测试 ≥ 315（312 + 新增），E2E ≥ 41（38 + 3）

- [ ] **Step 4: 提交**

```bash
git add e2e/tests/dialog-visual.spec.ts
git commit -m "test(e2e): 新增对话框视觉回归"
```

---

## Self-Review

### 规格覆盖

| Spec 要求             | 对应 Task                                    |
| --------------------- | -------------------------------------------- |
| D1 Modal 外壳规格     | Task 1 Step 5 + CSS                          |
| D2 新增三个 token     | Task 1 Step 3                                |
| D3 按钮映射规则       | Tasks 2-4                                    |
| D4 `.field-input`     | Task 1 Step 4；Tasks 2-4 使用                |
| D5 硬编码色替换       | Tasks 2-4                                    |
| D6 透明度改语义前景色 | Tasks 2-4                                    |
| D7 删除过渡设施       | Task 5                                       |
| D8 列表项选中态       | Task 1（`.dialog-list-item`）+ Task 3/4 使用 |
| D9 字号层级           | Tasks 2-4                                    |
| 验收 grep 四条        | Task 5 Step 7                                |
| 反转测试断言          | Task 5 Step 2/5                              |
| 双主题截图            | Task 1 Step 7 + Task 6 Step 2                |

### 占位符扫描

无「TBD」「TODO」「参考 Task N」。所有代码步骤均给出完整代码。

### 类型一致性

- `Button` 的 `variant` / `size` 与 Task 2 计划中的 `buttonVariants` 签名一致（`primary | secondary | ghost | danger`，`sm | md`）。
- `.dialog-list-item` 的 `data-selected` 属性在 Task 3（`ProfileList`）与 Task 4（`EnvBackupPanel`）用法一致。
- `envVar` / `profile` / `envBackup` 的 i18n key 均沿用现状，未新增。

## Execution Notes

（留给开发窗口回填：偏离记录、未覆盖项、截图路径、质量门实际输出）
