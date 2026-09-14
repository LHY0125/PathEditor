import { useTranslation } from 'react-i18next';
import type { ToolGroup } from '@/services/backend';
import { EmptyHint } from './EmptyHint';

interface ToolsTabProps {
  groups: ToolGroup[];
  query: string;
  onQueryChange: (query: string) => void;
}

export function ToolsTab({ groups, query, onQueryChange }: ToolsTabProps) {
  const { t } = useTranslation();

  return (
    <div>
      <input
        type="text"
        value={query}
        onChange={(event) => onQueryChange(event.target.value)}
        placeholder={t('analyze.searchPlaceholder')}
        className="w-full px-3 py-1.5 text-sm rounded mb-3 border outline-none"
        style={{
          backgroundColor: 'var(--app-list-bg)',
          color: 'var(--app-fg)',
          borderColor: 'var(--app-border)',
        }}
      />
      {groups.length === 0 ? (
        <EmptyHint text={t('analyze.noTools')} />
      ) : (
        groups.map((group) => (
          <div key={group.dir} className="mb-3">
            <div
              className="text-xs font-mono py-1 px-2 rounded"
              style={{
                backgroundColor: group.exists ? 'transparent' : 'rgba(239,68,68,0.1)',
                color: group.exists ? 'var(--app-fg)' : '#ef4444',
                opacity: group.exists ? 1 : 0.6,
              }}
            >
              {group.dir} {!group.exists && t('analyze.notExists')}
            </div>
            <div className="flex flex-wrap gap-1 mt-1 ml-2">
              {group.exes.map((exe) => (
                <span
                  key={exe}
                  className="text-xs font-mono px-1.5 py-0.5 rounded"
                  style={{ backgroundColor: 'var(--app-list-bg)', color: 'var(--app-fg)' }}
                >
                  {exe}
                </span>
              ))}
            </div>
          </div>
        ))
      )}
    </div>
  );
}
