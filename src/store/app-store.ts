import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import i18n from '@/i18n';
import { UndoRedoManager, OperationType, TargetType } from '@/core/undo-redo';
import { pathClean } from '@/core/path-manager';
import appConfig from '@/config/default.json';

export type TabId = 'system' | 'user' | 'merged';

interface AppState {
  sysPaths: string[];
  userPaths: string[];
  undoRedo: UndoRedoManager;
  _savedSys: string[]; // 上次保存时的快照，用于 isModified 判断
  _savedUser: string[];

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
  importPaths: (target: TargetType, importPaths: string[]) => void;
  clearPaths: (target: TargetType) => void;

  undo: () => void;
  redo: () => void;

  loadPaths: () => Promise<void>;
  savePaths: () => Promise<void>;
  initialize: () => Promise<void>;

  _markDirty: () => void;
}

function arraysEqual(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((v, i) => v === b[i]);
}

export const useAppStore = create<AppState>((set, get) => ({
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
    const newList = [...list, path];
    state.undoRedo.push({
      type: OperationType.ADD, target, index: newList.length - 1, count: 1,
      oldPaths: [], newPaths: [path],
    });
    if (target === TargetType.SYSTEM) set({ sysPaths: newList });
    else set({ userPaths: newList });
    get()._markDirty();
  },

  editPath: (index, newPath, target) => {
    const state = get();
    const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
    const oldPath = list[index];
    if (oldPath === undefined) return;
    state.undoRedo.push({
      type: OperationType.EDIT, target, index, count: 1,
      oldPaths: [oldPath], newPaths: [newPath],
    });
    const newList = [...list];
    newList[index] = newPath;
    if (target === TargetType.SYSTEM) set({ sysPaths: newList });
    else set({ userPaths: newList });
    get()._markDirty();
  },

  deletePaths: (indices, target) => {
    if (indices.length === 0) return;
    const state = get();
    const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
    const sortedDesc = [...indices].sort((a, b) => b - a);
    const sortedAsc = [...indices].sort((a, b) => a - b);
    const oldPaths = sortedAsc.map((i) => list[i]);

    state.undoRedo.push({
      type: OperationType.DELETE, target,
      index: sortedAsc[0], count: sortedAsc.length,
      oldPaths, newPaths: [],
      indices: sortedAsc,
    });

    const toRemove = new Set(sortedDesc);
    const newList = list.filter((_, i) => !toRemove.has(i));
    if (target === TargetType.SYSTEM) set({ sysPaths: newList, selectedIndices: [] });
    else set({ userPaths: newList, selectedIndices: [] });
    get()._markDirty();
  },

  moveUp: (index, target) => {
    if (index <= 0) return;
    const state = get();
    const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
    state.undoRedo.push({
      type: OperationType.MOVE_UP, target, index, count: 1, oldPaths: [], newPaths: [],
    });
    const newList = [...list];
    [newList[index - 1], newList[index]] = [newList[index], newList[index - 1]];
    if (target === TargetType.SYSTEM) set({ sysPaths: newList, selectedIndices: [index - 1] });
    else set({ userPaths: newList, selectedIndices: [index - 1] });
    get()._markDirty();
  },

  moveDown: (index, target) => {
    const state = get();
    const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
    if (index >= list.length - 1) return;
    state.undoRedo.push({
      type: OperationType.MOVE_DOWN, target, index, count: 1, oldPaths: [], newPaths: [],
    });
    const newList = [...list];
    [newList[index], newList[index + 1]] = [newList[index + 1], newList[index]];
    if (target === TargetType.SYSTEM) set({ sysPaths: newList, selectedIndices: [index + 1] });
    else set({ userPaths: newList, selectedIndices: [index + 1] });
    get()._markDirty();
  },

  cleanPaths: (target, validateFn) => {
    const state = get();
    const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
    const [kept, removed] = pathClean(list, validateFn);

    if (removed.length > 0) {
      state.undoRedo.push({
        type: OperationType.CLEAN, target, index: 0, count: removed.length,
        oldPaths: [...list], newPaths: kept,
      });
      if (target === TargetType.SYSTEM) set({ sysPaths: kept, selectedIndices: [] });
      else set({ userPaths: kept, selectedIndices: [] });
      get()._markDirty();
    }

    return removed;
  },

  importPaths: (target, importPaths) => {
    if (importPaths.length === 0) return;
    const state = get();
    const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;

    state.undoRedo.push({
      type: OperationType.IMPORT, target, index: 0, count: importPaths.length,
      oldPaths: [...list], newPaths: [...importPaths],
    });

    if (target === TargetType.SYSTEM) set({ sysPaths: [...importPaths], selectedIndices: [] });
    else set({ userPaths: [...importPaths], selectedIndices: [] });
    get()._markDirty();
  },

  clearPaths: (target) => {
    const state = get();
    const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
    if (list.length === 0) return;

    state.undoRedo.push({
      type: OperationType.CLEAR, target, index: 0, count: list.length,
      oldPaths: [...list], newPaths: [],
    });

    if (target === TargetType.SYSTEM) set({ sysPaths: [] });
    else set({ userPaths: [] });
    get()._markDirty();
  },

  undo: () => {
    const { undoRedo, sysPaths, userPaths, _savedSys, _savedUser } = get();
    const result = undoRedo.undo(sysPaths, userPaths);
    if (result) {
      set({
        sysPaths: result[0], userPaths: result[1], selectedIndices: [],
        isModified: !(arraysEqual(result[0], _savedSys) && arraysEqual(result[1], _savedUser)),
      });
    }
  },

  redo: () => {
    const { undoRedo, sysPaths, userPaths, _savedSys, _savedUser } = get();
    const result = undoRedo.redo(sysPaths, userPaths);
    if (result) {
      set({
        sysPaths: result[0], userPaths: result[1], selectedIndices: [],
        isModified: !(arraysEqual(result[0], _savedSys) && arraysEqual(result[1], _savedUser)),
      });
    }
  },

  _markDirty: () => {
    const { _savedSys, _savedUser, sysPaths, userPaths } = get();
    set({ isModified: !(arraysEqual(sysPaths, _savedSys) && arraysEqual(userPaths, _savedUser)) });
  },

  loadPaths: async () => {
    try {
      set({ isLoading: true });
      const [sysArr, userArr] = await Promise.all([
        invoke<string[]>('load_system_paths'),
        invoke<string[]>('load_user_paths'),
      ]);
      set({
        sysPaths: sysArr, userPaths: userArr,
        _savedSys: [...sysArr], _savedUser: [...userArr],
        undoRedo: new UndoRedoManager(appConfig.undo.maxHistory),
        isLoading: false, isModified: false,
        statusMessage: i18n.t('status.loaded', { sysCount: sysArr.length, userCount: userArr.length }),
      });
    } catch (e) {
      set({ isLoading: false, statusMessage: `${i18n.t('status.error')}: ${String(e)}` });
    }
  },

  savePaths: async () => {
    const state = get();
    if (state.isSaving) return;
    set({ isSaving: true, statusMessage: i18n.t('status.saving') });

    const { sysPaths, userPaths } = state;
    const sysJoined = sysPaths.join(';');
    const userJoined = userPaths.join(';');

    const { maxSystemLength, maxUserLength, maxCombinedLength } = appConfig.path;
    if (sysJoined.length > maxSystemLength || userJoined.length > maxUserLength || (sysJoined + userJoined).length > maxCombinedLength) {
      if (!window.confirm('PATH 长度超过建议值，是否继续保存？')) { set({ isSaving: false }); return; }
    }

    // 备份当前注册表（保存前备份旧值，失败仅警告不中断）
    invoke('backup_registry', { customDir: null })
      .catch(() => set({ statusMessage: i18n.t('status.warning_backup') }));

    const [sysResult, userResult] = await Promise.allSettled([
      invoke('save_system_paths', { paths: sysPaths }),
      invoke('save_user_paths', { paths: userPaths }),
    ]);

    const sysOk = sysResult.status === 'fulfilled';
    const userOk = userResult.status === 'fulfilled';

    if (sysOk && userOk) {
      invoke('broadcast_env_change').catch(() => {});
      const savedSys = [...sysPaths], savedUser = [...userPaths];
      set({ isModified: false, isSaving: false, statusMessage: i18n.t('status.saved'), _savedSys: savedSys, _savedUser: savedUser });
    } else {
      const reason = (!sysOk && sysResult.status === 'rejected') ? String(sysResult.reason) :
                     (!userOk && userResult.status === 'rejected') ? String(userResult.reason) : '';
      const msg = sysOk ? '用户 PATH 保存失败' : userOk ? '系统 PATH 保存失败' : `保存失败: ${reason}`;
      set({ isSaving: false, statusMessage: msg });
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
}));
