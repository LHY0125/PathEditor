import { useMemo, useRef } from 'react';
import { useAppStore } from '@/store/app-store';
import { useTranslation } from 'react-i18next';
import type { PathEntry } from '@/core/path-entry';
import { useVirtualizer } from '@tanstack/react-virtual';

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

  const parentRef = useRef<HTMLDivElement>(null);

  const rowVirtualizer = useVirtualizer({
    count: allPaths.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 28, // 预估行高 28px
    initialRect: { width: 800, height: 600 },
  });

  return (
    <div ref={parentRef} className="flex-1 overflow-auto relative">
      <div
        className="sticky top-0 z-10 flex text-left text-xs uppercase"
        style={{ backgroundColor: 'var(--app-list-alt)', color: 'var(--app-fg)' }}
      >
        <div className="w-10 px-2 py-1">#</div>
        <div className="px-2 py-1 flex-1">{t('dialog.pathLabel')}</div>
        <div className="w-16 px-2 py-1">{t('merge.source')}</div>
      </div>
      <div
        style={{
          height: `${rowVirtualizer.getTotalSize()}px`,
          width: '100%',
          position: 'relative',
        }}
      >
        {rowVirtualizer.getVirtualItems().map((virtualRow) => {
          const rowIdx = virtualRow.index;
          const { path, enabled, source, displayIndex } = allPaths[rowIdx];
          const textColor = enabled ? 'var(--app-fg)' : '#6b7280';
          const textDecoration = enabled ? 'none' : 'line-through';
          const opacity = enabled ? 1 : 0.6;

          return (
            <div
              key={`${source}-${displayIndex}`}
              className="flex items-center absolute top-0 left-0 w-full"
              style={{
                height: `${virtualRow.size}px`,
                transform: `translateY(${virtualRow.start}px)`,
                backgroundColor: rowIdx % 2 === 0 ? 'var(--app-list-bg)' : 'var(--app-list-alt)',
                color: 'var(--app-fg)',
              }}
            >
              <div className="w-10 px-2 py-0.5 text-xs opacity-50">{rowIdx + 1}</div>
              <div
                className="px-2 py-0.5 text-sm flex-1 truncate"
                style={{ color: textColor, textDecoration, opacity }}
              >
                {path}
              </div>
              <div className="w-16 px-2 py-0.5 text-xs opacity-60">{source}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
