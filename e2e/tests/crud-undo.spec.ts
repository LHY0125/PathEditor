import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
});

test('添加路径后可撤销和重做', async ({ page }) => {
  // 点击"新建"按钮
  await page.click('text=新建');
  // 填入路径（对话框内自动聚焦的 input）
  await page.locator('.fixed.inset-0 input[type="text"]').fill('C:\\\\NewPath');
  // 对话框确认按钮是"确认"
  await page.click('text=确认');
  // 路径应出现在列表中
  await page.waitForTimeout(300);
  await expect(page.locator('text=C:\\\\NewPath')).toBeVisible();

  // Ctrl+Z 撤销
  await page.keyboard.press('Control+z');
  await page.waitForTimeout(300);
  await expect(page.locator('text=C:\\\\NewPath')).not.toBeVisible();

  // Ctrl+Y 重做
  await page.keyboard.press('Control+y');
  await page.waitForTimeout(300);
  await expect(page.locator('text=C:\\\\NewPath')).toBeVisible();
});
