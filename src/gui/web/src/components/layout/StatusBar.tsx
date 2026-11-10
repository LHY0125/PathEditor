import { useAppStore } from '@/store/app-store';
import { useEnvStore } from '@/store/env-store';
import { useThemeStore } from '@/store/theme-store';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/Button';

export function StatusBar() {
  const { t } = useTranslation();
  const activeTab = useAppStore((s) => s.activeTab);
  const pathStatusMessage = useAppStore((s) => s.statusMessage);
  const pathStatusIsError = useAppStore((s) => s.statusIsError);
  const pathIsLoading = useAppStore((s) => s.isLoading);
  const isAdmin = useAppStore((s) => s.isAdmin);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const isModified = useAppStore((s) => s.isModified);
  const sysPaths = useAppStore((s) => s.sysPaths);
  const userPaths = useAppStore((s) => s.userPaths);
  const envSnapshot = useEnvStore((s) => s.snapshot);
  const envStatusMessage = useEnvStore((s) => s.statusMessage);
  const envStatusIsError = useEnvStore((s) => s.statusIsError);
  const envIsLoading = useEnvStore((s) => s.isLoading);
  const isDark = useThemeStore((s) => s.isDark);
  const toggleTheme = useThemeStore((s) => s.toggle);

  // 「全部变量」下展示环境变量操作的状态（create/remove/reveal 的成败）；
  // 其余 Tab 维持 PATH 语义不变。
  const isAllVars = activeTab === 'allVars';
  // 计数跟随当前视角：环境变量页统计 env，其余页统计 PATH。
  // 环境变量快照未就绪时显示 `—`，不要把「尚未加载」伪装成 0。
  const sysCount = isAllVars ? (envSnapshot?.system.length ?? null) : sysPaths.length;
  const userCount = isAllVars ? (envSnapshot?.user.length ?? null) : userPaths.length;
  const totalCount = sysCount === null || userCount === null ? null : sysCount + userCount;
  const countLabel = isAllVars ? t('status.envCountsLabel') : t('status.countsLabel');
  const statusMessage = isAllVars ? envStatusMessage : pathStatusMessage;
  const isLoading = isAllVars ? envIsLoading : pathIsLoading;
  // 只认结构化布尔：此前用 `statusMessage.includes(t('status.error'))` 判定，
  // 切到英文后旧的中文错误文案就再也匹配不上，重试按钮会消失。
  const hasError = isAllVars ? envStatusIsError : pathStatusIsError;

  const retry = () => {
    if (isAllVars) {
      // 先清掉错误态与旧文案：成功刷新后不应继续显示上一次的错误。
      useEnvStore.getState().setStatusMessage('');
      void useEnvStore.getState().load();
    } else {
      void useAppStore.getState().loadPaths();
    }
  };

  return (
    <footer className="statusbar">
      <div className="statusbar-counts" role="group" aria-label={countLabel}>
        <span className="statusbar-count">
          {t('status.systemCount')} <strong>{sysCount ?? '—'}</strong>
        </span>
        <span className="statusbar-count">
          {t('status.userCount')} <strong>{userCount ?? '—'}</strong>
        </span>
        <span className="statusbar-count">
          {t('status.totalCount')} <strong>{totalCount ?? '—'}</strong>
        </span>
      </div>
      <div className="statusbar-message" role="status" aria-live="polite">
        <span className="truncate">{isLoading ? t('status.loading') : statusMessage}</span>
        {hasError && !isLoading && (
          <Button size="sm" onClick={retry}>
            {t('button.retry')}
          </Button>
        )}
      </div>
      <div className="statusbar-actions">
        {isModified && <span className="statusbar-warn">● {t('status.modified')}</span>}
        {!isAdmin && !pathCapabilities.canWriteUser && (
          <span className="statusbar-warn">{t('status.readonly_label')}</span>
        )}
        <button
          type="button"
          className="statusbar-theme"
          title={t('status.toggleTheme')}
          onClick={toggleTheme}
        >
          {isDark ? t('status.dark') : t('status.light')}
        </button>
      </div>
    </footer>
  );
}
