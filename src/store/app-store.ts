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
  // 数据（用 StringList 而非 string[] 以支持 undo/redo）
  sysPaths: StringList;
  userPaths: StringList;
  undoRedo: UndoRedoManager;

  // UI 状态
  activeTab: TabId;
  searchQuery: string;
  selectedIndices: number[];
  isAdmin: boolean;
  statusMessage: string;
  isModified: boolean;
  isLoading: boolean;

  // 基本操作
  setActiveTab: (tab: TabId) => void;
  setSearchQuery: (query: string) => void;
  setSelectedIndices: (indices: number[]) => void;
  setStatusMessage: (msg: string) => void;

  // CRUD（带撤销/重做）
  addPath: (path: string, target: TargetType) => void;
  editPath: (index: number, newPath: string, target: TargetType) => void;
  deletePaths: (indices: number[], target: TargetType) => void;
  moveUp: (index: number, target: TargetType) => void;
  moveDown: (index: number, target: TargetType) => void;
  cleanPaths: (target: TargetType, validateFn: (p: string) => boolean) => string[];
  importPaths: (target: TargetType, importPaths: string[]) => void;
  clearPaths: (target: TargetType) => void;

  // 撤销/重做
  undo: () => void;
  redo: () => void;
  canUndo: () => boolean;
  canRedo: () => boolean;

  // 数据加载/保存
  loadPaths: () => Promise<void>;
  savePaths: () => Promise<void>;
  loadFromStringLists: (sys: string[], user: string[]) => void;

  // 初始化
  initialize: () => Promise<void>;

  // 内部辅助
  _getTargetList: (target: TargetType) => StringList;
  _markModified: () => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  sysPaths: new StringList(),
  userPaths: new StringList(),
  undoRedo: new UndoRedoManager(50),

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

  _markModified: () => set({ isModified: true }),

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
    get()._markModified();
  },

  editPath: (index, newPath, target) => {
    const list = get()._getTargetList(target);
    const oldPath = list.get(index);
    if (!oldPath) return;

    get().undoRedo.push({
      type: OperationType.EDIT,
      target,
      index,
      count: 1,
      oldPaths: [oldPath],
      newPaths: [newPath],
    });
    list.set(index, newPath);
    get()._markModified();
  },

  deletePaths: (indices, target) => {
    const list = get()._getTargetList(target);
    // 从大到小排
    const sorted = [...indices].sort((a, b) => b - a);
    const oldPaths = sorted.map((i) => list.get(i)!);

    // 记录单个 DELETE（合并多个删除为一条记录）
    if (sorted.length === 1) {
      get().undoRedo.push({
        type: OperationType.DELETE,
        target,
        index: sorted[0],
        count: 1,
        oldPaths,
        newPaths: [],
      });
    } else {
      get().undoRedo.push({
        type: OperationType.DELETE,
        target,
        index: sorted[sorted.length - 1],
        count: sorted.length,
        oldPaths,
        newPaths: [],
      });
    }

    for (const i of sorted) {
      list.removeAt(i);
    }
    set({ selectedIndices: [] });
    get()._markModified();
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
    get()._markModified();
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
    get()._markModified();
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
      get()._markModified();
    }

    return removed;
  },

  importPaths: (target, importPaths) => {
    if (importPaths.length === 0) return;
    const list = get()._getTargetList(target);
    const oldPaths = list.toArray();

    get().undoRedo.push({
      type: OperationType.IMPORT,
      target,
      index: 0,
      count: importPaths.length,
      oldPaths,
      newPaths: importPaths,
    });

    // 覆盖为导入的路径
    list.clear();
    for (const p of importPaths) {
      list.add(p);
    }
    get()._markModified();
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
    get()._markModified();
  },

  // ── 撤销/重做 ──

  undo: () => {
    const { undoRedo, sysPaths, userPaths } = get();
    if (undoRedo.undo(sysPaths, userPaths)) {
      set({ isModified: true, selectedIndices: [] });
    }
  },

  redo: () => {
    const { undoRedo, sysPaths, userPaths } = get();
    if (undoRedo.redo(sysPaths, userPaths)) {
      set({ isModified: true, selectedIndices: [] });
    }
  },

  canUndo: () => get().undoRedo.canUndo(),
  canRedo: () => get().undoRedo.canRedo(),

  // ── 数据加载/保存 ──

  loadFromStringLists: (sys: string[], user: string[]) => {
    const sysPaths = StringList.fromArray(sys);
    const userPaths = StringList.fromArray(user);
    set({ sysPaths, userPaths, isModified: false, isLoading: false });
  },

  loadPaths: async () => {
    try {
      set({ isLoading: true });
      const [sysArr, userArr] = await Promise.all([
        invoke<string[]>('load_system_paths'),
        invoke<string[]>('load_user_paths'),
      ]);

      const sysPaths = StringList.fromArray(sysArr);
      const userPaths = StringList.fromArray(userArr);
      const undoRedo = new UndoRedoManager(50);

      set({
        sysPaths,
        userPaths,
        undoRedo,
        isLoading: false,
        isModified: false,
        statusMessage: i18n.t('status.loaded', {
          sysCount: sysArr.length,
          userCount: userArr.length,
        }),
      });
    } catch (e) {
      set({ isLoading: false, statusMessage: `加载失败: ${e}` });
    }
  },

  savePaths: async () => {
    const { sysPaths, userPaths } = get();
    set({ statusMessage: '正在保存...' });
    try {
      await invoke('save_system_paths', { paths: sysPaths.toArray() });
      await invoke('save_user_paths', { paths: userPaths.toArray() });
      await invoke('broadcast_env_change');
      set({ isModified: false, statusMessage: '保存成功' });
    } catch (e) {
      set({ statusMessage: `保存失败: ${e}` });
    }
  },

  initialize: async () => {
    const isAdmin: boolean = await invoke('check_admin');
    set({ isAdmin });
    if (!isAdmin) {
      set({ statusMessage: '只读模式 — 需要管理员权限才能编辑' });
    }
    await get().loadPaths();
  },
}));
