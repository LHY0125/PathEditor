import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';

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
    <Modal open={open} onClose={onCancel} ariaLabel={title}>
      <h2 className="text-base font-semibold mb-4">{title}</h2>
      <label className="mb-2 block" style={{ color: 'var(--app-fg-muted)' }}>
        {t('dialog.pathLabel')}
      </label>
      <input
        type="text"
        autoFocus
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') onConfirm(value);
        }}
        className="field-input w-full min-w-[400px]"
      />
      <div className="flex justify-end gap-2 mt-4">
        <Button onClick={onCancel}>{t('dialog.cancel')}</Button>
        <Button variant="primary" onClick={() => onConfirm(value)}>
          {t('dialog.confirm')}
        </Button>
      </div>
    </Modal>
  );
}
