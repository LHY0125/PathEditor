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
});

test('切到「全部变量」显示两个 hive 的变量', async ({ page }) => {
  await page.getByRole('button', { name: '全部变量' }).click();

  await expect(page.locator('[data-env-var-key="user:JAVA_HOME"]')).toBeVisible();
  await expect(page.locator('[data-env-var-key="system:windir"]')).toBeVisible();
});

test('Path 不出现在「全部变量」列表中', async ({ page }) => {
  await page.getByRole('button', { name: '全部变量' }).click();

  await expect(page.locator('[data-env-var-key="user:JAVA_HOME"]')).toBeVisible();
  // mock 的 list_all_env_vars 不含 Path（Rust 侧过滤）；顶部引导文案里的 "Path"
  // 不是独立文本节点，exact 匹配不会误命中。
  await expect(page.getByText('Path', { exact: true })).toHaveCount(0);
});

test('敏感变量默认打码，点「显示」后出现明文', async ({ page }) => {
  await page.getByRole('button', { name: '全部变量' }).click();
  const row = page.locator('[data-env-var-key="user:MY_TOKEN"]');
  await expect(row).toBeVisible();

  // 打码占位符可见，明文不可见
  await expect(page.getByText('plaintext-secret-value')).toHaveCount(0);

  await row.getByRole('button', { name: '显示' }).click();
  await expect(page.getByText('plaintext-secret-value')).toBeVisible();
});

test('保护行与不支持类型的编辑按钮禁用', async ({ page }) => {
  await page.getByRole('button', { name: '全部变量' }).click();
  await expect(page.locator('[data-env-var-key="system:windir"]')).toBeVisible();

  // windir 是系统内置保护变量（canEdit=false）
  await expect(
    page.locator('[data-env-var-key="system:windir"]').getByRole('button', { name: '编辑 windir' }),
  ).toBeDisabled();
  // SYS_BINARY 是 Unsupported 类型，同样不可编辑
  await expect(
    page
      .locator('[data-env-var-key="system:SYS_BINARY"]')
      .getByRole('button', { name: '编辑 SYS_BINARY' }),
  ).toBeDisabled();
});

test('Unsupported 类型显示占位而非值', async ({ page }) => {
  await page.getByRole('button', { name: '全部变量' }).click();

  await expect(page.getByText('(不支持的注册表类型)')).toBeVisible();
});

test('「全部变量」下 PATH 专用按钮不可见', async ({ page }) => {
  await page.getByRole('button', { name: '全部变量' }).click();
  await expect(page.locator('[data-env-var-key="user:JAVA_HOME"]')).toBeVisible();

  await expect(page.getByRole('button', { name: '上移' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: '下移' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: '一键清理' })).toHaveCount(0);
});

test('「全部变量」下拖放文件夹无效（设计文档 a2）', async ({ page }) => {
  await page.getByRole('button', { name: '全部变量' }).click();
  await expect(page.locator('[data-env-var-key="user:JAVA_HOME"]')).toBeVisible();

  // webkitGetAsEntry 是函数，无法经 dispatchEvent 跨进程序列化，
  // 需在浏览器上下文内构造 drop 事件
  await page.evaluate(() => {
    const zone = document.querySelector('[data-testid="path-drop-zone"]');
    if (!zone) throw new Error('未找到拖放区');
    const dt = new DataTransfer();
    Object.defineProperty(dt, 'items', {
      value: [{ webkitGetAsEntry: () => ({ isDirectory: true }) }],
    });
    Object.defineProperty(dt, 'files', {
      value: [{ path: 'D:\\DroppedFolder' }],
    });
    zone.dispatchEvent(
      new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: dt }),
    );
  });

  // drop handler 在 allVars 下早退：PATH 快照通路未被触碰，
  // 切回系统 PATH 后仍是 mock 的 2 条，无新增
  await page.getByRole('button', { name: '系统 PATH' }).click();
  await expect(page.getByTestId('path-row')).toHaveCount(2);
});

test('切换来源筛选只显示对应 hive', async ({ page }) => {
  await page.getByRole('button', { name: '全部变量' }).click();
  await expect(page.locator('[data-env-var-key="user:JAVA_HOME"]')).toBeVisible();

  // 「系统」筛选按钮与「系统 PATH」tab 名部分重叠，需 exact 匹配
  await page.getByRole('button', { name: '系统', exact: true }).click();
  await expect(page.locator('[data-env-var-key="system:windir"]')).toBeVisible();
  await expect(page.getByText('JAVA_HOME')).toHaveCount(0);
});

test('编辑普通变量触发 update_env_var 并携带 revision', async ({ page }) => {
  await page.getByRole('button', { name: '全部变量' }).click();
  const row = page.locator('[data-env-var-key="user:JAVA_HOME"]');
  await expect(row).toBeVisible();

  // 选中 JAVA_HOME 行后点该行的「编辑」
  await row.click();
  await row.getByRole('button', { name: '编辑 JAVA_HOME' }).click();

  // 编辑弹窗：变量名只读展示，唯一输入框是「变量值」
  await expect(page.getByRole('heading', { name: '编辑环境变量' })).toBeVisible();
  await page.getByLabel('变量值').fill('C:\\NewJava');
  await page.getByRole('button', { name: '确定' }).click();

  // 断言：update_env_var 被调用，且 expectedRevision 等于列表下发的 revision
  const calls = await page.evaluate(
    () => (window as unknown as { __capturedCalls?: CapturedCall[] }).__capturedCalls ?? [],
  );
  const updateCall = calls.find((c) => c.cmd === 'update_env_var');
  expect(updateCall).toBeDefined();
  expect(updateCall?.args.hive).toBe('user');
  expect(updateCall?.args.name).toBe('JAVA_HOME');
  expect(updateCall?.args.value).toBe('C:\\NewJava');
  expect(updateCall?.args.expectedRevision).toBe('usr-java');
});

test('revision 冲突时显示错误并刷新，不静默覆盖', async ({ page }) => {
  // beforeEach 已 goto；addInitScript 只对后续导航生效，需重新加载注入冲突开关
  await page.addInitScript('window.__conflictOverride = true;');
  await page.goto('/');
  await page.getByRole('button', { name: '全部变量' }).click();
  const row = page.locator('[data-env-var-key="user:JAVA_HOME"]');
  await expect(row).toBeVisible();

  await row.click();
  await row.getByRole('button', { name: '编辑 JAVA_HOME' }).click();
  await page.getByLabel('变量值').fill('C:\\NewJava');
  await page.getByRole('button', { name: '确定' }).click();

  // 错误同时出现在编辑弹窗与状态栏，取第一处即可
  await expect(page.getByText(/已被其他进程修改/).first()).toBeVisible();
  // 弹窗保留（不静默关闭假装成功）
  await expect(page.getByRole('heading', { name: '编辑环境变量' })).toBeVisible();
});
