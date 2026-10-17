# GUI 前端重设计 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 PathEditor 的 GUI 从「18 个同权重按钮的两行工具栏 + 海军蓝深色主题」重构为「浅色中性现代 / 深色玻璃深色、共用几何骨架的单行分层工具栏 + ⋯ 菜单」。

**Architecture:** 三层推进——① 建立设计 token 层（`globals.css` 语义变量 + 过渡别名）与按钮原语（`buttons.ts` → `buttonVariants`），② 新建无依赖 `Menu` 组件并重组工具栏，③ 把各视图组件迁移到新 token。全程零 Rust 改动、零 IPC 契约改动、零新依赖。

**Tech Stack:** React 19、TypeScript strict、Tailwind 4、Zustand、i18next、Vitest、Playwright

**Spec:** `docs/superpowers/specs/2026-10-17-gui-redesign-design.md`（v3）

**裁断依据:** `docs/审核和开发/2026.10.17/PathEditor-gui-redesign裁断.md`、`PathEditor-gui-redesign裁断-v3.md`

## Global Constraints

- **零新依赖**：不新增任何 `dependencies` / `devDependencies`。图标一律手写内联 SVG。
- **零 Rust 改动**：`core/` / `gui/` / `cli/` 不动；`cargo test --workspace` 不受影响。
- **零 IPC 契约改动**：不改 `services/backend.ts`、不改 store 逻辑、不改 `PathEntry` / `EnvVarMeta` 契约。
- **几何统一**：所有控件圆角 **8px**（小控件 6px、容器 10px），表格行高 **28px**，工具栏间距 **6px**。两种主题完全相同。
- **过渡别名必须保留**：`--app-list-bg` / `--app-list-alt` / `--app-select-row` 三个变量**不得删除**（对话框 7 文件 14 处引用），必须带 `TODO(Wave 4.1)` 注释。
- **`.tab-active` 原样保留**：对话框 `EnvBackupPanel.tsx:61` 依赖它；主界面改用新类 `.tab-pill`。
- **`data-testid` 9 个全部保留、不得改名**：`path-table` / `path-row` / `path-enabled` / `path-text` / `path-drop-zone` / `env-var-table` / `backup-summary` / `backup-removed-names` / `backup-outcome`。
- **i18n 通则**：改任何 key 的值前，先 `grep -rn "t('<key>')" src` 数引用点。既有 `button.save` / `button.cancel` **改值禁止**，只新增独立 key。
- **不碰 `CLAUDE.md` / `AGENTS.md`**。
- **提交规范**：Conventional Commits（`feat` / `fix` / `refactor` / `style` / `test` / `chore`）。
- **质量门**：每个 Task 结束跑 `npm run lint && npx tsc -b --noEmit`；全部完成后跑 `npm run verify:all`。

---

## 文件结构

### 新建

| 文件                                     | 职责                                                                                     |
| ---------------------------------------- | ---------------------------------------------------------------------------------------- |
| `tests/setup.ts`                         | Vitest 全局 setup：注册 `@testing-library/jest-dom/vitest`（**既有缺口，本波一并修复**） |
| `src/components/ui/Menu.tsx`             | 无依赖下拉菜单：受控开合、点击外部关闭、`Escape` 关闭、键盘上下导航、焦点回还            |
| `src/components/ui/Button.tsx`           | 按钮渲染组件，消费 `buttonVariants`                                                      |
| `src/components/ui/icons.tsx`            | 6 个手写内联 SVG 图标（Plus / Pencil / Trash / ArrowUp / ArrowDown / More）              |
| `src/components/toolbar/ToolbarMenu.tsx` | ⋯ 菜单的内容定义（维护组 / 应用组 / 取消并关闭），组合 `Menu`                            |
| `tests/unit/menu.test.tsx`               | `Menu` 行为测试                                                                          |

### 修改

| 文件                                         | 改动                                                                             |
| -------------------------------------------- | -------------------------------------------------------------------------------- |
| `src/styles/globals.css`                     | **重写**：语义 token 层 + 几何层 + 过渡别名 + `.tab-pill` + `.glass`；删除死代码 |
| `src/components/ui/buttons.ts`               | **重写**为 `buttonVariants` 变体表；保留旧导出名作过渡别名                       |
| `src/components/toolbar/ToolBar.tsx`         | 两行 → 单行分层；接入 `ToolbarMenu`                                              |
| `src/components/toolbar/ActionButtons.tsx`   | 编辑组 + 排序组 + 分隔线                                                         |
| `src/components/toolbar/UndoRedoButtons.tsx` | 改用 ghost 变体                                                                  |
| `src/components/toolbar/SearchInput.tsx`     | 改用新 token                                                                     |
| `src/components/layout/TitleBar.tsx`         | 品牌色块 + 版本号弱化                                                            |
| `src/components/layout/AppShell.tsx`         | Tab 改用 `.tab-pill`；移除硬编码色值                                             |
| `src/components/layout/StatusBar.tsx`        | 改用新 token                                                                     |
| `src/components/path-list/PathTable.tsx`     | 移除斑马纹；语义变量                                                             |
| `src/components/path-list/MergePreview.tsx`  | 改用语义变量（脱离别名）                                                         |
| `src/components/env-list/EnvVarTable.tsx`    | 改用语义变量（脱离别名）                                                         |
| `src/components/env-list/EnvVarToolbar.tsx`  | 改用新 token（沿用 `.tab-active`）                                               |
| `src/i18n/locales/zh-CN.json` / `en.json`    | 新增 `button.savePath` / `button.cancelAndClose` / `menu.*`                      |
| `vitest.config.ts`                           | **新增 `setupFiles`**（Task 0）                                                  |
| `e2e/tests/analyze.spec.ts:12`               | 先开 ⋯ 再点「冲突分析」                                                          |
| `e2e/tests/profiles.spec.ts:11`              | 先开 ⋯ 再点「配置方案」                                                          |
| `e2e/tests/search-clean.spec.ts:45`          | 先开 ⋯ 再点「清理无效路径」                                                      |
| `e2e/tests/toggle-save.spec.ts:19`           | `text=确定` → `text=保存`                                                        |

> **不改**：`e2e/tests/env-vars.spec.ts`、`tests/unit/app-shell-env-vars.test.tsx`（v3 裁断确认不受影响）。

---

### Task 0: 测试基础设施（jest-dom 注册）

**Files:**

- Create: `tests/setup.ts`
- Modify: `vitest.config.ts`

**Interfaces:**

- Produces: 全局可用的 jest-dom 匹配器（`toBeInTheDocument` / `toBeDisabled` / `toHaveClass` / `toHaveTextContent` / `toHaveCSS` 等）

> **背景（B1）**：`@testing-library/jest-dom` 已在 `devDependencies`（`^6.9.1`），但 `vitest.config.ts` **从未配置 `setupFiles`**，仓库里也没有任何 setup 文件。现有 17 个测试文件因此只能用 `toBeNull()` / `toBe()`。**这是既有缺口**，本波一并修复，否则 Task 2 / Task 4 写的断言跑不起来。

- [ ] **Step 1: 先复现缺口（确认问题真实存在）**

创建临时探针 `tests/unit/_probe-jestdom.test.tsx`：

```tsx
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';

describe('jest-dom 探针', () => {
  it('toBeInTheDocument 应可用', () => {
    const { getByText } = render(<span>probe</span>);
    expect(getByText('probe')).toBeInTheDocument();
  });
});
```

Run: `npx vitest run tests/unit/_probe-jestdom.test.tsx`
Expected: **FAIL** —— `Invalid Chai property: toBeInTheDocument`（若意外 PASS，说明环境已有注册，Task 0 可跳过，但请在提交信息中注明）

- [ ] **Step 2: 创建 `tests/setup.ts`**

```ts
/**
 * Vitest 全局 setup。
 * 注册 jest-dom 匹配器 —— @testing-library/jest-dom v6 的 vitest 专用入口。
 */
import '@testing-library/jest-dom/vitest';
```

- [ ] **Step 3: 修改 `vitest.config.ts`，加入 `setupFiles`**

```ts
import { defineConfig } from 'vitest/config';
import path from 'node:path';

export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./tests/setup.ts'],
    exclude: ['e2e/**', 'node_modules/**', 'gui/**', '.claude/**'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov', 'cobertura'],
      include: ['src/core/**', 'src/store/**', 'src/hooks/**'],
      exclude: ['src/main.tsx', 'src/vite-env.d.ts'],
      thresholds: {
        lines: 80,
      },
    },
  },
});
```

- [ ] **Step 4: 运行探针确认通过**

Run: `npx vitest run tests/unit/_probe-jestdom.test.tsx`
Expected: **PASS**

- [ ] **Step 5: 删除探针，跑全量确认既有测试无副作用**

```bash
rm tests/unit/_probe-jestdom.test.tsx
npx vitest run
```

Expected: **全部既有测试 PASS**（注册 jest-dom 只是新增匹配器，不改变既有断言行为）

- [ ] **Step 6: 提交**

