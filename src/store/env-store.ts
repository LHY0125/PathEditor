import { create } from 'zustand';
import i18n from '@/i18n';
import {
  envVarKey,
  validateVarName,
  type EnvHive,
  type EnvValueKind,
  type EnvVarMeta,
  type EnvVarSnapshot,
  type HiveFilter,
  type RevealedValue,
} from '@/core/env-var';
import { backend } from '@/services/backend';

/**
 * 冲突错误的稳定前缀（Rust 侧所有 revision 冲突统一携带）。
 * 匹配前缀而非中文文案 —— Rust 错误措辞变化不会静默破坏冲突检测。
 *
 * 过渡期双形状（Wave 2 Task 2）：env 通路的 Rust 错误已迁移为结构化
 * `CoreError`（Tauri 序列化为 `{code:'conflict', message, ...}` 对象），
 * PATH 通路仍是字符串。判定同时兼容两种形状；Task 3 统一为按 `code` 判定。
 */
const CONFLICT_PREFIX = '[E_CONFLICT]';

/** 判断值是否为带 `code` 字段的结构化错误对象（Rust CoreError 序列化形状）。 */
function isStructuredError(error: unknown): error is { code?: unknown; message?: unknown } {
  return typeof error === 'object' && error !== null && 'code' in error;
}

function isConflictError(error: unknown): boolean {
  if (isStructuredError(error)) {
    return error.code === 'conflict';
  }
  return String(error).includes(CONFLICT_PREFIX);
}

/** 过渡期错误文案提取：结构化对象取 `message`，其余按字符串降级。 */
function errorMessage(error: unknown): string {
  if (isStructuredError(error) && typeof error.message === 'string') {
    return error.message;
  }
  return String(error);
}

interface EnvState {
  snapshot: EnvVarSnapshot | null;
  revealed: Map<string, string>;
  draft: Map<string, string>;
  hiveFilter: HiveFilter;
  isLoading: boolean;
  isSaving: boolean;
  statusMessage: string;

  load: () => Promise<void>;
  setHiveFilter: (filter: HiveFilter) => void;
  setDraft: (meta: EnvVarMeta, value: string) => void;
  setDraftByKey: (key: string, value: string) => void;
  clearDraft: (meta: EnvVarMeta) => void;
  clearDraftByKey: (key: string) => void;
  /** 取单个变量的完整明文及其读取时的 revision（编辑弹窗数据源）。不进入 revealed，不影响表格打码状态。 */
  fetchFullValue: (hive: EnvHive, name: string) => Promise<RevealedValue>;
  /** 返回是否写入成功；弹窗据此决定关闭还是保留并显示错误。readRevision 是弹窗读取原值时的 revision，用于保存点陈旧判定（F-01）。 */
  save: (meta: EnvVarMeta, readRevision: string | null) => Promise<boolean>;
  create: (hive: EnvHive, name: string, value: string, kind: EnvValueKind) => Promise<boolean>;
  remove: (meta: EnvVarMeta) => Promise<boolean>;
  reveal: (meta: EnvVarMeta) => Promise<void>;
  hide: (meta: EnvVarMeta) => void;
  hasDrafts: () => boolean;
  setStatusMessage: (message: string) => void;
}

