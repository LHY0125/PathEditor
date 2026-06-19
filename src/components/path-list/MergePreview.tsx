import { useMemo } from 'react';
import { useAppStore } from '@/store/app-store';
import { useTranslation } from 'react-i18next';
import type { PathEntry } from '@/core/path-entry';

export function MergePreview() {
  const sysPaths = useAppStore((s) => s.sysPaths);
  const userPaths = useAppStore((s) => s.userPaths);
  const searchQuery = useAppStore((s) => s.searchQuery);
  const { t } = useTranslation();

  const allPaths = useMemo(() => {
    const seen = new Set<string>();
    const merged: (PathEntry & { source: string; displayIndex: number })[] = [];

    for (const entry of sysPaths) {
      const lower = entry.path.toLowerCase();
      if (!seen.has(lower)) {
        seen.add(lower);
        merged.push({ ...entry, source: t('merge.system'), displayIndex: merged.length });
      }
    }
    for (const entry of userPaths) {
      const lower = entry.path.toLowerCase();
      if (!seen.has(lower)) {
        seen.add(lower);
        merged.push({ ...entry, source: t('merge.user'), displayIndex: merged.length });
      }
    }

    if (!searchQuery) return merged;
    const q = searchQuery.toLowerCase();
    return merged.filter((r) => r.path.toLowerCase().includes(q));
  }, [sysPaths, userPaths, searchQuery, t]);

  return (
    <div className="flex-1 overflow-auto">
      <table className="w-full border-collapse">
        <thead>
          <tr
            className="sticky top-0 z-10 text-left text-xs uppercase"
            style={{ backgroundColor: 'var(--app-list-alt)', color: 'var(--app-fg)' }}
          >
            <th className="w-10 px-2 py-1">#</th>
            <th className="px-2 py-1">{t('dialog.pathLabel')}</th>
            <th className="w-16 px-2 py-1">{t('merge.source')}</th>
          </tr>
        </thead>
        <tbody>
          {allPaths.map(({ path, enabled, source, displayIndex }, rowIdx) => {
            const textColor = enabled ? 'var(--app-fg)' : '#6b7280';
            const textDecoration = enabled ? 'none' : 'line-through';
            const opacity = enabled ? 1 : 0.6;

            return (
              <tr
                key={`${source}-${displayIndex}`}
                style={{
                  backgroundColor: rowIdx % 2 === 0 ? 'var(--app-list-bg)' : 'var(--app-list-alt)',
                  color: 'var(--app-fg)',
                }}
              >
                <td className="px-2 py-0.5 text-xs opacity-50">{rowIdx + 1}</td>
                <td
                  className="px-2 py-0.5 text-sm"
                  style={{ color: textColor, textDecoration, opacity }}
                >
                  {path}
                </td>
                <td className="px-2 py-0.5 text-xs opacity-60">{source}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
