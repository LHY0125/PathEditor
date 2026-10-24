#!/usr/bin/env node
/**
 * PathEditor 文档截图生成脚本
 *
 * 产出 `docs/screenshots/` 下 README 引用的整套图：
 *   main-window.png      系统 PATH 主界面
 *   user-path.png        用户 PATH
 *   path-edit.png        编辑路径对话框
 *   merge-preview.png    合并预览（系统 + 用户去重后的最终生效顺序）
 *   conflict-analysis.png 冲突分析（优先执行 / 被遮蔽）
 *   all-env-vars.png     全部环境变量
 *   env-backup.png       环境变量备份与恢复（差异预览）
 *   history.png          历史记录时间线
 *   search.png           全文检索
 *   stats.png            统计
 *   sync.png             同步中心（含未解决冲突）
 *   profiles.png         配置方案
 *   dark-mode.png        深色模式主界面
 *   cli-demo.png         CLI 帮助（真实 v6 `--help` 输出渲染）
 *
 * 前十三张用 Playwright + mock IPC 拍生产构建的网页界面 —— 不碰真实注册表，
 * 也不启动真实 GUI（那会走 `setup_database`，动到 `~/.patheditor`）。
 *
 * 最后一张说明：终端窗口无法在无头环境里可靠截取，因此把**真实的**
 * `patheditor --help` 输出渲染成一张终端样图。文字来自实际二进制，
 * 只是版式是渲染的。可用 `PATHEDITOR_CLI` 指定二进制；取不到时回退到内置文本。
 *
 * 用法：
 *   node src/gui/web/e2e/capture-screenshots.mjs
 *   node src/gui/web/e2e/capture-screenshots.mjs --skip-build
 *   node src/gui/web/e2e/capture-screenshots.mjs --only-gui   # 跳过 CLI 图
 */
import { chromium } from '@playwright/test';
import { spawnSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  BASE_URL,
  VIEWPORT,
  clickEl,
  ensurePreview,
  installMock,
  killTree,
  typeInto,
} from './demo-harness.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../../..');
const SHOTS = resolve(ROOT, 'docs/screenshots');
const CLI_BIN = process.env.PATHEDITOR_CLI || 'patheditor';

const H = 3600_000;
const D = 24 * H;
/** 2026-10-24T00:00:00Z，用作夹具时间基准。 */
const T0 = 1792800000000;

// ── 演示数据：贴近真实 Windows 开发机的 PATH 布局 ──
const SYSTEM_PATHS = [
  'C:\\Program Files\\dotnet\\',
  'C:\\Program Files\\Microsoft SQL Server\\150\\Tools\\Binn\\',
  'C:\\Program Files\\Microsoft SQL Server\\Client SDK\\ODBC\\170\\Tools\\Binn\\',
  'C:\\Program Files\\nodejs\\',
  'C:\\Program Files\\NVIDIA Corporation\\Nsight Compute 2022.1.0',
  'C:\\Program Files\\NVIDIA Corporation\\NVIDIA app\\NvDLISR',
  'C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v11.6\\bin',
  'C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v11.6\\include',
  'C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v11.6\\lib',
  'C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v11.6\\libnvvp',
  'C:\\Program Files (x86)\\NVIDIA Corporation\\PhysX\\Common',
  'C:\\Program Files (x86)\\Windows Kits\\10\\Windows Performance Toolkit',
  'C:\\WINDOWS',
  'C:\\WINDOWS\\system32',
  'C:\\WINDOWS\\System32\\OpenSSH',
  'C:\\WINDOWS\\System32\\Wbem',
  'C:\\WINDOWS\\System32\\WindowsPowerShell\\v1.0',
  'C:\\Users\\me\\AppData\\Roaming\\npm',
];
const USER_PATHS = [
  'D:\\settings\\settings\\Scoop\\shims',
  'C:\\Users\\me\\local\\bin',
  'C:\\Users\\me\\.bun\\bin',
  'C:\\Users\\me\\AppData\\Local\\GitHubDesktop\\bin',
  'D:\\settings\\VS Code\\bin',
  '%USERPROFILE%\\go\\bin',
];

