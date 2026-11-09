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
    const index = new Map<string, number>();
    const merged: (PathEntry & { source: string; displayIndex: number })[] = [];

    const push = (entry: PathEntry, source: string) => {
      const lower = entry.path.toLowerCase();
      const at = index.get(lower);
      if (at !== undefined) {
        // 同名条目出现在两侧时，生效状态取**并集**：任一侧启用，最终 PATH
        // 就包含它。保留系统侧 `enabled` 会让「系统禁用 + 用户启用」显示成禁用。
        if (entry.enabled && !merged[at].enabled) {
          merged[at] = { ...merged[at], enabled: true };
        }
        return;
      }
      index.set(lower, merged.length);
      merged.push({ ...entry, source, displayIndex: merged.length });
    };

    for (const entry of sysPaths) push(entry, t('merge.system'));
    for (const entry of userPaths) push(entry, t('merge.user'));

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
      <div className="path-table-head merge-head">
        <div className="w-10 px-2 py-1">#</div>
        <div className="px-2 py-1 flex-1">{t('table.path')}</div>
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

          return (
            <div
              key={`${source}-${displayIndex}`}
              // 与 PathTable 同规则：交替底色按逻辑索引，不用 :nth-child
              data-state={enabled ? 'ok' : 'off'}
              data-odd={rowIdx % 2 === 1}
              className="path-row"
              // `.path-row` 是 position:absolute + top:0（见 globals.css），
              // 虚拟列表必须自己写 height 与 translateY，否则所有行叠在 y=0。
              style={{
                height: `${virtualRow.size}px`,
                transform: `translateY(${virtualRow.start}px)`,
              }}
            >
              <div className="w-10 px-2 py-0.5 text-xs opacity-50">{rowIdx + 1}</div>
              <div className="path-text">{path}</div>
              <div className="merge-source">{source}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
