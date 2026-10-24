#!/usr/bin/env node
/**
 * PathEditor 演示 GIF 录制脚本
 *
 * 用 Playwright 驱动 **生产构建 + mock IPC**，录制 GUI 的一段操作演示，
 * 再用 ffmpeg 两遍调色板法转成 GIF（README「## 操作演示」引用的那一张）。
 *
 * 安全边界（与仓库红线一致）：
 * - 全程 `window.__TAURI_INTERNALS__.invoke` 由 `demo-harness.mjs` 注入，
 *   **不连接真实 Tauri 后端**，因此不读写真实注册表，也不碰 `~/.patheditor`。
 * - 只对 `vite preview` 的网页构建录屏，**绝不启动** `npx tauri dev` 或打包后的真实 GUI
 *   （那会走 `setup_database`，在真实 `~/.patheditor` 上归档/清理）。
 *
 * 用法：
 *   node src/gui/web/e2e/record-demo.mjs                     # → docs/screenshots/demo.gif
 *   node src/gui/web/e2e/record-demo.mjs out/xxx.gif         # 自定义输出
 *   node src/gui/web/e2e/record-demo.mjs --webm-only         # 只留 webm，不转 GIF
 *   node src/gui/web/e2e/record-demo.mjs --skip-build        # 复用已有 dist，跳过构建
 *
 * 前置：`npm install` 已完成；ffmpeg 在 PATH 上（或设 `FFMPEG` 环境变量）。
 */
import { chromium } from '@playwright/test';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, copyFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  BASE_URL,
  VIEWPORT,
  clickEl,
  ensurePreview,
  installCursor,
  installMock,
  killTree,
  typeInto,
} from './demo-harness.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../../..');
const FFMPEG = process.env.FFMPEG || 'ffmpeg';

// ── 演示数据：贴近真实 Windows 机器的 PATH 布局 ──
const SYSTEM_PATHS = [
  'C:\\Windows\\system32',
  'C:\\Windows',
  'C:\\Windows\\System32\\Wbem',
  'C:\\Windows\\System32\\WindowsPowerShell\\v1.0',
  'C:\\Program Files\\Git\\cmd',
  'C:\\Program Files\\nodejs\\',
  'C:\\Program Files\\dotnet\\',
  'C:\\Program Files\\Microsoft VS Code\\bin',
  'C:\\Program Files (x86)\\Microsoft VS Code\\bin',
  'D:\\Legacy\\OldTool\\bin', // 演示「清理无效路径」时被移除
];
const USER_PATHS = [
  'C:\\Users\\me\\AppData\\Local\\Programs\\Python\\Python313\\',
  'C:\\Users\\me\\.cargo\\bin',
  'C:\\Users\\me\\AppData\\Roaming\\npm',
];
const INVALID = ['D:\\Legacy\\OldTool\\bin'];

