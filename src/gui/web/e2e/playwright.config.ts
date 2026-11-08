import { defineConfig } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// webServer 默认以配置文件目录为 cwd，而 npm 脚本与 Vite 配置都在仓库根。
// 显式指到仓库根（本文件位于 src/gui/web/e2e/）。
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');

export default defineConfig({
  testDir: './tests',
  timeout: 30000,
  fullyParallel: false,
  workers: 2,
  use: {
    baseURL: 'http://127.0.0.1:4173',
    locale: 'zh-CN',
    actionTimeout: 15000,
    navigationTimeout: 30000,
  },
  webServer: {
    cwd: repoRoot,
    command: 'npm run build && npm run preview -- --host 127.0.0.1 --port 4173 --strictPort',
    url: 'http://127.0.0.1:4173',
    reuseExistingServer: false,
    timeout: 120000,
  },
});
