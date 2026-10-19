import { useTranslation } from 'react-i18next';
import type { ConflictEntry } from '@/services/backend';
import { EmptyHint } from './EmptyHint';

export function ConflictsTab({ conflicts }: { conflicts: ConflictEntry[] }) {
  const { t } = useTranslation();
  if (conflicts.length === 0) {
    return <EmptyHint text={t('analyze.noConflicts')} />;
  }

  return (
    <div>
      <p className="text-xs mb-2" style={{ color: 'var(--app-fg-muted)' }}>
        {t('analyze.conflictCount', { count: conflicts.length })}
      </p>
      <table className="w-full border-collapse">
        <thead>
          <tr style={{ borderBottom: '1px solid var(--app-border)' }}>
            <th className="text-left py-1.5 pr-3 font-medium text-xs">EXE</th>
            <th className="text-left py-1.5 font-medium text-xs">{t('analyze.priority')}</th>
          </tr>
        </thead>
        <tbody>
          {conflicts.map((conflict) => (
            <tr key={conflict.name} style={{ borderBottom: '1px solid var(--app-border)' }}>
              <td className="py-1.5 pr-3 font-mono text-xs">{conflict.name}</td>
              <td className="py-1.5">
                {conflict.locations.map((location, index) => (
                  <div
                    key={`${location.dir}-${location.priority}`}
                    className="text-xs py-0.5"
                    style={{
                      color: index === 0 ? 'var(--app-success)' : 'var(--app-danger)',
                    }}
                  >
                    {index === 0 ? '✓' : '✗'} {location.dir}
                    {index > 0 && (
                      <span className="ml-1" style={{ color: 'var(--app-fg-subtle)' }}>
                        ({t('analyze.shadowed')})
                      </span>
                    )}
                  </div>
                ))}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
