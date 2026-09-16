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
} from '@/core/env-var';
import { backend } from '@/services/backend';

/** 冲突错误的稳定特征，用于决定是否刷新。 */
const CONFLICT_MARKER = '已被其他进程修改';

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
  clearDraft: (meta: EnvVarMeta) => void;
  /** 返回是否写入成功；弹窗据此决定关闭还是保留并显示错误。 */
  save: (meta: EnvVarMeta) => Promise<boolean>;
  create: (hive: EnvHive, name: string, value: string, kind: EnvValueKind) => Promise<boolean>;
  remove: (meta: EnvVarMeta) => Promise<boolean>;
  reveal: (meta: EnvVarMeta) => Promise<void>;
  hide: (meta: EnvVarMeta) => void;
  hasDrafts: () => boolean;
  setStatusMessage: (message: string) => void;
}

export const useEnvStore = create<EnvState>((set, get) => {
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
      set({ isLoading: true });
      try {
        const snapshot = await backend.listAllEnvVars();
        // 刷新即恢复打码：明文不跨次加载存活
        set({ snapshot, revealed: new Map(), isLoading: false });
      } catch (error) {
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

    clearDraft: (meta) => {
      const draft = new Map(get().draft);
      draft.delete(envVarKey(meta));
      set({ draft });
    },

    save: async (meta) => {
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
        const message = String(error);
        if (message.includes(CONFLICT_MARKER)) {
          // 不做前端重试或比对：Rust 已拒绝，只需提示并刷新
          await refreshAfterError(message);
        } else {
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
        set({ isSaving: false, statusMessage: `${i18n.t('status.error')}: ${String(error)}` });
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
        const message = String(error);
        if (message.includes(CONFLICT_MARKER)) {
          await refreshAfterError(message);
        } else {
          set({ isSaving: false, statusMessage: `${i18n.t('status.error')}: ${message}` });
        }
        return false;
      }
    },

    reveal: async (meta) => {
      try {
        const value = await backend.revealEnvVar(meta.hive, meta.name);
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
