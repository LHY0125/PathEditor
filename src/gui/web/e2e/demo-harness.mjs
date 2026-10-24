/**
 * 演示/截图脚本的共享底座。
 *
 * 这里放两类东西：
 * 1. **注入进页面的函数**（`installMock` / `installCursor`）—— 会被 Playwright
 *    序列化后送进浏览器执行，因此必须自包含，不能引用模块作用域的变量。
 * 2. **宿主侧的工具**（点击、输入、起停 preview）—— 在 Node 里跑。
 *
 * 安全边界：`installMock` 覆盖 `window.__TAURI_INTERNALS__.invoke`，全程不连接
 * 真实 Tauri 后端，因此不读写真实注册表，也不碰 `~/.patheditor`。
 */
import { spawn, spawnSync } from 'node:child_process';

/** 与 docs/screenshots/ 现有截图一致的画布尺寸。 */
export const VIEWPORT = { width: 1127, height: 914 };
export const BASE_URL = 'http://127.0.0.1:4173';

/**
 * 注入 mock IPC。
 *
 * 参数形状严格对齐 `services/backend/`：
 * - validate_path / expand_env_vars 收 `{ path }`
 * - clean_path_entries 收 `{ entries }`
 * - save_path_with_sidecar 收 `{ system, user }`
 *
 * 除 PATH 通路外的 v6 面板数据都是可选的 —— 不传就返回空值，
 * 录制脚本只用到 PATH 通路，不必构造全部夹具。
 *
 * @param {{
 *   system: string[], user: string[], invalid: string[],
 *   scan?: object, history?: object, snapshot?: object, search?: any[],
 *   stats?: object, syncStatus?: object, syncConflicts?: any[],
 *   profiles?: any[], envVars?: object, envBackups?: any[],
 *   envPreview?: object, revealed?: Record<string, object>
 * }} data
 */
export function installMock(data) {
  const CAPS = {
    canReadSystem: true,
    canWriteSystem: true,
    canReadUser: true,
    canWriteUser: true,
  };
  const invalid = new Set(data.invalid ?? []);
  const EMPTY_DIFF = () => ({ added: [], removed: [], reordered: [], toggled: [] });

  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args) => {
      switch (cmd) {
        case 'check_admin':
          return true;
        case 'get_path_capabilities':
          return CAPS;
        case 'load_path_snapshot':
          return {
            system: data.system.map((path) => ({ path, enabled: true })),
            user: data.user.map((path) => ({ path, enabled: true })),
          };
        case 'load_system_paths':
          return data.system;
        case 'load_user_paths':
          return data.user;
        case 'validate_path':
          return !invalid.has(args?.path);
        case 'expand_env_vars':
          return args?.path ?? '';
        case 'clean_path_entries': {
          const entries = args?.entries ?? [];
          return [
            entries.filter((e) => !invalid.has(e.path)),
            entries.filter((e) => invalid.has(e.path)),
          ];
        }
        case 'scan_paths':
          return data.scan ?? { conflicts: [], tools: [] };
        case 'save_path_with_sidecar':
          return { system: 'applied', user: 'applied', sidecar: 'saved' };
        case 'backup_registry':
          return 'C:\\backup\\path.txt';
        case 'broadcast_env_change':
          return null;

        // ── v6：历史 / 检索 / 统计 / 同步 / 配置 / 备份 ──
        case 'history_list_events':
          return data.history ?? { items: [], total: 0 };
        case 'history_get_snapshot':
        case 'history_get_current':
          return data.snapshot ?? null;
        case 'history_restore_snapshot':
          return { system: 'applied', user: 'applied', sidecar: 'saved' };
        case 'search':
          return data.search ?? [];
        case 'stats_dashboard':
          return (
            data.stats ?? {
              days: [],
              topTargets: [],
              profileApplies: [],
              currentAvgLen: 0,
              currentPathItems: 0,
            }
          );
        case 'sync_status':
          return data.syncStatus ?? { pendingCount: 0, unresolvedConflicts: 0 };
        case 'sync_list_conflicts':
          return data.syncConflicts ?? [];
        case 'profile_list':
          return data.profiles ?? [];
        case 'profile_paths':
          return null;
        case 'profile_mark_applied':
        case 'profile_mark_applied_by_name':
          return true;
        case 'list_all_env_vars':
          return data.envVars ?? { system: [], user: [] };
        case 'reveal_env_var':
          return data.revealed?.[args?.name] ?? null;
        case 'list_env_backups':
          return data.envBackups ?? [];
        case 'preview_env_backup':
          return (
            data.envPreview ?? {
              changes: [],
              added: 0,
              modified: 0,
              removed: 0,
              conflicts: 0,
            }
          );
        case 'backup_env_vars':
          return 'C:\\backups\\env_backup_20261024_150000_000.json';
        case 'compare_snapshots':
          return {
            system: EMPTY_DIFF(),
            user: EMPTY_DIFF(),
            envAdded: [],
            envRemoved: [],
            envModified: [],
          };
        case 'refs_dependents':
        case 'refs_dependencies':
        case 'refs_cycles':
        case 'backup_list':
        case 'scan_history_list':
          return [];
        case 'stats_rebuild':
        case 'search_rebuild':
        case 'refs_rebuild':
        case 'search_rebuild_done':
          return 0;
        case 'scan_history_record':
          return 'scan-fixture';
        case 'plugin:dialog|message':
          return 'Cancel';
        default:
          console.warn('[demo-mock] 未处理的 IPC: ' + cmd);
          return null;
      }
    },
  };
}