/** `scan_paths`：真实感的可执行文件冲突 + 工具清单。 */
const SCAN_RESULT = {
  conflicts: [
    {
      name: 'python.exe',
      locations: [
        { dir: 'C:\\Users\\me\\AppData\\Local\\Programs\\Python\\Python313\\', priority: 1 },
        { dir: 'C:\\Program Files\\Python311\\', priority: 2 },
      ],
    },
    {
      name: 'node.exe',
      locations: [
        { dir: 'C:\\Program Files\\nodejs\\', priority: 1 },
        { dir: 'C:\\Program Files\\nvm\\v18.20.0\\', priority: 2 },
      ],
    },
    {
      name: 'git.exe',
      locations: [
        { dir: 'C:\\Program Files\\Git\\cmd', priority: 1 },
        { dir: 'C:\\Program Files\\Git\\bin', priority: 2 },
        { dir: 'C:\\Program Files\\Git\\mingw64\\bin', priority: 3 },
      ],
    },
    {
      name: 'code.exe',
      locations: [
        { dir: 'C:\\Program Files\\Microsoft VS Code\\bin', priority: 1 },
        { dir: 'C:\\Program Files (x86)\\Microsoft VS Code\\bin', priority: 2 },
      ],
    },
  ],
  tools: [
    {
      dir: 'C:\\Program Files\\Git\\cmd',
      exists: true,
      exes: ['git.exe', 'git-gui.exe', 'gitk.exe'],
    },
    { dir: 'C:\\Program Files\\nodejs\\', exists: true, exes: ['node.exe', 'npm.cmd', 'npx.cmd'] },
    {
      dir: 'C:\\WINDOWS\\system32',
      exists: true,
      exes: ['cmd.exe', 'ping.exe', 'where.exe', 'whoami.exe'],
    },
  ],
};

/** `list_all_env_vars`：与 EnvVarMeta 契约一致（不含 value）。 */
const ENV_VARS = {
  system: [
    {
      name: 'JAVA_HOME',
      kind: 'string',
      hive: 'system',
      canEdit: false,
      canDelete: false,
      sensitive: false,
      preview: 'C:\\Java\\jdk-21',
      revision: 'sys-java',
    },
    {
      name: 'SystemRoot',
      kind: 'string',
      hive: 'system',
      canEdit: false,
      canDelete: false,
      sensitive: false,
      preview: 'C:\\Windows',
      revision: 'sys-root',
    },
    {
      name: 'windir',
      kind: 'string',
      hive: 'system',
      canEdit: false,
      canDelete: false,
      sensitive: false,
      preview: 'C:\\WINDOWS',
      revision: 'sys-windir',
    },
    {
      name: 'TEMP',
      kind: 'expandString',
      hive: 'system',
      canEdit: false,
      canDelete: false,
      sensitive: false,
      preview: '%SystemRoot%\\TEMP',
      revision: 'sys-temp',
    },
  ],
  user: [
    {
      name: 'CARGO_HOME',
      kind: 'string',
      hive: 'user',
      canEdit: true,
      canDelete: true,
      sensitive: false,
      preview: 'D:\\settings\\Language\\Rust\\cargo',
      revision: 'usr-cargo',
    },
    {
      name: 'GOPATH',
      kind: 'string',
      hive: 'user',
      canEdit: true,
      canDelete: true,
      sensitive: false,
      preview: 'C:\\Users\\me\\go',
      revision: 'usr-gopath',
    },
    {
      name: 'PYTHONUTF8',
      kind: 'string',
      hive: 'user',
      canEdit: true,
      canDelete: true,
      sensitive: false,
      preview: '1',
      revision: 'usr-pyutf8',
    },
    {
      name: 'MY_TOKEN',
      kind: 'string',
      hive: 'user',
      canEdit: true,
      canDelete: true,
      sensitive: true,
      preview: null,
      revision: 'usr-token',
    },
  ],
};

