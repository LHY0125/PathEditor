import { useEffect } from 'react';
import { useAppStore } from '@/store/app-store';
import { initDarkMode, useThemeStore } from '@/store/theme-store';
import { AppShell } from '@/components/layout/AppShell';

export default function App() {
  const initialize = useAppStore((s) => s.initialize);

  useEffect(() => {
    initDarkMode();
    const saved = localStorage.getItem('darkMode');
    if (saved === '1') {
      useThemeStore.setState({ isDark: true });
    }
    initialize();
  }, [initialize]);

  return <AppShell />;
}
