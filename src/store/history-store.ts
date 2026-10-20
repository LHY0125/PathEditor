import { create } from 'zustand';
import { backend, type EventView, type SnapshotView } from '@/services/backend';
import type { CoreError } from '@/core/env-var';

/**
 * 把 rejection 转成用户可读文案。
 *
 * **不能写 `e instanceof Error`**：`parseCoreError` 抛出的是普通对象
 * `{code, message}`，不是 `Error` 实例——那样做会让所有错误都退化成
 * `String(e)` 即 `"[object Object]"`，降级条幅永远显示乱码（J-4）。
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

/** 错误文案；`crypto` 单独给出更具体的说明（spec §8 的高价值分支）。 */
function errorMessage(err: unknown): string {
  if (isCoreError(err)) {
    if (err.code === 'crypto') {
      return '历史库密钥不可用（可能更换了 Windows 账户或机器），历史暂时无法读取';
    }
    return err.message.trim().length > 0 ? err.message : '历史不可用';
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
      set({ loading: false, error: errorMessage(e) });
    }
  },

  /**
   * 选中一条事件并加载其快照。
   *
   * **J-8 整改**：用 `event.snapshotUid`（不是 `event.uid`）查快照。
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
}));

/** 仅用于测试重置。 */
export function resetHistoryStore(): void {
  useHistoryStore.setState({ events: [], total: 0, loading: false, error: null, detail: null });
}

/** 每页条数（与 core 的默认值一致）。 */
export const historyPageSize = PAGE_SIZE;