/** `history_list_events`：8 条事件，总数 42（触发分页文案）。 */
const HISTORY = {
  items: [
    {
      uid: 'ev-1',
      createdAt: T0 + 7 * H,
      source: 'gui',
      action: 'save',
      hive: 'system',
      target: null,
      result: 'success',
      snapshotUid: 'snap-1',
    },
    {
      uid: 'ev-2',
      createdAt: T0 + 6 * H,
      source: 'gui',
      action: 'envSet',
      hive: 'user',
      target: 'JAVA_HOME',
      result: 'success',
      snapshotUid: 'snap-2',
    },
    {
      uid: 'ev-3',
      createdAt: T0 + 4 * H,
      source: 'gui',
      action: 'apply',
      hive: null,
      target: '开发环境',
      result: 'success',
      snapshotUid: 'snap-3',
    },
    {
      uid: 'ev-4',
      createdAt: T0 - D + 8 * H,
      source: 'import',
      action: 'import',
      hive: null,
      target: null,
      result: 'partial',
      snapshotUid: 'snap-4',
    },
    {
      uid: 'ev-5',
      createdAt: T0 - D + 3 * H,
      source: 'gui',
      action: 'undo',
      hive: 'system',
      target: null,
      result: 'success',
      snapshotUid: 'snap-5',
    },
    {
      uid: 'ev-6',
      createdAt: T0 - 2 * D + 9 * H,
      source: 'gui',
      action: 'restore',
      hive: null,
      target: null,
      result: 'success',
      snapshotUid: 'snap-6',
    },
    {
      uid: 'ev-7',
      createdAt: T0 - 3 * D,
      source: 'cli',
      action: 'envAdd',
      hive: 'user',
      target: 'PYTHONUTF8',
      result: 'success',
      snapshotUid: 'snap-7',
    },
    {
      uid: 'ev-8',
      createdAt: T0 - 4 * D,
      source: 'cli',
      action: 'envRemove',
      hive: 'user',
      target: 'OLD_PATH',
      result: 'success',
      snapshotUid: 'snap-8',
    },
  ],
  total: 42,
};

/** `search`：命中项含 `[]` 高亮片段；第一条能在当前 PATH 中定位。 */
const SEARCH_HITS = [
  {
    kind: 'path',
    refUid: 'snap-1#system',
    content: 'C:\\Program Files\\nodejs\\',
    snippet: 'C:\\Program Files\\[node]js\\',
    rank: 1,
  },
  {
    kind: 'envName',
    refUid: 'snap-1#user',
    content: 'NODE_OPTIONS',
    snippet: '[NODE]_OPTIONS',
    rank: 2,
  },
  {
    kind: 'profile',
    refUid: 'prof-1',
    content: 'Node 开发环境',
    snippet: '[Node] 开发环境',
    rank: 3,
  },
];

/** `stats_dashboard`：历史序列与「当前」口径分开。 */
const STATS = {
  days: [
    { day: '2026-10-15', pathItems: 52, envItems: 11, events: 2, conflicts: 0 },
    { day: '2026-10-16', pathItems: 52, envItems: 11, events: 1, conflicts: 0 },
    { day: '2026-10-17', pathItems: 55, envItems: 12, events: 4, conflicts: 1 },
    { day: '2026-10-18', pathItems: 55, envItems: 12, events: 2, conflicts: 0 },
    { day: '2026-10-19', pathItems: 57, envItems: 12, events: 3, conflicts: 0 },
    { day: '2026-10-20', pathItems: 57, envItems: 13, events: 1, conflicts: 0 },
    { day: '2026-10-21', pathItems: 58, envItems: 13, events: 5, conflicts: 2 },
    { day: '2026-10-22', pathItems: 58, envItems: 13, events: 2, conflicts: 0 },
    { day: '2026-10-23', pathItems: 60, envItems: 14, events: 4, conflicts: 0 },
    { day: '2026-10-24', pathItems: 60, envItems: 14, events: 3, conflicts: 0 },
  ],
  topTargets: [
    { target: 'JAVA_HOME', count: 5 },
    { target: 'GOPATH', count: 3 },
    { target: 'PATH', count: 2 },
  ],
  profileApplies: [
    { name: '开发环境', applyCount: 4, lastAppliedAt: T0 + 4 * H },
    { name: '最小 PATH', applyCount: 2, lastAppliedAt: T0 - 2 * D },
  ],
  currentAvgLen: 42,
  currentPathItems: 24,
};

/** 同步中心：2 条未解决冲突。 */
const SYNC_STATUS = { pendingCount: 5, unresolvedConflicts: 2 };
const SYNC_CONFLICTS = [
  {
    id: 1,
    entityType: 'profile',
    entityUid: 'uid-abc',
    field: null,
    localValue: '开发环境（本机）',
    remoteValue: '开发环境（笔记本）',
    remoteDevice: 'LAPTOP-7F2A',
    createdAt: T0 + 2 * H,
    resolvedAt: null,
    resolution: null,
  },
  {
    id: 2,
    entityType: 'snapshot',
    entityUid: 'uid-def',
    field: null,
    localValue: '24 条 PATH',
    remoteValue: '22 条 PATH',
    remoteDevice: 'DESKTOP-9K3M',
    createdAt: T0 - 3 * H,
    resolvedAt: null,
    resolution: null,
  },
];