/**
 * 注入可见鼠标指针 + 点击涟漪。
 *
 * Playwright 原生录屏**不渲染鼠标指针**，不补这一层的话观众看不出点击落在哪。
 * 指针 `pointer-events:none`，不会干扰真实点击。静态截图不需要调用它。
 */
export function installCursor() {
  const style = document.createElement('style');
  style.textContent = `
    #demo-cursor{position:fixed;left:0;top:0;width:16px;height:16px;margin:-8px 0 0 -8px;
      border-radius:50%;background:rgba(37,99,235,.9);border:2px solid #fff;
      box-shadow:0 2px 8px rgba(0,0,0,.45);pointer-events:none;z-index:2147483647;
      opacity:0}
    .demo-ripple{position:fixed;width:14px;height:14px;margin:-7px 0 0 -7px;border-radius:50%;
      border:2px solid rgba(37,99,235,.95);pointer-events:none;z-index:2147483646;
      animation:demo-ripple .45s ease-out forwards}
    @keyframes demo-ripple{to{width:56px;height:56px;margin:-28px 0 0 -28px;opacity:0}}
  `;
  const mount = () => {
    document.head.appendChild(style);
    const cur = document.createElement('div');
    cur.id = 'demo-cursor';
    document.body.appendChild(cur);
    window.addEventListener(
      'mousemove',
      (e) => {
        cur.style.left = e.clientX + 'px';
        cur.style.top = e.clientY + 'px';
        cur.style.opacity = '1';
      },
      true,
    );
    window.addEventListener(
      'mousedown',
      (e) => {
        const r = document.createElement('div');
        r.className = 'demo-ripple';
        r.style.left = e.clientX + 'px';
        r.style.top = e.clientY + 'px';
        document.body.appendChild(r);
        setTimeout(() => r.remove(), 520);
      },
      true,
    );
  };
  if (document.body) mount();
  else document.addEventListener('DOMContentLoaded', mount);
}

/** 平滑移动鼠标到元素中心后点击（让录屏里的指针轨迹可读）。 */
export async function clickEl(page, locator, { hover = 170, after = 320 } = {}) {
  const box = await locator.boundingBox();
  if (!box) throw new Error('目标元素不可见，无法点击');
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 16 });
  await page.waitForTimeout(hover);
  await page.mouse.down();
  await page.waitForTimeout(70);
  await page.mouse.up();
  await page.waitForTimeout(after);
}

/** 逐字符输入，模拟真人打字节奏。 */
export async function typeInto(page, locator, text, delay = 40) {
  await clickEl(page, locator, { after: 140 });
  await locator.fill('');
  await locator.pressSequentially(text, { delay });
}

/** 轮询直到 preview 服务可用。 */
export async function waitForServer(base, timeoutMs = 60000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(base, { signal: AbortSignal.timeout(1500) });
      if (res.ok) return true;
    } catch {
      // 还没起来，继续等
    }
    await new Promise((r) => setTimeout(r, 400));
  }
  return false;
}

/** 结束 preview 进程树（Windows 用 taskkill /T，其余用进程组信号）。 */
export function killTree(child) {
  if (!child?.pid) return;
  if (process.platform === 'win32') {
    spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore' });
  } else {
    try {
      process.kill(-child.pid, 'SIGTERM');
    } catch {
      // 进程可能已退出
    }
  }
}

/**
 * 确保 preview 服务可用：已有服务则复用，否则自己构建 + 启动。
 *
 * @returns {{ server: import('node:child_process').ChildProcess | null }}
 */
export async function ensurePreview({ root, skipBuild = false } = {}) {
  if (!skipBuild) {
    console.log('▶ 构建前端…');
    // 传单个命令串而非「命令 + 参数数组」：后者配 shell:true 会触发 Node 的
    // DEP0190 警告（参数未转义）。这里没有外部输入，整串交给 shell 更合适。
    const build = spawnSync('npm run build', { cwd: root, stdio: 'inherit', shell: true });
    if (build.status !== 0) throw new Error('构建失败，先修好类型/打包错误再继续');
  }

  if (await waitForServer(BASE_URL, 1200)) return { server: null };

  console.log('▶ 启动 preview 服务…');
  const server = spawn('npm run preview -- --host 127.0.0.1 --port 4173 --strictPort', {
    cwd: root,
    shell: true,
    stdio: 'ignore',
    detached: process.platform !== 'win32',
  });
  if (!(await waitForServer(BASE_URL))) {
    killTree(server);
    throw new Error('preview 服务未能在 60 秒内就绪');
  }
  return { server };
}
