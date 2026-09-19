import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';

// 异步工厂 + vi.mocked(backend)：与 Task 5 的 env-store.test.ts 保持一致。
vi.mock('@/services/backend', async () => {
  const { vi: viModule } = await import('vitest');
  return {
    backend: {
      listAllEnvVars: viModule.fn(),
      revealEnvVar: viModule.fn(),
      updateEnvVar: viModule.fn(),
      createEnvVar: viModule.fn(),
      deleteEnvVar: viModule.fn(),
    },
  };
});

// i18n mock：**部分 mock**，保留 initReactI18next 等真实导出（src/i18n 在模块加载时要用），
// 只覆盖 useTranslation，并以真实 zh-CN.json 词条取值 —— 与 app-shell-env-vars.test.tsx
// 同一策略，避免手写文案副本与真实词条漂移（S6）。
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

// jsdom 下容器高度为 0，真实虚拟滚动不会渲染任何行；
// 按 merge-preview.test.tsx 的既有做法 mock 掉，让 getVirtualItems 返回全部条目。
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

import { EnvVarTable } from '@/components/env-list/EnvVarTable';
import { backend } from '@/services/backend';
import { useEnvStore } from '@/store/env-store';
import type { EnvVarMeta } from '@/core/env-var';

const mockBackend = vi.mocked(backend);

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

const snapshot = {
  system: [
    meta({ name: 'windir', hive: 'system' }),
    meta({
      name: 'SYS_BIN',
      hive: 'system',
      kind: 'unsupported',
      preview: null,
      canEdit: false,
      canDelete: false,
    }),
  ],
  user: [
    meta({ name: 'JAVA_HOME', hive: 'user' }),
    meta({ name: 'MY_TOKEN', hive: 'user', sensitive: true, preview: null }),
  ],
  capturedAt: 0,
};

beforeEach(() => {
  // resetAllMocks 而非 clearAllMocks：后者保留 mock 实现，会导致跨用例残留。
  vi.resetAllMocks();
  useEnvStore.setState({
    snapshot,
    revealed: new Map(),
    draft: new Map(),
    hiveFilter: 'all',
    isLoading: false,
    isSaving: false,
    statusMessage: '',
  });
});

