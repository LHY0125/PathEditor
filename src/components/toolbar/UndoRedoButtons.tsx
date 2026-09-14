import { useTranslation } from 'react-i18next';
import { canWriteTarget, targetForTab, useAppStore } from '@/store/app-store';
import { btnClass, btnStyle } from '@/components/ui/buttons';

export function UndoRedoButtons() {
  const { t } = useTranslation();
  const activeTab = useAppStore((s) => s.activeTab);
  const isAdmin = useAppStore((s) => s.isAdmin);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const undoRedo = useAppStore((s) => s.undoRedo);
  const target = targetForTab(activeTab);
  const canWrite = target === null ? false : canWriteTarget(isAdmin, pathCapabilities, target);
  // 订阅路径数组长度变化，确保 undoRedo 内部状态变化时触发重渲染
  useAppStore((s) => s.sysPaths.length);
  useAppStore((s) => s.userPaths.length);
  const undo = useAppStore((s) => s.undo);
  const redo = useAppStore((s) => s.redo);

  return (
    <div className="flex gap-1">
      <button
        className={btnClass}
        style={btnStyle}
        disabled={!canWrite || !undoRedo.canUndo()}
        onClick={undo}
      >
        {t('button.undo')}
      </button>
      <button
        className={btnClass}
        style={btnStyle}
        disabled={!canWrite || !undoRedo.canRedo()}
        onClick={redo}
      >
        {t('button.redo')}
      </button>
    </div>
  );
}