/** 配置方案。 */
const PROFILES = [
  {
    uid: 'p1',
    name: '开发环境',
    description: 'Node + Rust + Python 全套工具链',
    tags: ['dev', 'full'],
    createdAt: T0 - 30 * D,
    updatedAt: T0 - 2 * D,
    lastAppliedAt: T0 + 4 * H,
    applyCount: 4,
    version: 3,
    snapshotUid: 's1',
  },
  {
    uid: 'p2',
    name: '最小 PATH',
    description: '仅保留系统必需路径',
    tags: ['minimal'],
    createdAt: T0 - 20 * D,
    updatedAt: T0 - 5 * D,
    lastAppliedAt: T0 - 2 * D,
    applyCount: 2,
    version: 1,
    snapshotUid: 's2',
  },
  {
    uid: 'p3',
    name: 'CUDA 开发',
    description: null,
    tags: ['gpu', 'cuda'],
    createdAt: T0 - 12 * D,
    updatedAt: T0 - 9 * D,
    lastAppliedAt: T0 - 9 * D,
    applyCount: 1,
    version: 2,
    snapshotUid: 's3',
  },
];

/** 环境变量备份列表与差异预览。 */
const ENV_BACKUPS = [
  {
    file: 'env_backup_20261024_150000_000.json',
    path: 'C:\\backups\\env_backup_20261024_150000_000.json',
    timestamp: '20261024_150000_000',
    sizeBytes: 2048,
    variableCount: 0,
  },
  {
    file: 'env_backup_20261023_090000_000.json',
    path: 'C:\\backups\\env_backup_20261023_090000_000.json',
    timestamp: '20261023_090000_000',
    sizeBytes: 1984,
    variableCount: 0,
  },
  {
    file: 'env_backup_20261021_183000_000.json',
    path: 'C:\\backups\\env_backup_20261021_183000_000.json',
    timestamp: '20261021_183000_000',
    sizeBytes: 1920,
    variableCount: 0,
  },
];
const ENV_PREVIEW = {
  changes: [
    { hive: 'user', name: 'PYTHONUTF8', kind: 'added' },
    { hive: 'user', name: 'OLD_PATH', kind: 'removed' },
    { hive: 'system', name: 'LEGACY_HOME', kind: 'removed' },
    { hive: 'user', name: 'JAVA_HOME', kind: 'conflict' },
    { hive: 'system', name: 'windir', kind: 'conflict' },
    { hive: 'user', name: 'CARGO_HOME', kind: 'conflict' },
  ],
  added: 1,
  modified: 0,
  removed: 2,
  conflicts: 3,
};

/** 取真实 CLI 帮助文本；拿不到或明显不是 v6 时回退到内置文本。 */
function readCliHelp() {
  const result = spawnSync(CLI_BIN, ['--help'], { encoding: 'utf8' });
  const text = `${result.stdout ?? ''}${result.stderr ?? ''}`.trim();
  // v6 的标志：新增了 env / sync / maintenance 三个子命令
  if (result.status === 0 && text.includes('maintenance') && text.includes('sync')) {
    return { text, source: `${CLI_BIN}（真实输出）` };
  }
  console.warn(`⚠ 无法从 ${CLI_BIN} 取到 v6 帮助输出，使用内置文本`);
  return { text: FALLBACK_CLI_HELP, source: '内置文本' };
}

const FALLBACK_CLI_HELP = `Usage: patheditor.exe <COMMAND>

Commands:
  list         列出 PATH 路径
  add          添加一条路径
  remove       删除指定位置的路径
  edit         编辑指定位置的路径
  move-up      上移路径（--steps 指定移动格数，默认 1）
  move-down    下移路径（--steps 指定移动格数，默认 1）
  clean        清理无效和重复路径
  enable       启用指定位置的路径
  disable      禁用指定位置的路径
  import       从文件导入 PATH（JSON/CSV/TXT）
  export       导出 PATH 为文件
  backup       创建注册表备份
  conflicts    检测可执行文件冲突
  scan         列出 PATH 目录中的可执行文件
  check-admin  检查管理员权限
  env          管理通用环境变量（Path 除外，请使用 PATH 专用命令）
  profile      管理配置文件
  sync         同步（文件式变更包，跨设备搬运配置）
  maintenance  维护任务（保留策略清理、遗留明文备份清理）
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version`;

