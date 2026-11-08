import i18n from '@/i18n';
import { errorMessage } from '@/core/error';
import { UndoRedoManager, OperationType, TargetType } from '@/core/undo-redo';
import type { PathEntry } from '@/core/path-entry';
import appConfig from '@/config/default.json';
import { backend } from '@/services/backend';
import { arraysEqual, loadPathSnapshot, savePathSnapshot } from '@/services/path-session';
import type { PathSlice, StoreSlice } from '../app-store.types';

/**
 * PATH 数据与持久化切片。
 *
 * 所有写操作都会 push 撤销记录并调用 `markDirty`；`markDirty` 只比较
 * 「当前值 vs 最后一次成功保存的快照」，不维护独立的 dirty 计数。
 */
export const createPathSlice: StoreSlice<PathSlice> = (set, get) => {
  const markDirty = () => {
    const { _savedSys, _savedUser, sysPaths, userPaths } = get();
    set({
      isModified: !(arraysEqual(sysPaths, _savedSys) && arraysEqual(userPaths, _savedUser)),
    });
  };

  return {
    sysPaths: [],
    userPaths: [],
    undoRedo: new UndoRedoManager(appConfig.undo.maxHistory),
    _savedSys: [],
    _savedUser: [],
    isModified: false,
    isLoading: true,
    isSaving: false,

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

      // 期间用户继续编辑：这次结果基于旧列表，直接覆盖会吞掉并发改动。
      const current = get();
      const latest = target === TargetType.SYSTEM ? current.sysPaths : current.userPaths;
      if (!arraysEqual(latest, list)) {
        set({
          statusIsError: true,
          statusMessage: i18n.t('status.cleanChanged'),
        });
        return [];
      }

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
      const { undoRedo, sysPaths, userPaths, _savedSys, _savedUser } = get();
      const result = undoRedo.undo(sysPaths, userPaths);
      if (result) {
        set({
          sysPaths: result[0],
          userPaths: result[1],
          selectedIndices: [],
          isModified: !(arraysEqual(result[0], _savedSys) && arraysEqual(result[1], _savedUser)),
        });
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
          isModified: !(arraysEqual(result[0], _savedSys) && arraysEqual(result[1], _savedUser)),
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
          undoRedo: new UndoRedoManager(appConfig.undo.maxHistory),
          isLoading: false,
          isModified: false,
          statusIsError: false,
          statusMessage: i18n.t('status.loaded', {
            sysCount: sysEntries.length,
            userCount: usrEntries.length,
          }),
        });
      } catch (error) {
        set({
          isLoading: false,
          statusIsError: true,
          statusMessage: `${i18n.t('status.error')}: ${errorMessage(error)}`,
        });
      }
    },

    savePaths: async (force?: boolean) => {
      const state = get();
      if (state.isSaving) return { kind: 'blocked' };
      set({ isSaving: true, statusIsError: false, statusMessage: i18n.t('status.saving') });
      const outcome = await savePathSnapshot({
        sysPaths: state.sysPaths,
        userPaths: state.userPaths,
        savedSys: state._savedSys,
        savedUser: state._savedUser,
        isAdmin: state.isAdmin,
        capabilities: state.pathCapabilities,
        force,
      });
      // **await 之后必须重新取状态**：保存链路包含备份、两次注册表写与 DB 提交，
      // 期间用户可能继续编辑。用 await 前的快照判断 isModified，会把并发改动
      // 标成「已保存」，关窗确认随之失效（静默丢改动）。
      const current = get();
      set({
        isModified: !(
          arraysEqual(current.sysPaths, outcome.savedSys) &&
          arraysEqual(current.userPaths, outcome.savedUser)
        ),
        isSaving: false,
        statusIsError: outcome.result.kind === 'failure' || outcome.result.kind === 'partial',
        statusMessage: outcome.statusMessage,
        _savedSys: outcome.savedSys,
        _savedUser: outcome.savedUser,
      });
      return outcome.result;
    },
  };
};
