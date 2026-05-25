import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

interface ImportDialogProps {
  open: boolean;
  systemCount: number;
  userCount: number;
  onSelect: (target: 'system' | 'user' | 'both') => void;
  onCancel: () => void;
}

export function ImportDialog({
  open,
  systemCount,
  userCount,
  onSelect,
  onCancel,
}: ImportDialogProps) {
  const { t } = useTranslation();

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCancel();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onCancel]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: 'rgba(0,0,0,0.4)' }}
      onClick={onCancel}
    >
      <div
        className="rounded-lg p-6"
        style={{ backgroundColor: 'var(--app-bg)', color: 'var(--app-fg)' }}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-semibold mb-4">{t('dialog.importTarget')}</h2>
        <p className="text-sm mb-4 opacity-70">
          {systemCount > 0 && `系统变量: ${systemCount} 条`}
          {systemCount > 0 && userCount > 0 && ' | '}
          {userCount > 0 && `用户变量: ${userCount} 条`}
        </p>
        <div className="flex flex-col gap-2">
          {systemCount > 0 && (
            <button
              className="px-4 py-2 text-sm rounded border text-left"
              style={{ borderColor: 'var(--app-border)' }}
              onClick={() => onSelect('system')}
            >
              {t('dialog.importSystem')}
            </button>
          )}
          {userCount > 0 && (
            <button
              className="px-4 py-2 text-sm rounded border text-left"
              style={{ borderColor: 'var(--app-border)' }}
              onClick={() => onSelect('user')}
            >
              {t('dialog.importUser')}
            </button>
          )}
          {systemCount > 0 && userCount > 0 && (
            <button
              className="px-4 py-2 text-sm rounded border text-left"
              style={{ borderColor: 'var(--app-border)' }}
              onClick={() => onSelect('both')}
            >
              {t('dialog.importBoth')}
            </button>
          )}
          <button
            className="px-4 py-2 text-sm rounded border mt-2"
            style={{ borderColor: 'var(--app-border)' }}
            onClick={onCancel}
          >
            {t('dialog.cancel')}
          </button>
        </div>
      </div>
    </div>
  );
}
