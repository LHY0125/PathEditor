import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
});

test('启动后加载系统 PATH 和用户 PATH', async ({ page }) => {
  // 系统 tab 默认激活，显示 2 条路径
  await expect(page.getByTestId('path-row')).toHaveCount(2);

  // 切换到用户 tab
  await page.click('text=用户变量');
  await page.waitForTimeout(300);
  await expect(page.getByTestId('path-row')).toHaveCount(1);
});

test('普通用户可编辑用户 PATH，系统 tab 保持只读', async ({ page }) => {
  await page.addInitScript(
    createIpcMock({
      get_path_capabilities: {
        canReadSystem: true,
        canWriteSystem: false,
        canReadUser: true,
        canWriteUser: true,
      },
    }),
  );
  await page.goto('/');

  await expect(page.getByRole('button', { name: '新建' })).toBeDisabled();
  await page.getByRole('button', { name: '用户变量' }).click();
  await expect(page.getByRole('button', { name: '新建' })).toBeEnabled();
});