```bash
git add tests/setup.ts vitest.config.ts
git commit -m "test: 注册 jest-dom 匹配器（修复既有 setup 缺口）"
```

---

### Task 1: 设计 Token 层

**Files:**

- Modify: `src/styles/globals.css`（整体重写）

**Interfaces:**

- Produces: CSS 变量 `--app-*` 语义层（18 个）、`--radius*` / `--row-h` / `--gap` 几何层（5 个）、类 `.tab-pill`、`.glass`；**过渡别名** `--app-list-bg` / `--app-list-alt` / `--app-select-row`

- [ ] **Step 1: 写测试——断言新 token 存在且旧别名可用**

创建 `tests/unit/design-tokens.test.ts`：

```ts
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const raw = readFileSync(resolve(__dirname, '../../src/styles/globals.css'), 'utf8');

/**
 * 剥离 CSS 注释后再匹配（B4）。
 * 不剥离时：注释里提到「已删除 --color-path-invalid」会假红；
 *          把死变量注释掉而不删会假绿。
 */
const css = raw.replace(/\/\*[\s\S]*?\*\//g, '');

function declares(name: string): boolean {
  // 只匹配「--- 行首冒号」形式的声明，避免匹配到 var(--x) 的引用
  return new RegExp(`^\\s*${name}\\s*:`, 'm').test(css);
}

describe('设计 token 层', () => {
  it('声明全部语义变量', () => {
    for (const v of [
      '--app-bg',
      '--app-surface',
      '--app-surface-raised',
      '--app-border',
      '--app-border-strong',
      '--app-fg',
      '--app-fg-muted',
      '--app-fg-faint',
      '--app-accent',
      '--app-accent-fg',
      '--app-accent-bg',
      '--app-row-even',
      '--app-row-odd',
      '--app-row-selected',
      '--app-hover',
      '--app-danger',
      '--app-warning',
      '--app-disabled-fg',
    ]) {
      expect(declares(v), `缺少 ${v}`).toBe(true);
    }
  });

  it('声明几何变量', () => {
    for (const v of ['--radius-sm', '--radius', '--radius-lg', '--row-h', '--gap']) {
      expect(declares(v), `缺少 ${v}`).toBe(true);
    }
  });

  it('保留 R7 过渡别名（对话框 14 处引用）', () => {
    for (const v of ['--app-list-bg', '--app-list-alt', '--app-select-row']) {
      expect(declares(v), `缺少过渡别名 ${v}`).toBe(true);
    }
    // TODO 注释本身在 raw 里（已被 css 剥离），故查 raw
    expect(raw).toContain('TODO(Wave 4.1)');
  });

  it('保留 .tab-active（对话框依赖）并新增 .tab-pill', () => {
    expect(css).toMatch(/\.tab-active\s*\{/);
    expect(css).toMatch(/\.tab-pill\s*\{/);
  });

  it('不包含死代码', () => {
    for (const dead of [
      '.row-even',
      '.row-odd',
      '.btn-disabled',
      '.text-invalid',
      '.text-duplicate',
    ]) {
      expect(css.includes(dead), `死代码 ${dead} 未清理`).toBe(false);
    }
    for (const dead of ['--color-path-invalid', '--color-path-duplicate', '--color-path-valid']) {
      expect(css.includes(dead), `死变量 ${dead} 未清理`).toBe(false);
    }
  });
});
```

> **注意**：`.row-odd` 是本测试要清理的死代码，但新 token 里有 `--app-row-odd`。`css.includes('.row-odd')` **不会**匹配 `--app-row-odd`（前者带点、后者带双横线），故安全。

- [ ] **Step 2: 运行测试确认失败**

Run: `npx vitest run tests/unit/design-tokens.test.ts`
Expected: FAIL —— 「缺少 --app-surface」等

- [ ] **Step 3: 重写 `globals.css`**

```css
@import 'tailwindcss';

/* ─────────────────────────────────────────────────────────────
   语义 token 层 —— 组件只允许使用这一层的变量。
   浅色 = 中性现代；深色 = 玻璃深色。几何层两主题完全一致。
   ───────────────────────────────────────────────────────────── */

:root {
  /* 背景与表面 */
  --app-bg: #fafafa;
  --app-surface: #ffffff;
  --app-surface-raised: #ffffff;
  --app-border: #e4e4e7;
  --app-border-strong: #d4d4d8;

  /* 文字 */
  --app-fg: #18181b;
  --app-fg-muted: #71717a;
  --app-fg-faint: #a1a1aa;

  /* 强调 */
  --app-accent: #6366f1;
  --app-accent-fg: #ffffff;
  --app-accent-bg: #eef2ff;
  --app-accent-text: #4f46e5;
  --app-accent-glow: none;

  /* 行 */
  --app-row-even: #ffffff;
  --app-row-odd: #fafafa;
  --app-row-selected: rgba(99, 102, 241, 0.1);
  --app-hover: #f4f4f5;

  /* 语义色 */
  --app-danger: #dc2626;
  --app-warning: #d97706;
  --app-disabled-fg: #a1a1aa;

  /* 浮层 */
  --app-menu-bg: #ffffff;
  --app-menu-shadow: 0 12px 32px rgb(0 0 0 / 12%), 0 2px 6px rgb(0 0 0 / 6%);

  /* 几何（两主题一致，不随 .dark 变化） */
  --radius-sm: 6px;
  --radius: 8px;
  --radius-lg: 10px;
  --row-h: 28px;
  --gap: 6px;

  /* TODO(Wave 4.1)：以下为过渡别名 —— 对话框重做后删除。不得静默遗留。
     引用方：dialogs/ 下 7 个文件共 14 处（NewEnvVarDialog 4、ProfileDetail 3、
     EnvBackupPanel 2、ToolsTab 2、ProfileDialog 1、PathEditDialog 1、EditEnvVarDialog 1）。 */
  --app-list-bg: var(--app-surface);
  --app-list-alt: var(--app-row-odd);
  --app-select-row: var(--app-row-selected);
}

:root.dark {
  --app-bg: #0d0d12;
  --app-surface: rgb(255 255 255 / 4%);
  --app-surface-raised: rgb(30 30 40 / 92%);
  --app-border: rgb(255 255 255 / 9%);
  --app-border-strong: rgb(255 255 255 / 16%);

  --app-fg: #f4f4f6;
  --app-fg-muted: #8b8b96;
  --app-fg-faint: #63636e;

  --app-accent: #7c6cff;
  --app-accent-fg: #ffffff;
  --app-accent-bg: rgb(124 108 255 / 16%);
  --app-accent-text: #b3a8ff;
  --app-accent-glow: 0 0 18px rgb(124 108 255 / 50%);

  --app-row-even: transparent;
  --app-row-odd: rgb(255 255 255 / 2%);
  --app-row-selected: rgb(124 108 255 / 18%);
  --app-hover: rgb(255 255 255 / 6%);

  --app-danger: #ff6b6b;
  --app-warning: #ffb454;
  --app-disabled-fg: #63636e;

  --app-menu-bg: rgb(30 30 40 / 92%);
  --app-menu-shadow: 0 16px 40px rgb(0 0 0 / 55%), 0 0 0 1px rgb(255 255 255 / 4%);
}

/* ─────────────────────────── 全局基础 ─────────────────────────── */

html,
body,
#root {
  height: 100%;
}

body {
  margin: 0;
  padding: 0;
  font-family:
    system-ui,
    -apple-system,
    'Segoe UI',
    'Microsoft YaHei',
    sans-serif;
  font-size: 13px;
  line-height: 1.5;
  background: var(--app-bg);
  color: var(--app-fg);
  overflow: hidden;
  user-select: none;
  -webkit-font-smoothing: antialiased;
}

:root.dark body {
  background: linear-gradient(160deg, #15151d, #0d0d12);
}

/* 滚动条 */
::-webkit-scrollbar {
  width: 10px;
  height: 10px;
}

::-webkit-scrollbar-track {
  background: transparent;
}

::-webkit-scrollbar-thumb {
  background: var(--app-border-strong);
  border: 2px solid transparent;
  border-radius: 999px;
  background-clip: padding-box;
}

::-webkit-scrollbar-thumb:hover {
  background: var(--app-fg-faint);
  background-clip: padding-box;
}

/* ─────────────────────────── 工具类 ─────────────────────────── */

/* 主界面 Tab（对话框仍用 .tab-active，勿改其规则） */
.tab-pill {
  border-radius: var(--radius);
  padding: 4px 12px;
  color: var(--app-fg-muted);
  transition:
    background-color 120ms ease,
    color 120ms ease;
}

.tab-pill:hover {
  background: var(--app-hover);
  color: var(--app-fg);
}

.tab-pill[data-active='true'] {
  background: var(--app-accent-bg);
  color: var(--app-accent-text);
  font-weight: 500;
}

/* 保留：对话框 EnvBackupPanel.tsx:61 依赖。
   有意偏离（H5）：色值由硬编码 #3b82f6 改为 var(--app-accent)。
   结构（border-bottom: 2px）不变，仅色值跟随主题。
   理由：硬编码色在深色玻璃主题下与 --app-accent(#7c6cff) 不协调。 */
.tab-active {
  border-bottom: 2px solid var(--app-accent);
  color: var(--app-accent);
}

/* 深色浮层毛玻璃 */
.glass {
  backdrop-filter: blur(14px);
}

@media (prefers-reduced-motion: reduce) {
  * {
    transition: none !important;
    animation: none !important;
  }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `npx vitest run tests/unit/design-tokens.test.ts`
Expected: PASS（5 个用例全绿）

- [ ] **Step 5: 目视确认界面未坏**

Run: `npm run dev`
Expected: 浏览器打开后**浅色主题正常显示**。切深色（控制台 `document.documentElement.classList.add('dark')`）后**背景为深色渐变**。此时组件仍用旧变量名，靠过渡别名/同名新值保持可用——**界面不应出现透明底或白字白底**。

- [ ] **Step 6: 提交**

```bash
git add src/styles/globals.css tests/unit/design-tokens.test.ts
git commit -m "style: 建立设计 token 层与过渡别名"
```

---

### Task 2: 按钮原语

**Files:**

- Modify: `src/components/ui/buttons.ts`
- Create: `src/components/ui/Button.tsx`
- Test: `tests/unit/button.test.tsx`

**Interfaces:**

- Consumes: Task 1 的 CSS 变量
- Produces:
  - `type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger'`
  - `buttonVariants(variant?: ButtonVariant, size?: 'sm' | 'md'): string`（返回 className 字符串）
  - `export function Button(props: ButtonProps)`，`ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & { variant?: ButtonVariant; size?: 'sm' | 'md' }`
  - 兼容导出：`btnClass`、`btnStyle`（**保留**，Task 5-8 逐步替换）

