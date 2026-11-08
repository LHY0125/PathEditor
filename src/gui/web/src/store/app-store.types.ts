import type { StateCreator } from 'zustand';
import type { TargetType, UndoRedoManager } from '@/core/undo-redo';
import type { PathEntry } from '@/core/path-entry';
import type { PathCapabilities, TabId } from '@/core/path-capabilities';
import type { SaveResult } from '@/services/path-session';

/**
 * 数据与持久化切片：PATH 条目、撤销/重做、加载与保存。
 *
 * 字段名保持历史命名（含 `_saved*` 私有字段），因为测试与
 * `use-app-actions` 都直接读取它们。
 */
export interface PathSlice {
  sysPaths: PathEntry[];
  userPaths: PathEntry[];
  undoRedo: UndoRedoManager;
  /** 上次成功写入注册表的快照，用于 `isModified` 判断 */
  _savedSys: PathEntry[];
  _savedUser: PathEntry[];

  isModified: boolean;
  isLoading: boolean;
  isSaving: boolean;

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
}

/** UI 状态切片：标签页、搜索、选择、权限与状态栏。 */
export interface UiSlice {
  activeTab: TabId;
  searchQuery: string;
  selectedIndices: number[];
  /** 请求表格滚动到某条逻辑索引（搜索定位用）；消费后置回 null。 */
  scrollToIndex: number | null;
  isAdmin: boolean;
  pathCapabilities: PathCapabilities;
  statusMessage: string;
  /** 状态栏当前文案是否为错误（只认布尔，不靠本地化文案匹配）。 */
  statusIsError: boolean;

  setActiveTab: (tab: TabId) => void;
  setSearchQuery: (query: string) => void;
  setSelectedIndices: (indices: number[]) => void;
  setScrollToIndex: (index: number | null) => void;
  setStatusMessage: (msg: string) => void;
  setStatusError: (msg: string) => void;
}

/** 完整 store：两个切片 + 启动编排。 */
export interface AppState extends PathSlice, UiSlice {
  initialize: () => Promise<void>;
}

/** 切片创建器的统一签名（互斥量列表默认为空）。 */
export type StoreSlice<T> = StateCreator<AppState, [], [], T>;
