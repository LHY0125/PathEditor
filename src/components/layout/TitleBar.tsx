import { useAppStore } from '@/store/app-store';
import { useTranslation } from 'react-i18next';
import { version } from '../../../package.json';

export function TitleBar() {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);

  return (
    <header className="titlebar">
      <div className="titlebar-brand">
        <span className="titlebar-logo" aria-hidden="true" />
        <h1 className="titlebar-title">{isAdmin ? t('app.name') : t('app.nameReadonly')}</h1>
      </div>
      <span className="titlebar-version">v{version}</span>
    </header>
  );
}