- [ ] **Step 1: 写失败测试**

创建 `tests/unit/button.test.tsx`：

```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Button } from '@/components/ui/Button';
import { buttonVariants, btnClass, btnStyle } from '@/components/ui/buttons';

describe('buttonVariants', () => {
  it('每个变体返回不同 class', () => {
    const all = new Set(
      (['primary', 'secondary', 'ghost', 'danger'] as const).map((v) => buttonVariants(v)),
    );
    expect(all.size).toBe(4);
  });

  it('所有变体都含统一圆角与间距', () => {
    for (const v of ['primary', 'secondary', 'ghost', 'danger'] as const) {
      const cls = buttonVariants(v);
      expect(cls).toContain('rounded-lg');
      expect(cls).toContain('whitespace-nowrap');
    }
  });
});

describe('Button', () => {
  it('渲染 children 并透传 onClick', () => {
    let clicked = false;
    render(<Button onClick={() => (clicked = true)}>保存</Button>);
    screen.getByRole('button', { name: '保存' }).click();
    expect(clicked).toBe(true);
  });

  it('disabled 时阻止点击', () => {
    let clicked = false;
    render(
      <Button disabled onClick={() => (clicked = true)}>
        新建
      </Button>,
    );
    const btn = screen.getByRole('button', { name: '新建' });
    expect(btn).toBeDisabled();
    btn.click();
    expect(clicked).toBe(false);
  });

  it('primary 变体带强调底色', () => {
    render(<Button variant="primary">保存</Button>);
    expect(screen.getByRole('button').className).toContain('btn-primary');
  });
});

describe('兼容导出', () => {
  it('btnClass / btnStyle 仍可用', () => {
    expect(typeof btnClass).toBe('string');
    expect(btnStyle).toHaveProperty('backgroundColor');
  });
});
```

- [ ] **Step 2: 运行确认失败**

Run: `npx vitest run tests/unit/button.test.tsx`
Expected: FAIL —— `Cannot find module '@/components/ui/Button'`

- [ ] **Step 3: 重写 `buttons.ts`**

```ts
export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md';

/** 所有变体共享的基础类 */
const BASE =
  'inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-lg border ' +
  'font-medium transition-colors select-none ' +
  'disabled:opacity-40 disabled:cursor-not-allowed disabled:pointer-events-none';

const SIZE: Record<ButtonSize, string> = {
  sm: 'h-6 px-2 text-xs',
  md: 'h-7 px-2.5 text-xs',
};

/**
 * 返回按钮的 className。变体通过自定义类名（btn-*）表达，
 * 具体颜色在 globals.css 中按主题切换 —— 避免在 TS 里硬编码色值。
 */
export function buttonVariants(
  variant: ButtonVariant = 'secondary',
  size: ButtonSize = 'md',
): string {
  return `${BASE} ${SIZE[size]} btn-${variant}`;
}

/**
 * @deprecated 过渡导出：Task 5-8 完成后删除。
 * 使用 Button 组件或 buttonVariants 代替。
 */
export const btnClass =
  'px-3 py-1 text-sm rounded border transition-colors disabled:opacity-40 disabled:cursor-not-allowed';

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
```

在 `globals.css` 末尾追加（Task 1 已提交该文件，此处为补充）：

```css
/* 按钮变体 */
.btn-primary {
  background: var(--app-accent);
  border-color: transparent;
  color: var(--app-accent-fg);
  box-shadow: var(--app-accent-glow);
}

.btn-primary:hover:not(:disabled) {
  filter: brightness(1.08);
}

.btn-secondary {
  background: var(--app-surface);
  border-color: var(--app-border);
  color: var(--app-fg);
}

.btn-secondary:hover:not(:disabled) {
  background: var(--app-hover);
  border-color: var(--app-border-strong);
}

.btn-ghost {
  background: transparent;
  border-color: transparent;
  color: var(--app-fg-muted);
}

.btn-ghost:hover:not(:disabled) {
  background: var(--app-hover);
  color: var(--app-fg);
}

.btn-danger {
  background: transparent;
  border-color: transparent;
  color: var(--app-danger);
}

.btn-danger:hover:not(:disabled) {
  background: color-mix(in srgb, var(--app-danger) 12%, transparent);
}
```

- [ ] **Step 4: 创建 `Button.tsx`**

```tsx
import type { ButtonHTMLAttributes } from 'react';
import { buttonVariants, type ButtonSize, type ButtonVariant } from './buttons';

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
};

/** 统一按钮：所有变体共享几何，颜色由 globals.css 的 .btn-* 类按主题提供。 */
export function Button({ variant = 'secondary', size = 'md', className, ...rest }: ButtonProps) {
  const cls = buttonVariants(variant, size);
  return <button type="button" className={className ? `${cls} ${className}` : cls} {...rest} />;
}
```

- [ ] **Step 5: 运行确认通过**

Run: `npx vitest run tests/unit/button.test.tsx`
Expected: PASS

- [ ] **Step 6: 提交**

```bash
git add src/components/ui/buttons.ts src/components/ui/Button.tsx src/styles/globals.css tests/unit/button.test.tsx
git commit -m "feat: 新增按钮原语与变体"
```

---

### Task 3: 内联 SVG 图标

**Files:**

- Create: `src/components/ui/icons.tsx`
- Test: `tests/unit/icons.test.tsx`

**Interfaces:**

- Produces: `PlusIcon({ className?: string })`、`PencilIcon`、`TrashIcon`、`ArrowUpIcon`、`ArrowDownIcon`、`MoreIcon` —— 均为「接收 `className` 的纯函数组件，渲染 `<svg aria-hidden="true">`」

- [ ] **Step 1: 写失败测试**

创建 `tests/unit/icons.test.tsx`：

```tsx
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import {
  PlusIcon,
  PencilIcon,
  TrashIcon,
  ArrowUpIcon,
  ArrowDownIcon,
  MoreIcon,
} from '@/components/ui/icons';

describe('图标', () => {
  it('全部渲染为 aria-hidden 的 svg', () => {
    for (const Icon of [PlusIcon, PencilIcon, TrashIcon, ArrowUpIcon, ArrowDownIcon, MoreIcon]) {
      const { container, unmount } = render(<Icon />);
      const svg = container.querySelector('svg');
      expect(svg).not.toBeNull();
      expect(svg?.getAttribute('aria-hidden')).toBe('true');
      unmount();
    }
  });

  it('接受 className 并透传', () => {
    const { container } = render(<PlusIcon className="custom-class" />);
    expect(container.querySelector('svg')?.classList.contains('custom-class')).toBe(true);
  });
});
```

- [ ] **Step 2: 运行确认失败**

Run: `npx vitest run tests/unit/icons.test.tsx`
Expected: FAIL —— `Cannot find module '@/components/ui/icons'`

- [ ] **Step 3: 实现 `icons.tsx`**

