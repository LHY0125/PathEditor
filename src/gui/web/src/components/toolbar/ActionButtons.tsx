import { useTranslation } from 'react-i18next';
import { canWriteTarget, targetForTab, useAppStore } from '@/store/app-store';
import { Button } from '@/components/ui/Button';
import { ArrowDownIcon, ArrowUpIcon, PencilIcon, PlusIcon, TrashIcon } from '@/components/ui/icons';

interface ActionButtonsProps {
  onNew: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
}

/**
 * 编辑组 + 顺序组。
 * 视觉分组由 `.toolbar-group` 承担；按钮文字保持原样，图标只做辅助识别。
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
      <div className="toolbar-group" role="group" aria-label={t('toolbar.editGroup')}>
        <Button disabled={disabled} onClick={onNew}>
          <PlusIcon />
          <span>{t('button.new')}</span>
        </Button>
        <Button disabled={disabled} onClick={onEdit}>
          <PencilIcon />
          <span>{t('button.edit')}</span>
        </Button>
        <Button variant="danger" disabled={disabled} onClick={onDelete}>
          <TrashIcon />
          <span>{t('button.delete')}</span>
        </Button>
      </div>
      <div className="toolbar-group" role="group" aria-label={t('toolbar.orderGroup')}>
        <Button
          variant="ghost"
          disabled={disabled}
          onClick={onMoveUp}
          aria-label={t('button.moveUp')}
        >
          <ArrowUpIcon />
          <span>{t('button.moveUp')}</span>
        </Button>
        <Button
          variant="ghost"
          disabled={disabled}
          onClick={onMoveDown}
          aria-label={t('button.moveDown')}
        >
          <ArrowDownIcon />
          <span>{t('button.moveDown')}</span>
        </Button>
      </div>
    </>
  );
}
