import { create } from 'zustand';
import i18n from '@/i18n';
import { backend, type SearchHit } from '@/services/backend';

/**
 * 搜索状态。
 *
 * **空查询不发请求**：与 `core::db::search::search` 的「空查询返回空」口径一致。
 * 两层各守一次——core 侧保证「不会返回全量」，前端保证「不会白跑一次 IPC」。
 *
 * **明文不进入 store**：搜索结果只含 PATH 串、变量名、配置描述，
 * 不含任何环境变量值，
 * 因此 store 里不会出现需要清理的敏感数据。
 */

/** 把 rejection 转成可读文案；`parseCoreError` 抛的是普通对象，不是 Error 实例。 */
function isCoreError(err: unknown): err is { code: string; message: string } {
  return (
    typeof err === 'object' &&
    err !== null &&
    typeof (err as { code?: unknown }).code === 'string' &&
    typeof (err as { message?: unknown }).message === 'string'
  );
}

function errorMessage(err: unknown): string {
  if (isCoreError(err)) {
    if (err.code === 'database') {
      return i18n.t('search.errorDegraded');
    }
    return err.message.trim().length > 0 ? err.message : i18n.t('search.errorFailed');
  }
  return err instanceof Error ? err.message : String(err);
}

interface SearchState {
  /** 当前查询串（受控输入） */
  query: string;
  /** 最近一次**实际发出**的查询（用于区分「输入中」与「已搜过」） */
  submitted: string;
  hits: SearchHit[];
  loading: boolean;
  error: string | null;

  /** 更新输入框；**不**触发请求（由调用方在提交时调 `run()`）。 */
  setQuery: (query: string) => void;
  /** 执行搜索。空查询直接清空结果，不发请求。 */
  run: () => Promise<void>;
  /** 清空输入与结果。 */
  reset: () => void;
}

export const useSearchStore = create<SearchState>((set, get) => ({
  query: '',
  submitted: '',
  hits: [],
  loading: false,
  error: null,

  setQuery: (query) => set({ query }),

  run: async () => {
    const q = get().query.trim();
    if (q.length === 0) {
      // 空查询：清空结果但**不**清空 query（用户可能只是暂时删空）
      set({ hits: [], submitted: '', loading: false, error: null });
      return;
    }
    set({ loading: true, error: null });
    try {
      const hits = await backend.search(q, 50);
      // 竞态守卫：若期间用户又改了查询并重新提交，丢弃这次的结果
      if (get().query.trim() !== q) return;
      set({ hits, submitted: q, loading: false });
    } catch (e) {
      if (get().query.trim() !== q) return;
      set({ loading: false, error: errorMessage(e) });
    }
  },

  reset: () => set({ query: '', submitted: '', hits: [], loading: false, error: null }),
}));
