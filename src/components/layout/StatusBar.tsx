import { useAppStore } from '@/store/app-store';
import { useEnvStore } from '@/store/env-store';
import { useThemeStore } from '@/store/theme-store';
import { useTranslation } from 'react-i18next';

export function StatusBar() {
  const { t } = useTranslation();
  const activeTab = useAppStore((s) => s.activeTab);
  const pathStatusMessage = useAppStore((s) => s.statusMessage);
  const pathIsLoading = useAppStore((s) => s.isLoading);
  const isAdmin = useAppStore((s) => s.isAdmin);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const isModified = useAppStore((s) => s.isModified);
  const envStatusMessage = useEnvStore((s) => s.statusMessage);
  const envIsLoading = useEnvStore((s) => s.isLoading);
  const isDark = useThemeStore((s) => s.isDark);

  // 「全部变量」下展示环境变量操作的状态（create/remove/reveal 的成败）；
  // 其余 Tab 维持 PATH 语义不变。
  const isAllVars = activeTab === 'allVars';
  const statusMessage = isAllVars ? envStatusMessage : pathStatusMessage;
  const isLoading = isAllVars ? envIsLoading : pathIsLoading;
  const hasError = statusMessage.includes(t('status.error'));

  const retry = () => {
    if (isAllVars) void useEnvStore.getState().load();
    else void useAppStore.getState().loadPaths();
  };

  return (
    <footer
      className="flex items-center justify-between px-4 py-1 text-xs border-t select-none"
      style={{
        borderColor: 'var(--app-border)',
        backgroundColor: 'var(--app-list-bg)',
        color: 'var(--app-fg)',
      }}
    >
      <div className="flex items-center gap-2">
        <span>{isLoading ? t('status.loading') : statusMessage}</span>
        {hasError && !isLoading && (
          <button
            className="px-2 py-0.5 rounded border text-xs"
            style={{ borderColor: 'var(--app-border)' }}
            onClick={retry}
          >
            {t('button.retry')}
          </button>
        )}
      </div>
      <div className="flex gap-3">
        {isModified && <span className="text-yellow-500">● {t('status.modified')}</span>}
        {!isAdmin && !pathCapabilities.canWriteUser && (
          <span className="text-yellow-500">{t('status.readonly_label')}</span>
        )}
        <span style={{ opacity: 0.5 }}>{isDark ? t('status.dark') : t('status.light')}</span>
      </div>
    </footer>
  );
}
