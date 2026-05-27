import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
});

test('禁用路径后灰显并保存', async ({ page }) => {
  // 点击第一个路径的 checkbox 将其禁用
  const checkbox = page.locator('table tbody tr').first().locator('input[type="checkbox"]');
  await checkbox.click();
  await page.waitForTimeout(300);

  // 路径文本应有删除线样式（第 3 列是路径列，nth(2) 即 0-indexed 第 3 个 td）
  const row = page.locator('table tbody tr').first();
  await expect(row.locator('td').nth(2)).toHaveCSS('text-decoration-line', 'line-through');

  // 点击"确定"保存
  await page.click('text=确定');
  await page.waitForTimeout(500);
  // 状态栏应显示"保存成功"
  await expect(page.locator('text=保存成功')).toBeVisible();
});
