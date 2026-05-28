import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.waitForTimeout(500);
});

test('Ctrl+N 打开新建对话框', async ({ page }) => {
  await page.keyboard.press('Control+n');
  await page.waitForTimeout(300);
  await expect(page.locator('.fixed.inset-0 input[type="text"]')).toBeVisible();
});

test('Ctrl+F 聚焦搜索框', async ({ page }) => {
  await page.keyboard.press('Control+f');
  const searchInput = page.locator('input[placeholder]');
  await expect(searchInput).toBeFocused();
});

test('F1 打开帮助', async ({ page }) => {
  await page.keyboard.press('F1');
  await page.waitForTimeout(300);
  await expect(page.locator('text=快捷键')).toBeVisible();
});

test('Delete 删除选中行', async ({ page }) => {
  // 先选中第一行
  await page.locator('table tbody tr').first().click();
  await page.keyboard.press('Delete');
  await page.waitForTimeout(300);
  // 应有 1 行被删除 (原 2 行剩 1 行)
  await expect(page.locator('table tbody tr')).toHaveCount(1);
});
