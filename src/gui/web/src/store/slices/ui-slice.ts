import { EMPTY_CAPABILITIES } from '@/core/path-capabilities';
import type { UiSlice, StoreSlice } from '../app-store.types';

/**
 * UI 状态切片：标签页、搜索、选择、权限与状态栏。
 *
 * 这里不碰 PATH 数据，也不发 IPC；所有运行时数据由 path 切片持有。
 */
export const createUiSlice: StoreSlice<UiSlice> = (set) => ({
  activeTab: 'system',
  searchQuery: '',
  selectedIndices: [],
  scrollToIndex: null,
  isAdmin: false,
  pathCapabilities: { ...EMPTY_CAPABILITIES },
  statusMessage: '',
  statusIsError: false,

  setActiveTab: (tab) => set({ activeTab: tab }),
  setSearchQuery: (query) => set({ searchQuery: query }),
  setSelectedIndices: (indices) => set({ selectedIndices: indices }),
  setScrollToIndex: (index) => set({ scrollToIndex: index }),
  setStatusMessage: (msg) => set({ statusMessage: msg, statusIsError: false }),
  setStatusError: (msg) => set({ statusMessage: msg, statusIsError: true }),
});
