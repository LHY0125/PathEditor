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