```tsx
/** 手写内联 SVG 图标集 —— 不引入图标库依赖（spec N3）。 */

type IconProps = { className?: string };

const BASE = {
  viewBox: '0 0 16 16',
  width: 14,
  height: 14,
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.5,
  strokeLinecap: 'round' as const,
  strokeLinejoin: 'round' as const,
  'aria-hidden': true,
};

export function PlusIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M8 3v10M3 8h10" />
    </svg>
  );
}

export function PencilIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M11.5 2.5l2 2-8 8H3.5v-2l8-8z" />
    </svg>
  );
}

export function TrashIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8h5.8l.6-8" />
    </svg>
  );
}

export function ArrowUpIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M8 13V3M4 7l4-4 4 4" />
    </svg>
  );
}

export function ArrowDownIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <path d="M8 3v10M4 9l4 4 4-4" />
    </svg>
  );
}

export function MoreIcon({ className }: IconProps) {
  return (
    <svg {...BASE} className={className}>
      <circle cx="3.5" cy="8" r="1" fill="currentColor" stroke="none" />
      <circle cx="8" cy="8" r="1" fill="currentColor" stroke="none" />
      <circle cx="12.5" cy="8" r="1" fill="currentColor" stroke="none" />
    </svg>
  );
}
```

- [ ] **Step 4: 运行确认通过**

Run: `npx vitest run tests/unit/icons.test.tsx`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/components/ui/icons.tsx tests/unit/icons.test.tsx
git commit -m "feat: 新增内联 SVG 图标集"
```

---

### Task 4: Menu 下拉菜单组件

**Files:**

- Create: `src/components/ui/Menu.tsx`
- Test: `tests/unit/menu.test.tsx`

**Interfaces:**

- Consumes: Task 1 的 `--app-menu-*` / `.glass`；Task 3 的图标；Task 2 的 `Button`
- Produces:

  ```ts
  type MenuItemDef = {
    key: string;
    label: string;
    onSelect: () => void;
    disabled?: boolean;
    shortcut?: string; // 如 'Ctrl+P'，仅展示
    danger?: boolean;
  };
  type MenuGroupDef = { key: string; label?: string; items: MenuItemDef[] };
  type MenuProps = {
    groups: MenuGroupDef[];
    triggerLabel?: string; // 默认 '⋯'
    triggerAriaLabel: string; // 必填，供 aria-label
    footer?: MenuItemDef; // 底部（分隔线隔开），用于「取消并关闭」
  };
  export function Menu(props: MenuProps);
  ```

  菜单容器带 `data-testid="menu-panel"`，每个条目带 `role="menuitem"`。

- [ ] **Step 1: 写失败测试**

创建 `tests/unit/menu.test.tsx`：

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Menu, type MenuGroupDef } from '@/components/ui/Menu';

function makeGroups(onSelect = vi.fn()): MenuGroupDef[] {
  return [
    { key: 'maint', label: '维护', items: [{ key: 'clean', label: '清理无效路径', onSelect }] },
    {
      key: 'app',
      label: '应用',
      items: [{ key: 'help', label: '帮助', onSelect, shortcut: 'F1' }],
    },
  ];
}

describe('Menu', () => {
  it('初始关闭：不渲染菜单面板', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    expect(screen.queryByTestId('menu-panel')).toBeNull();
  });

  it('点击触发按钮打开菜单', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    expect(screen.getByTestId('menu-panel')).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: /清理无效路径/ })).toBeInTheDocument();
  });

  it('再次点击触发按钮关闭', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    const trigger = screen.getByRole('button', { name: '更多操作' });
    fireEvent.click(trigger);
    fireEvent.click(trigger);
    expect(screen.queryByTestId('menu-panel')).toBeNull();
  });

  it('按 Escape 关闭', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByTestId('menu-panel')).toBeNull();
  });

  it('点击外部关闭', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    fireEvent.mouseDown(document.body);
    expect(screen.queryByTestId('menu-panel')).toBeNull();
  });

  it('点击条目触发回调并关闭', () => {
    const onSelect = vi.fn();
    render(<Menu groups={makeGroups(onSelect)} triggerAriaLabel="更多操作" />);
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    fireEvent.click(screen.getByRole('menuitem', { name: /清理无效路径/ }));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('menu-panel')).toBeNull();
  });

  it('显示快捷键提示', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    expect(screen.getByText('F1')).toBeInTheDocument();
  });

  it('disabled 条目不可点击', () => {
    const onSelect = vi.fn();
    const groups: MenuGroupDef[] = [
      { key: 'g', items: [{ key: 'a', label: '禁用项', onSelect, disabled: true }] },
    ];
    render(<Menu groups={groups} triggerAriaLabel="更多操作" />);
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    fireEvent.click(screen.getByRole('menuitem', { name: '禁用项' }));
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('footer 条目渲染在分隔线之后', () => {
    const onSelect = vi.fn();
    render(
      <Menu
        groups={makeGroups()}
        triggerAriaLabel="更多操作"
        footer={{ key: 'cancel', label: '取消并关闭', onSelect }}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    fireEvent.click(screen.getByRole('menuitem', { name: '取消并关闭' }));
    expect(onSelect).toHaveBeenCalled();
  });

  it('关闭后焦点回到触发按钮', () => {
    render(<Menu groups={makeGroups()} triggerAriaLabel="更多操作" />);
    const trigger = screen.getByRole('button', { name: '更多操作' });
    fireEvent.click(trigger);
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(document.activeElement).toBe(trigger);
  });
});
```

- [ ] **Step 2: 运行确认失败**

Run: `npx vitest run tests/unit/menu.test.tsx`
Expected: FAIL —— `Cannot find module '@/components/ui/Menu'`

- [ ] **Step 3: 实现 `Menu.tsx`**

```tsx
import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { Button } from './Button';
import { MoreIcon } from './icons';

export type MenuItemDef = {
  key: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  /** 仅展示的快捷键提示，如 'Ctrl+P' */
  shortcut?: string;
  /** 破坏性操作（红色） */
  danger?: boolean;
};

export type MenuGroupDef = { key: string; label?: string; items: MenuItemDef[] };

export type MenuProps = {
  groups: MenuGroupDef[];
  /** 触发按钮的可见文字，默认 '⋯' */
  triggerLabel?: React.ReactNode;
  /** 触发按钮的无障碍名称（必填） */
  triggerAriaLabel: string;
  /** 底部条目，渲染在分隔线之后（如「取消并关闭」） */
  footer?: MenuItemDef;
};

/**
 * 无依赖下拉菜单。
 * - 点击触发按钮开合；点击外部 / Escape 关闭；关闭后焦点回到触发按钮。
 * - 条目不实现 aria-activedescendant 式键盘漫游，但可用 Tab 键遍历（各条目为原生 button）。
 */
export function Menu({ groups, triggerLabel, triggerAriaLabel, footer }: MenuProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const panelId = useId();

  const close = useCallback(() => {
    setOpen(false);
    triggerRef.current?.focus();
  }, []);

  // 点击外部关闭
  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', onPointerDown);
    return () => document.removeEventListener('mousedown', onPointerDown);
  }, [open]);

  // Escape 关闭
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        close();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [open, close]);

  const handleItem = (item: MenuItemDef) => {
    if (item.disabled) return;
    setOpen(false);
    item.onSelect();
  };

  const renderItem = (item: MenuItemDef) => (
    <button
      key={item.key}
      type="button"
      role="menuitem"
      disabled={item.disabled}
      onClick={() => handleItem(item)}
      className={`menu-item${item.danger ? ' menu-item-danger' : ''}`}
    >
      <span>{item.label}</span>
      {item.shortcut && <span className="menu-shortcut">{item.shortcut}</span>}
    </button>
  );

  return (
    <div className="relative" ref={rootRef}>
      <Button
        ref={triggerRef}
        variant="ghost"
        aria-label={triggerAriaLabel}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? panelId : undefined}
        onClick={() => setOpen((v) => !v)}
      >
        {triggerLabel ?? <MoreIcon />}
      </Button>
      {open && (
        <div id={panelId} role="menu" data-testid="menu-panel" className="menu-panel glass">
          {groups.map((group) => (
            <div key={group.key}>
              {group.label && <div className="menu-group-label">{group.label}</div>}
              {group.items.map(renderItem)}
            </div>
          ))}
          {footer && (
            <>
              <div className="menu-separator" />
              {renderItem(footer)}
            </>
          )}
        </div>
      )}
    </div>
  );
}
```

在 `globals.css` 末尾追加菜单样式：

```css
/* 下拉菜单 */
.menu-panel {
  position: absolute;
  right: 0;
  top: calc(100% + 4px);
  z-index: 40;
  min-width: 200px;
  padding: 5px;
  border: 1px solid var(--app-border);
  border-radius: var(--radius-lg);
  background: var(--app-menu-bg);
  box-shadow: var(--app-menu-shadow);
}

.menu-group-label {
  padding: 4px 8px 2px;
  font-size: 10.5px;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--app-fg-faint);
}

.menu-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 6px 8px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--app-fg);
  font-size: 12.5px;
  text-align: left;
  cursor: pointer;
}

.menu-item:hover:not(:disabled) {
  background: var(--app-hover);
}

.menu-item:disabled {
  color: var(--app-fg-faint);
  cursor: not-allowed;
}

.menu-item-danger {
  color: var(--app-danger);
}

.menu-shortcut {
  color: var(--app-fg-faint);
  font-family: ui-monospace, Consolas, monospace;
  font-size: 11px;
}

.menu-separator {
  height: 1px;
  margin: 4px 2px;
  background: var(--app-border);
}
```

