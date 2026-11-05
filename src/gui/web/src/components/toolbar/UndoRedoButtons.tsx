import { useTranslation } from 'react-i18next';
import { canWriteTarget, targetForTab, useAppStore } from '@/store/app-store';
import { Button } from '@/components/ui/Button';

export function UndoRedoButtons() {
  const { t } = useTranslation();
  const activeTab = useAppStore((s) => s.activeTab);
  const isAdmin = useAppStore((s) => s.isAdmin);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const target = targetForTab(activeTab);
  const canWrite = target === null ? false : canWriteTarget(isAdmin, pathCapabilities, target);
  // 订阅**布尔值**而不是数组长度：编辑已有条目/切换 enabled 时长度不变，
  // 只订阅长度会让按钮状态停留在旧值（Ctrl+Z 能用、按钮却是禁用的）。
  const canUndo = useAppStore((s) => s.undoRedo.canUndo());
  const canRedo = useAppStore((s) => s.undoRedo.canRedo());
  const undo = useAppStore((s) => s.undo);
  const redo = useAppStore((s) => s.redo);

  return (
    <div className="flex gap-1">
      <Button variant="ghost" disabled={!canWrite || !canUndo} onClick={undo}>
        {t('button.undo')}
      </Button>
      <Button variant="ghost" disabled={!canWrite || !canRedo} onClick={redo}>
        {t('button.redo')}
      </Button>
    </div>
  );
}
