import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.waitForTimeout(500);
});

test('打开分析对话框查看冲突和工具', async ({ page }) => {
  // 打开工具栏「更多操作」菜单，点击「冲突分析」
  await page.getByRole('button', { name: '更多操作' }).click();
  await page.getByRole('menuitem', { name: '冲突分析' }).click();
  await page.waitForTimeout(500);

  // 应显示冲突和工具两个标签
  await expect(page.locator('text=冲突检测')).toBeVisible();
  await expect(page.locator('text=工具清单')).toBeVisible();
});
