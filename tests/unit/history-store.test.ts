import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@/services/backend', () => ({
  backend: {
    historyListEvents: vi.fn(async () => ({ items: [], total: 0 })),
    historyGetSnapshot: vi.fn(),
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
    // parseCoreError 抛出的是普通对象，不是 Error 实例（J-4 的回归守卫）
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

    // spec §8：换机后应提示「密钥不匹配」，而不是笼统的「不可用」
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

    // 第一个参数必须是快照 uid（J-8），不是事件 uid
    expect(backend.historyGetSnapshot).toHaveBeenCalledWith('snap-1', false);
    expect(useHistoryStore.getState().detail).toEqual(snapshot);
  });

  it('snapshotUid 为 null 时不发起查询', async () => {
    await useHistoryStore.getState().selectEvent(event({ snapshotUid: null }));

    expect(backend.historyGetSnapshot).not.toHaveBeenCalled();
    expect(useHistoryStore.getState().detail).toBeNull();
  });
});