> **注意**：`Button` 需要支持 `ref`。React 19 允许函数组件直接接收 `ref` 作为普通 prop，因此把 Task 2 的 `Button` 签名改为：
>
> ```tsx
> export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
>   variant?: ButtonVariant;
>   size?: ButtonSize;
>   ref?: Ref<HTMLButtonElement>;
> };
> export function Button({
>   variant = 'secondary',
>   size = 'md',
>   className,
>   ref,
>   ...rest
> }: ButtonProps) {
>   const cls = buttonVariants(variant, size);
>   return (
>     <button
>       ref={ref}
>       type="button"
>       className={className ? `${cls} ${className}` : cls}
>       {...rest}
>     />
>   );
> }
> ```
>
> 并同步更新 Task 2 的 `Button.tsx` 代码。

- [ ] **Step 4: 运行确认通过**

Run: `npx vitest run tests/unit/menu.test.tsx`
Expected: PASS（10 个用例全绿）

- [ ] **Step 5: 提交**

```bash
git add src/components/ui/Menu.tsx src/components/ui/Button.tsx src/styles/globals.css tests/unit/menu.test.tsx
git commit -m "feat: 新增无依赖下拉菜单组件"
```

---

### Task 5: i18n 文案

**Files:**

- Modify: `src/i18n/locales/zh-CN.json`
- Modify: `src/i18n/locales/en.json`
- Test: `tests/unit/i18n-keys.test.ts`

**Interfaces:**

- Produces: `button.savePath`、`button.cancelAndClose`、`menu.label` / `menu.maintenance` / `menu.application` / `menu.clean` / `menu.analyze` / `menu.profiles` / `menu.language` / `menu.help`
- **复用不改**：`button.darkMode`（深色模式）/ `button.lightMode`（浅色模式）—— 主题切换项按当前状态二选一
- **不改**：`button.save`（保持「确定」/`OK`）、`button.cancel`（保持「取消」/`Cancel`）

- [ ] **Step 1: 写失败测试**

创建 `tests/unit/i18n-keys.test.ts`：

```ts
import { describe, it, expect } from 'vitest';
import zh from '@/i18n/locales/zh-CN.json';
import en from '@/i18n/locales/en.json';

function flat(obj: unknown, prefix = ''): Record<string, string> {
  const out: Record<string, string> = {};
  if (obj && typeof obj === 'object') {
    for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
      if (typeof v === 'string') out[prefix + k] = v;
      else if (v && typeof v === 'object') Object.assign(out, flat(v, `${prefix}${k}.`));
    }
  }
  return out;
}

const Z = flat(zh);
const E = flat(en);

describe('新增 i18n 键', () => {
  const NEW_KEYS = [
    'button.savePath',
    'button.cancelAndClose',
    'menu.label',
    'menu.maintenance',
    'menu.application',
    'menu.clean',
    'menu.analyze',
    'menu.profiles',
    'menu.language',
    'menu.help',
  ];

  it('中英文都声明了全部新键', () => {
    for (const k of NEW_KEYS) {
      expect(Z[k], `zh 缺少 ${k}`).toBeTruthy();
      expect(E[k], `en 缺少 ${k}`).toBeTruthy();
    }
  });

  it('工具栏保存用「保存」/Save，对话框仍用「确定」/OK', () => {
    expect(Z['button.savePath']).toBe('保存');
    expect(E['button.savePath']).toBe('Save');
    expect(Z['button.save']).toBe('确定');
    expect(E['button.save']).toBe('OK');
  });

  it('工具栏取消用「取消并关闭」，对话框仍用「取消」/Cancel', () => {
    expect(Z['button.cancelAndClose']).toBe('取消并关闭');
    expect(E['button.cancelAndClose']).toBe('Cancel');
    expect(Z['button.cancel']).toBe('取消');
    expect(E['button.cancel']).toBe('Cancel');
  });

  it('中英文键集合一致', () => {
    const onlyZh = Object.keys(Z).filter((k) => !(k in E) && !k.includes('_comment'));
    const onlyEn = Object.keys(E).filter((k) => !(k in Z) && !k.includes('_comment'));
    expect(onlyZh).toEqual([]);
    expect(onlyEn).toEqual([]);
  });
});
```

- [ ] **Step 2: 运行确认失败**

Run: `npx vitest run tests/unit/i18n-keys.test.ts`
Expected: FAIL —— 「zh 缺少 button.savePath」

- [ ] **Step 3: 在 `zh-CN.json` 的 `button` 对象内追加两个键**

在 `"close": "关闭"` 之后（保持 JSON 合法）：

```json
    "close": "关闭",
    "savePath": "保存",
    "cancelAndClose": "取消并关闭"
```

在 `zh-CN.json` 顶层（与 `"button"` 同级）新增 `menu`：

```json
  "menu": {
    "label": "更多操作",
    "maintenance": "维护",
    "application": "应用",
    "clean": "清理无效路径",
    "analyze": "冲突分析",
    "profiles": "配置方案",
    "language": "切换语言",
    "help": "帮助"
  },
```

> **注意**：`menu` 内**不含 `darkMode`** —— 主题切换项复用既有的 `button.darkMode` / `button.lightMode`（见 Task 6）。

- [ ] **Step 4: 在 `en.json` 中做同位置追加**

```json
    "close": "Close",
    "savePath": "Save",
    "cancelAndClose": "Cancel"
```

```json
  "menu": {
    "label": "More actions",
    "maintenance": "Maintenance",
    "application": "Application",
    "clean": "Clean invalid",
    "analyze": "Analyze",
    "profiles": "Profiles",
    "language": "Language",
    "help": "Help"
  },
```

- [ ] **Step 5: 运行确认通过**

Run: `npx vitest run tests/unit/i18n-keys.test.ts`
Expected: PASS

- [ ] **Step 6: 确认既有测试未受影响**

Run: `npx vitest run tests/unit/app-shell-env-vars.test.tsx tests/unit/env-var-toolbar.test.tsx`
Expected: PASS（`button.save` / `button.cancel` 未改值，12 处「确定」断言不受影响）

- [ ] **Step 7: 提交**

```bash
git add src/i18n/locales/zh-CN.json src/i18n/locales/en.json tests/unit/i18n-keys.test.ts
git commit -m "feat(i18n): 新增工具栏与菜单文案键"
```

---

### Task 6: 工具栏菜单与单行工具栏

**Files:**

- Create: `src/components/toolbar/ToolbarMenu.tsx`
- Modify: `src/components/toolbar/ToolBar.tsx`（整体重写）
- Modify: `src/components/toolbar/ActionButtons.tsx`
- Modify: `src/components/toolbar/UndoRedoButtons.tsx`
- Modify: `src/components/toolbar/SearchInput.tsx`

**Interfaces:**

- Consumes: `Button`、`buttonVariants`、`Menu`、图标、Task 5 的 `menu.*` / `button.savePath` / `button.cancelAndClose`
- Produces: `ToolBar` 保持原有 props 签名**完全不变**（`onNew` … `onProfiles` 共 **16** 个回调，见下方 `ToolBarProps`），因此 `AppShell.tsx` 无需改调用处

- [ ] **Step 1: 创建 `ToolbarMenu.tsx`**

```tsx
import { useTranslation } from 'react-i18next';
import { canWriteTarget, targetForTab, useAppStore } from '@/store/app-store';
import { Menu } from '@/components/ui/Menu';

interface ToolbarMenuProps {
  onBrowse: () => void;
  onClean: () => void;
  onAnalyze: () => void;
  onProfiles: () => void;
  onLanguage: () => void;
  onDarkMode: () => void;
  onHelp: () => void;
  onCancel: () => void;
}

/** 工具栏 ⋯ 菜单：收纳低频操作。高频编辑动作留在主行。 */
export function ToolbarMenu(props: ToolbarMenuProps) {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);
  const activeTab = useAppStore((s) => s.activeTab);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const isDark = useThemeStore((s) => s.isDark);
  const target = targetForTab(activeTab);
  const canWrite = target === null ? false : canWriteTarget(isAdmin, pathCapabilities, target);

  return (
    <Menu
      triggerAriaLabel={t('menu.label')}
      groups={[
        {
          key: 'maintenance',
          label: t('menu.maintenance'),
          items: [
            // 浏览与新建同源（open directory + addPath），高频度低于新建，故收纳于此
            {
              key: 'browse',
              label: t('button.browse'),
              onSelect: props.onBrowse,
              disabled: !canWrite,
            },
            { key: 'clean', label: t('menu.clean'), onSelect: props.onClean, disabled: !canWrite },
            { key: 'analyze', label: t('menu.analyze'), onSelect: props.onAnalyze },
            { key: 'profiles', label: t('menu.profiles'), onSelect: props.onProfiles },
          ],
        },
        {
          key: 'application',
          label: t('menu.application'),
          items: [
            // 深/浅双向切换：文案随当前状态显示「将要切到」的目标
            {
              key: 'theme',
              label: isDark ? t('button.lightMode') : t('button.darkMode'),
              onSelect: props.onDarkMode,
            },
            { key: 'lang', label: t('menu.language'), onSelect: props.onLanguage },
            { key: 'help', label: t('menu.help'), onSelect: props.onHelp, shortcut: 'F1' },
          ],
        },
      ]}
      footer={{
        key: 'cancel',
        label: t('button.cancelAndClose'),
        onSelect: props.onCancel,
        danger: true,
      }}
    />
  );
}
```

