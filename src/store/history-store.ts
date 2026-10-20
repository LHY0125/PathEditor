import { create } from 'zustand';
import { backend, type EventView, type SnapshotView } from '@/services/backend';

interface HistoryState {
  events: EventView[];
  total: number;
  loading: boolean;
  error: string | null;
  /** 当前选中的快照详情（含掩码后的值） */
  detail: SnapshotView | null;
  detailLoading: boolean;
  load: () => Promise<void>;
  selectEvent: (event: EventView) => Promise<void>;
  clearDetail: () => void;
}

const PAGE_SIZE = 50;

export const useHistoryStore = create<HistoryState>((set) => ({
  events: [],
  total: 0,
  loading: false,
  error: null,
  detail: null,
  detailLoading: false,

  load: async () => {
    set({ loading: true, error: null });
    try {
      const page = await backend.historyListEvents({ limit: PAGE_SIZE, offset: 0 });
      set({ events: page.items, total: page.total, loading: false });
    } catch (e) {
      // 降级模式（code = database）也会走到这里：错误条幅依赖这条信息
      set({ loading: false, error: e instanceof Error ? e.message : String(e) });
    }
  },

  /**
   * 选中一条事件并加载其快照。
   *
   * **恒用 `reveal = false`**：历史面板默认掩码，明文只能经显式操作获取。
   * 当前 `EventView` 未携带 snapshotUid（M2 会补），所以先用事件 uid 作占位查询；
   * 查不到时只清空详情，不报错——历史列表本身仍然可用。
   */
  selectEvent: async (event: EventView) => {
    set({ detailLoading: true });
    try {
      const detail = await backend.historyGetSnapshot(event.uid, false);
      set({ detail, detailLoading: false });
    } catch {
      set({ detail: null, detailLoading: false });
    }
  },

  clearDetail: () => set({ detail: null }),
}));

/** 仅用于测试重置。 */
export function resetHistoryStore(): void {
  useHistoryStore.setState({ events: [], total: 0, loading: false, error: null, detail: null });
}

/** 每页条数（与 core 的默认值一致）。 */
export const historyPageSize = PAGE_SIZE;