const escapeHtml = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/** 把帮助文本渲染成一张终端样图（1280×955，与旧图同尺寸）。 */
async function captureCli(browser, outPath) {
  const { text, source } = readCliHelp();
  const page = await browser.newPage({
    viewport: { width: 1280, height: 955 },
    deviceScaleFactor: 1,
  });

  const html = `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8">
<style>
  *{box-sizing:border-box}
  body{margin:0;height:955px;background:#1b1b1f;display:flex;align-items:center;
    justify-content:center;font-family:"Cascadia Code","Consolas",monospace}
  .win{width:1240px;height:915px;background:#0c0c0c;border-radius:10px;overflow:hidden;
    border:1px solid #2c2c30;box-shadow:0 18px 48px rgba(0,0,0,.55);display:flex;flex-direction:column}
  .bar{height:38px;flex:0 0 auto;background:#202024;display:flex;align-items:center;
    padding:0 14px;gap:8px;border-bottom:1px solid #2c2c30}
  .dot{width:12px;height:12px;border-radius:50%}
  .title{margin-left:10px;color:#c9c9ce;font-size:12px;letter-spacing:.02em}
  .body{flex:1;padding:18px 22px;color:#d6d6db;font-size:14.5px;line-height:1.62;
    white-space:pre;overflow:hidden}
  .prompt{color:#7bd88f}
  .cmd{color:#e8e8ee;font-weight:600}
</style></head><body>
  <div class="win">
    <div class="bar">
      <span class="dot" style="background:#ff5f57"></span>
      <span class="dot" style="background:#febc2e"></span>
      <span class="dot" style="background:#28c840"></span>
      <span class="title">PowerShell — patheditor</span>
    </div>
    <div class="body"><span class="prompt">PS D:\\Code&gt;</span> <span class="cmd">patheditor --help</span>
${escapeHtml(text)}</div>
  </div>
</body></html>`;

  await page.setContent(html, { waitUntil: 'load' });
  await page.screenshot({ path: outPath });
  await page.close();
  console.log(`  ✓ cli-demo.png（${source}）`);
}

/** 截图并打印进度。 */
async function shot(page, name) {
  await page.mouse.move(20, 20);
  await page.waitForTimeout(220);
  await page.screenshot({ path: resolve(SHOTS, name) });
  console.log(`  ✓ ${name}`);
}

/** 关闭当前弹窗并等待其卸载。 */
async function closeModal(page) {
  await page.keyboard.press('Escape');
  await page.locator('.modal-overlay').waitFor({ state: 'detached', timeout: 5000 });
  await page.waitForTimeout(180);
}

