import { useTranslation } from 'react-i18next';
import { useEnvStore } from '@/store/env-store';
import { btnClass, btnStyle } from '@/components/ui/buttons';
import type { EnvVarMeta, HiveFilter } from '@/core/env-var';

interface EnvVarToolbarProps {
  onCreate: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onRefresh: () => void;
  selected: EnvVarMeta | null;
}

const FILTERS: HiveFilter[] = ['all', 'system', 'user'];

const FILTER_LABEL_KEY: Record<HiveFilter, string> = {
  all: 'envVar.all',
  system: 'tab.system',
  user: 'tab.user',
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
  selected,
}: EnvVarToolbarProps) {
  const { t } = useTranslation();
  const hiveFilter = useEnvStore((s) => s.hiveFilter);
  const setHiveFilter = useEnvStore((s) => s.setHiveFilter);

  // 权限已由 Rust 算好：此处只读 canEdit / canDelete，不自行判断类型。
  const canEdit = selected !== null && selected.canEdit;
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
