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
   * 模拟「检测器已把语言写进缓存」的真实启动态：重跑一次 i18n.init 让检测器走完整路径 ——
   * 读 localStorage 的原始值 → 解析 → 把解析结果回写缓存。
   * 注意：不同区域变体的回写结果不同（见各用例中的具体断言），是否构成
   * 「缓存原始值 ≠ resolvedLanguage」的缺陷现场也随之不同。
   */
  async function bootWithCachedLanguage(cached: string): Promise<void> {
    localStorage.setItem('i18nextLng', cached);
    await i18n.init();
  }

  // 互补场景用例，不是回归用例：启动后缓存保留为 'en-US'，但 resolvedLanguage 是 'en'。
  // 旧实现「原始缓存 ≠ 'zh-CN' 即视为英文」在这一场景下恰好得出正确结论，因此本用例
  // 在修复前后都通过，不提供回归保护。回归能力由 zh-TW 用例承担；本用例只保证
  // 缓存英文区域变体时的启动与切换行为未退化。
  it('缓存为英文区域变体 en-US（解析为 en）时界面为英文并能切到中文', async () => {
    await bootWithCachedLanguage('en-US');
    expect(i18n.resolvedLanguage).toBe('en');
    // 断言缓存的实际值：检测器把 'en-US' 原样留在缓存里，resolvedLanguage 与之不同
    expect(localStorage.getItem('i18nextLng')).toBe('en-US');

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

  // 唯一的真实回归用例：zh-TW 在启动后缓存仍保留为非 'zh-CN' 的区域变体，
  // 旧实现按原始缓存字符串 === 'zh-CN' 比较会把中文界面当成英文，切换方向反了。
  it('zh-TW 等中文区域变体同样能切到英文（再切回中文）', async () => {
    // zh-TW 无对应资源，解析结果回落到 zh-CN/zh —— startsWith('zh') 判定成立
    await bootWithCachedLanguage('zh-TW');
    expect(i18n.resolvedLanguage?.startsWith('zh')).toBe(true);
    render(<AppShell />);
    expect(screen.getByRole('button', { name: '新建' })).toBeTruthy();

    clickToggleLanguage();
    await waitFor(() => expect(i18n.resolvedLanguage).toBe('en'));
    // 断言可见界面：工具栏切换为英文
    await waitFor(() => expect(screen.getByRole('button', { name: 'New' })).toBeTruthy());

    clickToggleLanguage();
    await waitFor(() => expect(i18n.resolvedLanguage?.startsWith('zh')).toBe(true));
    // 断言可见界面：工具栏切回中文
    await waitFor(() => expect(screen.getByRole('button', { name: '新建' })).toBeTruthy());
  });
});
