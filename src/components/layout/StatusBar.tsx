import { useAppStore } from '@/store/app-store';
import { useThemeStore } from '@/store/theme-store';

export function StatusBar() {
  const statusMessage = useAppStore((s) => s.statusMessage);
  const isLoading = useAppStore((s) => s.isLoading);
  const isAdmin = useAppStore((s) => s.isAdmin);
  const isModified = useAppStore((s) => s.isModified);
  const isDark = useThemeStore((s) => s.isDark);

  return (
    <footer
      className="flex items-center justify-between px-4 py-1 text-xs border-t select-none"
      style={{
        borderColor: 'var(--app-border)',
        backgroundColor: 'var(--app-list-bg)',
        color: 'var(--app-fg)',
      }}
    >
      <span>{isLoading ? '加载中...' : statusMessage}</span>
      <div className="flex gap-3">
        {isModified && <span className="text-yellow-500">● 已修改</span>}
        {!isAdmin && <span className="text-yellow-500">只读</span>}
        <span style={{ opacity: 0.5 }}>{isDark ? '深色' : '浅色'}</span>
      </div>
    </footer>
  );
}
