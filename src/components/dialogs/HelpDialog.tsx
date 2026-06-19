import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';

interface HelpDialogProps {
  open: boolean;
  onClose: () => void;
}

export function HelpDialog({ open, onClose }: HelpDialogProps) {
  const { t } = useTranslation();

  return (
    <Modal open={open} onClose={onClose}>
      <h2 className="text-lg font-semibold mb-4">{t('dialog.helpTitle')}</h2>
      <pre className="text-sm whitespace-pre-wrap font-sans leading-relaxed max-w-lg">
        {t('help.content')}
      </pre>
      <div className="flex justify-end mt-4">
        <button
          className="px-4 py-1.5 text-sm rounded text-white"
          style={{ backgroundColor: '#2563eb' }}
          onClick={onClose}
        >
          {t('dialog.confirm')}
        </button>
      </div>
    </Modal>
  );
}
