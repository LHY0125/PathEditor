import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';

// 与 env-var-table.test.tsx 保持一致的 mock 策略。
vi.mock('@/services/backend', async () => {
  const { vi: viModule } = await import('vitest');
  return {
    backend: {
      listAllEnvVars: viModule.fn(),
    },
  };
});

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>();
  const zh = ((await import('@/i18n/locales/zh-CN.json')).default ?? {}) as Record<string, unknown>;
  const t = (key: string): string => {
    let node: unknown = zh;
    for (const part of key.split('.')) {
      if (node === null || typeof node !== 'object') return key;
      node = (node as Record<string, unknown>)[part];
    }
    return typeof node === 'string' ? node : key;
  };
  return { ...actual, useTranslation: () => ({ t }) };
});

import { EnvVarToolbar } from '@/components/env-list/EnvVarToolbar';
import { useEnvStore } from '@/store/env-store';
import type { EnvVarMeta } from '@/core/env-var';

function meta(overrides: Partial<EnvVarMeta> = {}): EnvVarMeta {
  return {
    name: 'JAVA_HOME',
    kind: 'string',
    hive: 'user',
    canEdit: true,
    canDelete: true,
    sensitive: false,
    preview: 'C:\\Java',
    revision: 'rev-1',
    ...overrides,
  };
}

const editable = meta();
const locked = meta({ name: 'windir', hive: 'system', canEdit: false, canDelete: false });
const sensitiveLocked = meta({ name: 'MY_TOKEN', sensitive: true, preview: null });

function renderToolbar(selected: EnvVarMeta | null = null) {
  const handlers = {
    onCreate: vi.fn(),
    onEdit: vi.fn(),
    onDelete: vi.fn(),
    onRefresh: vi.fn(),
    onSearchChange: vi.fn(),
  };
  render(<EnvVarToolbar {...handlers} searchQuery="" selected={selected} />);
  return handlers;
}

beforeEach(() => {
  // resetAllMocks 而非 clearAllMocks：后者保留 mock 实现，会导致跨用例残留。
  vi.resetAllMocks();
  useEnvStore.setState({
    snapshot: { system: [], user: [], capturedAt: 0 },
    revealed: new Map(),
    draft: new Map(),
    hiveFilter: 'all',
    isLoading: false,
    isSaving: false,
    statusMessage: '',
  });
});

// 本仓库未开启 vitest globals，RTL 的自动 cleanup 不会生效。
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('EnvVarToolbar', () => {
  it('筛选按钮使用「系统/用户」短标签，而非 PATH Tab 文案', () => {
    renderToolbar();
    expect(screen.getByRole('button', { name: '全部' })).not.toBeNull();
    expect(screen.getByRole('button', { name: '系统' })).not.toBeNull();
    expect(screen.getByRole('button', { name: '用户' })).not.toBeNull();
    expect(screen.queryByRole('button', { name: '系统 PATH' })).toBeNull();
    expect(screen.queryByRole('button', { name: '用户 PATH' })).toBeNull();
  });

  it('点击筛选切换 hiveFilter 且不触发 IPC', () => {
    renderToolbar();
    fireEvent.click(screen.getByRole('button', { name: '系统' }));
    expect(useEnvStore.getState().hiveFilter).toBe('system');
  });

  it('搜索输入触发 onSearchChange', () => {
    const handlers = renderToolbar();
    fireEvent.change(screen.getByPlaceholderText('搜索变量名'), { target: { value: 'java' } });
    expect(handlers.onSearchChange).toHaveBeenCalledWith('java');
  });

  it('无选中时编辑/删除禁用，选中可编辑变量后启用', () => {
    renderToolbar(null);
    expect((screen.getByRole('button', { name: '编辑' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: '删除' }) as HTMLButtonElement).disabled).toBe(true);

    cleanup();
    renderToolbar(editable);
    expect((screen.getByRole('button', { name: '编辑' }) as HTMLButtonElement).disabled).toBe(
      false,
    );
    expect((screen.getByRole('button', { name: '删除' }) as HTMLButtonElement).disabled).toBe(
      false,
    );
  });

  it('不可编辑变量选中时编辑/删除保持禁用', () => {
    renderToolbar(locked);
    expect((screen.getByRole('button', { name: '编辑' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: '删除' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('敏感变量未显示时编辑禁用，显示明文后启用', () => {
    renderToolbar(sensitiveLocked);
    expect((screen.getByRole('button', { name: '编辑' }) as HTMLButtonElement).disabled).toBe(true);

    cleanup();
    useEnvStore.setState({ revealed: new Map([['user:MY_TOKEN', 'secret']]) });
    renderToolbar(sensitiveLocked);
    expect((screen.getByRole('button', { name: '编辑' }) as HTMLButtonElement).disabled).toBe(
      false,
    );
  });

  it('新建与刷新始终可用且回调触发', () => {
    const handlers = renderToolbar(null);
    fireEvent.click(screen.getByRole('button', { name: '新建变量' }));
    fireEvent.click(screen.getByRole('button', { name: '刷新' }));
    expect(handlers.onCreate).toHaveBeenCalledTimes(1);
    expect(handlers.onRefresh).toHaveBeenCalledTimes(1);
  });
});
