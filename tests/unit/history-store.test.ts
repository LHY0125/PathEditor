import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@/services/backend', () => ({
  backend: {
    historyListEvents: vi.fn(async () => ({ items: [], total: 0 })),
    historyGetSnapshot: vi.fn(),
  },
}));

import { backend } from '@/services/backend';
import { resetHistoryStore, useHistoryStore } from '@/store/history-store';

describe('history store', () => {
  beforeEach(() => {
    resetHistoryStore();
    vi.clearAllMocks();
  });

  it('load 写入事件与总数', async () => {
    (backend.historyListEvents as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      items: [
        {
          uid: 'a',
          createdAt: 1,
          source: 'gui',
          action: 'save',
          hive: null,
          target: null,
          result: 'success',
        },
      ],
      total: 7,
    });

    await useHistoryStore.getState().load();

    const s = useHistoryStore.getState();
    expect(s.events).toHaveLength(1);
    // total 是总行数，不等于本页条数
    expect(s.total).toBe(7);
    expect(s.loading).toBe(false);
    expect(s.error).toBeNull();
  });

  it('降级模式下 load 记录错误而不抛出', async () => {
    (backend.historyListEvents as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error('历史库不可用'),
    );

    await useHistoryStore.getState().load();

    const s = useHistoryStore.getState();
    expect(s.events).toEqual([]);
    expect(s.error).toContain('历史库不可用');
    expect(s.loading).toBe(false);
  });

  it('selectEvent 请求的是掩码视图（reveal=false）', async () => {
    const snapshot = {
      uid: 's1',
      createdAt: 1,
      kind: 'current',
      system: [],
      user: [],
      env: [],
    };
    (backend.historyGetSnapshot as ReturnType<typeof vi.fn>).mockResolvedValueOnce(snapshot);

    await useHistoryStore.getState().selectEvent({
      uid: 's1',
      createdAt: 1,
      source: 'gui',
      action: 'save',
      hive: null,
      target: null,
      result: 'success',
    });

    // 第二个参数必须是 false：历史面板默认掩码，明文只能经显式操作获取
    expect(backend.historyGetSnapshot).toHaveBeenCalledWith('s1', false);
    expect(useHistoryStore.getState().detail).toEqual(snapshot);
  });
});
