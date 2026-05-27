import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__TAURI_INTERNALS__ = {
      invoke: async (cmd, args) => {
        switch (cmd) {
          case 'check_admin': return true;
          case 'load_system_paths': return ['C:\\\\Windows', 'invalid_path', 'C:\\\\Temp'];
          case 'load_user_paths': return [];
          case 'load_disabled_state': return { system: [], user: [] };
          case 'save_system_paths': return undefined;
          case 'save_user_paths': return undefined;
          case 'save_disabled_state': return undefined;
          case 'backup_registry': return '';
          case 'broadcast_env_change': return undefined;
          case 'validate_path': return false;
          case 'expand_env_vars': return '';
          case 'read_text_file': return '';
          case 'get_appdata_dir': return '';
          default: return undefined;
        }
      }
    };
  });
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
