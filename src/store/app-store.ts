import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import i18n from '@/i18n';
import { StringList } from '@/core/string-list';
import {
  UndoRedoManager,
  OperationType,
  TargetType,
} from '@/core/undo-redo';
import { pathClean } from '@/core/path-manager';

export type TabId = 'system' | 'user' | 'merged';

interface AppState {
  sysPaths: StringList;
  userPaths: StringList;
  undoRedo: UndoRedoManager;
  dataVersion: number;

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
  loadFromStringLists: (sys: string[], user: string[]) => void;

  initialize: () => Promise<void>;

  _getTargetList: (target: TargetType) => StringList;
  _bumpVersion: () => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  sysPaths: new StringList(),
  userPaths: new StringList(),
  undoRedo: new UndoRedoManager(50),
  dataVersion: 0,

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

  _getTargetList: (target) => {
    const { sysPaths, userPaths } = get();
    return target === TargetType.SYSTEM ? sysPaths : userPaths;
  },

  _bumpVersion: () => set((s) => ({ isModified: true, dataVersion: s.dataVersion + 1 })),

  // ── CRUD ──

  addPath: (path, target) => {
    const list = get()._getTargetList(target);
    list.add(path);
    get().undoRedo.push({
      type: OperationType.ADD,
      target,
      index: list.length - 1,
      count: 1,
      oldPaths: [],
      newPaths: [path],
    });
    get()._bumpVersion();
  },

  editPath: (index, newPath, target) => {
    const list = get()._getTargetList(target);
    const oldPath = list.get(index);
    if (oldPath === undefined) return;

    get().undoRedo.push({
      type: OperationType.EDIT,
      target,
      index,
      count: 1,
      oldPaths: [oldPath],
      newPaths: [newPath],
    });
    list.set(index, newPath);
    get()._bumpVersion();
  },

  deletePaths: (indices, target) => {
    const list = get()._getTargetList(target);
    if (indices.length === 0) return;

    // 从大到小排序
    const sorted = [...indices].sort((a, b) => b - a);

    // 每个删除独立记录，保证撤销时顺序正确
    for (const idx of sorted) {
      const oldPath = list.get(idx)!;
      get().undoRedo.push({
        type: OperationType.DELETE,
        target,
        index: idx,
        count: 1,
        oldPaths: [oldPath],
        newPaths: [],
      });
      list.removeAt(idx);
    }

    set({ selectedIndices: [] });
    get()._bumpVersion();
  },

  moveUp: (index, target) => {
    if (index <= 0) return;
    const list = get()._getTargetList(target);
    get().undoRedo.push({
      type: OperationType.MOVE_UP,
      target,
      index,
      count: 1,
      oldPaths: [],
      newPaths: [],
    });
    list.swap(index, index - 1);
    set({ selectedIndices: [index - 1] });
    get()._bumpVersion();
  },

  moveDown: (index, target) => {
    const list = get()._getTargetList(target);
    if (index >= list.length - 1) return;
    get().undoRedo.push({
      type: OperationType.MOVE_DOWN,
      target,
      index,
      count: 1,
      oldPaths: [],
      newPaths: [],
    });
    list.swap(index, index + 1);
    set({ selectedIndices: [index + 1] });
    get()._bumpVersion();
  },

  cleanPaths: (target, validateFn) => {
    const list = get()._getTargetList(target);
    const oldPaths = list.toArray();
    const removed = pathClean(list, validateFn);

    if (removed.length > 0) {
      get().undoRedo.push({
        type: OperationType.CLEAN,
        target,
        index: 0,
        count: removed.length,
        oldPaths,
        newPaths: list.toArray(),
      });
      set({ selectedIndices: [] });
      get()._bumpVersion();
    }

    return removed;
  },

  importPaths: (target, importPaths) => {
    if (importPaths.length === 0) return;
    const list = get()._getTargetList(target);
    const oldPaths = list.toArray();
    const copied = [...importPaths];

    get().undoRedo.push({
      type: OperationType.IMPORT,
      target,
      index: 0,
      count: copied.length,
      oldPaths,
      newPaths: copied,
    });

    list.clear();
    for (const p of copied) {
      list.add(p);
    }
    set({ selectedIndices: [] });
    get()._bumpVersion();
  },

  clearPaths: (target) => {
    const list = get()._getTargetList(target);
    const oldPaths = list.toArray();
    if (oldPaths.length === 0) return;

    get().undoRedo.push({
      type: OperationType.CLEAR,
      target,
      index: 0,
      count: oldPaths.length,
      oldPaths,
      newPaths: [],
    });
    list.clear();
    get()._bumpVersion();
  },

  // ── 撤销/重做 ──

  undo: () => {
    const { undoRedo, sysPaths, userPaths } = get();
    if (undoRedo.undo(sysPaths, userPaths)) {
      set({ isModified: true, selectedIndices: [] });
      get()._bumpVersion();
    }
  },

  redo: () => {
    const { undoRedo, sysPaths, userPaths } = get();
    if (undoRedo.redo(sysPaths, userPaths)) {
      set({ isModified: true, selectedIndices: [] });
      get()._bumpVersion();
    }
  },

  canUndo: () => get().undoRedo.canUndo(),
  canRedo: () => get().undoRedo.canRedo(),

  // ── 数据加载/保存 ──

  loadFromStringLists: (sys: string[], user: string[]) => {
    set({
      sysPaths: StringList.fromArray(sys),
      userPaths: StringList.fromArray(user),
      undoRedo: new UndoRedoManager(50),
      isModified: false,
      isLoading: false,
      dataVersion: get().dataVersion + 1,
    });
  },

  loadPaths: async () => {
    try {
      set({ isLoading: true });
      const [sysArr, userArr] = await Promise.all([
        invoke<string[]>('load_system_paths'),
        invoke<string[]>('load_user_paths'),
      ]);

      set({
        sysPaths: StringList.fromArray(sysArr),
        userPaths: StringList.fromArray(userArr),
        undoRedo: new UndoRedoManager(50),
        isLoading: false,
        isModified: false,
        dataVersion: get().dataVersion + 1,
        statusMessage: i18n.t('status.loaded', {
          sysCount: sysArr.length,
          userCount: userArr.length,
        }),
      });
    } catch (e) {
      set({ isLoading: false, statusMessage: `${i18n.t('status.error')}: ${e}` });
    }
  },

  savePaths: async () => {
    const { sysPaths, userPaths } = get();
    set({ statusMessage: i18n.t('status.saving') });
    try {
      await invoke('save_system_paths', { paths: sysPaths.toArray() });
      await invoke('save_user_paths', { paths: userPaths.toArray() });
      await invoke('broadcast_env_change');
      set({ isModified: false, statusMessage: i18n.t('status.saved') });
    } catch (e) {
      set({ statusMessage: `${i18n.t('status.error')}: ${e}` });
    }
  },

  initialize: async () => {
    try {
      const isAdmin: boolean = await invoke('check_admin');
      set({ isAdmin });
      if (!isAdmin) {
        set({ statusMessage: i18n.t('status.readonly') });
      }
    } catch {
      set({ isAdmin: false, statusMessage: i18n.t('status.readonly') });
    }
    await get().loadPaths();
  },
}));
