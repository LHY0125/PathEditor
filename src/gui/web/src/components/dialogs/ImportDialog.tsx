import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';

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

  return (
    <Modal open={open} onClose={onCancel} ariaLabel={t('dialog.importTarget')}>
      <h2 className="text-base font-semibold mb-4">{t('dialog.importTarget')}</h2>
      <p className="text-xs mb-4" style={{ color: 'var(--app-fg-muted)' }}>
        {systemCount > 0 && t('dialog.importSystemCount', { count: systemCount })}
        {systemCount > 0 && userCount > 0 && ' | '}
        {userCount > 0 && t('dialog.importUserCount', { count: userCount })}
      </p>
      <div className="flex flex-col gap-2">
        {systemCount > 0 && (
          <Button
            className="btn-block"
            disabled={!canWriteSystem}
            onClick={() => onSelect('system')}
          >
            {t('dialog.importSystem')}
          </Button>
        )}
        {userCount > 0 && (
          <Button className="btn-block" disabled={!canWriteUser} onClick={() => onSelect('user')}>
            {t('dialog.importUser')}
          </Button>
        )}
        {systemCount > 0 && userCount > 0 && (
          <Button
            className="btn-block"
            disabled={!canWriteSystem || !canWriteUser}
            onClick={() => onSelect('both')}
          >
            {t('dialog.importBoth')}
          </Button>
        )}
        <Button className="mt-2" onClick={onCancel}>
          {t('dialog.cancel')}
        </Button>
      </div>
    </Modal>
  );
}
