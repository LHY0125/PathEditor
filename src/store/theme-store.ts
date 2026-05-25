import { create } from 'zustand';

interface ThemeState {
  isDark: boolean;
  toggle: () => void;
}

function getSavedDarkMode(): boolean {
  try {
    return localStorage.getItem('darkMode') === '1';
  } catch {
    return false;
  }
}

export const useThemeStore = create<ThemeState>((set) => ({
  isDark: getSavedDarkMode(),
  toggle: () =>
    set((state) => {
      const next = !state.isDark;
      const root = document.documentElement;
      if (next) {
        root.classList.add('dark');
      } else {
        root.classList.remove('dark');
      }
      localStorage.setItem('darkMode', next ? '1' : '0');
      return { isDark: next };
    }),
}));

/** 初始化深色模式（DOM 类名 + store 状态） */
export function initDarkMode(): void {
  if (getSavedDarkMode()) {
    document.documentElement.classList.add('dark');
    useThemeStore.setState({ isDark: true });
  }
}
