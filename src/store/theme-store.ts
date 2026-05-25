import { create } from 'zustand';

interface ThemeState {
  isDark: boolean;
  toggle: () => void;
}

export const useThemeStore = create<ThemeState>((set) => ({
  isDark: false,
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

/** 初始化深色模式状态（从 localStorage 读取） */
export function initDarkMode(): void {
  const saved = localStorage.getItem('darkMode');
  if (saved === '1') {
    document.documentElement.classList.add('dark');
  }
}
