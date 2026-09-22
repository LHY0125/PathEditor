import { test, expect } from '@playwright/test';
import { createIpcMock } from '../mocks/ipc';

/** mock invoke 捕获的调用记录（见 e2e/mocks/ipc.ts 的 __capturedCalls）。 */
interface CapturedCall {
  cmd: string;
  args: Record<string, unknown>;
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(createIpcMock());
  await page.goto('/');
  await page.getByRole('button', { name: '全部变量' }).click();
});

/** 打开「备份与恢复」对话框（入口挂在环境变量工具栏）。 */
async function openBackupDialog(page: import('@playwright/test').Page): Promise<void> {
  await page.getByRole('button', { name: '环境变量备份与恢复' }).click();
  await expect(page.getByRole('heading', { name: '环境变量备份与恢复' })).toBeVisible();
}

test('工具栏入口打开对话框并按时间倒序列出备份（前端不重排）', async ({ page }) => {
  await openBackupDialog(page);

  const rows = page.locator('[data-backup-file]');
  await expect(rows).toHaveCount(2);
  // 列表顺序原样透传 auto 的 Rust 排序结果（最新在前）
  await expect(rows.nth(0)).toContainText('env_backup_20260922_120000_000.json');
  await expect(rows.nth(1)).toContainText('env_backup_20260921_090000_000.json');
});

test('选中备份后按四个计数渲染差异，并单独列出将删除的变量名', async ({ page }) => {
  await openBackupDialog(page);
  await page.locator('[data-backup-file]').first().click();

  // 摘要文案：三个计数两两不等（1/0/2/3），任一对调都会让断言失败
  const summary = page.getByTestId('backup-summary');
  await expect(summary).toContainText('新增 1');
  await expect(summary).toContainText('删除 2');
  await expect(summary).toContainText('冲突 3');

  // 删除项必须逐个列名（验收标准 12）：删除最不可逆
  const removed = page.getByTestId('backup-removed-names');
  await expect(removed).toBeVisible();
  await expect(removed).toContainText('user:OLD_VAR');
  await expect(removed).toContainText('system:LEGACY_HOME');
});

test('确认弹窗含删除项与手工兜底命令，确认后以 force=false 调用恢复', async ({ page }) => {
  await page.addInitScript('window.__confirmResponse = true;');
  await page.goto('/');
  await page.getByRole('button', { name: '全部变量' }).click();
  await openBackupDialog(page);
  await page.locator('[data-backup-file]').first().click();
  await page.getByRole('button', { name: '恢复', exact: true }).click();

  const calls = await page.evaluate(
    () => (window as unknown as { __capturedCalls?: CapturedCall[] }).__capturedCalls ?? [],
  );
  const confirmCall = calls.find((c) => c.cmd === 'plugin:dialog|message');
  expect(confirmCall).toBeDefined();
  const message = String(confirmCall?.args.message ?? '');
  // 三项验收：差异摘要、将删除的变量名、手工兜底出口
  expect(message).toContain('删除 2');
  expect(message).toContain('user:OLD_VAR');
  expect(message).toContain('system:LEGACY_HOME');
  expect(message).toContain('patheditor backup');
  expect(message).toContain('patheditor env backup');

  // 确认后以 force=false 调用恢复，并展示结果
  const restoreCall = calls.find((c) => c.cmd === 'restore_env_backup');
  expect(restoreCall?.args.file).toBe('C:\\backups\\env_backup_20260922_120000_000.json');
  expect(restoreCall?.args.force).toBe(false);
  await expect(page.getByTestId('backup-outcome')).toContainText('成功 3 项');
});

test('取消确认时不调用恢复', async ({ page }) => {
  await openBackupDialog(page);
  await page.locator('[data-backup-file]').first().click();
  await page.getByRole('button', { name: '恢复', exact: true }).click();

  const calls = await page.evaluate(
    () => (window as unknown as { __capturedCalls?: CapturedCall[] }).__capturedCalls ?? [],
  );
  expect(calls.some((c) => c.cmd === 'plugin:dialog|message')).toBe(true);
  expect(calls.some((c) => c.cmd === 'restore_env_backup')).toBe(false);
});

test('code=conflict 时二次确认，同意后以 force=true 重试', async ({ page }) => {
  // 恢复确认与强制覆盖二次确认都返回 true
  await page.addInitScript('window.__confirmResponse = true; window.__conflictOverride = true;');
  await page.goto('/');
  await page.getByRole('button', { name: '全部变量' }).click();
  await openBackupDialog(page);
  await page.locator('[data-backup-file]').first().click();
  await page.getByRole('button', { name: '恢复', exact: true }).click();

  await expect(page.getByTestId('backup-outcome')).toContainText('成功 3 项');

  const calls = await page.evaluate(
    () => (window as unknown as { __capturedCalls?: CapturedCall[] }).__capturedCalls ?? [],
  );
  const restores = calls.filter((c) => c.cmd === 'restore_env_backup');
  expect(restores).toHaveLength(2);
  expect(restores[0].args.force).toBe(false);
  expect(restores[1].args.force).toBe(true);

  // 第二次确认文案说明「备份后有外部修改」
  const confirms = calls.filter((c) => c.cmd === 'plugin:dialog|message');
  expect(String(confirms[1].args.message)).toContain('备份后有外部修改');
});

test('恢复失败（权限不足）时提示需要管理员权限且不重试', async ({ page }) => {
  await page.addInitScript('window.__confirmResponse = true; window.__restoreForbidden = true;');
  await page.goto('/');
  await page.getByRole('button', { name: '全部变量' }).click();
  await openBackupDialog(page);
  await page.locator('[data-backup-file]').first().click();
  await page.getByRole('button', { name: '恢复', exact: true }).click();

  await expect(page.getByText(/管理员权限/)).toBeVisible();
  const calls = await page.evaluate(
    () => (window as unknown as { __capturedCalls?: CapturedCall[] }).__capturedCalls ?? [],
  );
  // 未提权不得重试，也不得走强制覆盖绕过
  expect(calls.filter((c) => c.cmd === 'restore_env_backup')).toHaveLength(1);
});
