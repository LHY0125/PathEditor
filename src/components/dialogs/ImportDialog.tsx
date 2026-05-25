import { useTranslation } from 'react-i18next';

interface ImportDialogProps {
  open: boolean;
  hasSystem: boolean;
  hasUser: boolean;
  onSelect: (target: 'system' | 'user' | 'both') => void;
  onCancel: () => void;
}

export function ImportDialog({
  open,
  hasSystem,
  hasUser,
  onSelect,
  onCancel,
}: ImportDialogProps) {
  const { t } = useTranslation();

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
          {hasSystem && `系统变量: ${hasSystem}`}
          {hasSystem && hasUser && ' | '}
          {hasUser && `用户变量: ${hasUser}`}
        </p>
        <div className="flex flex-col gap-2">
          {hasSystem && (
            <button
              className="px-4 py-2 text-sm rounded border text-left"
              style={{ borderColor: 'var(--app-border)' }}
              onClick={() => onSelect('system')}
            >
              {t('dialog.importSystem')}
            </button>
          )}
          {hasUser && (
            <button
              className="px-4 py-2 text-sm rounded border text-left"
              style={{ borderColor: 'var(--app-border)' }}
              onClick={() => onSelect('user')}
            >
              {t('dialog.importUser')}
            </button>
          )}
          {hasSystem && hasUser && (
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
