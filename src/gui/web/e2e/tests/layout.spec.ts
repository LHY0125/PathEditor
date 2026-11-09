import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

/**
 * 信息架构回归：工具栏与 Tab 的绑定关系。
 *
 * PATH 编辑动作只属于 system/user；merged 只读；history/search/stats/sync
 * 各自拥有操作区。此前整条 PATH 工具栏在所有 Tab 上常驻，搜索页会出现
 * 两个搜索框，历史/统计页出现无效的「新建/保存」按钮。
 */

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.waitForTimeout(300);
});

test('非 PATH 标签不再显示 PATH 工具栏', async ({ page }) => {
  for (const name of ['合并预览', '历史记录', '搜索', '统计', '同步']) {
    await page.getByRole('tab', { name, exact: true }).click();
    await page.waitForTimeout(150);
    await expect(page.getByTestId('toolbar')).toHaveCount(0);
    await expect(page.getByPlaceholder('搜索路径…')).toHaveCount(0);
  }

  // 回到 PATH 后工具栏恢复
  await page.getByRole('tab', { name: '系统 PATH' }).click();
  await expect(page.getByTestId('toolbar')).toBeVisible();
});

test('合并预览只读：表头是「路径」而不是输入提示', async ({ page }) => {
  await page.getByRole('tab', { name: '合并预览', exact: true }).click();
  await page.waitForTimeout(200);

  const head = page.locator('.path-table-head').first();
  await expect(head).toContainText('路径');
  await expect(head).not.toContainText('请输入路径');
  await expect(page.getByText(/合并预览为只读/)).toBeVisible();
});

test('对话框统一提供关闭按钮', async ({ page }) => {
  await page.getByRole('button', { name: '新建', exact: true }).click();
  const close = page.locator('.modal-close');
  await expect(close).toBeVisible();
  await close.click();
  await expect(page.locator('.modal-surface')).toHaveCount(0);
});
