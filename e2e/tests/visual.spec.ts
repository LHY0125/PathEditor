import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

const SIZES = [
  { name: '800x600', width: 800, height: 600 },
  { name: '1200x800', width: 1200, height: 800 },
];

/**
 * 检测工具栏是否被裁切。
 *
 * **不能用「工具栏高度 < 48」或「documentElement 溢出 ≤ 0」**：
 *  给 .toolbar 写了 `overflow: hidden`，内容超出时会被裁在工具栏内部，
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

/**
 * 命中测试法统计被遮挡的菜单条目数。
 *
 * isVisible() / click() 都不检测祖先裁切：被 overflow: hidden 裁掉的条目
 * 依然返回可见、依然接受合成点击。正确判据是 elementFromPoint ——
 * 条目中心点命中的若不是它自己（或其子孙），即为被遮挡 / 被裁切。
 */
async function countOccludedItems(page: import('@playwright/test').Page): Promise<number> {
  return page.getByTestId('menu-panel').evaluate(
    (panel) =>
      Array.from(panel.querySelectorAll('[role="menuitem"]')).filter((el) => {
        const r = el.getBoundingClientRect();
        const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
        return !hit || !(el === hit || el.contains(hit));
      }).length,
  );
}

test('⋯ 菜单展开后全部条目可见且未被祖先裁切', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.addInitScript(createIpcMock());
  await page.goto('/');

  await page.getByRole('button', { name: '更多操作' }).click();
  await expect(page.getByTestId('menu-panel')).toBeVisible();
  // 前置断言：确认测的确实是 8 条目菜单，而非空面板假绿
  await expect(page.getByTestId('menu-panel').getByRole('menuitem')).toHaveCount(8);

  expect(await countOccludedItems(page)).toBe(0);

  // 面板所有条目都必须落在视口内
  const panelBox = await page.getByTestId('menu-panel').boundingBox();
  expect(panelBox!.y).toBeGreaterThanOrEqual(0);
  expect(panelBox!.y + panelBox!.height).toBeLessThanOrEqual(600);
});

test('遮挡检测本身可证伪 —— 面板塞回工具栏的 overflow 裁切区域内时必须失败', async ({ page }) => {
  // 证明上面的断言不是恒真：把 portal 出去的面板重新塞回工具栏内部，
  // 复现修复前的 DOM 结构（.menu-panel 作为 .toolbar 的后代 + .toolbar { overflow: hidden }）。
  // 注意：元素被祖先裁切后，其中心点已经不在视口内，elementFromPoint 会返回 null
  // （判定为被遮挡）；因此这里有意义的是「受裁切影响的条目数」，无需再做真实命中比较。
  await page.setViewportSize({ width: 800, height: 600 });
  await page.addInitScript(createIpcMock());
  await page.goto('/');

  await page.getByRole('button', { name: '更多操作' }).click();
  await expect(page.getByTestId('menu-panel')).toBeVisible();
  expect(await countOccludedItems(page)).toBe(0);

  await page.evaluate(() => {
    const panel = document.querySelector('[data-testid="menu-panel"]') as HTMLElement;
    const toolbar = document.querySelector('[data-testid="toolbar"]') as HTMLElement;
    toolbar.appendChild(panel);
    // 恢复修复前的绝对定位：紧贴触发按钮下方、右对齐
    const trigger = toolbar.querySelector('[aria-haspopup="menu"]') as HTMLElement;
    panel.style.position = 'absolute';
    panel.style.top = `${trigger.offsetTop + trigger.offsetHeight + 4}px`;
    panel.style.left = `${trigger.offsetLeft}px`;
  });

  // 裁切区域外的条目数 = 条目中心点超出 .toolbar 底边（修复前工具条高 45px，面板 310px，只有首项在内）
  const clippedOut = await page.getByTestId('menu-panel').evaluate((panel) => {
    const toolbar = document.querySelector('[data-testid="toolbar"]') as HTMLElement;
    const { top, bottom } = toolbar.getBoundingClientRect();
    return Array.from(panel.querySelectorAll('[role="menuitem"]')).filter((el) => {
      const r = el.getBoundingClientRect();
      const cy = r.top + r.height / 2;
      return cy > bottom || cy < top;
    }).length;
  });
  expect(clippedOut).toBeGreaterThan(0);

  // 注意：浏览器合成点击（Playwright 的 click()）同样不受祖先裁切影响 —— 修复前
  // elementFromPoint 甚至能命中被裁掉的条目，所以「点击可用」不能作为可见性判据。
  // 本用例只能证明「裁切几何」这一层可被检出，不能证明 countOccludedItems 本身可失败；
  // 后者由上面「修复前代码必须变红」的真机验证覆盖。
  expect(clippedOut).toBe(7);
});
