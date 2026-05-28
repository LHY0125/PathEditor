import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.waitForTimeout(500);
});

test('打开分析对话框查看冲突和工具', async ({ page }) => {
  // 点击分析按钮
  await page.click('text=分析');
  await page.waitForTimeout(500);

  // 应显示冲突和工具两个标签
  await expect(page.locator('text=冲突检测')).toBeVisible();
  await expect(page.locator('text=工具清单')).toBeVisible();
});
