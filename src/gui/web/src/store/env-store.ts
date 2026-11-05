import { create } from 'zustand';
import { errorMessage, isCoreError } from '@/core/error';
import i18n from '@/i18n';
import {
  backupFailed,
  backupPlaintext,
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
import type { BackupOutcome } from '@/core/env-var';

/**
 * 错误判定完全按 `code` —— backend.ts 已把 Tauri rejection
 * 统一解析为 `CoreError`（PATH 纯文本兜底为 `code:'internal'`），这里不再
 * 匹配 `[E_CONFLICT]` 字符串前缀。core 的 message 仍保留该前缀，但仅作
 * 展示层文本，程序分支一律看 code。
 */
function isConflictError(err: unknown): boolean {
  return isCoreError(err) && err.code === 'conflict';
}

/** backend 解析层产物的形状校验（防御 mock / 上游回归）。 */
/**
 * 写入成功后的状态文案：写前自动备份失败时如实降级为「保存成功（备份失败）」。
 *
 * 复用既有 `status.saved_without_backup` 键，不新增文案源（J1a）。
 * 备份是 best-effort：失败**不改变**操作成功的判定 ——
 * 调用方的 `return true` 照旧，只有状态栏措辞变化。
 */
function savedStatusMessage(backup: BackupOutcome): string {
  // 明文回落必须单独告警：文件里有全部变量（含 `*_TOKEN`），只显示「已保存」
  // 会让用户以为敏感值仍是加密状态。三态互斥，按严重度排序。
  if (backupPlaintext(backup) !== null) return i18n.t('status.saved_plaintext_backup');
  return backupFailed(backup) !== null
    ? i18n.t('status.saved_without_backup')
    : i18n.t('status.saved');
}

/**
 * 错误文案：优先展示 `message` 原文（它是面向用户的中文完整句，含具体
 * 变量名与上下文）；message 缺失时兜底到按 code 本地化的 `error.code.*`。
 * `error.code.*` 键同时是未来按 code 分支扩展（如冲突引导重新加载）的锚点。
 */
/// 骨架错误文案统一走 `core/error.ts`；这里的唯一差异是「message 为空时按 code 本地化」。
function describeError(err: unknown): string {
  return errorMessage(err, (code) =>
    i18n.t(`error.code.${code}`, { defaultValue: i18n.t('error.code.internal') }),
  );
}

interface EnvState {
  snapshot: EnvVarSnapshot | null;
  revealed: Map<string, string>;
  draft: Map<string, string>;
  hiveFilter: HiveFilter;
  isLoading: boolean;
  isSaving: boolean;
  statusMessage: string;
  /** 状态栏当前文案是否为错误（只认布尔，不靠本地化文案匹配）。 */
  statusIsError: boolean;
  /** 当前选中的变量稳定键（`hive:name`）；搜索定位也写这里。 */
  selectedKey: string | null;
  /** 请求表格滚动到某个变量键；消费后置回 null。 */
  scrollToKey: string | null;

  load: () => Promise<void>;
  setHiveFilter: (filter: HiveFilter) => void;
  setDraft: (meta: EnvVarMeta, value: string) => void;
  setDraftByKey: (key: string, value: string) => void;
  clearDraft: (meta: EnvVarMeta) => void;
  clearDraftByKey: (key: string) => void;
  /** 取单个变量的完整明文及其读取时的 revision（编辑弹窗数据源）。不进入 revealed，不影响表格打码状态。 */
  fetchFullValue: (hive: EnvHive, name: string) => Promise<RevealedValue>;
  /** 返回是否写入成功；弹窗据此决定关闭还是保留并显示错误。readRevision 是弹窗读取原值时的 revision，用于保存点陈旧判定。 */
  save: (meta: EnvVarMeta, readRevision: string | null) => Promise<boolean>;
  create: (hive: EnvHive, name: string, value: string, kind: EnvValueKind) => Promise<boolean>;
  remove: (meta: EnvVarMeta) => Promise<boolean>;
  reveal: (meta: EnvVarMeta) => Promise<void>;
  hide: (meta: EnvVarMeta) => void;
  hasDrafts: () => boolean;
  setStatusMessage: (message: string) => void;
  setStatusError: (message: string) => void;
  setSelectedKey: (key: string | null) => void;
  setScrollToKey: (key: string | null) => void;
}

export const useEnvStore = create<EnvState>((set, get) => {
  /** 递增请求代次：晚到的旧 load 响应不得覆盖新响应（reveal/refresh 乱序防护）。 */
  let loadSeq = 0;

  /** 冲突或失败后统一刷新，并保留草稿避免用户输入丢失。 */
  const refreshAfterError = async (message: string) => {
    set({ statusMessage: message, statusIsError: true, isSaving: false });
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
    statusIsError: false,
    selectedKey: null,
    scrollToKey: null,

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
          statusIsError: true,
          statusMessage: `${i18n.t('status.error')}: ${describeError(error)}`,
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
      // revealed —— preview 是截断/净化后的展示摘要，严禁作为编辑初始值。
      // revision 用于提交时校验值是否已陈旧。
      return backend.revealEnvVar(hive, name);
    },

    save: async (meta, readRevision) => {
      // 编辑值的读取版本与将写入的 revision 不一致 → 值已陈旧，
      // 拒绝提交，刷新快照让弹窗重取，避免用旧值覆盖外部新值。
      if (readRevision !== null && readRevision !== meta.revision) {
        await refreshAfterError(i18n.t('envVar.staleReloaded'));
        return false;
      }
      const value = get().draft.get(envVarKey(meta));
      if (value === undefined) return false;
      set({ isSaving: true });
      try {
        const { backup } = await backend.updateEnvVar(meta.hive, meta.name, value, meta.revision);
        const draft = new Map(get().draft);
        draft.delete(envVarKey(meta));
        set({
          draft,
          isSaving: false,
          statusIsError: false,
          statusMessage: savedStatusMessage(backup),
        });
        await get().load();
        return true;
      } catch (error) {
        if (isConflictError(error)) {
          // 冲突：Rust 已拒绝写入，刷新拿最新 revision；草稿保留（输入不丢）
          const message = describeError(error);
          await refreshAfterError(message);
        } else {
          const message = describeError(error);
          set({
            isSaving: false,
            statusIsError: true,
            statusMessage: `${i18n.t('status.error')}: ${message}`,
          });
        }
        return false;
      }
    },

    create: async (hive, name, value, kind) => {
      const invalid = validateVarName(name);
      if (invalid !== null) {
        set({ statusMessage: i18n.t(`envVar.nameError.${invalid}`), statusIsError: true });
        return false;
      }
      set({ isSaving: true });
      try {
        const { backup } = await backend.createEnvVar(hive, name, value, kind);
        set({ isSaving: false, statusIsError: false, statusMessage: savedStatusMessage(backup) });
        await get().load();
        return true;
      } catch (error) {
        set({
          isSaving: false,
          statusIsError: true,
          statusMessage: `${i18n.t('status.error')}: ${describeError(error)}`,
        });
        return false;
      }
    },

    remove: async (meta) => {
      set({ isSaving: true });
      try {
        const { backup } = await backend.deleteEnvVar(meta.hive, meta.name, meta.revision);
        set({ isSaving: false, statusIsError: false, statusMessage: savedStatusMessage(backup) });
        await get().load();
        return true;
      } catch (error) {
        if (isConflictError(error)) {
          await refreshAfterError(describeError(error));
        } else {
          set({
            isSaving: false,
            statusIsError: true,
            statusMessage: `${i18n.t('status.error')}: ${describeError(error)}`,
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
        set({
          revealed,
          statusIsError: true,
          statusMessage: `${i18n.t('status.error')}: ${describeError(error)}`,
        });
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

    setStatusMessage: (message) => set({ statusMessage: message, statusIsError: false }),
    setStatusError: (message) => set({ statusMessage: message, statusIsError: true }),
    setSelectedKey: (key) => set({ selectedKey: key }),
    setScrollToKey: (key) => set({ scrollToKey: key }),
  };
});
