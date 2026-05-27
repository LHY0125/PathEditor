import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
});

test('启动后加载系统 PATH 和用户 PATH', async ({ page }) => {
  // 系统 tab 默认激活，显示 2 条路径
  await expect(page.locator('table tbody tr')).toHaveCount(2);

  // 切换到用户 tab
  await page.click('text=用户变量');
  await page.waitForTimeout(300);
  await expect(page.locator('table tbody tr')).toHaveCount(1);
});
