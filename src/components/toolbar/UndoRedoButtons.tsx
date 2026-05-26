import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/store/app-store';
import { btnClass, btnStyle } from '@/components/ui/buttons';

export function UndoRedoButtons() {
  const { t } = useTranslation();
  const isAdmin = useAppStore((s) => s.isAdmin);
  const undoRedo = useAppStore((s) => s.undoRedo);
  const undo = useAppStore((s) => s.undo);
  const redo = useAppStore((s) => s.redo);

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
