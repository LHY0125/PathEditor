import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

/**
 * 统计面板 E2E（Task 13）。
 *
 * 覆盖两条：
 * 1. 面板显示**当前状态**与**历史序列**两块（M-1 的口径必须分开）
 * 2. 降级模式下显示不可用
 *
 * **全部走 mock IPC，绝不触碰真实注册表**。
 */

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.waitForTimeout(300);
});

async function openStatsTab(page: import('@playwright/test').Page) {
  await page.getByRole('button', { name: '统计' }).click();
  await page.waitForTimeout(300);
}

test('统计面板分开显示「当前状态」与「历史序列」', async ({ page }) => {
  await page.addInitScript(() => {
    window.__statsFixture = {
      days: [{ day: '2026-10-21', pathItems: 5, envItems: 2, events: 7, conflicts: 1 }],
      topTargets: [{ target: 'Path', count: 7 }],
      profileApplies: [{ name: 'Work', applyCount: 3, lastAppliedAt: 1700000000000 }],
      currentAvgLen: 42.5,
      currentPathItems: 5,
    };
  });
  await page.reload();
  await page.waitForTimeout(300);

  await openStatsTab(page);

  // 当前状态块：条目数 + 平均长度
  await expect(page.getByText('当前状态')).toBeVisible();
  await expect(page.getByText('42.5')).toBeVisible();
  // 口径说明必须存在——否则用户会把「今天最后一次保存」当成「现在」
  await expect(page.getByText(/与下方历史序列口径不同/)).toBeVisible();

  // 历史序列块：日期与事件数
  await expect(page.getByText('最近 30 天')).toBeVisible();
  await expect(page.getByText('2026-10-21')).toBeVisible();
  await expect(page.getByText('7 次操作')).toBeVisible();

  // 聚合榜
  await expect(page.getByText('最常改变的变量')).toBeVisible();
  await expect(page.getByText('配置应用次数')).toBeVisible();
});

test('降级模式下统计面板显示不可用', async ({ page }) => {
  await page.addInitScript(() => {
    window.__m2Degraded = true;
  });
  await page.reload();
  await page.waitForTimeout(300);

  await openStatsTab(page);

  await expect(page.getByText(/降级模式/)).toBeVisible();
});

test('无数据时显示「暂无数据」而不是空白', async ({ page }) => {
  await page.addInitScript(() => {
    window.__statsFixture = {
      days: [],
      topTargets: [],
      profileApplies: [],
      currentAvgLen: 0,
      currentPathItems: 0,
    };
  });
  await page.reload();
  await page.waitForTimeout(300);

  await openStatsTab(page);

  // 空态必须是明确的文案，不是一块空白区域
  await expect(page.getByText('暂无数据').first()).toBeVisible();
});
