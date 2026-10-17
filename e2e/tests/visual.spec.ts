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
  // 前置断言：必须先确认真的是英文界面，否则后面的裁切检查测的是中文布局（假绿）。
  await expect(page.getByTestId('toolbar')).toContainText('Save');
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
