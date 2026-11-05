import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@/i18n', () => ({
  default: { t: (key: string) => key },
}));

const { syncBackend } = vi.hoisted(() => ({
  syncBackend: {
    syncStatus: vi.fn(),
    syncListConflicts: vi.fn(),
    syncExport: vi.fn(),
    syncImport: vi.fn(),
    syncResolveConflict: vi.fn(),
  },
}));

vi.mock('@/services/backend', () => ({
  backend: syncBackend,
}));

import { useSyncStore } from '@/store/sync-store';

describe('同步中心 store', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSyncStore.setState({
      status: null,
      conflicts: [],
      loading: false,
      error: null,
      notice: null,
    });
    syncBackend.syncStatus.mockResolvedValue({ pendingCount: 0, unresolvedConflicts: 0 });
    syncBackend.syncListConflicts.mockResolvedValue([]);
  });

  it('解决冲突成功后写结构化 notice 并刷新状态', async () => {
    syncBackend.syncResolveConflict.mockResolvedValue(undefined);
    syncBackend.syncStatus.mockResolvedValue({ pendingCount: 3, unresolvedConflicts: 0 });

    const ok = await useSyncStore.getState().resolve(7, 'remote');

    expect(ok).toBe(true);
    expect(useSyncStore.getState().notice).toEqual({
      kind: 'resolved',
      id: 7,
      resolution: 'remote',
    });
    // 成功后必须刷新：未解决数归零要立刻反映到界面
    expect(syncBackend.syncStatus).toHaveBeenCalled();
    expect(useSyncStore.getState().status?.pendingCount).toBe(3);
  });

  it('包格式错误原样展示后端文案（重试无用的错误要说清楚）', async () => {
    syncBackend.syncImport.mockRejectedValue({ code: 'syncFormat', message: '同步包已被篡改' });

    const ok = await useSyncStore.getState().importBundle('pw', 'x.pesync');

    expect(ok).toBe(false);
    expect(useSyncStore.getState().error).toBe('同步包已被篡改');
    expect(useSyncStore.getState().loading).toBe(false);
  });

  it('DB 不可用时显示降级文案而不是内部错误', async () => {
    syncBackend.syncExport.mockRejectedValue({ code: 'database', message: 'no such table' });

    const ok = await useSyncStore.getState().exportBundle('pw', 'x.pesync');

    expect(ok).toBe(false);
    expect(useSyncStore.getState().error).toBe('sync.errorDegraded');
  });
});
