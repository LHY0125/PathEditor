import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.waitForTimeout(500);
});

test('打开配置管理对话框', async ({ page }) => {
  // 打开工具栏「更多操作」菜单，点击「配置方案」
  await page.getByRole('button', { name: '更多操作' }).click();
  await page.getByRole('menuitem', { name: '配置方案' }).click();
  await page.waitForTimeout(500);
  await expect(page.getByText('PATH 配置文件')).toBeVisible();
});
