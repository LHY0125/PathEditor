import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

/**
 * 同步中心 E2E（M3 Task 10）。
 *
 * spec §3.2.1 要求 3–4 条主路径。这里覆盖四条：
 * 1. 打开同步中心 → 显示待导出数与未解决冲突数
 * 2. 导出 → 提示成功（变更条数来自返回值，不是硬编码）
 * 3. 导入遇到错误口令 → 显示错误且不崩溃
 * 4. 冲突列表 → 解决一条 → 列表真的更新（mock 会改 fixture，避免假绿）
 *
 * **全部走 mock IPC，绝不触碰真实注册表**（仓库硬约束）。
 */

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.waitForTimeout(300);
});

/** 切到「同步」标签页。 */
async function openSyncTab(page: import('@playwright/test').Page) {
  await page.getByRole('button', { name: '同步', exact: true }).click();
  await page.waitForTimeout(300);
}

/** 填入口令与路径。口令框必须接受输入但不回显。 */
async function fillCredentials(page: import('@playwright/test').Page, passphrase: string) {
  await page.getByPlaceholder('用于加解密同步包，不会被保存').fill(passphrase);
  await page.getByPlaceholder(/例如 D:/).fill('D:\\fixture\\a.pesync');
}

test('同步中心显示待导出数与未解决冲突数', async ({ page }) => {
  await page.addInitScript(() => {
    window.__syncStatusFixture = { pendingCount: 5, unresolvedConflicts: 2 };
  });
  await page.reload();
  await page.waitForTimeout(300);

  await openSyncTab(page);

  await expect(page.getByText('待导出 5 条 · 未解决冲突 2 条')).toBeVisible();
  // 「同步不写注册表」必须在界面上写明
  await expect(page.getByText(/不会改动系统 PATH/)).toBeVisible();
});

test('导出成功后提示条数，并清空口令输入', async ({ page }) => {
  await openSyncTab(page);
  await fillCredentials(page, 'correct horse');

  // 口令框必须是密码类型（不回显明文）
  await expect(page.getByPlaceholder('用于加解密同步包，不会被保存')).toHaveAttribute(
    'type',
    'password',
  );

  await page.locator('.sync-center').getByRole('button', { name: '导出', exact: true }).click();

  await expect(page.getByText(/已导出 3 条变更到/)).toBeVisible();
  // 导出后口令框清空：不在界面上留下任何凭据
  await expect(page.getByPlaceholder('用于加解密同步包，不会被保存')).toHaveValue('');
});

test('导入错误口令时显示错误且不崩溃', async ({ page }) => {
  await page.addInitScript(() => {
    window.__syncCryptoError = true;
  });
  await page.reload();
  await page.waitForTimeout(300);

  await openSyncTab(page);
  await fillCredentials(page, 'wrong');
  await page.locator('.sync-center').getByRole('button', { name: '导入', exact: true }).click();

  await expect(page.getByRole('alert')).toContainText('口令错误或同步包已被篡改');
  // 崩溃时 React 会渲染错误边界文案；这里必须仍是同步中心
  await expect(page.getByRole('heading', { name: '同步中心' })).toBeVisible();
});

test('解决一条冲突后列表更新', async ({ page }) => {
  await page.addInitScript(() => {
    window.__syncStatusFixture = { pendingCount: 0, unresolvedConflicts: 2 };
    window.__syncConflictsFixture = [
      {
        id: 7,
        entityType: 'profile',
        entityUid: 'p-1',
        field: 'name',
        localValue: '"本地"',
        remoteValue: '"远端"',
        remoteDevice: 'dev-b',
        createdAt: 1700000000000,
        resolvedAt: null,
        resolution: null,
      },
      {
        id: 8,
        entityType: 'snapshot',
        entityUid: 's-1',
        field: null,
        localValue: null,
        remoteValue: null,
        remoteDevice: 'dev-c',
        createdAt: 1700000000001,
        resolvedAt: null,
        resolution: null,
      },
    ];
  });
  await page.reload();
  await page.waitForTimeout(300);

  await openSyncTab(page);

  // 两条冲突、按实体类型分组
  await expect(page.getByRole('heading', { name: '配置方案' })).toBeVisible();
  await expect(page.getByRole('heading', { name: '快照' })).toBeVisible();
  await expect(
    page.locator('.sync-center').getByRole('button', { name: '采纳远端', exact: true }),
  ).toHaveCount(2);

  // 解决第一条（配置方案）
  await page
    .locator('.sync-center')
    .getByRole('button', { name: '采纳远端', exact: true })
    .first()
    .click();
  await page.waitForTimeout(300);

  // mock 把该条标成已解决，refresh 后未解决列表只剩一条
  await expect(
    page.locator('.sync-center').getByRole('button', { name: '采纳远端', exact: true }),
  ).toHaveCount(1);
  await expect(page.getByText(/已按 remote 解决/)).toBeVisible();
});
