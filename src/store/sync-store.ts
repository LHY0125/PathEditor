import { create } from 'zustand';
import {
  backend,
  type SyncConflict,
  type SyncImportReport,
  type SyncStatus,
} from '@/services/backend';

/**
 * 同步中心状态（M3 Task 9）。
 *
 * ## 口令永不进入 store
 *
 * `exportBundle` / `importBundle` 的口令通过**函数参数**传递，用完即弃：
 * store 里没有字段保存它，也就没有「忘记清理」的可能。
 * 组件侧同样用局部 `useState` 持有输入框内容（`type="password"`）。
 *
 * 这与历史快照的 `reveal_env_var` 同口径：敏感输入只在组件局部流转。
 *
 * ## 冲突值按敏感判定掩码
 *
 * `sync_conflicts.local_value` / `remote_value` 可能是环境变量值。
 * spec §4.2 第 9 条要求 UI 展示敏感值时掩码——`maskConflictValue`
 * 把判定收在一处，组件不各自实现。
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
    if (err.code === 'database') return '同步不可用（降级模式）';
    // `sync` = 合并冲突需人工介入；`syncFormat` = 包损坏/口令错误，重试无用
    if (err.code === 'syncFormat') return err.message || '同步包无效';
    return err.message.trim().length > 0 ? err.message : '同步操作失败';
  }
  return err instanceof Error ? err.message : String(err);
}

/**
 * 冲突值的展示文案。
 *
 * **这里不做敏感判定**——Rust 侧的 `env_var::is_sensitive` 是唯一权威。
 * 前端能做的是「不在列表里直接摊开长值」：超过阈值的值折叠，
 * 完整值由用户点开单条冲突时再看。真正的掩码在 Rust 侧生成视图时完成。
 */
export function maskConflictValue(value: string | null, maxLength = 60): string {
  if (value === null || value.length === 0) return '(空)';
  if (value.length <= maxLength) return value;
  return `${value.slice(0, maxLength)}…`;
}

/**
 * 操作成功的提示——**结构化，不是字符串**。
 *
 * store 不产出面向用户的文案：文案属于展示层，必须走 `t()`。
 * 早期版本在这里直接拼中文，导致切换英文界面时这条主反馈仍是中文
 * （审查报告 F-4）。现在由 `SyncCenter` 按 `kind` 选择 i18n 键渲染。
 */
export type SyncNotice =
  | { kind: 'exported'; changeCount: number; path: string }
  | { kind: 'imported'; report: SyncImportReport }
  | { kind: 'resolved'; id: number; resolution: 'local' | 'remote' };

interface SyncState {
  status: SyncStatus | null;
  conflicts: SyncConflict[];
  loading: boolean;
  error: string | null;
  /** 最近一次操作的结果（结构化，由组件渲染文案） */
  notice: SyncNotice | null;

  /** 刷新状态与冲突列表。 */
  refresh: (includeResolved?: boolean) => Promise<void>;
  /** 导出；口令只作为参数传入，不保存。 */
  exportBundle: (passphrase: string, path: string) => Promise<boolean>;
  /** 导入；口令只作为参数传入，不保存。 */
  importBundle: (passphrase: string, path: string) => Promise<boolean>;
  /** 解决一条冲突并刷新。 */
  resolve: (id: number, resolution: 'local' | 'remote') => Promise<boolean>;
  /** 清空错误与提示。 */
  clearMessages: () => void;
}

export const useSyncStore = create<SyncState>((set, get) => ({
  status: null,
  conflicts: [],
  loading: false,
  error: null,
  notice: null,

  refresh: async (includeResolved = false) => {
    set({ loading: true, error: null });
    try {
      const [status, conflicts] = await Promise.all([
        backend.syncStatus(),
        backend.syncListConflicts(includeResolved),
      ]);
      set({ status, conflicts, loading: false });
    } catch (e) {
      set({ loading: false, error: errorMessage(e) });
    }
  },

  exportBundle: async (passphrase, path) => {
    set({ loading: true, error: null, notice: null });
    try {
      const report = await backend.syncExport(passphrase, path);
      set({
        loading: false,
        notice: { kind: 'exported', changeCount: report.changeCount, path: report.path },
      });
      // 导出后待导出数应归零，刷新一次让 UI 反映真实状态
      await get().refresh();
      return true;
    } catch (e) {
      set({ loading: false, error: errorMessage(e) });
      return false;
    }
  },

  importBundle: async (passphrase, path) => {
    set({ loading: true, error: null, notice: null });
    try {
      const report = await backend.syncImport(passphrase, path);
      set({ loading: false, notice: { kind: 'imported', report } });
      await get().refresh();
      return true;
    } catch (e) {
      set({ loading: false, error: errorMessage(e) });
      return false;
    }
  },

  resolve: async (id, resolution) => {
    set({ loading: true, error: null, notice: null });
    try {
      await backend.syncResolveConflict(id, resolution);
      set({ loading: false, notice: { kind: 'resolved', id, resolution } });
      await get().refresh();
      return true;
    } catch (e) {
      set({ loading: false, error: errorMessage(e) });
      return false;
    }
  },

  clearMessages: () => set({ error: null, notice: null }),
}));
