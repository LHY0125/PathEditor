import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act, waitFor, cleanup } from '@testing-library/react';

// AppShell 挂载会触达 IPC / 对话框 / 虚拟滚动，沿用 app-shell-language.test.tsx 的 mock 边界。
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

// 拖放事件是窗口级订阅：把注册进来的回调存下来，由用例直接触发。
const dragDrop = vi.hoisted(() => ({
  handlers: [] as Array<(event: { payload: { type: string; paths: string[] } }) => void>,
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onCloseRequested: () => Promise.resolve(() => {}),
    onDragDropEvent: (handler: (event: { payload: { type: string; paths: string[] } }) => void) => {
      dragDrop.handlers.push(handler);
      return Promise.resolve(() => {});
    },
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

/** 等 AppShell 注册拖放回调后触发一次 drop 事件。 */
async function dropPaths(paths: string[]): Promise<void> {
  render(<AppShell />);
  await waitFor(() => expect(dragDrop.handlers).toHaveLength(1), { timeout: 2000 });
  await act(async () => {
    dragDrop.handlers[0]({ payload: { type: 'drop', paths } });
  });
}

beforeEach(() => {
  vi.resetAllMocks();
  dragDrop.handlers.length = 0;
  mockBackend.listAllEnvVars.mockResolvedValue({ system: [], user: [], capturedAt: 0 });
  mockBackend.loadPathSnapshot.mockResolvedValue({ system: [], user: [] });
  mockBackend.getPathCapabilities.mockResolvedValue({
    canReadSystem: true,
    canWriteSystem: true,
    canReadUser: true,
    canWriteUser: true,
  });
  mockBackend.expandEnvVars.mockResolvedValue('');

  useAppStore.setState({
    activeTab: 'system',
    isModified: false,
    sysPaths: [],
    userPaths: [],
    statusMessage: '',
    statusIsError: false,
    isLoading: false,
    isAdmin: false,
    pathCapabilities: {
      canReadSystem: true,
      canWriteSystem: true,
      canReadUser: true,
      canWriteUser: true,
    },
  });
  useEnvStore.setState({ draft: new Map(), snapshot: { system: [], user: [], capturedAt: 0 } });
});

afterEach(async () => {
  cleanup();
  await i18n.changeLanguage('zh-CN');
  vi.restoreAllMocks();
});

describe('拖放新增 PATH 条目', () => {
  it('拖入目录时按当前标签页新增条目', async () => {
    mockBackend.validatePath.mockResolvedValue(true);

    await dropPaths(['C:\\Tools\\bin']);

    await waitFor(() => expect(useAppStore.getState().sysPaths).toHaveLength(1));
    expect(useAppStore.getState().sysPaths[0].path).toBe('C:\\Tools\\bin');
    expect(useAppStore.getState().statusMessage).toBe('');
  });

  it('拖入文件时提示只能拖拽文件夹，且不新增条目', async () => {
    mockBackend.validatePath.mockResolvedValue(false);

    await dropPaths(['C:\\Tools\\readme.txt']);

    await waitFor(() =>
      expect(useAppStore.getState().statusMessage).toBe(i18n.t('status.dragFolderOnly')),
    );
    expect(useAppStore.getState().sysPaths).toHaveLength(0);
    expect(screen.getByText(i18n.t('status.dragFolderOnly'))).toBeTruthy();
  });
});
