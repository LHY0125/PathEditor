import { create } from 'zustand';
import i18n from '@/i18n';
import { backend, type EventFilter, type EventView, type SnapshotView } from '@/services/backend';
import type { CoreError } from '@/core/env-var';

/**
 * 把 rejection 转成用户可读文案。
 *
 * **不能写 `e instanceof Error`**：`parseCoreError` 抛出的是普通对象
 * `{code, message}`，不是 `Error` 实例——那样做会让所有错误都退化成
 * `String(e)` 即 `"[object Object]"`，降级条幅永远显示乱码。
 * 判定方式与 `env-store.ts` 的 `isCoreError` 保持一致。
 */
function isCoreError(err: unknown): err is Pick<CoreError, 'code' | 'message'> {
  return (
    typeof err === 'object' &&
    err !== null &&
    typeof (err as { code?: unknown }).code === 'string' &&
    typeof (err as { message?: unknown }).message === 'string'
  );
}

/** 错误文案；`crypto` 单独给出更具体的说明。 */
function errorMessage(err: unknown): string {
  if (isCoreError(err)) {
    if (err.code === 'crypto') {
      return i18n.t('history.errorCrypto');
    }
    return err.message.trim().length > 0 ? err.message : i18n.t('history.errorUnavailable');
  }
  return err instanceof Error ? err.message : String(err);
}

interface HistoryState {
  events: EventView[];
  total: number;
  loading: boolean;
  error: string | null;
  /** 当前选中的快照详情（含掩码后的值） */
  detail: SnapshotView | null;
  detailLoading: boolean;
  /** 恢复操作进行中 */
  restoring: boolean;
  /** 当前筛选条件 */
  filter: EventFilter;
  load: () => Promise<void>;
  /** 追加下一页；`events.length >= total` 时为空操作。 */
  loadMore: () => Promise<void>;
  selectEvent: (event: EventView) => Promise<void>;
  clearDetail: () => void;
  restore: (uid: string) => Promise<void>;
  /** 跨会话撤销：恢复最近一次可撤销操作前的快照 */
  undoLast: () => Promise<EventView | null>;
  /** 设置筛选并重新加载（offset 归零） */
  setFilter: (patch: Partial<EventFilter>) => Promise<void>;
}

const PAGE_SIZE = 50;

export const useHistoryStore = create<HistoryState>((set) => ({
  events: [],
  total: 0,
  loading: false,
  error: null,
  detail: null,
  detailLoading: false,
  restoring: false,
  filter: {},

  load: async () => {
    set({ loading: true, error: null });
    try {
      const page = await backend.historyListEvents({
        limit: PAGE_SIZE,
        offset: 0,
        ...useHistoryStore.getState().filter,
      });
      set({ events: page.items, total: page.total, loading: false });
    } catch (e) {
      // 降级模式（code = database）也会走到这里：错误条幅依赖这条信息
      set({ loading: false, error: errorMessage(e) });
    }
  },

  /**
   * 追加下一页。
   *
   * `historyListEvents` 用 offset 分页；筛选变化时由 `setFilter` 重新从 0 加载，
   * 因此这里只需用当前已加载条数作为 offset。
   */
  loadMore: async () => {
    const { events, total, loading } = useHistoryStore.getState();
    if (loading || events.length >= total) return;
    set({ loading: true, error: null });
    try {
      const page = await backend.historyListEvents({
        limit: PAGE_SIZE,
        offset: events.length,
        ...useHistoryStore.getState().filter,
      });
      set((state) => ({
        events: [...state.events, ...page.items],
        total: page.total,
        loading: false,
      }));
    } catch (e) {
      set({ loading: false, error: errorMessage(e) });
    }
  },

  /**
   * 选中一条事件并加载其快照。
   *
   * 整改：用 `event.snapshotUid`（不是 `event.uid`）查快照。
   * 旧写法拿事件 uid 当快照 uid，必然 `NotFound`，右侧详情恒空。
   * `snapshotUid` 为 `null` 时直接清空详情——这条事件本来就没有关联快照。
   *
   * **恒用 `reveal = false`**：历史面板默认掩码，明文只能经显式操作获取。
   */
  selectEvent: async (event: EventView) => {
    if (!event.snapshotUid) {
      set({ detail: null, detailLoading: false });
      return;
    }
    set({ detailLoading: true });
    try {
      const detail = await backend.historyGetSnapshot(event.snapshotUid, false);
      set({ detail, detailLoading: false });
    } catch {
      set({ detail: null, detailLoading: false });
    }
  },

  clearDetail: () => set({ detail: null }),

  setFilter: async (patch: Partial<EventFilter>) => {
    set({ filter: patch });
    await useHistoryStore.getState().load();
  },

  /**
   * 跨会话撤销：把「最近一次可撤销的操作」恢复到它执行前的状态。
   *
   * **不重建内存版的 `OpRecord`**：那是 UI 的即时栈（`UndoRedoManager`），
   * 关掉进程就没了。跨会话撤销的正确形态是——事件流本身就是撤销栈：
   * 每条事件在之后都指向「该操作执行前」的快照，
   * 恢复那个快照就等于撤销该操作。
   *
   * 排除 `restore` 自身：撤销一条恢复记录会让用户困惑（"撤销我的撤销"）。
   *
   * 返回被撤销的事件；没有可撤销的记录时返回 `null`。
   */
  undoLast: async (): Promise<EventView | null> => {
    const { events } = useHistoryStore.getState();
    const target = events.find((e) => e.snapshotUid !== null && e.action !== 'restore');
    if (!target || !target.snapshotUid) return null;
    await useHistoryStore.getState().restore(target.snapshotUid);
    return target;
  },

  /**
   * 把当前选中的快照恢复回注册表。
   *
   * **破坏性操作**：调用方必须先二次确认。成功后重新加载列表——
   * 恢复本身会追加一条新事件（append-only），列表需要刷新才看得到。
   */
  restore: async (uid: string) => {
    set({ restoring: true, error: null });
    try {
      await backend.historyRestoreSnapshot(uid);
      set({ restoring: false, detail: null });
      await useHistoryStore.getState().load();
    } catch (e) {
      set({ restoring: false, error: errorMessage(e) });
      throw e;
    }
  },
}));

/** 仅用于测试重置。 */
export function resetHistoryStore(): void {
  useHistoryStore.setState({
    events: [],
    total: 0,
    loading: false,
    error: null,
    detail: null,
    restoring: false,
    filter: {},
  });
}

/** 每页条数（与 core 的默认值一致）。 */
export const historyPageSize = PAGE_SIZE;
