import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/react';

/**
 * 同步中心组件。
 *
 * 三条不可退让的断言：
 * 1. **口令输入是 `type="password"`**，且提交后立刻清空（不留在界面/组件状态里）
 * 2. **同步不写注册表的说明必须可见**——否则用户以为导入完 PATH 就变了
 * 3. 冲突按实体类型分组，解决按钮把 `(id, 'local'|'remote')` 原样传给 store
 */

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>();
  const zh = ((await import('@/i18n/locales/zh-CN.json')).default ?? {}) as Record<string, unknown>;
  const t = (key: string, params?: Record<string, unknown>): string => {
    let node: unknown = zh;
    for (const part of key.split('.')) {
      if (node === null || typeof node !== 'object') return key;
      node = (node as Record<string, unknown>)[part];
    }
    if (typeof node !== 'string') return key;
    if (params) {
      return node.replace(/\{\{(\w+)\}\}/g, (_, k: string) => String(params[k] ?? `{{${k}}}`));
    }
    return node;
  };
  return { ...actual, useTranslation: () => ({ t }) };
});

const exportBundle = vi.fn().mockResolvedValue(true);
const importBundle = vi.fn().mockResolvedValue(true);
const resolve = vi.fn().mockResolvedValue(true);
const refresh = vi.fn().mockResolvedValue(undefined);

let mockState: Record<string, unknown>;

vi.mock('@/store/sync-store', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/store/sync-store')>();
  return {
    ...actual,
    useSyncStore: () => mockState,
  };
});

import { SyncCenter } from '@/components/sync/SyncCenter';

const conflict = (over: Record<string, unknown> = {}) => ({
  id: 7,
  entityType: 'profile',
  entityUid: 'p-1',
  field: 'name',
  localValue: '"local"',
  remoteValue: '"remote"',
  remoteDevice: 'dev-b',
  createdAt: 1_700_000_000_000,
  resolvedAt: null,
  resolution: null,
  ...over,
});

beforeEach(() => {
  mockState = {
    status: { pendingCount: 2, unresolvedConflicts: 1 },
    conflicts: [conflict()],
    loading: false,
    error: null,
    notice: null,
    refresh,
    exportBundle,
    importBundle,
    resolve,
    clearMessages: vi.fn(),
  };
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('SyncCenter', () => {
  it('显示待导出数与未解决冲突数', () => {
    render(<SyncCenter />);
    expect(screen.getByText('待导出 2 条 · 未解决冲突 1 条')).toBeTruthy();
  });

  it('把「导入不改注册表」的口径写出来', () => {
    render(<SyncCenter />);
    expect(screen.getByText(/导入只更新本机数据库，不会改动系统 PATH/)).toBeTruthy();
  });

  it('口令输入是密码框，且提交后立刻清空', async () => {
    render(<SyncCenter />);

    const passphrase = screen.getByPlaceholderText(/用于加解密同步包/) as HTMLInputElement;
    const filePath = screen.getByPlaceholderText(/例如 D:/) as HTMLInputElement;
    expect(passphrase.type).toBe('password');

    fireEvent.change(passphrase, { target: { value: 'correct horse' } });
    fireEvent.change(filePath, { target: { value: 'D:\\a.pesync' } });
    expect(passphrase.value).toBe('correct horse');

    fireEvent.click(screen.getByRole('button', { name: '导出' }));

    // 口令必须以明文参数传给 store，但界面上的值必须被清空
    expect(exportBundle).toHaveBeenCalledWith('correct horse', 'D:\\a.pesync');
    // `onExport` 是 async：清空发生在 await 之后，需等一次微任务
    await waitFor(() => expect(passphrase.value).toBe(''));
  });

  it('口令为空时导出/导入按钮不可用', () => {
    render(<SyncCenter />);
    const exportBtn = screen.getByRole('button', { name: '导出' }) as HTMLButtonElement;
    const importBtn = screen.getByRole('button', { name: '导入' }) as HTMLButtonElement;
    expect(exportBtn.disabled).toBe(true);
    expect(importBtn.disabled).toBe(true);
  });

  it('冲突按实体类型分组，解决按钮传递 (id, local|remote)', () => {
    mockState.conflicts = [
      conflict({ id: 7, entityType: 'profile' }),
      conflict({ id: 8, entityType: 'snapshot', entityUid: 's-1', field: null }),
    ];
    render(<SyncCenter />);

    expect(screen.getByRole('heading', { name: '配置方案' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '快照' })).toBeTruthy();

    const takeRemote = screen.getAllByRole('button', { name: '采纳远端' });
    fireEvent.click(takeRemote[1]);
    expect(resolve).toHaveBeenCalledWith(8, 'remote');

    const takeLocal = screen.getAllByRole('button', { name: '保留本地' });
    fireEvent.click(takeLocal[0]);
    expect(resolve).toHaveBeenCalledWith(7, 'local');
  });

  it('无冲突时显示明确空态，不是空白', () => {
    mockState.conflicts = [];
    mockState.status = { pendingCount: 0, unresolvedConflicts: 0 };
    render(<SyncCenter />);
    expect(screen.getByText('没有待解决的冲突')).toBeTruthy();
  });

  it('错误文案展示为 alert', () => {
    mockState.error = '同步不可用（降级模式）';
    render(<SyncCenter />);
    expect(screen.getByRole('alert').textContent).toContain('降级模式');
  });

  it('导入提示必须显式说出「有 N 条没同步」，不能只说应用了几条', () => {
    // store 只给结构化 notice，中文文案由组件经 t 产出
    mockState.notice = {
      kind: 'imported',
      report: {
        applied: 1,
        skipped: 0,
        conflicts: 0,
        skippedUnsupported: 2,
        renamedProfiles: 0,
      },
    };
    render(<SyncCenter />);
    const status = screen.getByRole('status').textContent ?? '';
    expect(status).toContain('应用 1');
    expect(status).toContain('2 条（配置方案）本版本还不支持同步');
  });

  it('重名自动改名时提示必须说出来', () => {
    // 用户会发现配置名字变了；不说出来就会以为是同步把名字弄坏了
    mockState.notice = {
      kind: 'imported',
      report: {
        applied: 1,
        skipped: 0,
        conflicts: 0,
        skippedUnsupported: 0,
        renamedProfiles: 1,
      },
    };
    render(<SyncCenter />);
    expect(screen.getByRole('status').textContent).toContain('1 条配置因与本地重名已自动改名');
  });

  it('没有不支持项时不显示那条附加说明', () => {
    mockState.notice = {
      kind: 'imported',
      report: {
        applied: 3,
        skipped: 0,
        conflicts: 0,
        skippedUnsupported: 0,
        renamedProfiles: 0,
      },
    };
    render(<SyncCenter />);
    expect(screen.getByRole('status').textContent).not.toContain('不支持同步');
  });
});
