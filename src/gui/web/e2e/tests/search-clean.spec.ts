import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(
    createIpcMock({
      load_path_snapshot: {
        system: [
          { path: 'C:\\Windows', enabled: true },
          { path: 'invalid_path', enabled: true },
          { path: 'C:\\Temp', enabled: true },
        ],
        user: [],
      },
      validate_path: false,
      clean_path_entries: [
        [
          { path: 'C:\\Windows', enabled: true },
          { path: 'C:\\Temp', enabled: true },
        ],
        [{ path: 'invalid_path', enabled: true }],
      ],
    }),
  );
  await page.goto('/');
});

test('搜索过滤后清理无效路径', async ({ page }) => {
  // 初始 3 条路径
  await page.waitForTimeout(500);
  await expect(page.getByTestId('path-row')).toHaveCount(3);

  // 搜索 "Windows"
  const searchInput = page.locator('input[placeholder]');
  await searchInput.fill('Windows');
  await page.waitForTimeout(300);
  await expect(page.getByTestId('path-row')).toHaveCount(1);

  // 清除搜索
  await searchInput.fill('');
  await page.waitForTimeout(300);
  await expect(page.getByTestId('path-row')).toHaveCount(3);

  // 打开工具栏「更多操作」菜单，点击「清理无效路径」
  await page.getByRole('button', { name: '更多操作' }).click();
  await page.getByRole('menuitem', { name: '清理无效路径' }).click();
  await page.waitForTimeout(300);
  // 清理由 Rust backend 统一执行；"invalid_path" 被移除，其余两条保留。
  await expect(page.getByTestId('path-row')).toHaveCount(2);
});
