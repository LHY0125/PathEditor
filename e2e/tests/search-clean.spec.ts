import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(
    createIpcMock({
      load_system_paths: ['C:\\Windows', 'invalid_path', 'C:\\Temp'],
      load_user_paths: [],
      validate_path: false,
    }),
  );
  await page.goto('/');
});

test('搜索过滤后清理无效路径', async ({ page }) => {
  // 初始 3 条路径
  await page.waitForTimeout(500);
  await expect(page.locator('table tbody tr')).toHaveCount(3);

  // 搜索 "Windows"
  const searchInput = page.locator('input[placeholder]');
  await searchInput.fill('Windows');
  await page.waitForTimeout(300);
  await expect(page.locator('table tbody tr')).toHaveCount(1);

  // 清除搜索
  await searchInput.fill('');
  await page.waitForTimeout(300);
  await expect(page.locator('table tbody tr')).toHaveCount(3);

  // 点击"一键清理"按钮
  await page.click('text=一键清理');
  await page.waitForTimeout(300);
  // is_valid_path_format 只校验格式，不检查存在性
  // "invalid_path" 格式无效被移除，C:\Windows 和 C:\Temp 格式有效保留
  await expect(page.locator('table tbody tr')).toHaveCount(2);
});
