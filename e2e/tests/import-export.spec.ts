import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.waitForTimeout(500);
});

test('导出按钮可见', async ({ page }) => {
  await expect(page.locator('text=导出')).toBeVisible();
});

test('导入按钮可见', async ({ page }) => {
  await expect(page.locator('text=导入')).toBeVisible();
});
