import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

/**
 * 历史面板 E2E。
 *
 *  测试要求：「E2E 只覆盖 3–4 条主路径（历史面板、恢复、配置应用）」。
 * 缺口清单指出首版 `grep -rln history e2e/` 零命中——历史面板没有任何
 * 端到端证据，只有 store 与 parse 的单元测试。
 *
 * 三条覆盖：
 * 1. 空态正确（不是错误、不是降级）
 * 2. 降级模式 → 常驻条幅可见
 * 3. 有事件且带 snapshotUid → 点击展开详情
 *
 * **全部走 mock IPC，绝不触碰真实注册表**（仓库硬约束）。
 */

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.waitForTimeout(300);
});

/** 切到「历史记录」标签页。 */
async function openHistoryTab(page: import('@playwright/test').Page) {
  await page.getByRole('tab', { name: '历史记录' }).click();
  await page.waitForTimeout(300);
}

test('历史面板空态：显示「还没有历史记录」而不是错误', async ({ page }) => {
  // 默认 mock 返回 { items: [], total: 0 }
  await openHistoryTab(page);

  await expect(page.getByText('还没有历史记录')).toBeVisible();
  // 空态不是错误：不应出现降级条幅
  await expect(page.getByText('历史不可用（降级模式）')).toHaveCount(0);
});

test('降级模式：常驻条幅在历史面板内可见', async ({ page }) => {
  // 让 history_list_events 抛 code=database
  await page.addInitScript(() => {
    window.__historyDegraded = true;
  });
  await page.reload();
  await page.waitForTimeout(300);

  await openHistoryTab(page);

  // 条幅文案来自 i18n 的 history.unavailable
  const banner = page.getByRole('alert').filter({ hasText: '历史不可用（降级模式）' });
  await expect(banner).toBeVisible();
  // 必须说明「编辑不受影响」——这是 fail-open 的用户可见承诺
  await expect(banner).toContainText('环境变量编辑不受影响');
});

test('有事件时可展开快照详情，敏感值仍被掩码', async ({ page }) => {
  await page.addInitScript(() => {
    window.__historyPageFixture = {
      items: [
        {
          uid: 'evt-1',
          createdAt: Date.now(),
          source: 'gui',
          action: 'save',
          hive: 'user',
          target: 'Path',
          result: 'success',
          snapshotUid: 'snap-1',
        },
      ],
      total: 1,
    };
    window.__snapshotFixture = {
      uid: 'snap-1',
      createdAt: Date.now(),
      kind: 'history',
      system: [{ path: 'C:\\Before', enabled: true }],
      user: [],
      env: [
        {
          hive: 'user',
          name: 'MY_TOKEN',
          kind: 'String',
          // Rust 侧已掩码：reveal=false 时 value 恒为 null
          value: null,
          revision: 'r1',
          sensitive: true,
          restorable: true,
        },
      ],
    };
  });
  await page.reload();
  await page.waitForTimeout(300);

  await openHistoryTab(page);

  // 事件行出现，且按钮可点（snapshotUid 非 null）
  const eventButton = page.getByRole('button', { name: /保存 PATH/ });
  await expect(eventButton).toBeEnabled();
  await eventButton.click();
  await page.waitForTimeout(300);

  // 详情区展开：显示快照的 PATH 条目
  await expect(page.getByText('C:\\Before')).toBeVisible();
  // 敏感变量显示圆点而不是明文
  await expect(page.getByText('••••••')).toBeVisible();
  await expect(page.getByText('已掩码')).toBeVisible();
  // 恢复按钮存在
  await expect(page.getByRole('button', { name: '恢复到此快照' })).toBeVisible();
});
