import { create } from 'zustand';
import i18n from '@/i18n';
import { backend } from '@/services/backend';
import { EMPTY_CAPABILITIES } from '@/core/path-capabilities';
import { createPathSlice } from './slices/path-slice';
import { createUiSlice } from './slices/ui-slice';
import type { AppState } from './app-store.types';

export { canWriteTarget, EMPTY_CAPABILITIES, targetForTab } from '@/core/path-capabilities';
export type { PathCapabilities, TabId } from '@/core/path-capabilities';
export type { SaveResult } from '@/services/path-session';
export type { AppState, PathSlice, UiSlice } from './app-store.types';

/**
 * 应用 store：由 UI 切片与 PATH 切片组合而成。
 *
 * 拆分的目的是让「新增一个界面交互」与「新增一种 PATH 操作」落在不同文件，
 * 而不是都堆进同一个对象。对外仍然只有一个 `useAppStore`。
 */
export const useAppStore = create<AppState>()((...args) => {
  const [set, get] = args;
  return {
    ...createUiSlice(...args),
    ...createPathSlice(...args),

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
          statusIsError: false,
          statusMessage: i18n.t('status.readonly'),
        });
      }
      await get().loadPaths();
    },
  };
});
