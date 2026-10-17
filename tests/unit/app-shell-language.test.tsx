import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';

// 与 app-shell-env-vars.test.tsx 同一套 mock 边界：IPC、对话框、虚拟滚动。
// 本文件只关心语言切换，但 AppShell 挂载会触达这些边界，必须在 jsdom 下隔离。
vi.mock('@/services/backend', async () => {
  const { vi: viModule } = await import('vitest');
  const { confirm: tauriConfirm } = await import('@tauri-apps/plugin-dialog');
  return {
    backend: {
      listAllEnvVars: viModule.fn(),
      loadPathSnapshot: viModule.fn(),
      getPathCapabilities: viModule.fn(),
      expandEnvVars: viModule.fn(),
      validatePath: viModule.fn(),
      confirmDialog: (message: string) => tauriConfirm(message),
    },
  };
});

vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onCloseRequested: () => Promise.resolve(() => {}),
    destroy: () => Promise.resolve(),
  }),
}));

vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: (options: Record<string, number>) => ({
    getVirtualItems: () =>
      Array.from({ length: options.count }).map((_, index) => ({
        index,
        start: index * 34,
        size: 34,
        key: `mock-key-${index}`,
      })),
    getTotalSize: () => options.count * 34,
    measureElement: () => {},
  }),
}));

import { AppShell } from '@/components/layout/AppShell';
import { backend } from '@/services/backend';
import i18n from '@/i18n';
import { useAppStore } from '@/store/app-store';
import { useEnvStore } from '@/store/env-store';

const mockBackend = vi.mocked(backend);

/** 打开 ⋯ 菜单并点击「切换语言」条目（菜单文案随当前语言变化）。 */
function clickToggleLanguage(): void {
  fireEvent.click(screen.getByRole('button', { name: i18n.t('menu.label') }));
  fireEvent.click(screen.getByRole('menuitem', { name: i18n.t('menu.language') }));
}

beforeEach(() => {
  vi.resetAllMocks();
  mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [], capturedAt: 0 });
  mockBackend.loadPathSnapshot.mockResolvedValue({ system: [], user: [] });
  mockBackend.getPathCapabilities.mockResolvedValue({
    canReadSystem: false,
    canWriteSystem: false,
    canReadUser: true,
    canWriteUser: true,
  });
  mockBackend.expandEnvVars.mockResolvedValue('');
  mockBackend.validatePath.mockResolvedValue(true);

  useAppStore.setState({
    activeTab: 'system',
    isModified: false,
    sysPaths: [],
    userPaths: [],
    isAdmin: false,
    pathCapabilities: {
      canReadSystem: false,
      canWriteSystem: false,
      canReadUser: true,
      canWriteUser: true,
    },
  });
  useEnvStore.setState({ draft: new Map(), snapshot: { system: [], user: [], capturedAt: 0 } });
});

// 本仓库未开启 vitest globals，RTL 自动 cleanup 不生效
afterEach(async () => {
  cleanup();
  localStorage.removeItem('i18nextLng');
  // 复位到中文并清理缓存键：keepCache 使 changeLanguage 会写回 localStorage，避免残留到其它用例
  await i18n.changeLanguage('zh-CN');
  vi.restoreAllMocks();
});

describe('语言切换按解析后语言判定（区域变体回归）', () => {
  /**
   * 模拟「英文 Windows + 检测器已把区域变体写进缓存」的真实启动态：
   * 重跑一次 i18n.init 让检测器走完整路径 —— 读 localStorage 的 'en-US' →
   * 解析为 'en' → 再把解析结果回写缓存。此时 localStorage 与 resolvedLanguage 不一致，
   * 正是旧实现（读 localStorage 原始值）误判的前提条件。
   */
  async function bootWithCachedLanguage(cached: string): Promise<void> {
    // 检测器在存在区域变体时返回 [lang, baseLang]，i18next 解析后会把两者都回写缓存。
    // 无论回写成 'en' 还是 'en-US'，旧实现读到的都不是 'zh-CN' —— 缺陷现场得以保留。
    localStorage.setItem('i18nextLng', cached);
    await i18n.init();
  }

  it('缓存为 en-US 时点击「切换语言」能切到中文', async () => {
    await bootWithCachedLanguage('en-US');
    expect(i18n.resolvedLanguage).toBe('en');
    // 启动后缓存不再是干净的 'zh-CN'：旧实现按原始缓存字符串比较即会误判
    expect(localStorage.getItem('i18nextLng')).not.toBe('zh-CN');

    render(<AppShell />);
    // 前置断言：初始界面确为英文
    expect(screen.getByRole('button', { name: 'New' })).toBeInTheDocument();

    clickToggleLanguage();

    // 切换后界面为中文：工具栏主行出现「新建」
    await waitFor(() => expect(screen.getByRole('button', { name: '新建' })).toBeInTheDocument());
    expect(i18n.resolvedLanguage).toBe('zh-CN');
  });

  it('中文界面点击「切换语言」切到英文', async () => {
    await bootWithCachedLanguage('zh-CN');

    render(<AppShell />);
    expect(screen.getByRole('button', { name: '新建' })).toBeInTheDocument();

    clickToggleLanguage();

    await waitFor(() => expect(screen.getByRole('button', { name: 'New' })).toBeInTheDocument());
    expect(i18n.resolvedLanguage).toBe('en');
  });

  it('zh-TW 等中文区域变体同样能切到英文（再切回中文）', async () => {
    // zh-TW 无对应资源，解析结果回落到 zh-CN/zh —— startsWith('zh') 判定成立
    await bootWithCachedLanguage('zh-TW');
    expect(i18n.resolvedLanguage?.startsWith('zh')).toBe(true);
    render(<AppShell />);

    clickToggleLanguage();
    await waitFor(() => expect(i18n.resolvedLanguage).toBe('en'));

    clickToggleLanguage();
    await waitFor(() => expect(i18n.resolvedLanguage?.startsWith('zh')).toBe(true));
  });
});