> **注意**：需在文件顶部增加 `import { useThemeStore } from '@/store/theme-store';`。
> **说明**：既有 `button.darkMode`（「深色模式」）/ `button.lightMode`（「浅色模式」）两个 key 保留复用，**不新增** `menu.darkMode`。因此 Task 5 无需声明 `menu.darkMode`。

- [ ] **Step 2: 重写 `SearchInput.tsx`**

```tsx
import { useRef, useEffect } from 'react';
import { useAppStore } from '@/store/app-store';
import { useTranslation } from 'react-i18next';

export function SearchInput() {
  const { t } = useTranslation();
  const searchQuery = useAppStore((s) => s.searchQuery);
  const setSearchQuery = useAppStore((s) => s.setSearchQuery);
  const inputRef = useRef<HTMLInputElement>(null);

  // Ctrl+F 聚焦搜索框
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === 'f') {
        e.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  return (
    <input
      ref={inputRef}
      type="text"
      placeholder={t('dialog.search')}
      value={searchQuery}
      onChange={(e) => setSearchQuery(e.target.value)}
      aria-label={t('dialog.search')}
      className="toolbar-search"
    />
  );
}
```

在 `globals.css` 追加：

```css
.toolbar-search {
  height: 28px;
  width: 160px;
  flex: 0 0 auto;
  padding: 0 10px;
  border: 1px solid var(--app-border);
  border-radius: var(--radius);
  background: var(--app-surface);
  color: var(--app-fg);
  font-size: 12px;
  outline: none;
}

.toolbar-search::placeholder {
  color: var(--app-fg-faint);
}

.toolbar-search:focus {
  border-color: var(--app-accent);
}
```

- [ ] **Step 3: 重写 `ActionButtons.tsx`**

```tsx
import { useTranslation } from 'react-i18next';
import { canWriteTarget, targetForTab, useAppStore } from '@/store/app-store';
import { Button } from '@/components/ui/Button';

interface ActionButtonsProps {
  onNew: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
}

/**
 * 主行编辑组 + 排序组。
 * `onBrowse` 已移入 ⋯ 菜单（与「新建」同源，低频），`onClean` 同理。
 */
export function ActionButtons({
  onNew,
  onEdit,
  onDelete,
  onMoveUp,
  onMoveDown,
}: ActionButtonsProps) {
  const { t } = useTranslation();
  const activeTab = useAppStore((s) => s.activeTab);
  const isAdmin = useAppStore((s) => s.isAdmin);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const target = targetForTab(activeTab);
  const disabled = target === null || !canWriteTarget(isAdmin, pathCapabilities, target);

  return (
    <>
      <Button disabled={disabled} onClick={onNew}>
        {t('button.new')}
      </Button>
      <Button disabled={disabled} onClick={onEdit}>
        {t('button.edit')}
      </Button>
      <Button variant="danger" disabled={disabled} onClick={onDelete}>
        {t('button.delete')}
      </Button>
      <span className="toolbar-sep" aria-hidden="true" />
      <Button
        variant="ghost"
        disabled={disabled}
        onClick={onMoveUp}
        aria-label={t('button.moveUp')}
      >
        {t('button.moveUp')}
      </Button>
      <Button
        variant="ghost"
        disabled={disabled}
        onClick={onMoveDown}
        aria-label={t('button.moveDown')}
      >
        {t('button.moveDown')}
      </Button>
    </>
  );
}
```

> **注意**：`onBrowse` / `onClean` 从 `ActionButtonsProps` 中**移除**，改由 `ToolbarMenu` 消费。`ToolBar` 仍接收这两个 prop 并传给 `ToolbarMenu`。

- [ ] **Step 4: 重写 `UndoRedoButtons.tsx`**

```tsx
import { useTranslation } from 'react-i18next';
import { canWriteTarget, targetForTab, useAppStore } from '@/store/app-store';
import { Button } from '@/components/ui/Button';

export function UndoRedoButtons() {
  const { t } = useTranslation();
  const activeTab = useAppStore((s) => s.activeTab);
  const isAdmin = useAppStore((s) => s.isAdmin);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const undoRedo = useAppStore((s) => s.undoRedo);
  const target = targetForTab(activeTab);
  const canWrite = target === null ? false : canWriteTarget(isAdmin, pathCapabilities, target);
  // 订阅路径数组长度变化，确保 undoRedo 内部状态变化时触发重渲染
  useAppStore((s) => s.sysPaths.length);
  useAppStore((s) => s.userPaths.length);
  const undo = useAppStore((s) => s.undo);
  const redo = useAppStore((s) => s.redo);

  return (
    <>
      <Button variant="ghost" disabled={!canWrite || !undoRedo.canUndo()} onClick={undo}>
        {t('button.undo')}
      </Button>
      <Button variant="ghost" disabled={!canWrite || !undoRedo.canRedo()} onClick={redo}>
        {t('button.redo')}
      </Button>
    </>
  );
}
```

- [ ] **Step 5: 重写 `ToolBar.tsx`（单行）**

```tsx
import { useTranslation } from 'react-i18next';
import { canWriteTarget, targetForTab, useAppStore } from '@/store/app-store';
import { Button } from '@/components/ui/Button';
import { SearchInput } from './SearchInput';
import { ActionButtons } from './ActionButtons';
import { UndoRedoButtons } from './UndoRedoButtons';
import { ToolbarMenu } from './ToolbarMenu';

interface ToolBarProps {
  onNew: () => void;
  onEdit: () => void;
  onBrowse: () => void;
  onDelete: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onClean: () => void;
  onImport: () => void;
  onExport: () => void;
  onSave: () => void;
  onCancel: () => void;
  onHelp: () => void;
  onLanguage: () => void;
  onDarkMode: () => void;
  onAnalyze: () => void;
  onProfiles: () => void;
}

/** 单行分层工具栏：搜索 │ 编辑 │ 排序 │ 撤销 ...推... 批量 │ 保存 │ ⋯ */
export function ToolBar(props: ToolBarProps) {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);
  const activeTab = useAppStore((s) => s.activeTab);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const isModified = useAppStore((s) => s.isModified);
  const target = targetForTab(activeTab);
  const canWrite = target === null ? false : canWriteTarget(isAdmin, pathCapabilities, target);

  return (
    <div className="toolbar" data-testid="toolbar">
      <SearchInput />
      <span className="toolbar-sep" aria-hidden="true" />
      <ActionButtons
        onNew={props.onNew}
        onEdit={props.onEdit}
        onDelete={props.onDelete}
        onMoveUp={props.onMoveUp}
        onMoveDown={props.onMoveDown}
      />
      <span className="toolbar-sep" aria-hidden="true" />
      <UndoRedoButtons />
      <div className="flex-1" />
      <Button disabled={!canWrite} onClick={props.onImport}>
        {t('button.import')}
      </Button>
      <Button onClick={props.onExport}>{t('button.export')}</Button>
      <Button variant="primary" disabled={!canWrite} onClick={props.onSave}>
        {t('button.savePath')}
        {/* H3：用 aria-hidden 而非 aria-label —— 后者会改变按钮的可访问名称，
            使 getByRole('button', { name: '保存' }) 的精确匹配失效。 */}
        {isModified && (
          <span className="dirty-dot" aria-hidden="true" title={t('status.modified')} />
        )}
      </Button>
      <ToolbarMenu
        onBrowse={props.onBrowse}
        onClean={props.onClean}
        onAnalyze={props.onAnalyze}
        onProfiles={props.onProfiles}
        onLanguage={props.onLanguage}
        onDarkMode={props.onDarkMode}
        onHelp={props.onHelp}
        onCancel={props.onCancel}
      />
    </div>
  );
}
```

在 `globals.css` 追加：

```css
.toolbar {
  display: flex;
  align-items: center;
  gap: var(--gap);
  padding: 8px 10px;
  border-bottom: 1px solid var(--app-border);
  overflow: hidden;
}

.toolbar-sep {
  width: 1px;
  height: 18px;
  background: var(--app-border);
  flex: 0 0 auto;
}

.dirty-dot {
  display: inline-block;
  width: 5px;
  height: 5px;
  border-radius: 999px;
  background: var(--app-accent-fg);
  margin-left: 2px;
}
```

- [ ] **Step 6: 运行类型检查与既有测试**

Run: `npx tsc -b --noEmit && npx vitest run tests/unit/`
Expected: PASS（`ActionButtons` 的 props 变了，但 `AppShell` 只通过 `ToolBar` 调用；若 `ActionButtons` 有直接测试，需同步删掉 `onBrowse` / `onClean`）

- [ ] **Step 7: 目视验证 800px 不换行**

