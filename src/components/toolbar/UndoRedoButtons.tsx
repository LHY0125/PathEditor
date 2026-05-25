import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/store/app-store';

export function UndoRedoButtons() {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);
  const undoRedo = useAppStore((s) => s.undoRedo);
  const undo = useAppStore((s) => s.undo);
  const redo = useAppStore((s) => s.redo);

  const btnClass =
    'px-3 py-1 text-sm rounded border transition-colors disabled:opacity-40 disabled:cursor-not-allowed';
  const btnStyle = {
    backgroundColor: 'var(--app-bg)',
    color: 'var(--app-fg)',
    borderColor: 'var(--app-border)',
  };

  return (
    <div className="flex gap-1">
      <button
        className={btnClass}
        style={btnStyle}
        disabled={!isAdmin || !undoRedo.canUndo()}
        onClick={undo}
      >
        {t('button.undo')}
      </button>
      <button
        className={btnClass}
        style={btnStyle}
        disabled={!isAdmin || !undoRedo.canRedo()}
        onClick={redo}
      >
        {t('button.redo')}
      </button>
    </div>
  );
}
