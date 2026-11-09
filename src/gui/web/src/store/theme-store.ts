import { create } from 'zustand';

/** 主题模式：显式浅色/深色，或跟随系统。 */
export type ThemeMode = 'light' | 'dark' | 'system';

interface ThemeState {
  isDark: boolean;
  mode: ThemeMode;
  toggle: () => void;
  setMode: (mode: ThemeMode) => void;
}

const SYSTEM_QUERY = '(prefers-color-scheme: dark)';

function systemPrefersDark(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia(SYSTEM_QUERY).matches
  );
}

function savedMode(): ThemeMode {
  try {
    const mode = localStorage.getItem('themeMode');
    if (mode === 'light' || mode === 'dark' || mode === 'system') return mode;
    // 兼容旧键：只有显式亮/暗，没有 system
    return localStorage.getItem('darkMode') === '1' ? 'dark' : 'light';
  } catch {
    return 'system';
  }
}

function resolve(mode: ThemeMode): boolean {
  if (mode === 'system') return systemPrefersDark();
  return mode === 'dark';
}

function apply(isDark: boolean): void {
  document.documentElement.classList.toggle('dark', isDark);
}

function persist(mode: ThemeMode): void {
  try {
    localStorage.setItem('themeMode', mode);
    // 保留旧键，避免降级版本读不到主题偏好
    localStorage.setItem('darkMode', resolve(mode) ? '1' : '0');
  } catch {
    // localStorage 不可用时只影响记忆，不影响当前会话
  }
}

const initialMode = savedMode();

export const useThemeStore = create<ThemeState>((set) => ({
  isDark: resolve(initialMode),
  mode: initialMode,
  toggle: () =>
    set((state) => {
      const nextMode: ThemeMode = state.isDark ? 'light' : 'dark';
      const isDark = resolve(nextMode);
      apply(isDark);
      persist(nextMode);
      return { isDark, mode: nextMode };
    }),
  setMode: (mode) => {
    const isDark = resolve(mode);
    apply(isDark);
    persist(mode);
    set({ isDark, mode });
  },
}));

/** 初始化主题：应用已保存模式；跟随系统时监听系统主题变化。 */
export function initDarkMode(): void {
  // 启动时重新读一次持久化值：模块加载早于测试/扩展写入 localStorage 的场景
  const mode = savedMode();
  const isDark = resolve(mode);
  apply(isDark);
  useThemeStore.setState({ isDark, mode });
  if (mode !== 'system' || typeof window.matchMedia !== 'function') return;
  const media = window.matchMedia(SYSTEM_QUERY);
  const onChange = () => {
    if (useThemeStore.getState().mode !== 'system') return;
    const next = systemPrefersDark();
    apply(next);
    useThemeStore.setState({ isDark: next });
  };
  // addEventListener 在旧 WebView 上可能不存在，退回 addListener
  if (typeof media.addEventListener === 'function') {
    media.addEventListener('change', onChange);
  } else {
    media.addListener(onChange);
  }
}
