import { useTranslation } from 'react-i18next';
import { useEnvStore } from '@/store/env-store';
import { envVarKey } from '@/core/env-var';
import { Button } from '@/components/ui/Button';
import {
  PencilIcon,
  PlusIcon,
  RedoIcon,
  RefreshIcon,
  SaveIcon,
  SearchIcon,
  TrashIcon,
  UndoIcon,
} from '@/components/ui/icons';
import type { EnvVarMeta, HiveFilter } from '@/core/env-var';

interface EnvVarToolbarProps {
  onCreate: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onRefresh: () => void;
  /** 打开「备份与恢复」对话框（环境变量通路的整体备份/恢复入口）。 */
  onBackup: () => void;
  onSearchChange: (query: string) => void;
  searchQuery: string;
  selected: EnvVarMeta | null;
}

const FILTERS: HiveFilter[] = ['all', 'system', 'user'];

const FILTER_LABEL_KEY: Record<HiveFilter, string> = {
  all: 'envVar.all',
  system: 'envVar.sourceSystem',
  user: 'envVar.sourceUser',
};

/**
 * 环境变量专用工具栏。
 *
 * 刻意不复用 PATH 工具栏的动作集合 —— PATH 的上移/下移/一键清理/导入/导出
 * 对普通变量语义不成立。但**外观**与主工具栏完全一致（同为 .toolbar 布局、
 * Button 原语、.tab-pill 过滤器），避免切换 Tab 时视觉跳变。
 */
export function EnvVarToolbar({
  onCreate,
  onEdit,
  onDelete,
  onRefresh,
  onBackup,
  onSearchChange,
  searchQuery,
  selected,
}: EnvVarToolbarProps) {
  const { t } = useTranslation();
  const hiveFilter = useEnvStore((s) => s.hiveFilter);
  const setHiveFilter = useEnvStore((s) => s.setHiveFilter);
  const revealed = useEnvStore((s) => s.revealed);
  const envUndo = useEnvStore((s) => s.envUndo);
  const envRedo = useEnvStore((s) => s.envRedo);
  const undoEnv = useEnvStore((s) => s.undoEnv);
  const redoEnv = useEnvStore((s) => s.redoEnv);
  const busy = useEnvStore((s) => s.isSaving);

  // 权限已由 Rust 算好：此处只读 canEdit / canDelete，不自行判断类型。
  // 敏感变量在打码状态下不允许进入编辑（Spec：必须先「显示」）。
  const maskedSensitive =
    selected !== null && selected.sensitive && !revealed.has(envVarKey(selected));
  const canEdit = selected !== null && selected.canEdit && !maskedSensitive;
  const canDelete = selected !== null && selected.canDelete;

  return (
    <div className="toolbar" data-testid="env-var-toolbar">
      <div className="toolbar-group" role="group" aria-label={t('toolbar.editGroup')}>
        <Button onClick={onCreate}>
          <PlusIcon />
          <span>{t('envVar.newVar')}</span>
        </Button>
        <Button disabled={!canEdit} onClick={onEdit}>
          <PencilIcon />
          <span>{t('button.edit')}</span>
        </Button>
        <Button variant="danger" disabled={!canDelete} onClick={onDelete}>
          <TrashIcon />
          <span>{t('button.delete')}</span>
        </Button>
      </div>
      <div className="toolbar-group" role="group" aria-label={t('toolbar.fileGroup')}>
        <Button variant="ghost" onClick={onRefresh}>
          <RefreshIcon />
          <span>{t('envVar.refresh')}</span>
        </Button>
        <Button onClick={onBackup}>
          <SaveIcon />
          <span>{t('envBackup.title')}</span>
        </Button>
      </div>
      <div className="toolbar-group" role="group" aria-label={t('toolbar.historyGroup')}>
        <Button
          variant="ghost"
          disabled={busy || envUndo.length === 0}
          onClick={() => void undoEnv()}
        >
          <UndoIcon />
          <span>{t('button.undo')}</span>
        </Button>
        <Button
          variant="ghost"
          disabled={busy || envRedo.length === 0}
          onClick={() => void redoEnv()}
        >
          <RedoIcon />
          <span>{t('button.redo')}</span>
        </Button>
      </div>
      <div className="toolbar-search-wrap">
        <SearchIcon className="toolbar-search-icon" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          placeholder={t('envVar.search')}
          aria-label={t('envVar.search')}
          className="toolbar-search"
        />
      </div>
      <div className="toolbar-group" role="group" aria-label={t('envVar.source')}>
        {FILTERS.map((filter) => (
          <button
            key={filter}
            type="button"
            data-hive-filter={filter}
            data-active={hiveFilter === filter}
            aria-pressed={hiveFilter === filter}
            className="tab-pill"
            onClick={() => setHiveFilter(filter)}
          >
            {t(FILTER_LABEL_KEY[filter])}
          </button>
        ))}
      </div>
    </div>
  );
}
