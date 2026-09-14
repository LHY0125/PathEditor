import { create } from 'zustand';
import i18n from '@/i18n';
import { UndoRedoManager, OperationType, TargetType } from '@/core/undo-redo';
import type { PathEntry } from '@/core/path-entry';
import appConfig from '@/config/default.json';
import { backend } from '@/services/backend';
import {
  arraysEqual,
  loadPathSnapshot,
  savePathSnapshot,
  type SaveResult,
} from '@/services/path-session';
import { EMPTY_CAPABILITIES, type PathCapabilities, type TabId } from '@/core/path-capabilities';

export { canWriteTarget, EMPTY_CAPABILITIES, targetForTab } from '@/core/path-capabilities';
export type { PathCapabilities, TabId } from '@/core/path-capabilities';
export type { SaveResult } from '@/services/path-session';

interface AppState {
  sysPaths: PathEntry[];
  userPaths: PathEntry[];
  undoRedo: UndoRedoManager;
  _savedSys: PathEntry[]; // 上次成功写入注册表的快照，用于 isModified 判断
  _savedUser: PathEntry[];
  _pendingSys: PathEntry[] | null; // 注册表已提交、但 disabled.json 待补写的快照
  _pendingUser: PathEntry[] | null;

  activeTab: TabId;
  searchQuery: string;
  selectedIndices: number[];
  isAdmin: boolean;
  pathCapabilities: PathCapabilities;
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
  cleanPaths: (target: TargetType) => Promise<string[]>;
  replacePaths: (target: TargetType, newEntries: PathEntry[]) => void;
  replaceBothPaths: (sysEntries: PathEntry[], userEntries: PathEntry[]) => void;
  clearPaths: (target: TargetType) => void;

  togglePath: (index: number, target: TargetType) => void;

  undo: () => void;
  redo: () => void;

  loadPaths: () => Promise<void>;
  savePaths: (force?: boolean) => Promise<SaveResult>;
  initialize: () => Promise<void>;
}

export const useAppStore = create<AppState>((set, get) => {
  const markDirty = () => {
    const { _savedSys, _savedUser, _pendingSys, _pendingUser, sysPaths, userPaths } = get();
    set({
      isModified:
        !(arraysEqual(sysPaths, _savedSys) && arraysEqual(userPaths, _savedUser)) ||
        Boolean(_pendingSys) ||
        Boolean(_pendingUser),
    });
  };

  return {
    sysPaths: [],
    userPaths: [],
    undoRedo: new UndoRedoManager(appConfig.undo.maxHistory),
    _savedSys: [],
    _savedUser: [],
    _pendingSys: null,
    _pendingUser: null,

    activeTab: 'system',
    searchQuery: '',
    selectedIndices: [],
    isAdmin: false,
    pathCapabilities: { ...EMPTY_CAPABILITIES },
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

    cleanPaths: async (target) => {
      const state = get();
      const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
      const [kept, removed] = await backend.cleanPathEntries(list);

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

      return removed.map((entry) => entry.path);
    },

    replacePaths: (target, newEntries) => {
      if (newEntries.length === 0) return;
      const state = get();
      const list = target === TargetType.SYSTEM ? state.sysPaths : state.userPaths;
      const entries = newEntries.map((entry) => ({ ...entry }));

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

    replaceBothPaths: (sysEntries, userEntries) => {
      const state = get();
      const nextSys = sysEntries.map((entry) => ({ ...entry }));
      const nextUser = userEntries.map((entry) => ({ ...entry }));
      state.undoRedo.push({
        type: OperationType.IMPORT_BOTH,
        target: TargetType.SYSTEM,
        index: 0,
        count: nextSys.length + nextUser.length,
        oldPaths: [...state.sysPaths],
        newPaths: [...nextSys],
        oldPathsOther: [...state.userPaths],
        newPathsOther: [...nextUser],
      });
      set({ sysPaths: [...nextSys], userPaths: [...nextUser], selectedIndices: [] });
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
    },

    undo: () => {
      const { undoRedo, sysPaths, userPaths, _savedSys, _savedUser, _pendingSys, _pendingUser } =
        get();
      const result = undoRedo.undo(sysPaths, userPaths);
      if (result) {
        set({
          sysPaths: result[0],
          userPaths: result[1],
          selectedIndices: [],
          isModified:
            !(arraysEqual(result[0], _savedSys) && arraysEqual(result[1], _savedUser)) ||
            Boolean(_pendingSys) ||
            Boolean(_pendingUser),
        });
      }
    },

    redo: () => {
      const { undoRedo, sysPaths, userPaths, _savedSys, _savedUser, _pendingSys, _pendingUser } =
        get();
      const result = undoRedo.redo(sysPaths, userPaths);
      if (result) {
        set({
          sysPaths: result[0],
          userPaths: result[1],
          selectedIndices: [],
          isModified:
            !(arraysEqual(result[0], _savedSys) && arraysEqual(result[1], _savedUser)) ||
            Boolean(_pendingSys) ||
            Boolean(_pendingUser),
        });
      }
    },

    loadPaths: async () => {
      try {
        set({ isLoading: true });
        const { sysEntries, usrEntries } = await loadPathSnapshot();

        set({
          sysPaths: sysEntries,
          userPaths: usrEntries,
          _savedSys: [...sysEntries],
          _savedUser: [...usrEntries],
          _pendingSys: null,
          _pendingUser: null,
          undoRedo: new UndoRedoManager(appConfig.undo.maxHistory),
          isLoading: false,
          isModified: false,
          statusMessage: i18n.t('status.loaded', {
            sysCount: sysEntries.length,
            userCount: usrEntries.length,
          }),
        });
      } catch (error) {
        set({ isLoading: false, statusMessage: `${i18n.t('status.error')}: ${String(error)}` });
      }
    },

    savePaths: async (force?: boolean) => {
      const state = get();
      if (state.isSaving) return { kind: 'blocked' };
      set({ isSaving: true, statusMessage: i18n.t('status.saving') });
      const outcome = await savePathSnapshot({
        sysPaths: state.sysPaths,
        userPaths: state.userPaths,
        savedSys: state._savedSys,
        savedUser: state._savedUser,
        pendingSys: state._pendingSys,
        pendingUser: state._pendingUser,
        isAdmin: state.isAdmin,
        capabilities: state.pathCapabilities,
        force,
      });
      set({
        isModified:
          !(
            arraysEqual(state.sysPaths, outcome.savedSys) &&
            arraysEqual(state.userPaths, outcome.savedUser)
          ) ||
          Boolean(outcome.pendingSys) ||
          Boolean(outcome.pendingUser),
        isSaving: false,
        statusMessage: outcome.statusMessage,
        _savedSys: outcome.savedSys,
        _savedUser: outcome.savedUser,
        _pendingSys: outcome.pendingSys,
        _pendingUser: outcome.pendingUser,
      });
      return outcome.result;
    },

    initialize: async () => {
      try {
        const capabilities = await backend.getPathCapabilities().catch(() => null);
        if (capabilities) {
          set({
            pathCapabilities: capabilities,
            isAdmin: capabilities.canWriteSystem,
          });
        } else {
          const isAdmin = await backend.checkAdmin().catch(() => false);
          set({
            isAdmin,
            pathCapabilities: {
              canReadSystem: isAdmin,
              canWriteSystem: isAdmin,
              canReadUser: true,
              canWriteUser: true,
            },
          });
        }
      } catch {
        set({
          isAdmin: false,
          pathCapabilities: { ...EMPTY_CAPABILITIES },
          statusMessage: i18n.t('status.readonly'),
        });
      }
      await get().loadPaths();
    },
  };
});