Run: `npm run dev`，把窗口拖到最窄。
Expected: 工具栏**单行**，无换行、无横向滚动条溢出。

- [ ] **Step 8: 提交**

```bash
git add src/components/toolbar/ src/styles/globals.css
git commit -m "refactor: 工具栏收口为单行分层结构"
```

---

### Task 7: 布局与表格视图迁移

**Files:**

- Modify: `src/components/layout/TitleBar.tsx`
- Modify: `src/components/layout/AppShell.tsx`
- Modify: `src/components/layout/StatusBar.tsx`
- Modify: `src/components/path-list/PathTable.tsx`
- Modify: `src/components/path-list/MergePreview.tsx`
- Modify: `src/components/env-list/EnvVarTable.tsx`
- Modify: `src/components/env-list/EnvVarToolbar.tsx`

**Interfaces:**

- Consumes: Task 1 的 token 层、`buttonVariants`、`.tab-pill`
- Produces: 所有范围内组件**不再引用过渡别名**

- [ ] **Step 1: 重写 `TitleBar.tsx`**

```tsx
import { useAppStore } from '@/store/app-store';
import { useTranslation } from 'react-i18next';
import { version } from '../../../package.json';

export function TitleBar() {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);

  return (
    <header className="titlebar">
      <div className="titlebar-brand">
        <span className="titlebar-logo" aria-hidden="true" />
        <h1 className="titlebar-title">{isAdmin ? t('app.name') : t('app.nameReadonly')}</h1>
      </div>
      <span className="titlebar-version">v{version}</span>
    </header>
  );
}
```

在 `globals.css` 追加：

```css
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 40px;
  padding: 0 14px;
  border-bottom: 1px solid var(--app-border);
  flex: 0 0 auto;
}

.titlebar-brand {
  display: flex;
  align-items: center;
  gap: 8px;
}

.titlebar-logo {
  width: 18px;
  height: 18px;
  border-radius: var(--radius-sm);
  background: linear-gradient(135deg, var(--app-accent), var(--app-accent-text));
}

.titlebar-title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}

.titlebar-version {
  font-size: 11px;
  color: var(--app-fg-faint);
}
```

- [ ] **Step 2: 修改 `AppShell.tsx` 的 Tab 区（约 155-169 行）**

把：

```tsx
<div className="flex border-b px-4" style={{ borderColor: 'var(--app-border)' }}>
  {tabConfig.map((tab) => (
    <button
      key={tab.id}
      onClick={() => {
        setActiveTab(tab.id);
        setSelectedIndices([]);
      }}
      className={`px-4 py-1.5 text-sm font-medium transition-colors ${activeTab === tab.id ? 'tab-active' : 'opacity-60'}`}
      style={{ color: activeTab === tab.id ? '#3b82f6' : 'var(--app-fg)' }}
    >
      {tab.label}
    </button>
  ))}
</div>
```

替换为：

```tsx
<nav className="tabs" aria-label={t('app.name')}>
  {tabConfig.map((tab) => (
    <button
      key={tab.id}
      type="button"
      data-active={activeTab === tab.id}
      onClick={() => {
        setActiveTab(tab.id);
        setSelectedIndices([]);
      }}
      className="tab-pill text-xs"
    >
      {tab.label}
    </button>
  ))}
</nav>
```

并在根节点移除内联样式（改用 CSS）：

```tsx
    <div className="app-root">
```

在 `globals.css` 追加：

```css
.app-root {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--app-bg);
  color: var(--app-fg);
}

.tabs {
  display: flex;
  gap: 2px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--app-border);
  flex: 0 0 auto;
}
```

- [ ] **Step 3: 修改 `PathTable.tsx`**

把行背景逻辑（约 120-133 行的 `textColor` / `textDecoration` / `opacity`）：

```tsx
          let textColor = 'var(--app-fg)';
          if (v.state === 'invalid') textColor = '#dc3545';
          else if (v.isDuplicate) textColor = '#fd7e14';
          // ...
          if (!enabled) {
            textColor = '#6b7280';
```

替换为**语义变量 + data 属性**，并把内联样式换成类名：

```tsx
const rowState = !enabled
  ? 'off'
  : v.state === 'invalid'
    ? 'invalid'
    : v.isDuplicate
      ? 'dup'
      : 'ok';
```

行容器改为（**H1：交替底色由逻辑索引决定，不用 `nth-child`**）：

```tsx
            <div
              key={virtualRow.key}
              data-testid="path-row"
              data-path={path}
              data-state={rowState}
              data-odd={rowIdx % 2 === 1}
              onClick={(e) => handleClick(index, e)}
              onDoubleClick={() => handleDoubleClick(index)}
              className={`path-row${isSelected ? ' is-selected' : ''}`}
              style={{
                height: `${virtualRow.size}px`,
                transform: `translateY(${virtualRow.start}px)`,
              }}
            >
```

> **为什么不用 `nth-child(odd)`**（H1）：虚拟滚动只渲染可视窗口内的行，`nth-child` 的奇偶取决于**当前渲染了哪些节点**，滚动时判定会跳变导致行色闪烁。必须用**逻辑索引** `rowIdx % 2`。

路径文字改为：

```tsx
<div data-testid="path-text" className="path-text" title={expandedCache.get(path) || undefined}>
  {path}
</div>
```

表头改为使用类名 `path-table-head`，去掉内联 `backgroundColor`。

在 `globals.css` 追加：

```css
.path-row {
  position: absolute;
  top: 0;
  left: 0;
  display: flex;
  align-items: center;
  width: 100%;
  cursor: pointer;
  border-bottom: 1px solid transparent;
}

.path-row[data-odd='true'] {
  background: var(--app-row-odd);
}

.path-row:hover {
  background: var(--app-hover);
}

.path-row.is-selected {
  background: var(--app-row-selected);
}

.path-text {
  flex: 1;
  padding: 0 8px;
  font-family: ui-monospace, Consolas, monospace;
  font-size: 12px;
  color: var(--app-fg);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.path-row[data-state='invalid'] .path-text {
  color: var(--app-danger);
}

.path-row[data-state='dup'] .path-text {
  color: var(--app-warning);
}

.path-row[data-state='off'] .path-text {
  color: var(--app-disabled-fg);
  text-decoration: line-through;
}

.path-table-head {
  position: sticky;
  top: 0;
  z-index: 10;
  display: flex;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--app-fg-faint);
  background: var(--app-surface);
  border-bottom: 1px solid var(--app-border);
}
```

- [ ] **Step 4: 修改 `MergePreview.tsx` / `EnvVarTable.tsx` / `EnvVarToolbar.tsx` / `StatusBar.tsx`**

将文件内所有 `var(--app-list-bg)` → `var(--app-surface)`，`var(--app-list-alt)` → `var(--app-row-odd)`，`var(--app-select-row)` → `var(--app-row-selected)`，硬编码色值 → 语义变量。

**范围内应当清空的硬编码色（含 H2 补全的 `#6b7280`）**：

| 色值      | 位置                                       | 替换为                                      |
| --------- | ------------------------------------------ | ------------------------------------------- |
| `#3b82f6` | `AppShell.tsx:164`                         | `var(--app-accent)`（Task 7 Step 2 已处理） |
| `#dc3545` | `PathTable.tsx:121`                        | `var(--app-danger)`（Step 3 已处理）        |
| `#fd7e14` | `PathTable.tsx:122`                        | `var(--app-warning)`（Step 3 已处理）       |
| `#6b7280` | `PathTable.tsx:129`、`MergePreview.tsx:66` | `var(--app-disabled-fg)`                    |

验证命令（应无输出）：

```bash
grep -rn -- '--app-list\|--app-select-row\|#3b82f6\|#2563eb\|#dc3545\|#fd7e14\|#6b7280' \
  src/components/layout src/components/path-list src/components/env-list
```

> **范围外保留**：`src/components/dialogs/` 下的硬编码色（`AnalyzeDialog`、`EditEnvVarDialog`、`HelpDialog`、`NewEnvVarDialog`、`PathEditDialog`、`ProfileDetail`、`ProfileList`、`ProfileDialog`、`EnvBackupPanel`）属于 R7，**本波不动**。

- [ ] **Step 5: 运行类型检查与测试**

Run: `npx tsc -b --noEmit && npx vitest run`
Expected: PASS

- [ ] **Step 6: 目视验证两套主题**

Run: `npm run dev`
Expected:

- 浅色：近白背景、细边框、靛蓝 Tab pill
- 深色（点击 ⋯ → 深色模式）：深色渐变背景、半透明菜单带毛玻璃、无效路径红色、重复路径橙色、禁用路径灰色删除线
- **对话框仍可正常打开**（如 Ctrl+N 或「配置」），**输入框不是透明底**

- [ ] **Step 7: 提交**

```bash
git add src/components/layout/ src/components/path-list/ src/components/env-list/ src/styles/globals.css
git commit -m "style: 布局与表格视图迁移至新 token 层"
```

---

### Task 8: E2E 同步

**Files:**

