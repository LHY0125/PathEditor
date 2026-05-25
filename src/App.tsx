import { useEffect } from 'react';
import { useAppStore } from '@/store/app-store';
import { initDarkMode } from '@/store/theme-store';
import { AppShell } from '@/components/layout/AppShell';
import { ErrorBoundary } from '@/components/layout/ErrorBoundary';

export default function App() {
  const initialize = useAppStore((s) => s.initialize);

  useEffect(() => {
    initDarkMode();
    initialize();
  }, [initialize]);

  return (
    <ErrorBoundary>
      <AppShell />
    </ErrorBoundary>
  );
}
