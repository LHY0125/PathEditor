import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import packageJson from '../../../package.json';

interface HelpDialogProps {
  open: boolean;
  onClose: () => void;
}

export function HelpDialog({ open, onClose }: HelpDialogProps) {
  const { t } = useTranslation();

  return (
    <Modal open={open} onClose={onClose}>
      <h2 className="text-base font-semibold mb-4">{t('dialog.helpTitle')}</h2>
      <pre
        className="whitespace-pre-wrap font-sans leading-relaxed max-w-lg"
        style={{ color: 'var(--app-fg-muted)' }}
      >
        {t('help.content', { version: packageJson.version })}
      </pre>
      <div className="flex justify-end mt-4">
        <Button variant="primary" onClick={onClose}>
          {t('dialog.confirm')}
        </Button>
      </div>
    </Modal>
  );
}