/** 依次拍完 V6 的功能面。 */
async function captureGui(browser) {
  const context = await browser.newContext({ viewport: VIEWPORT, locale: 'zh-CN' });
  const page = await context.newPage();
  await page.addInitScript(installMock, {
    system: SYSTEM_PATHS,
    user: USER_PATHS,
    invalid: [],
    scan: SCAN_RESULT,
    history: HISTORY,
    search: SEARCH_HITS,
    stats: STATS,
    syncStatus: SYNC_STATUS,
    syncConflicts: SYNC_CONFLICTS,
    profiles: PROFILES,
    envVars: ENV_VARS,
    envBackups: ENV_BACKUPS,
    envPreview: ENV_PREVIEW,
  });

  await page.goto(BASE_URL, { waitUntil: 'networkidle' });
  await page.getByTestId('path-row').first().waitFor();
  await page.waitForTimeout(350);

  // 1) 主界面
  await shot(page, 'main-window.png');

  // 2) 编辑路径对话框
  await clickEl(page, page.getByTestId('path-row').first(), { after: 220 });
  await clickEl(page, page.getByRole('button', { name: '编辑', exact: true }), { after: 420 });
  await page.locator('.modal-overlay').waitFor();
  await shot(page, 'path-edit.png');
  await closeModal(page);

  // 3) 冲突分析
  await clickEl(page, page.getByRole('button', { name: '更多操作' }), { after: 320 });
  await clickEl(page, page.getByRole('menuitem', { name: '冲突分析' }), { after: 260 });
  await page.getByText(/发现 \d+ 个文件冲突/).waitFor({ timeout: 15000 });
  await shot(page, 'conflict-analysis.png');
  await closeModal(page);

  // 4) 配置方案
  await clickEl(page, page.getByRole('button', { name: '更多操作' }), { after: 320 });
  await clickEl(page, page.getByRole('menuitem', { name: '配置方案' }), { after: 260 });
  await page.getByText('开发环境').first().waitFor({ timeout: 10000 });
  await shot(page, 'profiles.png');
  await closeModal(page);

  // 5) 用户 PATH
  await clickEl(page, page.locator('#tab-user'), { after: 500 });
  await shot(page, 'user-path.png');

  // 6) 合并预览
  await clickEl(page, page.locator('#tab-merged'), { after: 550 });
  await shot(page, 'merge-preview.png');

  // 7) 全部变量
  await clickEl(page, page.locator('#tab-allVars'), { after: 300 });
  await page.getByText('CARGO_HOME').waitFor({ timeout: 10000 });
  await shot(page, 'all-env-vars.png');

  // 8) 环境变量备份与恢复：选中一份备份以显示差异
  await clickEl(page, page.getByRole('button', { name: '环境变量备份与恢复' }), { after: 400 });
  await page.locator('[data-backup-file]').first().waitFor({ timeout: 10000 });
  await clickEl(page, page.locator('[data-backup-file]').first(), { after: 500 });
  await page.getByTestId('backup-summary').waitFor({ timeout: 10000 });
  await shot(page, 'env-backup.png');
  await closeModal(page);

  // 9) 历史记录
  await clickEl(page, page.locator('#tab-history'), { after: 300 });
  await page.getByText(/已显示 \d+ \/ \d+ 条/).waitFor({ timeout: 10000 });
  await shot(page, 'history.png');

  // 10) 搜索：输入并提交
  await clickEl(page, page.locator('#tab-search'), { after: 350 });
  const searchBox = page.getByPlaceholder('搜索路径、变量名、配置…');
  await typeInto(page, searchBox, 'node', 45);
  await clickEl(page, page.getByRole('button', { name: '搜索', exact: true }), { after: 600 });
  await page.getByText('C:\\Program Files\\nodejs\\').first().waitFor({ timeout: 10000 });
  await shot(page, 'search.png');

  // 11) 统计
  await clickEl(page, page.locator('#tab-stats'), { after: 300 });
  await page.getByText('最近 30 天').waitFor({ timeout: 10000 });
  await shot(page, 'stats.png');

  // 12) 同步中心
  await clickEl(page, page.locator('#tab-sync'), { after: 300 });
  await page.getByText(/待导出 \d+ 条/).waitFor({ timeout: 10000 });
  await shot(page, 'sync.png');

  await context.close();

  // 13) 深色模式（独立 context，避免复用到已渲染的浅色页面）
  const darkContext = await browser.newContext({ viewport: VIEWPORT, locale: 'zh-CN' });
  const darkPage = await darkContext.newPage();
  await darkPage.addInitScript(() => localStorage.setItem('darkMode', '1'));
  await darkPage.addInitScript(installMock, {
    system: SYSTEM_PATHS,
    user: USER_PATHS,
    invalid: [],
  });
  await darkPage.goto(BASE_URL, { waitUntil: 'networkidle' });
  await darkPage.getByTestId('path-row').first().waitFor();
  await darkPage.waitForTimeout(400);
  await darkPage.screenshot({ path: resolve(SHOTS, 'dark-mode.png') });
  console.log('  ✓ dark-mode.png');
  await darkContext.close();
}

const main = async () => {
  const argv = process.argv.slice(2);
  const flags = new Set(argv.filter((a) => a.startsWith('--')));
  mkdirSync(SHOTS, { recursive: true });

  const { server } = await ensurePreview({ root: ROOT, skipBuild: flags.has('--skip-build') });
  let browser;
  try {
    browser = await chromium.launch({ headless: true });
    await captureGui(browser);
    if (!flags.has('--only-gui')) {
      await captureCli(browser, resolve(SHOTS, 'cli-demo.png'));
    }
  } finally {
    await browser?.close();
    killTree(server);
  }
};

main().catch((error) => {
  console.error('❌ 截图失败:', error.message ?? error);
  process.exit(1);
});
