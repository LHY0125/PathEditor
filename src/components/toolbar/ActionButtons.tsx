import { useTranslation } from 'react-i18next';
import { canWriteTarget, targetForTab, useAppStore } from '@/store/app-store';
import { Button } from '@/components/ui/Button';

interface ActionButtonsProps {
  onNew: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
}

/**
 * 主行编辑组 + 排序组。
 * `onBrowse` 已移入 ⋯ 菜单（与「新建」同源，低频），`onClean` 同理。
 */
export function ActionButtons({
  onNew,
  onEdit,
  onDelete,
  onMoveUp,
  onMoveDown,
}: ActionButtonsProps) {
  const { t } = useTranslation();
  const activeTab = useAppStore((s) => s.activeTab);
  const isAdmin = useAppStore((s) => s.isAdmin);
  const pathCapabilities = useAppStore((s) => s.pathCapabilities);
  const target = targetForTab(activeTab);
  const disabled = target === null || !canWriteTarget(isAdmin, pathCapabilities, target);

  return (
    <>
      <Button disabled={disabled} onClick={onNew}>
        {t('button.new')}
      </Button>
      <Button disabled={disabled} onClick={onEdit}>
        {t('button.edit')}
      </Button>
      <Button variant="danger" disabled={disabled} onClick={onDelete}>
        {t('button.delete')}
      </Button>
      <span className="toolbar-sep" aria-hidden="true" />
      <Button
        variant="ghost"
        disabled={disabled}
        onClick={onMoveUp}
        aria-label={t('button.moveUp')}
      >
        {t('button.moveUp')}
      </Button>
      <Button
        variant="ghost"
        disabled={disabled}
        onClick={onMoveDown}
        aria-label={t('button.moveDown')}
      >
        {t('button.moveDown')}
      </Button>
    </>
  );
}
