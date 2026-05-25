import { useAppStore } from '@/store/app-store';
import { useThemeStore } from '@/store/theme-store';
import { useTranslation } from 'react-i18next';

export function StatusBar() {
  const { t } = useTranslation();
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
      <span>{isLoading ? t('status.loading') : statusMessage}</span>
      <div className="flex gap-3">
        {isModified && <span className="text-yellow-500">● {t('status.modified')}</span>}
        {!isAdmin && <span className="text-yellow-500">{t('status.readonly_label')}</span>}
        <span style={{ opacity: 0.5 }}>{isDark ? t('status.dark') : t('status.light')}</span>
      </div>
    </footer>
  );
}
