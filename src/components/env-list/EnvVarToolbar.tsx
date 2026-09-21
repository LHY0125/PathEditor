import { useTranslation } from 'react-i18next';
import { useEnvStore } from '@/store/env-store';
import { envVarKey } from '@/core/env-var';
import { btnClass, btnStyle } from '@/components/ui/buttons';
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
 * 刻意不复用 PATH 工具栏 —— PATH 的上移/下移/一键清理/导入/导出
 * 对普通变量语义不成立。
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

  // 权限已由 Rust 算好：此处只读 canEdit / canDelete，不自行判断类型。
  // 敏感变量在打码状态下不允许进入编辑（Spec：必须先「显示」）。
  const maskedSensitive =
    selected !== null && selected.sensitive && !revealed.has(envVarKey(selected));
  const canEdit = selected !== null && selected.canEdit && !maskedSensitive;
  const canDelete = selected !== null && selected.canDelete;

  return (
    <div className="flex items-center gap-2 flex-wrap">
      <button className={btnClass} style={btnStyle} onClick={onCreate}>
        {t('envVar.newVar')}
      </button>
      <button className={btnClass} style={btnStyle} onClick={onEdit} disabled={!canEdit}>
        {t('button.edit')}
      </button>
      <button className={btnClass} style={btnStyle} onClick={onDelete} disabled={!canDelete}>
        {t('button.delete')}
      </button>
      <button className={btnClass} style={btnStyle} onClick={onRefresh}>
        {t('envVar.refresh')}
      </button>
      <button className={btnClass} style={btnStyle} onClick={onBackup}>
        {t('envBackup.title')}
      </button>
      <input
        type="text"
        value={searchQuery}
        onChange={(e) => onSearchChange(e.target.value)}
        placeholder={t('envVar.search')}
        className="px-2 py-1 text-sm rounded border w-48"
        style={{ backgroundColor: 'var(--app-list-bg)', borderColor: 'var(--app-border)' }}
      />
      <div className="ml-auto flex items-center gap-1">
        {FILTERS.map((filter) => (
          <button
            key={filter}
            data-hive-filter={filter}
            className={`px-3 py-1 text-sm rounded border transition-colors ${
              hiveFilter === filter ? 'tab-active' : 'opacity-60'
            }`}
            style={btnStyle}
            onClick={() => setHiveFilter(filter)}
          >
            {t(FILTER_LABEL_KEY[filter])}
          </button>
        ))}
      </div>
    </div>
  );
}
