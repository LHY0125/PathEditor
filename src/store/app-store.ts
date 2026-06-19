import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import i18n from '@/i18n';
import { UndoRedoManager, OperationType, TargetType } from '@/core/undo-redo';
import { pathClean } from '@/core/path-manager';
import type { PathEntry } from '@/core/path-entry';
import appConfig from '@/config/default.json';

export type TabId = 'system' | 'user' | 'merged';

export type SaveResult =
  | { kind: 'success' }
  | { kind: 'warning'; reason: 'lengthExceeded' }
  | { kind: 'failure'; message: string }
  | { kind: 'partial'; message: string }
  | { kind: 'blocked' };

interface AppState {
  sysPaths: PathEntry[];
  userPaths: PathEntry[];
  undoRedo: UndoRedoManager;
  _savedSys: PathEntry[]; // 上次保存时的快照，用于 isModified 判断
  _savedUser: PathEntry[];

  activeTab: TabId;
  searchQuery: string;
  selectedIndices: number[];
  isAdmin: boolean;
  statusMessage: string;
  isModified: boolean;
  isLoading: boolean;
  isSaving: boolean;

  setActiveTab: (tab: TabId) => void;
  setSearchQuery: (query: string) => void;
  setSelectedIndices: (indices: number[]) => void;
  setStatusMessage: (msg: string) => void;

  addPath: (path: string, target: TargetType) => void;
  editPath: (index: number, newPath: string, target: TargetType) => void;
  deletePaths: (indices: number[], target: TargetType) => void;
  moveUp: (index: number, target: TargetType) => void;
  moveDown: (index: number, target: TargetType) => void;
  cleanPaths: (target: TargetType, validateFn: (p: string) => boolean) => string[];
  replacePaths: (target: TargetType, newPaths: string[]) => void;
  replaceBothPaths: (sysPaths: string[], userPaths: string[]) => void;
  clearPaths: (target: TargetType) => void;

  togglePath: (index: number, target: TargetType) => void;

  undo: () => void;
  redo: () => void;

  loadPaths: () => Promise<void>;
  savePaths: (force?: boolean) => Promise<SaveResult>;
  initialize: () => Promise<void>;
}

function arraysEqual(a: readonly PathEntry[], b: readonly PathEntry[]): boolean {
  return (
    a.length === b.length && a.every((v, i) => v.path === b[i].path && v.enabled === b[i].enabled)
  );
}

