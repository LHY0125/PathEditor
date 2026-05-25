import { useAppStore } from '@/store/app-store';
import { useTranslation } from 'react-i18next';

export function TitleBar() {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);

  return (
    <header
      className="flex items-center justify-between px-4 py-2 border-b select-none"
      style={{ borderColor: 'var(--app-border)' }}
    >
      <h1 className="text-lg font-semibold">
        {isAdmin ? t('app.name') : t('app.nameReadonly')}
      </h1>
      <span className="text-sm opacity-60">v4.0</span>
    </header>
  );
}
