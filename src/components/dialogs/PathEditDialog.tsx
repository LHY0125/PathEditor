import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';

interface PathEditDialogProps {
  open: boolean;
  title: string;
  initialValue: string;
  onConfirm: (value: string) => void;
  onCancel: () => void;
}

export function PathEditDialog({
  open,
  title,
  initialValue,
  onConfirm,
  onCancel,
}: PathEditDialogProps) {
  const { t } = useTranslation();
  const [value, setValue] = useState(initialValue);
  const prevOpen = useRef(open);

  useEffect(() => {
    if (open && !prevOpen.current) setValue(initialValue);
    prevOpen.current = open;
  }, [open, initialValue]);

  return (
    <Modal open={open} onClose={onCancel}>
      <h2 className="text-lg font-semibold mb-4">{title}</h2>
      <label className="text-sm mb-2 block">{t('dialog.pathLabel')}</label>
      <input
        type="text"
        autoFocus
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') onConfirm(value);
        }}
        className="w-full min-w-[400px] px-3 py-2 rounded border text-sm outline-none"
        style={{
          backgroundColor: 'var(--app-list-bg)',
          color: 'var(--app-fg)',
          borderColor: 'var(--app-border)',
        }}
      />
      <div className="flex justify-end gap-2 mt-4">
        <button
          className="px-4 py-1.5 text-sm rounded border"
          style={{ borderColor: 'var(--app-border)' }}
          onClick={onCancel}
        >
          {t('dialog.cancel')}
        </button>
        <button
          className="px-4 py-1.5 text-sm rounded text-white"
          style={{ backgroundColor: '#2563eb' }}
          onClick={() => onConfirm(value)}
        >
          {t('dialog.confirm')}
        </button>
      </div>
    </Modal>
  );
}