// 本仓库未开启 vitest globals，RTL 的自动 cleanup 不会生效（S6）：
// 不显式清理会让 screen（绑定 document.body）跨用例累积多个组件树。
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('EnvVarTable', () => {
  it('渲染两个 hive 的变量', () => {
    render(<EnvVarTable />);
    expect(screen.getByText('JAVA_HOME')).not.toBeNull();
    expect(screen.getByText('windir')).not.toBeNull();
  });

  it('不渲染 Path（Rust 侧已过滤，此处防御性验证）', () => {
    useEnvStore.setState({
      snapshot: { system: [meta({ name: 'Path', hive: 'system' })], user: [], capturedAt: 0 },
    });
    render(<EnvVarTable />);
    expect(screen.queryByText('Path')).toBeNull();
  });

  it('敏感值默认打码，不出现明文', () => {
    render(<EnvVarTable />);
    expect(screen.getAllByText('••••••••').length).toBeGreaterThan(0);
    expect(screen.queryByText('real-secret')).toBeNull();
  });

  it('点击「显示」后触发 reveal 并渲染明文', async () => {
    mockBackend.revealEnvVar.mockResolvedValue({ value: 'real-secret', revision: 'rev-1' });
    const { container } = render(<EnvVarTable />);

    const showButtons = screen.getAllByRole('button', { name: '显示' });
    fireEvent.click(showButtons[0]);

    // 明文出现在 MY_TOKEN 那一行内（同表其他行不含该文案）
    await waitFor(() => {
      const row = container.querySelector('[data-env-var-key="user:MY_TOKEN"]');
      expect(row?.textContent).toContain('real-secret');
    });
    expect(mockBackend.revealEnvVar).toHaveBeenCalledWith('user', 'MY_TOKEN');
  });

  it('保护行与只读行的编辑按钮禁用', () => {
    render(<EnvVarTable />);
    const editButtons = screen.getAllByRole('button', { name: /^编辑/ });
    // system 两条（windir 保护、SYS_BIN 不支持）+ 敏感打码行（MY_TOKEN）均应禁用
    const disabled = editButtons.filter((b) => (b as HTMLButtonElement).disabled);
    expect(disabled.length).toBeGreaterThanOrEqual(2);
  });

  it('顶部引导提供「前往」入口并回调', () => {
    const onGoToPath = vi.fn();
    render(<EnvVarTable onGoToPath={onGoToPath} />);
    fireEvent.click(screen.getByRole('button', { name: '前往' }));
    expect(onGoToPath).toHaveBeenCalledTimes(1);
  });

  it('Unsupported 行显示类型占位而非值', () => {
    const { container } = render(<EnvVarTable />);
    const row = container.querySelector('[data-env-var-key="system:SYS_BIN"]');
    expect(row?.textContent).toContain('(不支持的注册表类型)');
  });

  it('可编辑行点击编辑回调，敏感变量未显示时不允许编辑', () => {
    const onEdit = vi.fn();
    const { container } = render(<EnvVarTable onEdit={onEdit} />);

    const javaRow = container.querySelector('[data-env-var-key="user:JAVA_HOME"]');
    const javaEdit = javaRow?.querySelector('button[aria-label="编辑 JAVA_HOME"]');
    expect(javaEdit).not.toBeNull();
    fireEvent.click(javaEdit!);
    expect(onEdit).toHaveBeenCalledWith(expect.objectContaining({ name: 'JAVA_HOME' }));

    const tokenRow = container.querySelector('[data-env-var-key="user:MY_TOKEN"]');
    const tokenEdit = tokenRow?.querySelector('button[aria-label="编辑 MY_TOKEN"]');
    expect((tokenEdit as HTMLButtonElement | null)?.disabled).toBe(true);
  });

  it('可删除行点击删除回调', () => {
    const onDelete = vi.fn();
    const { container } = render(<EnvVarTable onDelete={onDelete} />);

    const javaRow = container.querySelector('[data-env-var-key="user:JAVA_HOME"]');
    const javaDelete = javaRow?.querySelector('button[aria-label="删除 JAVA_HOME"]');
    expect(javaDelete).not.toBeNull();
    fireEvent.click(javaDelete!);

    expect(onDelete).toHaveBeenCalledWith(expect.objectContaining({ name: 'JAVA_HOME' }));
  });

  it('顶部显示 Path 引导提示', () => {
    const { container } = render(<EnvVarTable />);
    expect(container.textContent).toContain('Path 请在');
  });

  it('切换 hiveFilter 后只显示对应 hive', () => {
    useEnvStore.setState({ hiveFilter: 'user' });
    const { container } = render(<EnvVarTable />);
    const keys = Array.from(container.querySelectorAll('[data-env-var-key]')).map((r) =>
      r.getAttribute('data-env-var-key'),
    );
    expect(keys).toContain('user:JAVA_HOME');
    expect(keys).not.toContain('system:windir');
  });
});

describe('data-env-var-key 行定位属性（Task 8 E2E 契约）', () => {
  it('每行带 `${hive}:${name}` 属性，可按 user:JAVA_HOME 精确定位', () => {
    const { container } = render(<EnvVarTable />);
    const rows = container.querySelectorAll('[data-env-var-key]');
    const keys = Array.from(rows).map((row) => row.getAttribute('data-env-var-key'));
    expect(keys).toContain('user:JAVA_HOME');
    expect(keys).toContain('user:MY_TOKEN');
    expect(keys).toContain('system:windir');
    expect(keys).toContain('system:SYS_BIN');
  });

  it('同名跨 hive 变量可被属性区分', () => {
    useEnvStore.setState({
      snapshot: {
        system: [meta({ name: 'JAVA_HOME', hive: 'system' })],
        user: [meta({ name: 'JAVA_HOME', hive: 'user' })],
        capturedAt: 0,
      },
    });
    const { container } = render(<EnvVarTable />);
    expect(container.querySelector('[data-env-var-key="system:JAVA_HOME"]')).not.toBeNull();
    expect(container.querySelector('[data-env-var-key="user:JAVA_HOME"]')).not.toBeNull();
  });
});
