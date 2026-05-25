import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/store/app-store';

interface ActionButtonsProps {
  onNew: () => void;
  onEdit: () => void;
  onBrowse: () => void;
  onDelete: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onClean: () => void;
}

export function ActionButtons({
  onNew,
  onEdit,
  onBrowse,
  onDelete,
  onMoveUp,
  onMoveDown,
  onClean,
}: ActionButtonsProps) {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);
  const disabled = !isAdmin;

  const btnClass =
    'px-3 py-1 text-sm rounded border transition-colors disabled:opacity-40 disabled:cursor-not-allowed';
  const btnStyle = {
    backgroundColor: 'var(--app-bg)',
    color: 'var(--app-fg)',
    borderColor: 'var(--app-border)',
  };

  return (
    <div className="flex gap-1 flex-wrap">
      <button className={btnClass} style={btnStyle} disabled={disabled} onClick={onNew}>
        {t('button.new')}
      </button>
      <button className={btnClass} style={btnStyle} disabled={disabled} onClick={onEdit}>
        {t('button.edit')}
      </button>
      <button className={btnClass} style={btnStyle} disabled={disabled} onClick={onBrowse}>
        {t('button.browse')}
      </button>
      <button className={btnClass} style={btnStyle} disabled={disabled} onClick={onDelete}>
        {t('button.delete')}
      </button>
      <button className={btnClass} style={btnStyle} disabled={disabled} onClick={onMoveUp}>
        {t('button.moveUp')}
      </button>
      <button className={btnClass} style={btnStyle} disabled={disabled} onClick={onMoveDown}>
        {t('button.moveDown')}
      </button>
      <button className={btnClass} style={btnStyle} disabled={disabled} onClick={onClean}>
        {t('button.clean')}
      </button>
    </div>
  );
}
