import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.waitForTimeout(500);
});

test('打开配置管理对话框', async ({ page }) => {
  await page.click('text=配置');
  await page.waitForTimeout(500);
  await expect(page.locator('text=保存当前配置')).toBeVisible();
});
