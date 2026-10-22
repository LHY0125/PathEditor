import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

/**
 * 搜索面板 E2E。
 *
 *  要求 3–4 条主路径。这里覆盖两条：
 * 1. 输入后出结果，且**高亮**来自 Rust 侧的 snippet（`[命中]` → `<mark>`）
 * 2. 空查询不发请求（`__capturedCalls` 里不得出现 `search`）
 *
 * **全部走 mock IPC，绝不触碰真实注册表**（仓库硬约束）。
 */

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.waitForTimeout(300);
});

/** 切到「搜索」标签页。
 *
 * **必须用 exact + 限定在标签栏内**：`搜索` 同时是标签页名和提交按钮名，
 * 裸 `getByRole('button', { name: '搜索' })` 会命中两个元素（strict mode 报错）。
 */
async function openSearchTab(page: import('@playwright/test').Page) {
  await page.getByRole('button', { name: '搜索', exact: true }).first().click();
  await page.waitForTimeout(200);
}

/** 提交搜索：在输入框按 Enter。
 *
 * 用 Enter 而不是点按钮，是为了回避「搜索」这个词同时出现在标签页与按钮上
 * 造成的选择器歧义——表单的 onSubmit 与按钮点击走同一条路径。
 */
async function submitSearch(page: import('@playwright/test').Page) {
  await page.getByRole('searchbox').press('Enter');
  await page.waitForTimeout(300);
}

test('输入关键字后显示结果，命中处高亮', async ({ page }) => {
  await page.addInitScript(() => {
    window.__searchFixture = [
      {
        kind: 'path',
        refUid: 'snap-1#system',
        // Rust 侧 FTS5 snippet() 的输出格式：命中处用 [] 包裹
        snippet: 'C:\\Program Files\\Java\\[jdk21]\\bin',
        rank: -1.5,
      },
    ];
  });
  await page.reload();
  await page.waitForTimeout(300);

  await openSearchTab(page);
  await page.getByRole('searchbox').fill('jdk21');
  await submitSearch(page);

  // 结果行出现，且命中词被 <mark> 包裹
  const mark = page.locator('mark', { hasText: 'jdk21' });
  await expect(mark).toBeVisible();
  // 前后文也在（说明渲染的是完整 snippet，不是只显示命中词）
  await expect(page.getByText(/Program Files/)).toBeVisible();
});

test('空查询不发起 IPC 请求', async ({ page }) => {
  await openSearchTab(page);

  // 输入纯空白后点搜索
  await page.getByRole('searchbox').fill('   ');
  await submitSearch(page);

  // 断言的是「没有发生调用」——这是行为断言，不是元层声明
  const calls = await page.evaluate(
    () => (window as unknown as { __capturedCalls?: Array<{ cmd: string }> }).__capturedCalls ?? [],
  );
  const searchCalls = calls.filter((c) => c.cmd === 'search');
  expect(searchCalls).toHaveLength(0);
});

test('降级模式下搜索面板显示不可用', async ({ page }) => {
  await page.addInitScript(() => {
    window.__m2Degraded = true;
  });
  await page.reload();
  await page.waitForTimeout(300);

  await openSearchTab(page);
  await page.getByRole('searchbox').fill('jdk21');
  await submitSearch(page);

  await expect(page.getByText(/降级模式/)).toBeVisible();
});
