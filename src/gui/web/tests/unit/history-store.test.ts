import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@/services/backend', () => ({
  backend: {
    historyListEvents: vi.fn(async () => ({ items: [], total: 0 })),
    historyGetSnapshot: vi.fn(),
    historyRestoreSnapshot: vi.fn(),
  },
}));

import { backend } from '@/services/backend';
import { resetHistoryStore, useHistoryStore } from '@/store/history-store';

/** 构造一条最小合法事件；`snapshotUid` 可覆盖。 */
function event(overrides: Record<string, unknown> = {}) {
  return {
    uid: 'evt-1',
    createdAt: 1,
    source: 'gui',
    action: 'save' as const,
    hive: null,
    target: null,
    result: 'success' as const,
    snapshotUid: 'snap-1',
    ...overrides,
  };
}

describe('history store', () => {
  beforeEach(() => {
    resetHistoryStore();
    vi.clearAllMocks();
  });

  it('load 写入事件与总数', async () => {
    (backend.historyListEvents as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      items: [event()],
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
    // parseCoreError 抛出的是普通对象，不是 Error 实例
    (backend.historyListEvents as ReturnType<typeof vi.fn>).mockRejectedValueOnce({
      code: 'database',
      message: '历史库不可用，当前处于降级模式',
    });

    await useHistoryStore.getState().load();

    const s = useHistoryStore.getState();
    expect(s.events).toEqual([]);
    expect(s.error).toContain('历史库不可用');
    // 关键：绝不能是 "[object Object]"
    expect(s.error).not.toContain('[object Object]');
    expect(s.loading).toBe(false);
  });

  it('crypto 错误给出可操作的具体提示', async () => {
    (backend.historyListEvents as ReturnType<typeof vi.fn>).mockRejectedValueOnce({
      code: 'crypto',
      message: 'DPAPI 解密失败',
    });

    await useHistoryStore.getState().load();

    // 换机后应提示「密钥不匹配」，而不是笼统的「不可用」
    expect(useHistoryStore.getState().error).toContain('密钥');
  });

  it('selectEvent 用 snapshotUid 查询，且 reveal=false', async () => {
    const snapshot = {
      uid: 'snap-1',
      createdAt: 1,
      kind: 'current',
      system: [],
      user: [],
      env: [],
    };
    (backend.historyGetSnapshot as ReturnType<typeof vi.fn>).mockResolvedValueOnce(snapshot);

    await useHistoryStore.getState().selectEvent(event());

    // 第一个参数必须是快照 uid，不是事件 uid
    expect(backend.historyGetSnapshot).toHaveBeenCalledWith('snap-1', false);
    expect(useHistoryStore.getState().detail).toEqual(snapshot);
  });

  it('snapshotUid 为 null 时不发起查询', async () => {
    await useHistoryStore.getState().selectEvent(event({ snapshotUid: null }));

    expect(backend.historyGetSnapshot).not.toHaveBeenCalled();
    expect(useHistoryStore.getState().detail).toBeNull();
  });

  describe('跨会话撤销', () => {
    it('undoLast 恢复最近一次可撤销操作前的快照', async () => {
      (backend.historyListEvents as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
        items: [
          event({ uid: 'e1', snapshotUid: 'snap-before-1' }),
          event({ uid: 'e2', snapshotUid: 'snap-before-2' }),
        ],
        total: 2,
      });
      await useHistoryStore.getState().load();

      (backend.historyRestoreSnapshot as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
        system: 'applied',
        user: 'applied',
        sidecar: 'saved',
      });
      // restore 成功后 store 会重新 load，给一个空页避免二次 mock 缺失
      (backend.historyListEvents as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
        items: [],
        total: 0,
      });

      const undone = await useHistoryStore.getState().undoLast();

      expect(undone?.uid).toBe('e1');
      // 必须恢复「最近一条」的**操作前**快照
      expect(backend.historyRestoreSnapshot).toHaveBeenCalledWith('snap-before-1');
    });

    it('undoLast 跳过没有快照的事件', async () => {
      (backend.historyListEvents as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
        items: [
          event({ uid: 'e1', snapshotUid: null }),
          event({ uid: 'e2', snapshotUid: 'snap-2' }),
        ],
        total: 2,
      });
      await useHistoryStore.getState().load();

      (backend.historyRestoreSnapshot as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
        system: 'applied',
        user: 'applied',
        sidecar: 'saved',
      });
      (backend.historyListEvents as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
        items: [],
        total: 0,
      });

      const undone = await useHistoryStore.getState().undoLast();
      expect(undone?.uid).toBe('e2');
      expect(backend.historyRestoreSnapshot).toHaveBeenCalledWith('snap-2');
    });

    it('undoLast 跳过 restore 自身（不撤销撤销）', async () => {
      (backend.historyListEvents as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
        items: [
          event({ uid: 'e1', action: 'restore', snapshotUid: 'snap-restore' }),
          event({ uid: 'e2', action: 'save', snapshotUid: 'snap-save' }),
        ],
        total: 2,
      });
      await useHistoryStore.getState().load();

      (backend.historyRestoreSnapshot as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
        system: 'applied',
        user: 'applied',
        sidecar: 'saved',
      });
      (backend.historyListEvents as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
        items: [],
        total: 0,
      });

      const undone = await useHistoryStore.getState().undoLast();
      // 跳过 restore，选 save 那条
      expect(undone?.uid).toBe('e2');
      expect(backend.historyRestoreSnapshot).toHaveBeenCalledWith('snap-save');
    });

    it('没有可撤销记录时返回 null 且不发起恢复', async () => {
      (backend.historyListEvents as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
        items: [event({ uid: 'e1', snapshotUid: null })],
        total: 1,
      });
      await useHistoryStore.getState().load();

      const undone = await useHistoryStore.getState().undoLast();
      expect(undone).toBeNull();
      expect(backend.historyRestoreSnapshot).not.toHaveBeenCalled();
    });
  });
});
