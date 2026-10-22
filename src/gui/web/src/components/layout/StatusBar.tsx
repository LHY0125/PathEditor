import { useAppStore } from '@/store/app-store';
import { useEnvStore } from '@/store/env-store';
import { useThemeStore } from '@/store/theme-store';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/Button';

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
    <footer className="statusbar">
      <div className="flex items-center gap-2">
        <span>{isLoading ? t('status.loading') : statusMessage}</span>
        {hasError && !isLoading && (
          <Button size="sm" onClick={retry}>
            {t('button.retry')}
          </Button>
        )}
      </div>
      <div className="flex gap-3">
        {isModified && <span className="statusbar-warn">● {t('status.modified')}</span>}
        {!isAdmin && !pathCapabilities.canWriteUser && (
          <span className="statusbar-warn">{t('status.readonly_label')}</span>
        )}
        <span className="statusbar-muted">{isDark ? t('status.dark') : t('status.light')}</span>
      </div>
    </footer>
  );
}