export const useAppStore = create<AppState>((set, get) => {
  const markDirty = () => {
    const { _savedSys, _savedUser, sysPaths, userPaths } = get();
    set({ isModified: !(arraysEqual(sysPaths, _savedSys) && arraysEqual(userPaths, _savedUser)) });
  };

  return {
    sysPaths: [],
    userPaths: [],
    undoRedo: new UndoRedoManager(appConfig.undo.maxHistory),
    _savedSys: [],
    _savedUser: [],

    activeTab: 'system',
    searchQuery: '',
    selectedIndices: [],
    isAdmin: false,
    statusMessage: '',
    isModified: false,
    isLoading: true,
    isSaving: false,

    setActiveTab: (tab) => set({ activeTab: tab }),
    setSearchQuery: (query) => set({ searchQuery: query }),
    setSelectedIndices: (indices) => set({ selectedIndices: indices }),
    setStatusMessage: (msg) => set({ statusMessage: msg }),

    addPath: (path, target) => {
      const state = get();
      const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
      const entry: PathEntry = { path, enabled: true };
      const newList = [...list, entry];
      state.undoRedo.push({
        type: OperationType.ADD,
        target,
        index: newList.length - 1,
        count: 1,
        oldPaths: [],
        newPaths: [entry],
      });
      if (target === TargetType.SYSTEM) set({ sysPaths: newList });
      else set({ userPaths: newList });
      markDirty();
    },

    editPath: (index, newPath, target) => {
      const state = get();
      const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
      const oldEntry = list[index];
      if (!oldEntry) return;
      const newEntry: PathEntry = { path: newPath, enabled: oldEntry.enabled };
      state.undoRedo.push({
        type: OperationType.EDIT,
        target,
        index,
        count: 1,
        oldPaths: [oldEntry],
        newPaths: [newEntry],
      });
      const newList = [...list];
      newList[index] = newEntry;
      if (target === TargetType.SYSTEM) set({ sysPaths: newList });
      else set({ userPaths: newList });
      markDirty();
    },

    deletePaths: (indices, target) => {
      if (indices.length === 0) return;
      const state = get();
      const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
      const sortedDesc = [...indices].sort((a, b) => b - a);
      const sortedAsc = [...indices].sort((a, b) => a - b);
      const oldPaths = sortedAsc.map((i) => list[i]);

      state.undoRedo.push({
        type: OperationType.DELETE,
        target,
        index: sortedAsc[0],
        count: sortedAsc.length,
        oldPaths,
        newPaths: [],
        indices: sortedAsc,
      });

      const toRemove = new Set(sortedDesc);
      const newList = list.filter((_, i) => !toRemove.has(i));
      if (target === TargetType.SYSTEM) set({ sysPaths: newList, selectedIndices: [] });
      else set({ userPaths: newList, selectedIndices: [] });
      markDirty();
    },

    moveUp: (index, target) => {
      if (index <= 0) return;
      const state = get();
      const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
      state.undoRedo.push({
        type: OperationType.MOVE_UP,
        target,
        index,
        count: 1,
        oldPaths: [],
        newPaths: [],
      });
      const newList = [...list];
      [newList[index - 1], newList[index]] = [newList[index], newList[index - 1]];
      if (target === TargetType.SYSTEM) set({ sysPaths: newList, selectedIndices: [index - 1] });
      else set({ userPaths: newList, selectedIndices: [index - 1] });
      markDirty();
    },

    moveDown: (index, target) => {
      const state = get();
      const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
      if (index >= list.length - 1) return;
      state.undoRedo.push({
        type: OperationType.MOVE_DOWN,
        target,
        index,
        count: 1,
        oldPaths: [],
        newPaths: [],
      });
      const newList = [...list];
      [newList[index], newList[index + 1]] = [newList[index + 1], newList[index]];
      if (target === TargetType.SYSTEM) set({ sysPaths: newList, selectedIndices: [index + 1] });
      else set({ userPaths: newList, selectedIndices: [index + 1] });
      markDirty();
    },

    cleanPaths: (target, validateFn) => {
      const state = get();
      const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
      const [kept, removed] = pathClean(list, validateFn);

      if (removed.length > 0) {
        state.undoRedo.push({
          type: OperationType.CLEAN,
          target,
          index: 0,
          count: removed.length,
          oldPaths: [...list],
          newPaths: kept,
        });
        if (target === TargetType.SYSTEM) set({ sysPaths: kept, selectedIndices: [] });
        else set({ userPaths: kept, selectedIndices: [] });
        markDirty();
      }

      return removed.map((e) => e.path);
    },

    replacePaths: (target, newPaths) => {
      if (newPaths.length === 0) return;
      const state = get();
      const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
      const entries: PathEntry[] = newPaths.map((p) => ({ path: p, enabled: true }));

      state.undoRedo.push({
        type: OperationType.IMPORT,
        target,
        index: 0,
        count: entries.length,
        oldPaths: [...list],
        newPaths: [...entries],
      });

      if (target === TargetType.SYSTEM) set({ sysPaths: [...entries], selectedIndices: [] });
      else set({ userPaths: [...entries], selectedIndices: [] });
      markDirty();
    },

    replaceBothPaths: (sysPaths, userPaths) => {
      const state = get();
      const sysEntries: PathEntry[] = sysPaths.map((p) => ({ path: p, enabled: true }));
      const usrEntries: PathEntry[] = userPaths.map((p) => ({ path: p, enabled: true }));
      state.undoRedo.push({
        type: OperationType.IMPORT_BOTH,
        target: TargetType.SYSTEM,
        index: 0,
        count: sysEntries.length + usrEntries.length,
        oldPaths: [...state.sysPaths],
        newPaths: [...sysEntries],
        oldPathsOther: [...state.userPaths],
        newPathsOther: [...usrEntries],
      });
      set({ sysPaths: [...sysEntries], userPaths: [...usrEntries], selectedIndices: [] });
      markDirty();
    },

    clearPaths: (target) => {
      const state = get();
      const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
      if (list.length === 0) return;

      state.undoRedo.push({
        type: OperationType.CLEAR,
        target,
        index: 0,
        count: list.length,
        oldPaths: [...list],
        newPaths: [],
      });

      if (target === TargetType.SYSTEM) set({ sysPaths: [] });
      else set({ userPaths: [] });
      markDirty();
    },

    togglePath: (index, target) => {
      const state = get();
      const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
      const oldEntry = list[index];
      if (!oldEntry) return;
      const newEntry: PathEntry = { path: oldEntry.path, enabled: !oldEntry.enabled };

      state.undoRedo.push({
        type: OperationType.TOGGLE,
        target,
        index,
        count: 1,
        oldPaths: [oldEntry],
        newPaths: [newEntry],
      });

      const newList = [...list];
      newList[index] = newEntry;
      if (target === TargetType.SYSTEM) set({ sysPaths: newList });
      else set({ userPaths: newList });
      markDirty();

      // 即时保存禁用状态
      const { sysPaths: sys, userPaths: usr } = get();
      const sysDisabled = sys.filter((e) => !e.enabled).map((e) => e.path);
      const usrDisabled = usr.filter((e) => !e.enabled).map((e) => e.path);
      invoke('save_disabled_state', { system: sysDisabled, user: usrDisabled }).catch((e) =>
        console.warn('保存禁用状态失败:', e),
      );
    },

    undo: () => {
      const { undoRedo, sysPaths, userPaths, _savedSys, _savedUser } = get();
      const result = undoRedo.undo(sysPaths, userPaths);
      if (result) {
        set({
          sysPaths: result[0],
          userPaths: result[1],
          selectedIndices: [],
          // 内联计算 isModified 而非调用 markDirty()，避免两次 set() 导致额外渲染
          isModified: !(arraysEqual(result[0], _savedSys) && arraysEqual(result[1], _savedUser)),
        });
        // 同步持久化 disabled 状态，与 togglePath 保持一致
        invoke('save_disabled_state', {
          system: result[0].filter((e) => !e.enabled).map((e) => e.path),
          user: result[1].filter((e) => !e.enabled).map((e) => e.path),
        }).catch((e) => console.warn('保存禁用状态失败:', e));
      }
    },

    redo: () => {
      const { undoRedo, sysPaths, userPaths, _savedSys, _savedUser } = get();
      const result = undoRedo.redo(sysPaths, userPaths);
      if (result) {
        set({
          sysPaths: result[0],
          userPaths: result[1],
          selectedIndices: [],
          // 内联计算 isModified 而非调用 markDirty()，避免两次 set() 导致额外渲染
          isModified: !(arraysEqual(result[0], _savedSys) && arraysEqual(result[1], _savedUser)),
        });
        // 同步持久化 disabled 状态，与 togglePath 保持一致
        invoke('save_disabled_state', {
          system: result[0].filter((e) => !e.enabled).map((e) => e.path),
          user: result[1].filter((e) => !e.enabled).map((e) => e.path),
        }).catch((e) => console.warn('保存禁用状态失败:', e));
      }
    },

    loadPaths: async () => {
      try {
        set({ isLoading: true });
        const [sysArr, userArr] = await Promise.all([
          invoke<string[]>('load_system_paths'),
          invoke<string[]>('load_user_paths'),
        ]);

        // 加载禁用状态（文件不存在时返回空）
        let sysDisabled: string[] = [];
        let usrDisabled: string[] = [];
        try {
          const result = await invoke<[string[], string[]]>('load_disabled_state');
          sysDisabled = result[0];
          usrDisabled = result[1];
        } catch {
          // 文件不存在或损坏，忽略
        }

        const sysSet = new Set(sysDisabled);
        const usrSet = new Set(usrDisabled);

        const sysEntries: PathEntry[] = sysArr.map((p) => ({ path: p, enabled: !sysSet.has(p) }));
        const usrEntries: PathEntry[] = userArr.map((p) => ({ path: p, enabled: !usrSet.has(p) }));

        set({
          sysPaths: sysEntries,
          userPaths: usrEntries,
          _savedSys: [...sysEntries],
          _savedUser: [...usrEntries],
          undoRedo: new UndoRedoManager(appConfig.undo.maxHistory),
          isLoading: false,
          isModified: false,
          statusMessage: i18n.t('status.loaded', {
            sysCount: sysArr.length,
            userCount: userArr.length,
          }),
        });
      } catch (e) {
        set({ isLoading: false, statusMessage: `${i18n.t('status.error')}: ${String(e)}` });
      }
    },

    savePaths: async (force?: boolean) => {
      const state = get();
      if (state.isSaving) return { kind: 'blocked' };
      set({ isSaving: true, statusMessage: i18n.t('status.saving') });

      // 只保存 enabled 的路径到注册表
      const sysPaths = state.sysPaths.filter((e) => e.enabled).map((e) => e.path);
      const userPaths = state.userPaths.filter((e) => e.enabled).map((e) => e.path);
      const sysJoined = sysPaths.join(';');
      const userJoined = userPaths.join(';');

      // 长度检查：非强制模式下返回警告，由 UI 层确认
      const { maxSystemLength, maxUserLength, maxCombinedLength } = appConfig.path;
      if (
        !force &&
        (sysJoined.length > maxSystemLength ||
          userJoined.length > maxUserLength ||
          (sysJoined + userJoined).length > maxCombinedLength)
      ) {
        set({ isSaving: false, statusMessage: i18n.t('status.saveWarningLongPaths') });
        return { kind: 'warning', reason: 'lengthExceeded' };
      }

      // 备份当前注册表（保存前备份旧值，失败仅警告不中断）
      let backupFailed = false;
      await invoke('backup_registry', { customDir: null }).catch(() => {
        backupFailed = true;
      });

      const origSys = state._savedSys.filter((e) => e.enabled).map((e) => e.path);
      const origUser = state._savedUser.filter((e) => e.enabled).map((e) => e.path);

      const [sysResult, userResult] = await Promise.allSettled([
        invoke('save_system_paths', { paths: sysPaths, original: origSys }),
        invoke('save_user_paths', { paths: userPaths, original: origUser }),
      ]);

      const sysOk = sysResult.status === 'fulfilled';
      const userOk = userResult.status === 'fulfilled';

      if (sysOk && userOk) {
        invoke('broadcast_env_change').catch(() => {});
        const savedSys = [...state.sysPaths],
          savedUser = [...state.userPaths];
        set({
          isModified: false,
          isSaving: false,
          statusMessage: backupFailed
            ? i18n.t('status.saved_without_backup')
            : i18n.t('status.saved'),
          _savedSys: savedSys,
          _savedUser: savedUser,
        });
        return { kind: 'success' };
      } else {
        const sysErr = !sysOk && sysResult.status === 'rejected' ? String(sysResult.reason) : '';
        const usrErr = !userOk && userResult.status === 'rejected' ? String(userResult.reason) : '';
        const parts = [sysErr, usrErr].filter(Boolean);

        const msg = sysOk
          ? `用户 PATH 保存失败: ${usrErr}`
          : userOk
            ? `系统 PATH 保存失败: ${sysErr}`
            : `保存失败: ${parts.join('; ')}`;

        if (sysOk || userOk) {
          // partial success
          set({ isSaving: false });
          await get().loadPaths(); // reload to avoid state drift
          set({ statusMessage: msg }); // restore the error message overwritten by loadPaths
          return { kind: 'partial', message: msg };
        } else {
          set({ isSaving: false, statusMessage: msg });
          return { kind: 'failure', message: msg };
        }
      }
    },

    initialize: async () => {
      try {
        const isAdmin: boolean = await invoke('check_admin');
        set({ isAdmin });
        if (!isAdmin) set({ statusMessage: i18n.t('status.readonly') });
      } catch {
        set({ isAdmin: false, statusMessage: i18n.t('status.readonly') });
      }
      await get().loadPaths();
    },
  };
});