/** 录制主流程：打开界面 → 执行一串核心操作 → 关闭并落 webm。 */
async function record(webmDir) {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: VIEWPORT,
    recordVideo: { dir: webmDir, size: VIEWPORT },
    locale: 'zh-CN',
  });

  const page = await context.newPage();
  await page.addInitScript(installMock, {
    system: SYSTEM_PATHS,
    user: USER_PATHS,
    invalid: INVALID,
  });
  await page.addInitScript(installCursor);

  // ── 1. 主界面：系统 PATH 列表 ──
  await page.goto(BASE_URL, { waitUntil: 'networkidle' });
  await page.getByTestId('path-row').first().waitFor();
  await page.mouse.move(VIEWPORT.width - 200, VIEWPORT.height - 120, { steps: 10 });
  await page.waitForTimeout(1600);

  // ── 2. 切到「用户 PATH」 ──
  await clickEl(page, page.locator('#tab-user'), { after: 1400 });

  // ── 3. 切回「系统 PATH」 ──
  await clickEl(page, page.locator('#tab-system'), { after: 900 });

  // ── 4. 搜索过滤 ──
  await typeInto(page, page.locator('.toolbar-search'), 'Program', 45);
  await page.waitForTimeout(1400);

  // ── 5. 清空搜索 ──
  await clickEl(page, page.locator('.toolbar-search'), { hover: 120, after: 120 });
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Delete');
  await page.waitForTimeout(800);

  // ── 6. 新建路径（弹窗输入 + 确认） ──
  await clickEl(page, page.getByRole('button', { name: '新建', exact: true }), { after: 500 });
  await typeInto(
    page,
    page.locator('.modal-overlay input[type="text"]'),
    'C:\\Program Files\\PostgreSQL\\16\\bin',
    28,
  );
  await page.waitForTimeout(450);
  await clickEl(page, page.locator('.modal-overlay').getByRole('button', { name: '确认' }), {
    after: 900,
  });

  // ── 7. 选中刚新建的第 11 行 ──
  await clickEl(page, page.getByTestId('path-row').nth(10), { after: 550 });

  // ── 8. 编辑这一行：16 → 17 ──
  await clickEl(page, page.getByRole('button', { name: '编辑', exact: true }), { after: 500 });
  await typeInto(
    page,
    page.locator('.modal-overlay input[type="text"]'),
    'C:\\Program Files\\PostgreSQL\\17\\bin',
    28,
  );
  await page.waitForTimeout(400);
  await clickEl(page, page.locator('.modal-overlay').getByRole('button', { name: '确认' }), {
    after: 900,
  });

  // ── 9. 上移（提升优先级） ──
  await clickEl(page, page.getByRole('button', { name: '上移', exact: true }), { after: 1100 });

  // ── 10. 清理无效路径（⋯ 菜单） ──
  await clickEl(page, page.getByRole('button', { name: '更多操作' }), { after: 600 });
  await clickEl(page, page.getByRole('menuitem', { name: '清理无效路径' }), { after: 1300 });

  // ── 11. 保存 ──
  await clickEl(page, page.getByRole('button', { name: '保存', exact: true }), { after: 1400 });

  // ── 12. 撤销（Ctrl+Z 回到清理前） ──
  await page.keyboard.press('Control+z');
  await page.waitForTimeout(1100);

  // ── 13. 深色模式收尾 ──
  await clickEl(page, page.getByRole('button', { name: '更多操作' }), { after: 600 });
  await clickEl(page, page.getByRole('menuitem', { name: '深色模式' }), { after: 1500 });

  // 指针移开，留出干净的结尾画面
  await page.mouse.move(VIEWPORT.width / 2, VIEWPORT.height - 60, { steps: 12 });
  await page.waitForTimeout(1200);

  const video = page.video();
  await context.close();
  await browser.close();
  return video.path();
}

/** webm → GIF。两遍调色板：先统计真实颜色分布，再用它量化，文字边缘才锐利。 */
function toGif(webmPath, gifPath) {
  const palette = join(dirname(gifPath), '.demo-palette.png');
  const scale = 'fps=12,scale=1000:-2:flags=lanczos';
  const pass1 = spawnSync(
    FFMPEG,
    [
      '-v',
      'error',
      '-y',
      '-ss',
      '0.15',
      '-i',
      webmPath,
      '-vf',
      `${scale},palettegen=stats_mode=diff`,
      palette,
    ],
    { stdio: 'inherit' },
  );
  if (pass1.status !== 0) throw new Error('palettegen 失败，检查 ffmpeg 是否可用');
  const pass2 = spawnSync(
    FFMPEG,
    [
      '-v',
      'error',
      '-y',
      '-ss',
      '0.15',
      '-i',
      webmPath,
      '-i',
      palette,
      '-lavfi',
      `${scale}[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle`,
      gifPath,
    ],
    { stdio: 'inherit' },
  );
  if (pass2.status !== 0) throw new Error('paletteuse 失败');
  rmSync(palette, { force: true });
}

const main = async () => {
  const argv = process.argv.slice(2);
  const flags = new Set(argv.filter((a) => a.startsWith('--')));
  const positional = argv.filter((a) => !a.startsWith('--'));
  const webmOnly = flags.has('--webm-only');
  const outPath = resolve(
    ROOT,
    positional[0] ?? `docs/screenshots/demo.${webmOnly ? 'webm' : 'gif'}`,
  );
  mkdirSync(dirname(outPath), { recursive: true });

  const { server } = await ensurePreview({ root: ROOT, skipBuild: flags.has('--skip-build') });
  const workDir = mkdtempSync(join(tmpdir(), 'patheditor-demo-'));

  try {
    console.log('▶ 录制中（约 32 秒）…');
    const webmPath = await record(workDir);

    if (webmOnly) {
      copyFileSync(webmPath, outPath);
    } else {
      if (spawnSync(FFMPEG, ['-version'], { stdio: 'ignore' }).status !== 0) {
        throw new Error(
          '找不到 ffmpeg。安装后重试，或用 FFMPEG 环境变量指向可执行文件，或加 --webm-only 跳过转码。',
        );
      }
      console.log('▶ 转 GIF…');
      toGif(webmPath, outPath);
    }

    const mb = (statSync(outPath).size / 1024 / 1024).toFixed(2);
    console.log(`✅ 已生成 ${outPath}（${mb} MB）`);
  } finally {
    killTree(server);
    rmSync(workDir, { recursive: true, force: true });
  }
};

main().catch((error) => {
  console.error('❌ 录制失败:', error.message ?? error);
  process.exit(1);
});
