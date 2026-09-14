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
      <p className="text-sm mb-2" style={{ color: 'var(--app-fg)', opacity: 0.7 }}>
        {t('analyze.conflictCount', { count: conflicts.length })}
      </p>
      <table className="w-full text-sm border-collapse">
        <thead>
          <tr className="border-b" style={{ borderColor: 'var(--app-border)' }}>
            <th className="text-left py-1.5 pr-3 font-medium">EXE</th>
            <th className="text-left py-1.5 font-medium">{t('analyze.priority')}</th>
          </tr>
        </thead>
        <tbody>
          {conflicts.map((conflict) => (
            <tr
              key={conflict.name}
              className="border-b"
              style={{ borderColor: 'var(--app-border)' }}
            >
              <td className="py-1.5 pr-3 font-mono">{conflict.name}</td>
              <td className="py-1.5">
                {conflict.locations.map((location, index) => (
                  <div
                    key={`${location.dir}-${location.priority}`}
                    className="text-xs py-0.5"
                    style={{ color: index === 0 ? '#22c55e' : '#ef4444' }}
                  >
                    {index === 0 ? '✓' : '✗'} {location.dir}
                    {index > 0 && (
                      <span className="ml-1" style={{ opacity: 0.5 }}>
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
