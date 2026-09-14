import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';

interface ImportDialogProps {
  open: boolean;
  systemCount: number;
  userCount: number;
  canWriteSystem: boolean;
  canWriteUser: boolean;
  onSelect: (target: 'system' | 'user' | 'both') => void;
  onCancel: () => void;
}

export function ImportDialog({
  open,
  systemCount,
  userCount,
  canWriteSystem,
  canWriteUser,
  onSelect,
  onCancel,
}: ImportDialogProps) {
  const { t } = useTranslation();
  const optionClass = (allowed: boolean) =>
    `px-4 py-2 text-sm rounded border text-left ${
      allowed ? 'cursor-pointer' : 'cursor-not-allowed opacity-50'
    }`;

  return (
    <Modal open={open} onClose={onCancel}>
      <h2 className="text-lg font-semibold mb-4">{t('dialog.importTarget')}</h2>
      <p className="text-sm mb-4 opacity-70">
        {systemCount > 0 && t('dialog.importSystemCount', { count: systemCount })}
        {systemCount > 0 && userCount > 0 && ' | '}
        {userCount > 0 && t('dialog.importUserCount', { count: userCount })}
      </p>
      <div className="flex flex-col gap-2">
        {systemCount > 0 && (
          <button
            className={optionClass(canWriteSystem)}
            style={{ borderColor: 'var(--app-border)' }}
            disabled={!canWriteSystem}
            onClick={() => onSelect('system')}
          >
            {t('dialog.importSystem')}
          </button>
        )}
        {userCount > 0 && (
          <button
            className={optionClass(canWriteUser)}
            style={{ borderColor: 'var(--app-border)' }}
            disabled={!canWriteUser}
            onClick={() => onSelect('user')}
          >
            {t('dialog.importUser')}
          </button>
        )}
        {systemCount > 0 && userCount > 0 && (
          <button
            className={optionClass(canWriteSystem && canWriteUser)}
            style={{ borderColor: 'var(--app-border)' }}
            disabled={!canWriteSystem || !canWriteUser}
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
    </Modal>
  );
}
