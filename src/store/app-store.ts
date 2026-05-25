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

  activeTab: TabId;
  searchQuery: string;
  selectedIndices: number[];
  isAdmin: boolean;
  statusMessage: string;
  isModified: boolean;
  isLoading: boolean;

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
  canUndo: () => boolean;
  canRedo: () => boolean;

  loadPaths: () => Promise<void>;
  savePaths: () => Promise<void>;

  initialize: () => Promise<void>;
}

export const useAppStore = create<AppState>((set, get) => ({
  sysPaths: [],
  userPaths: [],
  undoRedo: new UndoRedoManager(appConfig.undo.maxHistory),

  activeTab: 'system',
  searchQuery: '',
  selectedIndices: [],
  isAdmin: false,
  statusMessage: '',
  isModified: false,
  isLoading: true,

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
    if (target === TargetType.SYSTEM) set({ sysPaths: newList, isModified: true });
    else set({ userPaths: newList, isModified: true });
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
    if (target === TargetType.SYSTEM) set({ sysPaths: newList, isModified: true });
    else set({ userPaths: newList, isModified: true });
  },

  deletePaths: (indices, target) => {
    if (indices.length === 0) return;
    const state = get();
    const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
    const sorted = [...indices].sort((a, b) => b - a);

    for (const idx of sorted) {
      state.undoRedo.push({
        type: OperationType.DELETE, target, index: idx, count: 1,
        oldPaths: [list[idx]], newPaths: [],
      });
    }

    const toRemove = new Set(sorted);
    const newList = list.filter((_, i) => !toRemove.has(i));
    if (target === TargetType.SYSTEM) set({ sysPaths: newList, selectedIndices: [], isModified: true });
    else set({ userPaths: newList, selectedIndices: [], isModified: true });
  },

  moveUp: (index, target) => {
    if (index <= 0) return;
    const state = get();
    const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
    state.undoRedo.push({
      type: OperationType.MOVE_UP, target, index, count: 1,
      oldPaths: [], newPaths: [],
    });
    const newList = [...list];
    [newList[index - 1], newList[index]] = [newList[index], newList[index - 1]];
    if (target === TargetType.SYSTEM) set({ sysPaths: newList, selectedIndices: [index - 1], isModified: true });
    else set({ userPaths: newList, selectedIndices: [index - 1], isModified: true });
  },

  moveDown: (index, target) => {
    const state = get();
    const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
    if (index >= list.length - 1) return;
    state.undoRedo.push({
      type: OperationType.MOVE_DOWN, target, index, count: 1,
      oldPaths: [], newPaths: [],
    });
    const newList = [...list];
    [newList[index], newList[index + 1]] = [newList[index + 1], newList[index]];
    if (target === TargetType.SYSTEM) set({ sysPaths: newList, selectedIndices: [index + 1], isModified: true });
    else set({ userPaths: newList, selectedIndices: [index + 1], isModified: true });
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
      if (target === TargetType.SYSTEM) set({ sysPaths: kept, selectedIndices: [], isModified: true });
      else set({ userPaths: kept, selectedIndices: [], isModified: true });
    }

    return removed;
  },

  importPaths: (target, importPaths) => {
    if (importPaths.length === 0) return;
    const state = get();
    const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
    const copied = [...importPaths];

    state.undoRedo.push({
      type: OperationType.IMPORT, target, index: 0, count: copied.length,
      oldPaths: [...list], newPaths: copied,
    });

    if (target === TargetType.SYSTEM) set({ sysPaths: copied, selectedIndices: [], isModified: true });
    else set({ userPaths: copied, selectedIndices: [], isModified: true });
  },

  clearPaths: (target) => {
    const state = get();
    const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
    if (list.length === 0) return;

    state.undoRedo.push({
      type: OperationType.CLEAR, target, index: 0, count: list.length,
      oldPaths: [...list], newPaths: [],
    });

    if (target === TargetType.SYSTEM) set({ sysPaths: [], isModified: true });
    else set({ userPaths: [], isModified: true });
  },

  undo: () => {
    const { undoRedo, sysPaths, userPaths } = get();
    const result = undoRedo.undo(sysPaths, userPaths);
    if (result) set({ sysPaths: result[0], userPaths: result[1], isModified: true, selectedIndices: [] });
  },

  redo: () => {
    const { undoRedo, sysPaths, userPaths } = get();
    const result = undoRedo.redo(sysPaths, userPaths);
    if (result) set({ sysPaths: result[0], userPaths: result[1], isModified: true, selectedIndices: [] });
  },

  canUndo: () => get().undoRedo.canUndo(),
  canRedo: () => get().undoRedo.canRedo(),

  loadPaths: async () => {
    try {
      set({ isLoading: true });
      const [sysArr, userArr] = await Promise.all([
        invoke<string[]>('load_system_paths'),
        invoke<string[]>('load_user_paths'),
      ]);
      set({
        sysPaths: sysArr,
        userPaths: userArr,
        undoRedo: new UndoRedoManager(appConfig.undo.maxHistory),
        isLoading: false,
        isModified: false,
        statusMessage: i18n.t('status.loaded', { sysCount: sysArr.length, userCount: userArr.length }),
      });
    } catch (e) {
      set({ isLoading: false, statusMessage: `${i18n.t('status.error')}: ${String(e)}` });
    }
  },

  savePaths: async () => {
    const { sysPaths, userPaths } = get();
    const sysJoined = sysPaths.join(';');
    const userJoined = userPaths.join(';');

    const { maxSystemLength, maxUserLength, maxCombinedLength } = appConfig.path;
    if (sysJoined.length > maxSystemLength || userJoined.length > maxUserLength || (sysJoined + userJoined).length > maxCombinedLength) {
      if (!window.confirm(`${i18n.t('status.error')}: PATH 长度超过建议值，是否继续？`)) return;
    }

    set({ statusMessage: i18n.t('status.saving') });

    // 备份（不阻塞保存）
    invoke('backup_registry', { customDir: null, sysPaths, userPaths }).catch(() => {});

    // 并行保存
    const [sysResult, userResult] = await Promise.allSettled([
      invoke('save_system_paths', { paths: sysPaths }),
      invoke('save_user_paths', { paths: userPaths }),
    ]);

    const sysOk = sysResult.status === 'fulfilled';
    const userOk = userResult.status === 'fulfilled';

    if (sysOk && userOk) {
      invoke('broadcast_env_change').catch(() => {});
      set({ isModified: false, statusMessage: i18n.t('status.saved') });
    } else if (sysOk) {
      set({ statusMessage: '用户 PATH 保存失败，系统 PATH 已保存' });
    } else if (userOk) {
      set({ statusMessage: '系统 PATH 保存失败，用户 PATH 已保存' });
    } else {
      set({ statusMessage: `${i18n.t('status.error')}: 保存失败` });
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
