import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useAppStore } from '@/store/app-store';
import { useEnvStore } from '@/store/env-store';
import type { EnvVarMeta } from '@/core/env-var';

const hoisted = vi.hoisted(() => ({
  reset: vi.fn(),
  state: {
    query: 'jdk',
    submitted: 'jdk',
    hits: [] as Array<Record<string, unknown>>,
    loading: false,
    error: null as string | null,
    setQuery: vi.fn(),
    run: vi.fn(),
  },
}));

vi.mock('@/store/search-store', () => {
  const hook = () => hoisted.state;
  (hook as unknown as { getState: () => unknown }).getState = () => ({ reset: hoisted.reset });
  return { useSearchStore: hook };
});

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>();
  return {
    ...actual,
    useTranslation: () => ({ t: (key: string) => key }),
  };
});

import { SearchPanel } from '@/components/search/SearchPanel';

function hit(over: Record<string, unknown> = {}) {
  return {
    kind: 'path',
    refUid: 'snap#system',
    content: 'C:\\Java\\jdk21\\bin',
    snippet: 'C:\\Java\\[jdk21]\\bin',
    rank: -1,
    ...over,
  };
}

const envMeta = (name: string): EnvVarMeta => ({
  name,
  kind: 'string',
  hive: 'user',
  canEdit: true,
  canDelete: true,
  sensitive: false,
  preview: name,
  revision: 'r1',
});

beforeEach(() => {
  hoisted.state.hits = [];
  useAppStore.setState({
    sysPaths: [{ path: 'C:\\Java\\jdk21\\bin', enabled: true }],
    userPaths: [],
    activeTab: 'system',
    selectedIndices: [],
    scrollToIndex: null,
  });
  useEnvStore.setState({
    snapshot: { system: [], user: [], capturedAt: 0 },
    selectedKey: null,
    scrollToKey: null,
    hiveFilter: 'all',
  });
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => cleanup());

describe('SearchPanel 结果可操作性', () => {
  it('命中仍在当前 PATH 时，定位按钮可用并写入 tab / 选中 / 滚动请求', () => {
    hoisted.state.hits = [hit()];
    render(<SearchPanel />);

    const locate = screen.getByRole('button', {
      name: 'search.openPath',
    }) as HTMLButtonElement;
    expect(locate.disabled).toBe(false);
    fireEvent.click(locate);

    const state = useAppStore.getState();
    expect(state.activeTab).toBe('system');
    expect(state.selectedIndices).toEqual([0]);
    expect(state.scrollToIndex).toBe(0);
  });

  it('命中来自历史快照时，定位按钮禁用而不是点了没反应', () => {
    hoisted.state.hits = [hit({ content: 'D:\\old\\gone' })];
    render(<SearchPanel />);

    const locate = screen.getByRole('button', {
      name: 'search.openPath',
    }) as HTMLButtonElement;
    expect(locate.disabled).toBe(true);
  });

  it('复制按钮写入完整 content，而不是截断的 snippet', async () => {
    hoisted.state.hits = [hit()];
    render(<SearchPanel />);

    fireEvent.click(screen.getByRole('button', { name: 'search.copy' }));

    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith('C:\\Java\\jdk21\\bin'),
    );
  });

  it('变量命中定位到「全部变量」并设置选中键与滚动请求', () => {
    hoisted.state.hits = [
      hit({ kind: 'envName', content: 'JAVA_HOME', refUid: 'snap#user#JAVA_HOME' }),
    ];
    useEnvStore.setState({
      snapshot: { system: [], user: [envMeta('JAVA_HOME')], capturedAt: 0 },
    });
    render(<SearchPanel />);

    fireEvent.click(screen.getByRole('button', { name: 'search.openVar' }));

    expect(useAppStore.getState().activeTab).toBe('allVars');
    expect(useEnvStore.getState().selectedKey).toBe('user:JAVA_HOME');
    expect(useEnvStore.getState().scrollToKey).toBe('user:JAVA_HOME');
  });
});