- Modify: `e2e/tests/analyze.spec.ts:12`
- Modify: `e2e/tests/profiles.spec.ts:11`
- Modify: `e2e/tests/search-clean.spec.ts:45`
- Modify: `e2e/tests/toggle-save.spec.ts:19`

**Interfaces:**

- Consumes: Task 4 的 `data-testid="menu-panel"`、Task 5 的 `menu.*` 文案、Task 6 的 `button.savePath`

- [ ] **Step 1: 改 `toggle-save.spec.ts:19`**

```ts
// 点击工具栏「保存」
await page.getByRole('button', { name: '保存' }).click();
```

- [ ] **Step 2: 改 `analyze.spec.ts:12`**

```ts
await page.getByRole('button', { name: '更多操作' }).click();
await page.getByRole('menuitem', { name: '冲突分析' }).click();
```

- [ ] **Step 3: 改 `profiles.spec.ts:11`**

```ts
await page.getByRole('button', { name: '更多操作' }).click();
await page.getByRole('menuitem', { name: '配置方案' }).click();
```

- [ ] **Step 4: 改 `search-clean.spec.ts:45`**

```ts
await page.getByRole('button', { name: '更多操作' }).click();
await page.getByRole('menuitem', { name: '清理无效路径' }).click();
```

- [ ] **Step 5: 运行 E2E**

Run: `npm run test:e2e`
Expected: **全部 10 个 spec 通过**（`env-vars.spec.ts` 与 `startup.spec.ts` 无需改动即应通过 —— v3 裁断确认它们不受影响）

- [ ] **Step 6: 提交**

```bash
git add e2e/tests/
git commit -m "test(e2e): 适配工具栏菜单与保存文案"
```

---

### Task 9: 视觉回归与质量门

**Files:**

- Create: `e2e/tests/visual.spec.ts`

**Interfaces:**

- Consumes: 全部前序 Task

- [ ] **Step 1: 创建视觉回归测试**

```ts
import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

const SIZES = [
  { name: '800x600', width: 800, height: 600 },
  { name: '1200x800', width: 1200, height: 800 },
];

/**
 * 检测工具栏是否被裁切（B2）。
 *
 * **不能用「工具栏高度 < 48」或「documentElement 溢出 ≤ 0」**：
 * Task 6 给 .toolbar 写了 `overflow: hidden`，内容超出时会被裁在工具栏内部，
 * 这两条断言都会恒真 —— 实测撑爆 1618px 仍全部通过。
 *
 * 正确判据：**.toolbar 自身**的 scrollWidth 与 clientWidth 之差。
 * 裁切时 scrollWidth > clientWidth；放得下时二者相等。
 */
async function toolbarClip(page: import('@playwright/test').Page): Promise<number> {
  return page.getByTestId('toolbar').evaluate((el) => el.scrollWidth - el.clientWidth);
}

for (const size of SIZES) {
  for (const theme of ['light', 'dark'] as const) {
    test(`工具栏在 ${size.name} / ${theme} 下不被裁切`, async ({ page }) => {
      await page.setViewportSize({ width: size.width, height: size.height });
      await page.addInitScript(createIpcMock());
      if (theme === 'dark') {
        await page.addInitScript(() => localStorage.setItem('darkMode', '1'));
      }
      await page.goto('/');

      const toolbar = page.getByTestId('toolbar');
      await expect(toolbar).toBeVisible();

      // 没有被裁切的像素：内容宽度必须 ≤ 容器宽度
      expect(await toolbarClip(page)).toBeLessThanOrEqual(0);

      // 工具栏保持单行（高度不超过两倍行高）
      const box = await toolbar.boundingBox();
      expect(box!.height).toBeLessThan(48);
    });
  }
}

test('800px 下英文工具栏同样不被裁切', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.addInitScript(createIpcMock());
  await page.addInitScript(() => localStorage.setItem('i18nextLng', 'en'));
  await page.goto('/');

  await expect(page.getByTestId('toolbar')).toBeVisible();
  expect(await toolbarClip(page)).toBeLessThanOrEqual(0);

  const box = await (await page.getByTestId('toolbar')).boundingBox();
  expect(box!.height).toBeLessThan(48);
});

test('裁切断言本身可证伪 —— 注入超宽内容时必须失败', async ({ page }) => {
  // 证明上面的断言不是恒真：故意把第 3 组按钮宽度撑爆，断言必须变红。
  await page.setViewportSize({ width: 800, height: 600 });
  await page.addInitScript(createIpcMock());
  await page.goto('/');

  await page.evaluate(() => {
    const toolbar = document.querySelector('[data-testid="toolbar"]') as HTMLElement;
    const probe = document.createElement('div');
    probe.style.width = '3000px';
    probe.style.flex = '0 0 auto';
    toolbar.appendChild(probe);
  });

  expect(await toolbarClip(page)).toBeGreaterThan(0);
});
```

> **最后一条是「证明断言可证伪」的元测试**——故意撑爆，断言**必须**失败。它保证前 5 条不是永远为真的装饰品。若这条也「通过」了，说明检测方法本身有问题。

- [ ] **Step 2: 运行视觉测试**

Run: `npx playwright test --config e2e/playwright.config.ts e2e/tests/visual.spec.ts`
Expected: PASS（6 个用例：2 尺寸 × 2 主题 + 英文 + 可证伪探针）

- [ ] **Step 3: 跑完整质量门**

Run: `npm run verify:all`
Expected: **退出码 0**。若覆盖率跌破 80%，补充 `Menu` / `Button` / `icons` 的测试覆盖。

- [ ] **Step 4: 提交**

```bash
git add e2e/tests/visual.spec.ts
git commit -m "test(e2e): 新增 800px 与双主题视觉回归"
```

---

## 自审记录

**v2（2026-10-17）：按 `PathEditor-gui-redesign计划裁断.md` 修订 11 项。**

| #   | 修订                                                                        | 来源 |
| --- | --------------------------------------------------------------------------- | ---- |
| 1   | 新增 **Task 0**：注册 `tests/setup.ts` + `vitest.config.ts` 加 `setupFiles` | B1   |
| 2   | Task 9 断言改为 `.toolbar` 内部裁切检测 + 可证伪探针                        | B2   |
| 3   | Task 5 JSON 删除 `"darkMode"` 两行（zh / en）                               | B3   |
| 4   | Task 1 测试加 `stripComments()`                                             | B4   |
| 5   | Task 7 删除 `nth-child(odd)`，改 `data-odd` + 逻辑索引                      | H1   |
| 6   | Task 7 验证命令补 `#6b7280` 并列表化                                        | H2   |
| 7   | `dirty-dot` 改 `aria-hidden` + `title`                                      | H3   |
| 8   | `btnStyle` 值保持 `var(--app-bg)` 不变                                      | H4   |
| 9   | `.tab-active` 加注释记为有意偏离                                            | H5   |
| 10  | 「17 个回调」→「16 个回调」                                                 | F1   |
| 11  | 本表                                                                        | —    |

### 1. Spec 覆盖检查

| Spec 要求                            | 对应 Task                                        |
| ------------------------------------ | ------------------------------------------------ |
| D1 双主题 token                      | Task 1                                           |
| D2 分层工具栏 + 8 动作归属           | Task 6（`浏览` 进菜单）                          |
| D3 ⋯ 菜单分组                        | Task 6 `ToolbarMenu`                             |
| D4 英文简写                          | Task 5（既有值未动，简写已在 v1 确认）           |
| D5 `savePath` 独立 key               | Task 5                                           |
| D4.1 `cancelAndClose` 独立 key       | Task 5                                           |
| 过渡别名 B1                          | Task 1                                           |
| `.tab-pill` H3                       | Task 1 + Task 7                                  |
| 死代码清理                           | Task 1                                           |
| `MergePreview` / `EnvVarTable`（V4） | Task 7                                           |
| N1 800px                             | Task 6 Step 7 + Task 9                           |
| N2 i18n 最长语言                     | Task 9                                           |
| N3 零依赖                            | Task 3                                           |
| N4 可访问性                          | Task 4（`role`/`aria`）、Task 3（`aria-hidden`） |
| N5 减少动效                          | Task 1（`prefers-reduced-motion`）               |
| E2E 4 文件（v3）                     | Task 8                                           |
| `data-testid` 保活                   | 全程（Task 7 保留原 4 个）                       |

### 2. 占位符扫描

无 TBD / TODO（除 `.css` 内计划内的 `TODO(Wave 4.1)` 注释）/ 「类似 Task N」。

### 3. 类型一致性

- `Button` 的 `ref` prop 在 Task 2 定义、Task 4 使用 —— **已在 Task 4 Step 3 明确给出更新后的签名**。
- `buttonVariants` 在 Task 2 定义，Task 4/6 通过 `Button` 间接使用。
- `MenuGroupDef` / `MenuItemDef` 在 Task 4 导出，Task 6 `ToolbarMenu` 使用。
- `ActionButtonsProps` 在 Task 6 收窄（移除 `onBrowse` / `onClean`），`ToolBar` 仍全量接收并转发 —— 已注明。
