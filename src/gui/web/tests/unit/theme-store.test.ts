import { describe, it, expect, vi, beforeEach } from 'vitest';
import { initDarkMode, useThemeStore } from '@/store/theme-store';

describe('theme-store', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.classList.remove('dark');
    useThemeStore.setState({ isDark: false });
    vi.restoreAllMocks();
  });

  it('初始化时恢复深色模式', () => {
    localStorage.setItem('darkMode', '1');
    initDarkMode();
    expect(useThemeStore.getState().isDark).toBe(true);
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  it('切换深色模式时同步 DOM 与 localStorage', () => {
    useThemeStore.getState().toggle();
    expect(useThemeStore.getState().isDark).toBe(true);
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(localStorage.getItem('darkMode')).toBe('1');

    useThemeStore.getState().toggle();
    expect(useThemeStore.getState().isDark).toBe(false);
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(localStorage.getItem('darkMode')).toBe('0');
  });

  it('localStorage 读取异常时回退为浅色', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked');
    });
    expect(() => initDarkMode()).not.toThrow();
    expect(useThemeStore.getState().isDark).toBe(false);
  });
});
