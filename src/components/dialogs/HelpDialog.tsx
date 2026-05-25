import { useTranslation } from 'react-i18next';

interface HelpDialogProps {
  open: boolean;
  onClose: () => void;
}

export function HelpDialog({ open, onClose }: HelpDialogProps) {
  const { t } = useTranslation();

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: 'rgba(0,0,0,0.4)' }}
      onClick={onClose}
    >
      <div
        className="rounded-lg p-6 max-w-lg"
        style={{ backgroundColor: 'var(--app-bg)', color: 'var(--app-fg)' }}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-semibold mb-4">{t('dialog.helpTitle')}</h2>
        <pre className="text-sm whitespace-pre-wrap font-sans leading-relaxed">
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
      </div>
    </div>
  );
}