export const useEnvStore = create<EnvState>((set, get) => {
  /** 递增请求代次：晚到的旧 load 响应不得覆盖新响应（reveal/refresh 乱序防护）。 */
  let loadSeq = 0;

  /** 冲突或失败后统一刷新，并保留草稿避免用户输入丢失。 */
  const refreshAfterError = async (message: string) => {
    set({ statusMessage: message, isSaving: false });
    await get().load();
  };

  return {
    snapshot: null,
    revealed: new Map(),
    draft: new Map(),
    hiveFilter: 'all',
    isLoading: false,
    isSaving: false,
    statusMessage: '',

    load: async () => {
      const seq = ++loadSeq;
      set({ isLoading: true });
      try {
        const snapshot = await backend.listAllEnvVars();
        if (seq !== loadSeq) return; // 已有更新的请求，丢弃过期响应
        // 刷新即恢复打码：明文不跨次加载存活
        set({ snapshot, revealed: new Map(), isLoading: false });
      } catch (error) {
        if (seq !== loadSeq) return;
        set({
          isLoading: false,
          statusMessage: `${i18n.t('status.error')}: ${String(error)}`,
        });
      }
    },

    setHiveFilter: (filter) => set({ hiveFilter: filter }),

    setDraft: (meta, value) => {
      const draft = new Map(get().draft);
      draft.set(envVarKey(meta), value);
      set({ draft });
    },

    setDraftByKey: (key, value) => {
      const draft = new Map(get().draft);
      draft.set(key, value);
      set({ draft });
    },

    clearDraft: (meta) => {
      const draft = new Map(get().draft);
      draft.delete(envVarKey(meta));
      set({ draft });
    },

    clearDraftByKey: (key) => {
      const draft = new Map(get().draft);
      draft.delete(key);
      set({ draft });
    },

    fetchFullValue: async (hive, name) => {
      // 编辑数据源专用：完整明文 + 读取时 revision 直接返回给调用方，绝不写入
      // revealed —— preview 是截断/净化后的展示摘要，严禁作为编辑初始值（F-01）。
      // revision 用于提交时校验值是否已陈旧。
      return backend.revealEnvVar(hive, name);
    },

    save: async (meta, readRevision) => {
      // F-01：编辑值的读取版本与将写入的 revision 不一致 → 值已陈旧，
      // 拒绝提交，刷新快照让弹窗重取，避免用旧值覆盖外部新值。
      if (readRevision !== null && readRevision !== meta.revision) {
        await refreshAfterError(i18n.t('envVar.staleReloaded'));
        return false;
      }
      const value = get().draft.get(envVarKey(meta));
      if (value === undefined) return false;
      set({ isSaving: true });
      try {
        await backend.updateEnvVar(meta.hive, meta.name, value, meta.revision);
        const draft = new Map(get().draft);
        draft.delete(envVarKey(meta));
        set({ draft, isSaving: false, statusMessage: i18n.t('status.saved') });
        await get().load();
        return true;
      } catch (error) {
        if (isConflictError(error)) {
          // 冲突：Rust 已拒绝写入，刷新拿最新 revision；草稿保留（输入不丢）
          const message = errorMessage(error);
          await refreshAfterError(message);
        } else {
          const message = errorMessage(error);
          set({ isSaving: false, statusMessage: `${i18n.t('status.error')}: ${message}` });
        }
        return false;
      }
    },

    create: async (hive, name, value, kind) => {
      const invalid = validateVarName(name);
      if (invalid !== null) {
        set({ statusMessage: invalid });
        return false;
      }
      set({ isSaving: true });
      try {
        await backend.createEnvVar(hive, name, value, kind);
        set({ isSaving: false, statusMessage: i18n.t('status.saved') });
        await get().load();
        return true;
      } catch (error) {
        set({
          isSaving: false,
          statusMessage: `${i18n.t('status.error')}: ${errorMessage(error)}`,
        });
        return false;
      }
    },

    remove: async (meta) => {
      set({ isSaving: true });
      try {
        await backend.deleteEnvVar(meta.hive, meta.name, meta.revision);
        set({ isSaving: false, statusMessage: i18n.t('status.saved') });
        await get().load();
        return true;
      } catch (error) {
        if (isConflictError(error)) {
          await refreshAfterError(errorMessage(error));
        } else {
          set({
            isSaving: false,
            statusMessage: `${i18n.t('status.error')}: ${errorMessage(error)}`,
          });
        }
        return false;
      }
    },

    reveal: async (meta) => {
      const requestedRevision = meta.revision;
      try {
        const { value } = await backend.revealEnvVar(meta.hive, meta.name);
        // 竞态防护：请求期间快照若已换代（revision 变化或条目消失），
        // 旧明文绑定不到当前状态，直接丢弃 —— 避免旧值经新 revision 覆盖外部更新。
        const snap = get().snapshot;
        const current = snap
          ? [...snap.system, ...snap.user].find((m) => envVarKey(m) === envVarKey(meta))
          : undefined;
        if (!current || current.revision !== requestedRevision) return;
        const revealed = new Map(get().revealed);
        revealed.set(envVarKey(meta), value);
        set({ revealed });
      } catch (error) {
        const revealed = new Map(get().revealed);
        revealed.delete(envVarKey(meta));
        set({ revealed, statusMessage: `${i18n.t('status.error')}: ${String(error)}` });
        // 值可能已不存在或类型不受支持，刷新以同步真实状态
        await get().load();
      }
    },

    hide: (meta) => {
      const revealed = new Map(get().revealed);
      revealed.delete(envVarKey(meta));
      set({ revealed });
    },

    hasDrafts: () => get().draft.size > 0,

    setStatusMessage: (message) => set({ statusMessage: message }),
  };
});
